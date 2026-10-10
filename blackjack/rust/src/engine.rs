//! AVX-512 lockstep engine: games run one per 32-bit vector lane, in G groups of 16 lanes.
//! Mirrors ../cpp/engine.hpp step for step; see the comments there for the design.
//!
//! Each lane is a full Sim (its own pre-shuffled shoe, count and stats). A vector round plays one
//! round in every lane at once, with control flow turned into lane masks. All four initial cards
//! come from one 32-bit gather, the next 8 cards sit in a register "card window", decisions are
//! strategy-matrix gathers plus one exact true-count compare, and the hit and dealer loops run
//! until no lane is still drawing. The G groups advance phase by phase in turn so their latency
//! chains overlap. A lane that would split a second time is rewound and replayed by the scalar
//! Sim::play_round(), and reshuffles are done by the scalar Shoe::shuffle(), so results are
//! bit-identical to running each lane's Sim on its own.
#![allow(unused_unsafe, clippy::needless_range_loop)]

use crate::*;
use std::arch::x86_64::*;
use std::sync::OnceLock;

pub const VLANES: usize = 16;

/// Strategy matrix flattened for gathers: base | alt << 8 | k << 16 (k sign-extended).
fn flat_table(counting: bool) -> &'static [i32] {
    static HILO: OnceLock<Vec<i32>> = OnceLock::new();
    static BASIC: OnceLock<Vec<i32>> = OnceLock::new();
    let build = |c: bool| {
        let t = strategy_table(c);
        let mut e = vec![0i32; NCLASS * 11 * NFLAGS];
        for cl in 0..NCLASS {
            for u in 0..11 {
                for f in 0..NFLAGS {
                    let x = t.entry(cl, u, f);
                    e[(cl * 11 + u) * NFLAGS + f] = x.0 as i32 | (x.1 as i32) << 8 | ((x.2 as i32) << 16);
                }
            }
        }
        e
    };
    if counting { HILO.get_or_init(|| build(true)) } else { BASIC.get_or_init(|| build(false)) }
}

/// Hit-or-stand rule for hands of 3+ cards (no double, split or surrender), derived from the
/// strategy matrix: stand iff value >= S, where for hard hands S = hard[up] minus one for each
/// breakpoint b1[up], b2[up] with tc >= b, and for soft hands S = soft[up]. The builder checks the
/// matrix really has this shape at every true count from -40 to +40. In the engine, tc >= b is
/// kept as D_b = rc * 52 - b * unseen >= 0, updated by adds on every draw, so a hit-loop decision
/// needs no gather and no multiply.
struct HitRule {
    hard: [i32; 16],
    soft: [i32; 16],
    b1: [i32; 16],
    b2: [i32; 16],
}

const NEVER: i32 = 1000; // breakpoint no count reaches

fn hit_rule(counting: bool) -> &'static HitRule {
    static HILO: OnceLock<HitRule> = OnceLock::new();
    static BASIC: OnceLock<HitRule> = OnceLock::new();
    let build = |c: bool| {
        let t = strategy_table(c);
        let stands = |cls: usize, up: usize, tc: i32| {
            let (base, alt, k) = t.entry(cls, up, 0);
            (if tc >= k as i32 { alt } else { base }) == Action::Stand as u8
        };
        // Smallest value from which every higher value stands (and every lower one hits).
        let threshold = |first_cls: usize, lo: i32, hi: i32, up: usize, tc: i32| {
            let mut s = hi + 1;
            let mut v = hi;
            while v >= lo && stands(first_cls + (v - lo) as usize, up, tc) {
                s = v;
                v -= 1;
            }
            assert!((lo..s).all(|v| !stands(first_cls + (v - lo) as usize, up, tc)), "not a threshold rule");
            s
        };
        let mut r = HitRule { hard: [0; 16], soft: [0; 16], b1: [NEVER; 16], b2: [NEVER; 16] };
        for up in 1..=10 {
            let mut prev = threshold(4, 4, 21, up, -40);
            let mut steps = 0;
            r.hard[up] = prev;
            r.soft[up] = threshold(22, 12, 21, up, -40); // soft classes 22..31 = soft 12..21
            for tc in -39..=40 {
                assert_eq!(threshold(22, 12, 21, up, tc), r.soft[up], "soft play must not depend on the count");
                let s = threshold(4, 4, 21, up, tc);
                assert!(s <= prev, "a higher count must never mean standing less");
                while prev > s {
                    if steps == 0 { r.b1[up] = tc } else { r.b2[up] = tc }
                    prev -= 1;
                    steps += 1;
                }
                assert!(steps <= 2);
            }
        }
        r
    };
    if counting { HILO.get_or_init(|| build(true)) } else { BASIC.get_or_init(|| build(false)) }
}

const FLUSH: u64 = 1 << 12;
/// True-count histogram cells pack rounds << 19 plus the signed flat-net sum (|sum| < 2^18 and
/// rounds < 2^12 between flushes), so one gather + scatter updates both.
const HIST_SHIFT: i32 = 19;
const NACC: usize = 19;
const ROUNDS: usize = 0;
const HANDS: usize = 1;
const NET: usize = 2;
const INIT: usize = 3;
const ACTION: usize = 4;
const NETSQ: usize = 5;
const FLAT: usize = 6;
const FLATSQ: usize = 7;
const WINS: usize = 8;
const LOSSES: usize = 9;
const PUSHES: usize = 10;
const PBJ: usize = 11;
const DBJ: usize = 12;
const DOUBLES: usize = 13;
const SURR: usize = 14;
const BUSTS: usize = 15;
const INS: usize = 16;
const INSW: usize = 17;
const SPLITS: usize = 18;

#[inline(always)]
fn splat(x: i32) -> __m512i {
    unsafe { _mm512_set1_epi32(x) }
}

pub struct Engine<'a, const G: usize> {
    lanes: &'a mut [Sim],
    counting: bool,
    table: &'static [i32],
    acc: [[[i32; VLANES]; NACC]; G], // per group
    hist: Vec<i32>, // per lane x bucket, packed (see HIST_SHIFT)
    base: *const u8,
    // per-group state
    off: [__m512i; G],
    pos: [__m512i; G],
    rc: [__m512i; G],
    win1: [__m512i; G],
    win2: [__m512i; G],
    left: [__m512i; G],
    // per-round temporaries; hand-indexed ones are [hand][group]
    up: [__m512i; G],
    c1: [__m512i; G],
    sum: [[__m512i; G]; 2],
    ace: [[__mmask16; G]; 2],
    act: [[__m512i; G]; 2],
    sp_aces: [__mmask16; G],
    // hit rule per lane (by upcard) and breakpoint distances D_b = rc*52 - b*unseen
    b1: [__m512i; G],
    b2: [__m512i; G],
    d1: [__m512i; G],
    d2: [__m512i; G],
    stand_hard: [__m512i; G],
    stand_soft: [__m512i; G],
    rule: &'static HitRule,
    resplit: [__mmask16; G],
    more: [__mmask16; G],
}

impl<'a, const G: usize> Engine<'a, G> {
    pub const LANES: usize = G * VLANES;

    pub fn new(lanes: &'a mut [Sim], counting: bool) -> Self {
        assert_eq!(lanes.len(), Self::LANES);
        let base = lanes[0].shoe.cards.as_ptr();
        let z = unsafe { _mm512_setzero_si512() };
        Engine {
            lanes,
            counting,
            table: flat_table(counting),
            acc: [[[0; VLANES]; NACC]; G],
            hist: vec![0; Self::LANES * TC_BUCKETS],
            base,
            off: [z; G],
            pos: [z; G],
            rc: [z; G],
            win1: [z; G],
            win2: [z; G],
            left: [z; G],
            up: [z; G],
            c1: [z; G],
            sum: [[z; G]; 2],
            ace: [[0; G]; 2],
            act: [[z; G]; 2],
            sp_aces: [0; G],
            b1: [z; G],
            b2: [z; G],
            d1: [z; G],
            d2: [z; G],
            stand_hard: [z; G],
            stand_soft: [z; G],
            rule: hit_rule(counting),
            resplit: [0; G],
            more: [0; G],
        }
    }

    fn flush(&mut self) {
        for l in 0..Self::LANES {
            let a = &self.acc[l / VLANES];
            let k = l % VLANES;
            let st = &mut self.lanes[l].st;
            let u = |x: i32| x as u32 as u64;
            st.rounds += u(a[ROUNDS][k]);
            st.hands += u(a[HANDS][k]);
            st.net += a[NET][k] as i64;
            st.initial_bet += u(a[INIT][k]);
            st.action += u(a[ACTION][k]);
            st.net_sq += u(a[NETSQ][k]);
            st.flat_net += a[FLAT][k] as i64;
            st.flat_sq += u(a[FLATSQ][k]);
            st.wins += u(a[WINS][k]);
            st.losses += u(a[LOSSES][k]);
            st.pushes += u(a[PUSHES][k]);
            st.player_bj += u(a[PBJ][k]);
            st.dealer_bj += u(a[DBJ][k]);
            st.doubles += u(a[DOUBLES][k]);
            st.surrenders += u(a[SURR][k]);
            st.busts += u(a[BUSTS][k]);
            st.ins_taken += u(a[INS][k]);
            st.ins_won += u(a[INSW][k]);
            st.splits += u(a[SPLITS][k]);
            for b in 0..TC_BUCKETS {
                let v = self.hist[l * TC_BUCKETS + b];
                let half = 1 << (HIST_SHIFT - 1);
                let flat = ((v + half) & ((1 << HIST_SHIFT) - 1)) - half;
                st.tc_rounds[b] += (v.wrapping_sub(flat) as u32 >> HIST_SHIFT) as u64;
                st.tc_flat_net[b] += flat as i64;
                self.hist[l * TC_BUCKETS + b] = 0;
            }
        }
        for a in self.acc.iter_mut() {
            *a = [[0; VLANES]; NACC];
        }
    }

    // ---- small helpers
    #[inline(always)]
    fn add(&mut self, g: usize, which: usize, m: __mmask16, v: __m512i) {
        unsafe {
            let p = self.acc[g][which].as_mut_ptr() as *mut __m512i;
            let a = _mm512_loadu_si512(p);
            _mm512_storeu_si512(p, _mm512_mask_add_epi32(a, m, a, v));
        }
    }
    #[inline(always)]
    fn inc(&mut self, g: usize, which: usize, m: __mmask16) {
        self.add(g, which, m, splat(1));
    }
    #[inline(always)]
    fn hilo(c: __m512i) -> __m512i {
        unsafe { _mm512_permutexvar_epi32(c, _mm512_setr_epi32(0, -1, 1, 1, 1, 1, 1, 0, 0, 0, -1, 0, 0, 0, 0, 0)) }
    }
    /// Exact true-count test rc / (unseen / 52) >= k, as rc * 52 >= k * unseen. During play the
    /// hole card is still unseen.
    #[inline(always)]
    fn tc_ge(&self, g: usize, k: __m512i) -> __mmask16 {
        unsafe {
            let unseen = _mm512_sub_epi32(splat(CARDS as i32 + 1), self.pos[g]);
            _mm512_cmpge_epi32_mask(_mm512_mullo_epi32(self.rc[g], splat(52)), _mm512_mullo_epi32(k, unseen))
        }
    }
    #[inline(always)]
    fn lookup(&self, g: usize, m: __mmask16, cls: __m512i, flags: i32) -> __m512i {
        unsafe {
            let idx = _mm512_add_epi32(
                _mm512_slli_epi32::<3>(_mm512_add_epi32(_mm512_mullo_epi32(cls, splat(11)), self.up[g])),
                splat(flags),
            );
            let e = _mm512_mask_i32gather_epi32::<4>(_mm512_setzero_si512(), m, idx, self.table.as_ptr());
            let ge = self.tc_ge(g, _mm512_srai_epi32::<16>(e));
            let byte = splat(0xFF);
            _mm512_mask_blend_epi32(ge, _mm512_and_si512(e, byte), _mm512_and_si512(_mm512_srli_epi32::<8>(e), byte))
        }
    }
    /// Masked draw of one card per lane in m, from the card window while it has cards left.
    #[inline(always)]
    fn draw(&mut self, g: usize, m: __mmask16) -> __m512i {
        unsafe {
            let byte = splat(0xFF);
            let one = splat(1);
            let mut c = _mm512_and_si512(self.win1[g], byte);
            let past = m & _mm512_cmple_epi32_mask(self.left[g], _mm512_setzero_si512());
            if past != 0 {
                let at = _mm512_add_epi32(self.off[g], self.pos[g]);
                c = _mm512_and_si512(_mm512_mask_i32gather_epi32::<1>(c, past, at, self.base as *const i32), byte);
            }
            self.win1[g] = _mm512_mask_mov_epi32(
                self.win1[g],
                m,
                _mm512_or_si512(_mm512_srli_epi32::<8>(self.win1[g]), _mm512_slli_epi32::<24>(self.win2[g])),
            );
            self.win2[g] = _mm512_mask_srli_epi32::<8>(self.win2[g], m, self.win2[g]);
            self.left[g] = _mm512_mask_sub_epi32(self.left[g], m, self.left[g], one);
            self.pos[g] = _mm512_mask_add_epi32(self.pos[g], m, self.pos[g], one);
            self.rc[g] = _mm512_mask_add_epi32(self.rc[g], m, self.rc[g], Self::hilo(c));
            // D_b = rc*52 - b*unseen: a draw adds 52*hilo(c) to rc*52 and takes 1 from unseen.
            let h52 = _mm512_permutexvar_epi32(c, _mm512_setr_epi32(0, -52, 52, 52, 52, 52, 52, 0, 0, 0, -52, 0, 0, 0, 0, 0));
            self.d1[g] = _mm512_mask_add_epi32(self.d1[g], m, self.d1[g], _mm512_add_epi32(h52, self.b1[g]));
            self.d2[g] = _mm512_mask_add_epi32(self.d2[g], m, self.d2[g], _mm512_add_epi32(h52, self.b2[g]));
            c
        }
    }
    #[inline(always)]
    fn value(sum: __m512i, ace: __mmask16) -> __m512i {
        unsafe { _mm512_mask_add_epi32(sum, ace & _mm512_cmple_epi32_mask(sum, splat(11)), sum, splat(10)) }
    }
    #[inline(always)]
    fn hand_cls(sum: __m512i, ace: __mmask16, pair: __mmask16, first: __m512i) -> __m512i {
        unsafe {
            let sft = ace & _mm512_cmple_epi32_mask(sum, splat(11));
            let cl = _mm512_mask_blend_epi32(sft, sum, _mm512_add_epi32(sum, splat(20)));
            _mm512_mask_blend_epi32(pair, cl, _mm512_add_epi32(first, splat(31)))
        }
    }
    /// Hand h restarted by a split: (c1, new card). Its first decision may double or split again
    /// (no surrender); a second split marks the lane for scalar replay. Split aces stand.
    #[inline(always)]
    fn split_hand(&mut self, g: usize, h: usize, m: __mmask16) {
        unsafe {
            let one = splat(1);
            let c = self.draw(g, m);
            let c1 = self.c1[g];
            self.sum[h][g] = _mm512_mask_add_epi32(self.sum[h][g], m, c1, c);
            self.ace[h][g] = (self.ace[h][g] & !m) | (m & (_mm512_cmpeq_epi32_mask(c1, one) | _mm512_cmpeq_epi32_mask(c, one)));
            let dec = m & !self.sp_aces[g];
            let cls = Self::hand_cls(self.sum[h][g], self.ace[h][g], _mm512_cmpeq_epi32_mask(c, c1), c1);
            let a2 = self.lookup(g, dec, cls, 3);
            self.resplit[g] |= dec & _mm512_cmpeq_epi32_mask(a2, splat(Action::Split as i32));
            let a = _mm512_mask_blend_epi32(dec, self.act[h][g], a2);
            self.act[h][g] = _mm512_mask_mov_epi32(a, m & self.sp_aces[g], splat(Action::Stand as i32));
        }
    }
    /// Start playing hands h in m: a double takes one card; a hit takes one and leaves the lane
    /// in `more` for the hit loop. Returns the doubles.
    #[inline(always)]
    fn play_start(&mut self, g: usize, h: usize, m: __mmask16) -> __mmask16 {
        unsafe {
            let a = self.act[h][g];
            let dbl = m & _mm512_cmpeq_epi32_mask(a, splat(Action::Double as i32));
            self.more[g] = m & _mm512_cmpeq_epi32_mask(a, splat(Action::Hit as i32));
            let dr = dbl | self.more[g];
            let c = self.draw(g, dr);
            self.sum[h][g] = _mm512_mask_add_epi32(self.sum[h][g], dr, self.sum[h][g], c);
            self.ace[h][g] |= dr & _mm512_cmpeq_epi32_mask(c, splat(1));
            dbl
        }
    }
    /// One hit-loop step for multi-card hands (they can only hit or stand): see HitRule.
    #[inline(always)]
    fn hit_step(&mut self, g: usize, h: usize) {
        unsafe {
            let (sum, ace) = (self.sum[h][g], self.ace[h][g]);
            let one = splat(1);
            let zero = _mm512_setzero_si512();
            let sft = ace & _mm512_cmple_epi32_mask(sum, splat(11));
            let val = _mm512_mask_add_epi32(sum, sft, sum, splat(10));
            let mut s = self.stand_hard[g];
            s = _mm512_mask_sub_epi32(s, _mm512_cmpge_epi32_mask(self.d1[g], zero), s, one);
            s = _mm512_mask_sub_epi32(s, _mm512_cmpge_epi32_mask(self.d2[g], zero), s, one);
            s = _mm512_mask_mov_epi32(s, sft, self.stand_soft[g]);
            let m = self.more[g] & _mm512_cmplt_epi32_mask(val, splat(21)) & _mm512_cmplt_epi32_mask(val, s);
            if m != 0 {
                let c = self.draw(g, m);
                self.sum[h][g] = _mm512_mask_add_epi32(sum, m, sum, c);
                self.ace[h][g] |= m & _mm512_cmpeq_epi32_mask(c, splat(1));
            }
            self.more[g] = m;
        }
    }
    fn any_more(&self) -> bool {
        self.more.iter().any(|&m| m != 0)
    }
    fn store_lanes(&mut self, g: usize, mut m: __mmask16) {
        let mut p = [0i32; VLANES];
        let mut r = [0i32; VLANES];
        unsafe {
            _mm512_storeu_si512(p.as_mut_ptr() as *mut __m512i, self.pos[g]);
            _mm512_storeu_si512(r.as_mut_ptr() as *mut __m512i, self.rc[g]);
        }
        while m != 0 {
            let k = m.trailing_zeros() as usize;
            self.lanes[g * VLANES + k].shoe.pos = p[k] as usize;
            self.lanes[g * VLANES + k].rc = r[k];
            m &= m - 1;
        }
    }
    fn load_lanes(&mut self, g: usize, m: __mmask16) {
        let mut p = [0i32; VLANES];
        let mut r = [0i32; VLANES];
        for k in 0..VLANES {
            p[k] = self.lanes[g * VLANES + k].shoe.pos as i32;
            r[k] = self.lanes[g * VLANES + k].rc;
        }
        unsafe {
            self.pos[g] = _mm512_mask_loadu_epi32(self.pos[g], m, p.as_ptr());
            self.rc[g] = _mm512_mask_loadu_epi32(self.rc[g], m, r.as_ptr());
        }
    }

    pub fn run(&mut self, rounds_per_lane: u64) {
        unsafe { self.run_inner(rounds_per_lane) }
    }

    unsafe fn run_inner(&mut self, rounds_per_lane: u64) {
        let zero = _mm512_setzero_si512();
        let one = splat(1);
        let byte = splat(0xFF);
        let ramp = _mm512_setr_epi32(1, 1, 2, 4, 6, 8, 10, 12, 0, 0, 0, 0, 0, 0, 0, 0);
        let lane_idx = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
        for g in 0..G {
            let mut o = [0i32; VLANES];
            for k in 0..VLANES {
                o[k] = (self.lanes[g * VLANES + k].shoe.cards.as_ptr() as isize - self.base as isize) as i32;
            }
            self.off[g] = _mm512_loadu_si512(o.as_ptr() as *const __m512i);
            self.load_lanes(g, 0xFFFF);
        }

        let mut pos0 = [zero; G];
        let mut rc0 = [zero; G];
        let mut units = [zero; G];
        let mut bb = [zero; G];
        let mut bucket = [zero; G];
        let mut hole = [zero; G];
        let mut net = [zero; G];
        let mut action = [zero; G];
        let mut dsum = [zero; G];
        let mut dval = [zero; G];
        let mut dace = [0 as __mmask16; G];
        let mut up_ace = [0 as __mmask16; G];
        let mut dbj = [0 as __mmask16; G];
        let mut pbj = [0 as __mmask16; G];
        let mut ins = [0 as __mmask16; G];
        let mut split = [0 as __mmask16; G];
        let mut surr = [0 as __mmask16; G];
        let mut m0 = [0 as __mmask16; G];
        let mut m1 = [0 as __mmask16; G];
        let mut dbl0 = [0 as __mmask16; G];
        let mut dbl1 = [0 as __mmask16; G];
        let mut live = [0 as __mmask16; G];
        let mut live1 = [0 as __mmask16; G];
        let mut bust = [0 as __mmask16; G];
        let mut bust1 = [0 as __mmask16; G];

        for r in 0..rounds_per_lane {
            // ---- reshuffles (scalar, ~1 in 48 rounds per lane), bet, deal, blackjacks, first decision
            for g in 0..G {
                let m = _mm512_cmpge_epi32_mask(self.pos[g], splat(CUT as i32));
                if m != 0 {
                    let mut b = m;
                    while b != 0 {
                        let s = &mut self.lanes[g * VLANES + b.trailing_zeros() as usize];
                        s.shoe.shuffle();
                        s.st.shuffles += 1;
                        b &= b - 1;
                    }
                    self.pos[g] = _mm512_mask_mov_epi32(self.pos[g], m, zero);
                    self.rc[g] = _mm512_mask_mov_epi32(self.rc[g], m, zero);
                }
                pos0[g] = self.pos[g];
                rc0[g] = self.rc[g];

                // Bet from the floored true count (float division is exact enough here).
                let unseen = _mm512_sub_epi32(splat(CARDS as i32), self.pos[g]);
                let q = _mm512_div_ps(_mm512_cvtepi32_ps(_mm512_mullo_epi32(self.rc[g], splat(52))), _mm512_cvtepi32_ps(unseen));
                let tc = _mm512_cvtps_epi32(_mm512_roundscale_ps::<{ _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC }>(q));
                units[g] = if self.counting {
                    _mm512_permutexvar_epi32(_mm512_min_epi32(_mm512_max_epi32(tc, one), splat(7)), ramp)
                } else {
                    one
                };
                bb[g] = _mm512_add_epi32(units[g], units[g]);
                let half_b = units[g];
                bucket[g] = _mm512_add_epi32(_mm512_min_epi32(_mm512_max_epi32(tc, splat(TC_MIN)), splat(TC_MAX)), splat(-TC_MIN));

                // Deal p1, up, p2, hole (one gather) and load the 8-card window (two more).
                let at = _mm512_add_epi32(self.off[g], self.pos[g]);
                let bp = self.base as *const i32;
                let w = _mm512_i32gather_epi32::<1>(at, bp);
                self.win1[g] = _mm512_i32gather_epi32::<1>(_mm512_add_epi32(at, splat(4)), bp);
                self.win2[g] = _mm512_i32gather_epi32::<1>(_mm512_add_epi32(at, splat(8)), bp);
                self.left[g] = splat(8);
                let upv = _mm512_and_si512(_mm512_srli_epi32::<8>(w), byte);
                let ld = |a: &[i32; 16]| _mm512_loadu_si512(a.as_ptr() as *const __m512i);
                self.b1[g] = _mm512_permutexvar_epi32(upv, ld(&self.rule.b1));
                self.b2[g] = _mm512_permutexvar_epi32(upv, ld(&self.rule.b2));
                self.stand_hard[g] = _mm512_permutexvar_epi32(upv, ld(&self.rule.hard));
                self.stand_soft[g] = _mm512_permutexvar_epi32(upv, ld(&self.rule.soft));
                let c1 = _mm512_and_si512(w, byte);
                self.c1[g] = c1;
                self.up[g] = _mm512_and_si512(_mm512_srli_epi32::<8>(w), byte);
                let c2 = _mm512_and_si512(_mm512_srli_epi32::<16>(w), byte);
                hole[g] = _mm512_srli_epi32::<24>(w);
                self.pos[g] = _mm512_add_epi32(self.pos[g], splat(4));
                self.rc[g] = _mm512_add_epi32(self.rc[g], _mm512_add_epi32(Self::hilo(c1), _mm512_add_epi32(Self::hilo(self.up[g]), Self::hilo(c2))));

                self.sum[0][g] = _mm512_add_epi32(c1, c2);
                self.ace[0][g] = _mm512_cmpeq_epi32_mask(c1, one) | _mm512_cmpeq_epi32_mask(c2, one);
                up_ace[g] = _mm512_cmpeq_epi32_mask(self.up[g], one);
                let up_ten = _mm512_cmpeq_epi32_mask(self.up[g], splat(10));
                dbj[g] = (up_ace[g] & _mm512_cmpeq_epi32_mask(hole[g], splat(10))) | (up_ten & _mm512_cmpeq_epi32_mask(hole[g], one));
                pbj[g] = self.ace[0][g] & _mm512_cmpeq_epi32_mask(self.sum[0][g], splat(11));
                net[g] = zero;
                action[g] = bb[g];

                {
                    // D_b after the deal (hole card unseen); draw() keeps it current
                    let rc52 = _mm512_mullo_epi32(self.rc[g], splat(52));
                    let unseen_now = _mm512_sub_epi32(splat(CARDS as i32 + 1), self.pos[g]);
                    self.d1[g] = _mm512_sub_epi32(rc52, _mm512_mullo_epi32(self.b1[g], unseen_now));
                    self.d2[g] = _mm512_sub_epi32(rc52, _mm512_mullo_epi32(self.b2[g], unseen_now));
                }
                ins[g] = if self.counting { up_ace[g] & self.tc_ge(g, splat(3)) } else { 0 };
                net[g] = _mm512_mask_add_epi32(net[g], ins[g] & dbj[g], net[g], bb[g]);
                net[g] = _mm512_mask_sub_epi32(net[g], ins[g] & !dbj[g], net[g], half_b);
                action[g] = _mm512_mask_add_epi32(action[g], ins[g], action[g], half_b);

                // Blackjacks settle immediately.
                let done = dbj[g] | pbj[g];
                net[g] = _mm512_mask_sub_epi32(net[g], dbj[g] & !pbj[g], net[g], bb[g]);
                net[g] = _mm512_mask_add_epi32(net[g], pbj[g] & !dbj[g], net[g], _mm512_add_epi32(bb[g], half_b));

                // First decision: double, split and surrender all allowed.
                let act = !done;
                let cls = Self::hand_cls(self.sum[0][g], self.ace[0][g], _mm512_cmpeq_epi32_mask(c1, c2), c1);
                self.act[0][g] = self.lookup(g, act, cls, 7);
                split[g] = act & _mm512_cmpeq_epi32_mask(self.act[0][g], splat(Action::Split as i32));
                surr[g] = act & _mm512_cmpeq_epi32_mask(self.act[0][g], splat(Action::Surrender as i32));
                self.resplit[g] = 0;
                self.sp_aces[g] = split[g] & _mm512_cmpeq_epi32_mask(c1, one);
                self.sum[1][g] = zero;
                self.ace[1][g] = 0;
                m1[g] = 0;
                dbl1[g] = 0;
                if split[g] != 0 {
                    self.split_hand(g, 0, split[g]);
                }
                m0[g] = act & !surr[g] & !self.resplit[g];
                dbl0[g] = self.play_start(g, 0, m0[g]);
            }
            // ---- hand 0 hit loop, groups interleaved
            while self.any_more() {
                for g in 0..G {
                    if self.more[g] != 0 {
                        self.hit_step(g, 0);
                    }
                }
            }
            // ---- hand 1 of split lanes
            for g in 0..G {
                self.more[g] = 0;
                let s1 = split[g] & !self.resplit[g];
                if s1 != 0 {
                    self.act[1][g] = zero;
                    self.split_hand(g, 1, s1);
                    m1[g] = s1 & !self.resplit[g];
                    dbl1[g] = self.play_start(g, 1, m1[g]);
                }
            }
            while self.any_more() {
                for g in 0..G {
                    if self.more[g] != 0 {
                        self.hit_step(g, 1);
                    }
                }
            }
            // ---- dealer: hole card revealed, then draws to 17 if any hand is live
            for g in 0..G {
                self.rc[g] = _mm512_add_epi32(self.rc[g], Self::hilo(hole[g]));
                bust[g] = m0[g] & _mm512_cmpgt_epi32_mask(Self::value(self.sum[0][g], self.ace[0][g]), splat(21));
                bust1[g] = m1[g] & _mm512_cmpgt_epi32_mask(Self::value(self.sum[1][g], self.ace[1][g]), splat(21));
                live[g] = m0[g] & !bust[g];
                live1[g] = m1[g] & !bust1[g];
                dsum[g] = _mm512_add_epi32(self.up[g], hole[g]);
                dace[g] = up_ace[g] | _mm512_cmpeq_epi32_mask(hole[g], one);
                dval[g] = Self::value(dsum[g], dace[g]);
                self.more[g] = (live[g] | live1[g]) & _mm512_cmplt_epi32_mask(dval[g], splat(17));
            }
            while self.any_more() {
                for g in 0..G {
                    let m = self.more[g];
                    if m != 0 {
                        let c = self.draw(g, m);
                        dsum[g] = _mm512_mask_add_epi32(dsum[g], m, dsum[g], c);
                        dace[g] |= m & _mm512_cmpeq_epi32_mask(c, one);
                        dval[g] = Self::value(dsum[g], dace[g]);
                        self.more[g] = m & _mm512_cmplt_epi32_mask(dval[g], splat(17));
                    }
                }
            }
            // ---- settle and account
            for g in 0..G {
                let dbust = _mm512_cmpgt_epi32_mask(dval[g], splat(21));
                let b = bb[g];
                let settle = |m: __mmask16, lv: __mmask16, pv: __m512i, dbl: __mmask16, net: &mut __m512i, action: &mut __m512i| {
                    let win = lv & (dbust | _mm512_cmpgt_epi32_mask(pv, dval[g]));
                    let push = lv & !dbust & _mm512_cmpeq_epi32_mask(pv, dval[g]);
                    let loss = m & !win & !push;
                    let wager = _mm512_mask_add_epi32(b, dbl, b, b);
                    *net = _mm512_mask_add_epi32(*net, win, *net, wager);
                    *net = _mm512_mask_sub_epi32(*net, loss, *net, wager);
                    *action = _mm512_mask_add_epi32(*action, dbl, *action, b);
                    [win, loss, push]
                };
                let r0 = settle(m0[g], live[g], Self::value(self.sum[0][g], self.ace[0][g]), dbl0[g], &mut net[g], &mut action[g]);
                let r1 = settle(m1[g], live1[g], Self::value(self.sum[1][g], self.ace[1][g]), dbl1[g], &mut net[g], &mut action[g]);
                net[g] = _mm512_mask_sub_epi32(net[g], surr[g], net[g], units[g]);
                action[g] = _mm512_mask_add_epi32(action[g], split[g], action[g], b);

                // Lanes being replayed (resplit) are left out; the scalar replay counts them.
                let vec = !self.resplit[g];
                self.inc(g, ROUNDS, vec);
                self.inc(g, HANDS, vec);
                self.inc(g, HANDS, m1[g]);
                self.inc(g, SPLITS, split[g] & vec);
                self.add(g, NET, vec, net[g]);
                self.add(g, INIT, vec, b);
                self.add(g, ACTION, vec, action[g]);
                self.add(g, NETSQ, vec, _mm512_mullo_epi32(net[g], net[g]));
                // net is an exact multiple of units, so float division is exact.
                let flat = _mm512_cvtps_epi32(_mm512_div_ps(_mm512_cvtepi32_ps(net[g]), _mm512_cvtepi32_ps(units[g])));
                self.add(g, FLAT, vec, flat);
                self.add(g, FLATSQ, vec, _mm512_mullo_epi32(flat, flat));
                self.inc(g, WINS, (r0[0] & vec) | (pbj[g] & !dbj[g]));
                self.inc(g, WINS, r1[0]); // a lane can win both hands
                self.inc(g, LOSSES, (r0[1] & vec) | surr[g] | (dbj[g] & !pbj[g]));
                self.inc(g, LOSSES, r1[1]);
                self.inc(g, PUSHES, (r0[2] & vec) | (dbj[g] & pbj[g]));
                self.inc(g, PUSHES, r1[2]);
                self.inc(g, PBJ, pbj[g]);
                self.inc(g, DBJ, dbj[g]);
                self.inc(g, DOUBLES, dbl0[g] & vec);
                self.inc(g, DOUBLES, dbl1[g]);
                self.inc(g, SURR, surr[g]);
                self.inc(g, BUSTS, bust[g] & vec);
                self.inc(g, BUSTS, bust1[g]);
                self.inc(g, INS, ins[g] & vec);
                self.inc(g, INSW, ins[g] & dbj[g]);
                // Per-lane histogram: indices are distinct per lane, so no scatter conflicts.
                let hidx = _mm512_add_epi32(
                    _mm512_mullo_epi32(_mm512_add_epi32(lane_idx, splat((g * VLANES) as i32)), splat(TC_BUCKETS as i32)),
                    bucket[g],
                );
                let hv = _mm512_mask_i32gather_epi32::<4>(zero, vec, hidx, self.hist.as_ptr());
                let hv = _mm512_add_epi32(hv, _mm512_add_epi32(flat, splat(1 << HIST_SHIFT)));
                _mm512_mask_i32scatter_epi32::<4>(self.hist.as_mut_ptr(), vec, hidx, hv);

                // Re-split lanes: rewind to the start of the round and replay on the scalar engine.
                let rs = self.resplit[g];
                if rs != 0 {
                    self.pos[g] = _mm512_mask_mov_epi32(self.pos[g], rs, pos0[g]);
                    self.rc[g] = _mm512_mask_mov_epi32(self.rc[g], rs, rc0[g]);
                    self.store_lanes(g, rs);
                    let mut b = rs;
                    while b != 0 {
                        self.lanes[g * VLANES + b.trailing_zeros() as usize].play_round();
                        b &= b - 1;
                    }
                    self.load_lanes(g, rs);
                }
            }
            if r & (FLUSH - 1) == FLUSH - 1 {
                self.flush();
            }
        }
        self.flush();
        for g in 0..G {
            self.store_lanes(g, 0xFFFF);
        }
    }
}
