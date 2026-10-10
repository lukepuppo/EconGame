//! Blackjack simulator core: 6-deck shoe, 5 decks dealt, Hi-Lo with Illustrious 18 + Fab 4.
//! Rules: S17, DAS, late surrender, dealer peeks, split to 4 hands, split aces once
//! (one card each, no resplit), 3:2 blackjack, insurance offered.
//!
//! This is the scalar reference engine; engine.rs is the AVX-512 engine, which produces
//! bit-identical results. Both mirror ../cpp step for step, including the RNG and the order
//! cards are drawn, so C++ and Rust produce identical results for the same seed.

#[cfg(target_feature = "avx512f")]
pub mod engine;

pub const DECKS: usize = 6;
pub const CARDS: usize = DECKS * 52; // 312
pub const CUT: usize = 5 * 52; // reshuffle once 260 cards (5 decks) are dealt
pub const MAX_HANDS: usize = 4;
pub const TC_MIN: i32 = -10;
pub const TC_MAX: i32 = 10;
pub const TC_BUCKETS: usize = (TC_MAX - TC_MIN + 1) as usize;

/// Card ranks are 1..10 (1 = ace, 10 = any ten-value card).
pub const HILO: [i8; 11] = [0, -1, 1, 1, 1, 1, 1, 0, 0, 0, -1];

// ---------------------------------------------------------------- RNG
// xoshiro256++ (Blackman & Vigna) in 8 independent streams stored structure-of-arrays, so one
// refill is a few AVX-512 instructions (the 8 dependency chains run in parallel; a single
// stream is a serial ~5-cycle-per-step chain). A refill makes 4 steps x 8 lanes = 32 outputs,
// and both 32-bit halves of each are used: 64 words. Bounded integers use Lemire's multiply-
// shift with rejection, one 32-bit word per value in the common case (never 64 random bits for
// a value < 2^9: the "batched" idea of Brackett-Rozinsky & Lemire 2024).
pub fn splitmix64(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Seed for lane set `set`: a splitmix64 hash of (seed, set). Lanes in a set then take successive
/// splitmix64 outputs from it. Hashing matters: if set s started its chain at seed + s * golden,
/// set s + 1 would reuse set s's lane seeds shifted by one, replaying the same games.
pub fn set_seed(seed: u64, set: u64) -> u64 {
    let mut s = seed ^ set.wrapping_mul(0xD1B54A32D192ED03);
    splitmix64(&mut s);
    splitmix64(&mut s)
}

pub const RNG_LANES: usize = 8;
pub const RNG_STEPS: usize = 4;
pub const RNG_BUF: usize = 2 * RNG_LANES * RNG_STEPS;

#[derive(Clone)]
#[repr(C, align(64))]
pub struct Rng {
    pub s: [[u64; RNG_LANES]; 4], // s[k][lane]
    pub buf: [u32; RNG_BUF],
    pub idx: usize,
}

impl Rng {
    pub fn new(mut seed: u64) -> Self {
        let mut s = [[0u64; RNG_LANES]; 4];
        for l in 0..RNG_LANES {
            for k in 0..4 {
                s[k][l] = splitmix64(&mut seed);
            }
        }
        Rng { s, buf: [0; RNG_BUF], idx: RNG_BUF }
    }

    /// Buffer order per step: the 8 high halves, then the 8 low halves.
    pub fn refill(&mut self) {
        #[cfg(target_feature = "avx512f")]
        unsafe {
            use std::arch::x86_64::*;
            let p = |k: usize| self.s[k].as_ptr() as *const __m512i;
            let (mut a, mut b, mut c, mut d) =
                (_mm512_loadu_si512(p(0)), _mm512_loadu_si512(p(1)), _mm512_loadu_si512(p(2)), _mm512_loadu_si512(p(3)));
            let hi_lo = _mm512_setr_epi32(1, 3, 5, 7, 9, 11, 13, 15, 0, 2, 4, 6, 8, 10, 12, 14);
            for step in 0..RNG_STEPS {
                let r = _mm512_add_epi64(_mm512_rol_epi64::<23>(_mm512_add_epi64(a, d)), a);
                let t = _mm512_slli_epi64::<17>(b);
                c = _mm512_xor_si512(c, a);
                d = _mm512_xor_si512(d, b);
                b = _mm512_xor_si512(b, c);
                a = _mm512_xor_si512(a, d);
                c = _mm512_xor_si512(c, t);
                d = _mm512_rol_epi64::<45>(d);
                // One 512-bit store per block so a 512-bit reload can be store-forwarded.
                let out = self.buf.as_mut_ptr().add(step * 2 * RNG_LANES) as *mut __m512i;
                _mm512_storeu_si512(out, _mm512_permutexvar_epi32(hi_lo, r));
            }
            for (k, v) in [a, b, c, d].into_iter().enumerate() {
                _mm512_storeu_si512(self.s[k].as_mut_ptr() as *mut __m512i, v);
            }
        }
        #[cfg(not(target_feature = "avx512f"))]
        for step in 0..RNG_STEPS {
            for l in 0..RNG_LANES {
                let s = &mut self.s;
                let r = s[0][l].wrapping_add(s[3][l]).rotate_left(23).wrapping_add(s[0][l]);
                let t = s[1][l] << 17;
                s[2][l] ^= s[0][l];
                s[3][l] ^= s[1][l];
                s[1][l] ^= s[2][l];
                s[0][l] ^= s[3][l];
                s[2][l] ^= t;
                s[3][l] = s[3][l].rotate_left(45);
                self.buf[step * 2 * RNG_LANES + l] = (r >> 32) as u32;
                self.buf[step * 2 * RNG_LANES + RNG_LANES + l] = r as u32;
            }
        }
        self.idx = 0;
    }

    #[inline(always)]
    pub fn next32(&mut self) -> u32 {
        if self.idx == RNG_BUF {
            self.refill();
        }
        let w = self.buf[self.idx];
        self.idx += 1;
        w
    }

    /// Uniform in [0, range).
    #[inline(always)]
    pub fn below(&mut self, range: u32) -> u32 {
        let mut m = self.next32() as u64 * range as u64;
        let mut l = m as u32;
        if l < range {
            let t = range.wrapping_neg() % range;
            while l < t {
                m = self.next32() as u64 * range as u64;
                l = m as u32;
            }
        }
        (m >> 32) as u32
    }
}

// ---------------------------------------------------------------- Shoe
// Pre-shuffled: a reshuffle runs Fisher-Yates over the first SHUF_EAGER positions at once, so
// dealing is a plain array read and the RNG is off the game's critical path. A round that runs
// past the shuffled prefix (only possible with splits) extends it one Fisher-Yates step at a
// time; the card at each position is the same either way. A round starts at pos <= 259, and a
// round without re-splits uses at most ~25 cards, so the vector engine never needs to extend.
// Each shoe owns its RNG, so its card sequence depends only on its seed.
pub const SHUF_EAGER: usize = 288;
pub const SHOE_BYTES: usize = 320; // padded so 4-byte reads near the end stay in bounds

#[derive(Clone)]
#[repr(C, align(64))]
pub struct Shoe {
    pub cards: [u8; SHOE_BYTES],
    pub pos: usize,
    pub shuffled: usize,
    pub rng: Rng,
}

impl Shoe {
    pub fn new(seed: u64) -> Self {
        let mut cards = [0u8; SHOE_BYTES];
        let mut i = 0;
        for _ in 0..DECKS * 4 {
            for r in 1..=13u8 {
                cards[i] = r.min(10);
                i += 1;
            }
        }
        let mut s = Shoe { cards, pos: 0, shuffled: 0, rng: Rng::new(seed) };
        s.shuffle();
        s
    }

    pub fn fy_step(&mut self, i: usize) {
        let j = i + self.rng.below((CARDS - i) as u32) as usize;
        self.cards.swap(i, j);
    }

    /// Same result as `for i < SHUF_EAGER: fy_step(i)`. The swap targets depend only on the RNG
    /// stream, not on the swaps, so they are all computed first: 16 at a time with one vector
    /// multiply whenever the buffer holds an aligned block of 16 words and none of them can hit
    /// Lemire's rejection zone (probability ~1e-6 per block), else one at a time via below().
    pub fn shuffle(&mut self) {
        let mut jt = [0u16; SHUF_EAGER];
        let mut i = 0;
        #[cfg(target_feature = "avx512f")]
        unsafe {
            use std::arch::x86_64::*;
            let lane = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
            while i + 16 <= SHUF_EAGER {
                if self.rng.idx == RNG_BUF {
                    self.rng.refill();
                }
                if self.rng.idx & 15 != 0 {
                    // unaligned after a scalar step: realign one word at a time
                    jt[i] = (i + self.rng.below((CARDS - i) as u32) as usize) as u16;
                    i += 1;
                    continue;
                }
                let first = _mm512_add_epi32(_mm512_set1_epi32(i as i32), lane);
                let range = _mm512_sub_epi32(_mm512_set1_epi32(CARDS as i32), first);
                let w = _mm512_loadu_si512(self.rng.buf.as_ptr().add(self.rng.idx) as *const __m512i);
                let me = _mm512_mul_epu32(w, range); // even lanes: 64-bit products
                let mo = _mm512_mul_epu32(_mm512_srli_epi64::<32>(w), _mm512_srli_epi64::<32>(range));
                let hi = _mm512_mask_blend_epi32(0xAAAA, _mm512_srli_epi64::<32>(me), mo);
                let lo = _mm512_mask_blend_epi32(0xAAAA, me, _mm512_slli_epi64::<32>(mo));
                if _mm512_cmplt_epu32_mask(lo, range) != 0 {
                    // possible rejection: exact scalar path
                    for _ in 0..16 {
                        jt[i] = (i + self.rng.below((CARDS - i) as u32) as usize) as u16;
                        i += 1;
                    }
                    continue;
                }
                // Unaligned store: i is not a multiple of 16 after a scalar realignment step.
                _mm256_storeu_si256(jt.as_mut_ptr().add(i) as *mut __m256i, _mm512_cvtepi32_epi16(_mm512_add_epi32(first, hi)));
                self.rng.idx += 16;
                i += 16;
            }
        }
        while i < SHUF_EAGER {
            jt[i] = (i + self.rng.below((CARDS - i) as u32) as usize) as u16;
            i += 1;
        }
        for (i, &j) in jt.iter().enumerate() {
            // load both before storing: fewer store-forwarding stalls
            let (x, y) = (self.cards[i], self.cards[j as usize]);
            self.cards[i] = y;
            self.cards[j as usize] = x;
        }
        self.shuffled = SHUF_EAGER;
        self.pos = 0;
    }

    #[inline(always)]
    pub fn draw(&mut self) -> u8 {
        if self.pos >= self.shuffled {
            self.fy_step(self.shuffled);
            self.shuffled += 1;
        }
        let c = self.cards[self.pos];
        self.pos += 1;
        c
    }
}

// ---------------------------------------------------------------- Hand
#[derive(Clone, Copy)]
pub struct Hand {
    pub sum: u8,      // aces counted as 1
    pub ncards: u8,
    pub first: u8,    // first card rank (pair detection)
    pub bet_mult: u8, // 1, or 2 after doubling
    pub ace: bool,
    pub split_aces: bool,
    pub surrendered: bool,
}

impl Default for Hand {
    fn default() -> Self {
        Hand { sum: 0, ncards: 0, first: 0, bet_mult: 1, ace: false, split_aces: false, surrendered: false }
    }
}

impl Hand {
    pub fn from(cards: &[u8]) -> Self {
        let mut h = Hand::default();
        for &c in cards {
            h.add(c);
        }
        h
    }
    #[inline(always)]
    pub fn add(&mut self, c: u8) {
        if self.ncards == 0 {
            self.first = c;
        }
        self.sum += c;
        self.ace |= c == 1;
        self.ncards += 1;
    }
    #[inline(always)]
    pub fn soft(&self) -> bool {
        self.ace & (self.sum <= 11) // bitwise: no short-circuit branch
    }
    #[inline(always)]
    pub fn value(&self) -> i32 {
        self.sum as i32 + 10 * self.soft() as i32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Action {
    Hit,
    Stand,
    Double,
    Split,
    Surrender,
}
use Action::*;

// ---------------------------------------------------------------- Stats
/// Money is tracked in half-units so 3:2, insurance and surrender stay integral.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Stats {
    pub rounds: u64,
    pub hands: u64,
    pub shuffles: u64,
    pub net: i64,         // spread-bet result, half-units
    pub initial_bet: u64, // sum of initial bets, half-units
    pub action: u64,      // all money put at risk incl. doubles/splits/insurance
    pub net_sq: u64,      // sum of squared per-round net (half-units^2)
    pub flat_net: i64,    // same play with a 1-unit bet every round
    pub flat_sq: u64,
    pub wins: u64,
    pub losses: u64,
    pub pushes: u64,
    pub player_bj: u64,
    pub dealer_bj: u64,
    pub doubles: u64,
    pub splits: u64,
    pub surrenders: u64,
    pub busts: u64,
    pub ins_taken: u64,
    pub ins_won: u64,
    pub tc_rounds: [u64; TC_BUCKETS],
    pub tc_flat_net: [i64; TC_BUCKETS],
}

impl Stats {
    pub fn merge(&mut self, o: &Stats) {
        self.rounds += o.rounds;
        self.hands += o.hands;
        self.shuffles += o.shuffles;
        self.net += o.net;
        self.initial_bet += o.initial_bet;
        self.action += o.action;
        self.net_sq += o.net_sq;
        self.flat_net += o.flat_net;
        self.flat_sq += o.flat_sq;
        self.wins += o.wins;
        self.losses += o.losses;
        self.pushes += o.pushes;
        self.player_bj += o.player_bj;
        self.dealer_bj += o.dealer_bj;
        self.doubles += o.doubles;
        self.splits += o.splits;
        self.surrenders += o.surrenders;
        self.busts += o.busts;
        self.ins_taken += o.ins_taken;
        self.ins_won += o.ins_won;
        for i in 0..TC_BUCKETS {
            self.tc_rounds[i] += o.tc_rounds[i];
            self.tc_flat_net[i] += o.tc_flat_net[i];
        }
    }
}

/// Bet ramp in units, keyed on floored true count: 1-12 spread.
pub fn bet_units(tc: i32) -> i32 {
    const RAMP: [i8; 8] = [1, 1, 2, 4, 6, 8, 10, 12]; // tc <= 1, 2, ..., >= 7
    RAMP[tc.clamp(1, 7) as usize] as i32
}

// ---------------------------------------------------------------- Strategy matrix
// Every decision is one lookup in a [hand class][upcard][rule flags] table, then one exact
// true-count compare: action = tc >= k ? alt : base. The table is generated from Sim::decide()
// (the readable spec) by probing it across true counts, so the two can't drift apart.
// Hand classes: hard total 0..21, soft 22..31 (sum with ace as 1 + 20), pair 32..41 (31 + rank).
pub const NCLASS: usize = 42;
pub const NFLAGS: usize = 8; // flags: double | split << 1 | surrender << 2

#[derive(Clone, Copy)]
pub struct Entry {
    base: Action,
    alt: Action,
    k: i8,
}

/// ~11 KB, built once, shared read-only by all threads.
pub struct Table {
    e: [[[Entry; NFLAGS]; 11]; NCLASS],
}

impl Table {
    /// (base action, deviation action, index) for a hand class, upcard and rule flags.
    pub fn entry(&self, cls: usize, up: usize, flags: usize) -> (u8, u8, i8) {
        let e = self.e[cls][up][flags];
        (e.base as u8, e.alt as u8, e.k)
    }
}

#[inline(always)]
fn hand_class(h: &Hand) -> usize {
    let (pair, soft, hard) = (31 + h.first as usize, 20 + h.sum as usize, h.sum as usize);
    let is_pair = (h.ncards == 2) & (h.sum == 2 * h.first);
    if is_pair { pair } else if h.soft() { soft } else { hard }
}

/// Probe decide() at true counts -40..+40 (two decks unseen, rc = 2 * tc) for every hand class,
/// upcard and flag combination. Each cell must be a single step: base below k, alt at k and above.
fn build_table(counting: bool) -> Table {
    let mut t = Table { e: [[[Entry { base: Hit, alt: Hit, k: 0 }; NFLAGS]; 11]; NCLASS] };
    let mut probe = Sim::with_table(0, counting, None);
    probe.shoe.pos = CARDS - 104;
    for cls in 0..NCLASS {
        let h = if cls >= 32 {
            let r = (cls - 31) as u8;
            Hand::from(&[r, r])
        } else if cls >= 22 {
            Hand { sum: (cls - 20) as u8, ace: true, ncards: 3, ..Hand::default() }
        } else {
            Hand { sum: cls as u8, ncards: 3, ..Hand::default() }
        };
        for up in 1..=10u8 {
            for f in 0..NFLAGS {
                let mut at = |tc: i32| {
                    probe.rc = 2 * tc;
                    probe.decide(&h, up, f & 1 != 0, f & 2 != 0, f & 4 != 0)
                };
                let mut e = Entry { base: at(-40), alt: at(40), k: 0 };
                if e.base != e.alt {
                    e.k = -40;
                    while at(e.k as i32) != e.alt {
                        e.k += 1;
                    }
                }
                for tc in -40..=40 {
                    let want = if tc >= e.k as i32 { e.alt } else { e.base };
                    assert!(at(tc) == want, "not a single-step index");
                }
                t.e[cls][up as usize][f] = e;
            }
        }
    }
    t
}

pub fn strategy_table(counting: bool) -> &'static Table {
    static HILO: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    static BASIC: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    if counting { HILO.get_or_init(|| build_table(true)) } else { BASIC.get_or_init(|| build_table(false)) }
}

// ---------------------------------------------------------------- Player
#[derive(Clone)]
pub struct Sim {
    pub shoe: Shoe,
    pub rc: i32,           // Hi-Lo running count
    pub unseen_extra: i32, // 1 while the dealer's hole card is drawn but not yet seen
    pub counting: bool,    // false = flat-bet basic strategy (for validation)
    pub st: Stats,
    table: Option<&'static Table>,
}

impl Sim {
    pub fn new(seed: u64, counting: bool) -> Self {
        Self::with_table(seed, counting, Some(strategy_table(counting)))
    }

    fn with_table(seed: u64, counting: bool, table: Option<&'static Table>) -> Self {
        Sim { shoe: Shoe::new(seed), rc: 0, unseen_extra: 0, counting, st: Stats::default(), table }
    }

    /// Table-driven equivalent of decide().
    #[inline(always)]
    pub fn lookup(&self, h: &Hand, up: u8, can_double: bool, can_split: bool, can_surrender: bool) -> Action {
        let flags = can_double as usize | (can_split as usize) << 1 | (can_surrender as usize) << 2;
        let e = &self.table.unwrap().e[hand_class(h)][up as usize][flags];
        if self.tc_ge(e.k as i32) { e.alt } else { e.base }
    }

    #[inline(always)]
    fn unseen(&self) -> i32 {
        (CARDS - self.shoe.pos) as i32 + self.unseen_extra
    }

    /// Exact true-count comparison: rc / (unseen/52) >= k, with no division or rounding.
    #[inline(always)]
    pub fn tc_ge(&self, k: i32) -> bool {
        self.rc * 52 >= k * self.unseen()
    }

    /// Index plays fire only when counting; otherwise fall back to basic strategy.
    #[inline(always)]
    fn idx(&self, k: i32, basic: bool) -> bool {
        if self.counting { self.tc_ge(k) } else { basic }
    }

    #[inline(always)]
    fn draw_seen(&mut self) -> u8 {
        let c = self.shoe.draw();
        self.rc += HILO[c as usize] as i32;
        c
    }

    pub fn decide(&self, h: &Hand, up: u8, can_double: bool, can_split: bool, can_surrender: bool) -> Action {
        let v = h.value();
        let pair = h.ncards == 2 && h.sum == 2 * h.first;

        if can_surrender && !h.soft() && !(pair && h.first == 8) {
            if v == 16 && (up == 9 || up == 10 || up == 1) {
                return Surrender;
            }
            if v == 15 && up == 10 && self.idx(0, true) {
                return Surrender;
            }
            if v == 15 && up == 9 && self.idx(2, false) {
                return Surrender;
            }
            if v == 15 && up == 1 && self.idx(1, false) {
                return Surrender;
            }
            if v == 14 && up == 10 && self.idx(3, false) {
                return Surrender;
            }
        }

        if pair && can_split {
            let split = match h.first {
                1 | 8 => true,
                10 => (up == 5 && self.idx(5, false)) || (up == 6 && self.idx(4, false)),
                9 => (2..=6).contains(&up) || up == 8 || up == 9,
                7 => (2..=7).contains(&up),
                6 => (2..=6).contains(&up),
                4 => up == 5 || up == 6,
                2 | 3 => (2..=7).contains(&up),
                _ => false, // 5,5 plays as hard 10
            };
            if split {
                return Split;
            }
        }

        if h.soft() {
            if v >= 19 {
                return Stand;
            }
            if v == 18 {
                if (3..=6).contains(&up) {
                    return if can_double { Double } else { Stand };
                }
                if up == 2 || up == 7 || up == 8 {
                    return Stand;
                }
                return Hit;
            }
            let dbl = (v == 17 && (3..=6).contains(&up))
                || ((v == 15 || v == 16) && (4..=6).contains(&up))
                || ((v == 13 || v == 14) && (5..=6).contains(&up));
            return if dbl && can_double { Double } else { Hit };
        }

        if v >= 17 {
            return Stand;
        }
        if v >= 13 {
            let stand = match (v, up) {
                (16, 10) => self.idx(0, false),
                (15, 10) => self.idx(4, false),
                (16, 9) => self.idx(5, false),
                (13, 2) => self.idx(-1, true),
                (13, 3) => self.idx(-2, true),
                _ => (2..=6).contains(&up),
            };
            return if stand { Stand } else { Hit };
        }
        if v == 12 {
            let stand = match up {
                2 => self.idx(3, false),
                3 => self.idx(2, false),
                4 => self.idx(0, true),
                5 => self.idx(-2, true),
                6 => self.idx(-1, true),
                _ => false,
            };
            return if stand { Stand } else { Hit };
        }
        let dbl = match v {
            11 => up != 1 || self.idx(1, false),
            10 => (2..=9).contains(&up) || ((up == 10 || up == 1) && self.idx(4, false)),
            9 => (3..=6).contains(&up) || (up == 2 && self.idx(1, false)) || (up == 7 && self.idx(3, false)),
            _ => false,
        };
        if dbl && can_double { Double } else { Hit }
    }

    pub fn play_round(&mut self) {
        if self.shoe.pos >= CUT {
            self.shoe.shuffle();
            self.rc = 0;
            self.st.shuffles += 1;
        }
        let tc = (self.rc * 52).div_euclid((CARDS - self.shoe.pos) as i32); // floor
        let units = if self.counting { bet_units(tc) } else { 1 };
        let b = 2 * units as i64; // initial bet, half-units
        let mut net: i64 = 0;
        self.st.rounds += 1;
        self.st.initial_bet += b as u64;
        self.st.action += b as u64;

        let mut hands = [Hand::default(); MAX_HANDS];
        let mut nh = 1;
        let c = self.draw_seen();
        hands[0].add(c);
        let up = self.draw_seen();
        let c = self.draw_seen();
        hands[0].add(c);
        let hole = self.shoe.draw();
        self.unseen_extra = 1;

        let dealer_bj = (up == 1 && hole == 10) || (up == 10 && hole == 1);
        let player_bj = hands[0].value() == 21;

        if up == 1 && self.counting && self.tc_ge(3) {
            self.st.ins_taken += 1;
            self.st.action += (b / 2) as u64;
            if dealer_bj {
                net += b;
                self.st.ins_won += 1;
            } else {
                net -= b / 2;
            }
        }

        if dealer_bj || player_bj {
            self.rc += HILO[hole as usize] as i32;
            self.unseen_extra = 0;
            self.st.hands += 1;
            if dealer_bj {
                self.st.dealer_bj += 1;
            }
            if player_bj {
                self.st.player_bj += 1;
            }
            if dealer_bj && player_bj {
                self.st.pushes += 1;
            } else if dealer_bj {
                net -= b;
                self.st.losses += 1;
            } else {
                net += 3 * b / 2;
                self.st.wins += 1;
            }
            self.finish(net, units, tc);
            return;
        }

        let mut any_live = false;
        let mut i = 0;
        while i < nh {
            let mut h = hands[i];
            if h.ncards == 1 {
                // second card for a split hand
                let c = self.draw_seen();
                h.add(c);
                if h.split_aces {
                    hands[i] = h;
                    any_live = true;
                    i += 1;
                    continue;
                }
            }
            loop {
                if h.value() >= 21 {
                    break;
                }
                let two = h.ncards == 2;
                // Split aces never reach here, so no resplitting aces falls out naturally.
                match self.lookup(&h, up, two, two && nh < MAX_HANDS, two && nh == 1) {
                    Surrender => {
                        h.surrendered = true;
                        break;
                    }
                    Stand => break,
                    Hit => {
                        let c = self.draw_seen();
                        h.add(c);
                    }
                    Double => {
                        h.bet_mult = 2;
                        self.st.doubles += 1;
                        self.st.action += b as u64;
                        let c = self.draw_seen();
                        h.add(c);
                        break;
                    }
                    Split => {
                        self.st.splits += 1;
                        self.st.action += b as u64;
                        let c = h.first;
                        let mut nhand = Hand::default();
                        nhand.add(c);
                        h = Hand::default();
                        h.add(c);
                        if c == 1 {
                            h.split_aces = true;
                            nhand.split_aces = true;
                        }
                        hands[nh] = nhand;
                        nh += 1;
                        let c = self.draw_seen();
                        h.add(c);
                        if h.split_aces {
                            break;
                        }
                    }
                }
            }
            if !h.surrendered && h.value() <= 21 {
                any_live = true;
            }
            hands[i] = h;
            i += 1;
        }

        self.rc += HILO[hole as usize] as i32;
        self.unseen_extra = 0;
        let mut d = Hand::default();
        d.add(up);
        d.add(hole);
        if any_live {
            while d.value() < 17 {
                let c = self.draw_seen();
                d.add(c);
            }
        }
        let dv = d.value();

        for h in &hands[..nh] {
            let w = b * h.bet_mult as i64;
            let pv = h.value();
            // Branchless settle: outcomes are random, so if-chains here mispredict constantly.
            let surr = h.surrendered;
            let bust = !surr & (pv > 21);
            let win = !surr & !bust & ((dv > 21) | (pv > dv));
            let push = !surr & !bust & (dv <= 21) & (pv == dv);
            let loss = !surr & !win & !push;
            net += w * (win as i64 - loss as i64) - if surr { b / 2 } else { 0 };
            let st = &mut self.st;
            st.hands += 1;
            st.wins += win as u64;
            st.pushes += push as u64;
            st.losses += (loss | surr) as u64;
            st.busts += bust as u64;
            st.surrenders += surr as u64;
        }
        self.finish(net, units, tc);
    }

    fn finish(&mut self, net: i64, units: i32, tc: i32) {
        let st = &mut self.st;
        st.net += net;
        st.net_sq += (net * net) as u64;
        let flat = net / units as i64; // every amount scales with the bet, so this is exact
        st.flat_net += flat;
        st.flat_sq += (flat * flat) as u64;
        let bucket = (tc.clamp(TC_MIN, TC_MAX) - TC_MIN) as usize;
        st.tc_rounds[bucket] += 1;
        st.tc_flat_net[bucket] += flat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_tc(tc: i32) -> Sim {
        let mut s = Sim::new(1, true);
        s.shoe.pos = CARDS - 104; // two decks unseen
        s.rc = tc * 2;
        s
    }

    fn same_rng(a: &Rng, b: &Rng) -> bool {
        a.s == b.s && a.idx == b.idx && a.buf[a.idx..] == b.buf[b.idx..]
    }

    /// Reference definition of a shuffle: textbook Fisher-Yates, one bounded draw per position.
    fn reference_shuffle(s: &mut Shoe) {
        for i in 0..SHUF_EAGER {
            s.fy_step(i);
        }
        s.shuffled = SHUF_EAGER;
        s.pos = 0;
    }

    #[test]
    fn rng_refill_matches_scalar_xoshiro() {
        let (mut r, mut s) = (Rng::new(42), Rng::new(42).s);
        for _ in 0..10 {
            r.refill();
            for step in 0..RNG_STEPS {
                for l in 0..RNG_LANES {
                    let out = s[0][l].wrapping_add(s[3][l]).rotate_left(23).wrapping_add(s[0][l]);
                    let t = s[1][l] << 17;
                    s[2][l] ^= s[0][l];
                    s[3][l] ^= s[1][l];
                    s[1][l] ^= s[2][l];
                    s[0][l] ^= s[3][l];
                    s[2][l] ^= t;
                    s[3][l] = s[3][l].rotate_left(45);
                    assert_eq!(r.buf[step * 16 + l], (out >> 32) as u32);
                    assert_eq!(r.buf[step * 16 + 8 + l], out as u32);
                }
            }
        }
    }

    #[test]
    fn lane_seeds_never_repeat_across_sets() {
        let mut seen = std::collections::HashSet::new();
        for seed in [1u64, 2] {
            for set in 0..128 {
                let mut seeder = set_seed(seed, set);
                for _ in 0..48 {
                    assert!(seen.insert(splitmix64(&mut seeder)));
                }
            }
        }
    }

    #[test]
    fn rng_bounded_uniform() {
        let mut rng = Rng::new(3);
        let mut b = [0; 13];
        for _ in 0..1_300_000 {
            b[rng.below(13) as usize] += 1;
        }
        assert!(b.iter().all(|&x| x > 99_000 && x < 101_000));
    }

    #[test]
    fn shoe_composition_and_balance() {
        let mut sh = Shoe::new(9);
        for _ in 0..1000 {
            let mut counts = [0; 11];
            let mut sum = 0i32;
            for &c in &sh.cards[..CARDS] {
                counts[c as usize] += 1;
                sum += HILO[c as usize] as i32;
            }
            assert!(counts[1..=9].iter().all(|&n| n == 24));
            assert_eq!(counts[10], 96);
            assert_eq!(sum, 0);
            sh.shuffle();
        }
    }

    #[test]
    fn vector_shuffle_matches_textbook_fisher_yates() {
        // From misaligned RNG positions, and with blocks forced onto the exact rejection path.
        for skew in 0..40 {
            let mut a = Shoe::new(100 + skew as u64);
            for _ in 0..skew {
                a.rng.next32();
            }
            if skew >= 20 {
                let i = a.rng.idx;
                for w in &mut a.rng.buf[i..] {
                    *w &= 0xFF;
                }
            }
            let mut b = a.clone();
            a.shuffle();
            reference_shuffle(&mut b);
            assert_eq!(a.cards, b.cards, "skew {skew}");
            assert!(same_rng(&a.rng, &b.rng), "skew {skew}");
        }
    }

    #[test]
    fn extension_continues_fisher_yates() {
        let (mut a, mut b) = (Shoe::new(5), Shoe::new(5));
        for i in SHUF_EAGER..CARDS {
            b.fy_step(i);
        }
        a.pos = SHUF_EAGER;
        for i in SHUF_EAGER..CARDS {
            assert_eq!(a.draw(), b.cards[i]);
        }
    }

    #[test]
    fn positions_uniform_over_ranks() {
        // Chi-square, 9 degrees of freedom; the 99.99% critical value is 33.7.
        const SHOES: usize = 60_000;
        for p in [0usize, 1, 100, 259, 287] {
            let mut sh = Shoe::new(77 + p as u64);
            let mut counts = [0f64; 11];
            for _ in 0..SHOES {
                sh.shuffle();
                counts[sh.cards[p] as usize] += 1.0;
            }
            let chi: f64 = (1..=10)
                .map(|r| {
                    let e = SHOES as f64 * if r == 10 { 96.0 } else { 24.0 } / CARDS as f64;
                    (counts[r] - e).powi(2) / e
                })
                .sum();
            assert!(chi < 33.7, "position {p}: chi-square {chi:.1}");
        }
    }

    #[test]
    fn hand_values() {
        assert!(Hand::from(&[1, 6]).value() == 17 && Hand::from(&[1, 6]).soft());
        assert!(Hand::from(&[1, 6, 10]).value() == 17 && !Hand::from(&[1, 6, 10]).soft());
        assert_eq!(Hand::from(&[1, 1, 9]).value(), 21);
        assert_eq!(Hand::from(&[10, 1]).value(), 21);
        assert_eq!((-1i32).div_euclid(4), -1);
        assert_eq!((-4i32).div_euclid(4), -1);
        assert_eq!([1, 2, 4, 9].map(bet_units), [1, 2, 6, 12]);
    }

    #[test]
    fn basic_strategy_spots() {
        let s = at_tc(0);
        let d = |c: &[u8], up, dbl, spl, sur| s.decide(&Hand::from(c), up, dbl, spl, sur);
        assert_eq!(d(&[10, 6], 10, true, true, true), Surrender);
        assert_eq!(d(&[8, 8], 10, true, true, true), Split);
        assert_eq!(d(&[1, 1], 1, true, true, true), Split);
        assert_eq!(d(&[5, 5], 6, true, true, true), Double);
        assert_eq!(d(&[9, 9], 7, true, true, true), Stand);
        assert_eq!(d(&[9, 9], 8, true, true, true), Split);
        assert_eq!(d(&[6, 5], 1, true, true, true), Hit);
        assert_eq!(d(&[1, 7], 9, true, true, true), Hit);
        assert_eq!(d(&[1, 7], 6, true, true, true), Double);
        assert_eq!(d(&[1, 7], 6, false, true, true), Stand);
        assert_eq!(d(&[1, 6], 2, true, true, true), Hit);
        assert_eq!(d(&[10, 2], 4, true, true, true), Stand);
        assert_eq!(d(&[10, 3], 2, true, true, true), Stand);
        assert_eq!(d(&[10, 6], 7, true, true, true), Hit);
        assert_eq!(d(&[5, 4, 7], 10, false, false, false), Stand); // 16v10 at TC 0
    }

    #[test]
    fn illustrious_18_and_fab_4_thresholds() {
        // (cards, upcard, index, action one below the index, action at the index)
        let devs: &[(&[u8], u8, i32, Action, Action)] = &[
            (&[10, 5], 10, 4, Surrender, Surrender),
            (&[4, 3, 2, 6], 10, 4, Hit, Stand),
            (&[10, 10], 5, 5, Stand, Split),
            (&[10, 10], 6, 4, Stand, Split),
            (&[6, 4], 10, 4, Hit, Double),
            (&[10, 2], 3, 2, Hit, Stand),
            (&[10, 2], 2, 3, Hit, Stand),
            (&[6, 5], 1, 1, Hit, Double),
            (&[5, 4], 2, 1, Hit, Double),
            (&[6, 4], 1, 4, Hit, Double),
            (&[5, 4], 7, 3, Hit, Double),
            (&[3, 4, 9], 9, 5, Hit, Stand),
            (&[10, 3], 2, -1, Hit, Stand),
            (&[10, 2], 4, 0, Hit, Stand),
            (&[10, 2], 5, -2, Hit, Stand),
            (&[10, 2], 6, -1, Hit, Stand),
            (&[10, 3], 3, -2, Hit, Stand),
            (&[10, 4], 10, 3, Hit, Surrender),
            (&[10, 5], 9, 2, Hit, Surrender),
            (&[10, 5], 1, 1, Hit, Surrender),
            (&[9, 7], 10, 0, Hit, Stand), // 16v10 stand index, surrender unavailable
        ];
        for &(cards, up, index, below, at) in devs {
            let h = Hand::from(cards);
            let two = h.ncards == 2;
            let surr = two && at != Stand;
            assert_eq!(at_tc(index - 1).decide(&h, up, two, true, surr), below, "{cards:?} v {up} below");
            assert_eq!(at_tc(index).decide(&h, up, two, true, surr), at, "{cards:?} v {up} at");
        }
        assert_eq!(at_tc(-1).decide(&Hand::from(&[10, 5]), 10, true, true, true), Hit);
    }

    #[test]
    fn table_matches_decide_in_play() {
        for counting in [true, false] {
            let mut s = Sim::new(5, counting);
            let mut r = Rng::new(6);
            for _ in 0..20_000 {
                s.play_round();
                let up = (r.below(10) + 1) as u8;
                let h = Hand::from(&[(r.below(10) + 1) as u8, (r.below(10) + 1) as u8]);
                for f in 0..8 {
                    let (d, p, u) = (f & 1 != 0, f & 2 != 0, f & 4 != 0);
                    assert_eq!(s.lookup(&h, up, d, p, u), s.decide(&h, up, d, p, u));
                }
            }
        }
    }

    #[test]
    fn basic_mode_ignores_count() {
        let mut s = Sim::new(1, false);
        s.shoe.pos = CARDS - 52;
        s.rc = 20;
        assert_eq!(s.decide(&Hand::from(&[10, 10]), 6, true, true, true), Stand);
        assert_eq!(s.decide(&Hand::from(&[10, 6]), 10, true, true, false), Hit);
    }

    #[test]
    fn running_count_matches_dealt_cards() {
        let mut s = Sim::new(11, true);
        for _ in 0..200_000 {
            s.play_round();
            let rc: i32 = s.shoe.cards[..s.shoe.pos].iter().map(|&c| HILO[c as usize] as i32).sum();
            assert_eq!(rc, s.rc);
        }
        assert_eq!(s.st.rounds, 200_000);
    }

    /// Engine<G> must match independent scalar Sims exactly, lane by lane.
    #[cfg(target_feature = "avx512f")]
    fn check_engine<const G: usize>(mut seed: u64, counting: bool, rounds: u64, extra: usize) {
        let mut vec: Vec<Sim> = (0..G * 16).map(|_| Sim::new(splitmix64(&mut seed), counting)).collect();
        let mut reference = vec.clone();
        engine::Engine::<G>::new(&mut vec, counting).run(rounds);
        for s in &mut vec[..extra] {
            s.play_round();
        }
        for (l, s) in reference.iter_mut().enumerate() {
            for _ in 0..rounds + u64::from(l < extra) {
                s.play_round();
            }
        }
        for (a, b) in vec.iter().zip(&reference) {
            assert_eq!(a.st, b.st);
            assert_eq!((a.shoe.pos, a.rc, a.shoe.shuffled), (b.shoe.pos, b.rc, b.shoe.shuffled));
            assert_eq!(a.shoe.cards, b.shoe.cards);
            assert!(same_rng(&a.shoe.rng, &b.shoe.rng));
        }
        assert!(reference.iter().map(|s| s.st.splits).sum::<u64>() > 0);
    }

    #[cfg(target_feature = "avx512f")]
    #[test]
    fn vector_engine_matches_reference() {
        check_engine::<1>(1, true, 20_000, 0);
        check_engine::<1>(2, false, 20_000, 7);
        check_engine::<3>(3, true, 6_000, 13);
        check_engine::<3>(4, false, 6_000, 0);
    }

    #[cfg(target_feature = "avx512f")]
    #[test]
    fn edges_are_plausible() {
        // Flat-bet basic strategy loses close to the published house edge for these rules
        // (~0.35-0.40%); the Hi-Lo spread player wins. 20M rounds: CI about +/-0.05%.
        let edge = |counting: bool| {
            let mut seed = if counting { 21 } else { 22 };
            let mut lanes: Vec<Sim> = (0..48).map(|_| Sim::new(splitmix64(&mut seed), counting)).collect();
            engine::Engine::<3>::new(&mut lanes, counting).run(20_000_000 / 48);
            let mut t = Stats::default();
            for s in &lanes {
                t.merge(&s.st);
            }
            100.0 * t.net as f64 / t.initial_bet as f64
        };
        let (basic, hilo) = (edge(false), edge(true));
        assert!(basic > -0.50 && basic < -0.25, "basic {basic}");
        assert!(hilo > 0.8 && hilo < 1.6, "hilo {hilo}");
    }
}
