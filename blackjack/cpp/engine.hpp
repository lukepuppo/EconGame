// AVX-512 lockstep engine: games run one per 32-bit vector lane, in G groups of 16 lanes.
//
// Each lane is a full bj::Sim (its own pre-shuffled shoe, count and stats). A vector round plays
// one round in every lane at once, with control flow turned into lane masks (predication):
//   - all four initial cards come from one 32-bit gather per lane (the shoe is pre-shuffled, so
//     p1, up, p2 and the hole card are the 4 consecutive bytes at pos), and the next 8 cards are
//     gathered alongside into a register "card window" so later draws are byte shifts;
//   - decisions are gathers from the strategy matrix plus one exact true-count compare;
//   - the hit loops and the dealer's draw loop run until no lane is still drawing.
// The G groups are independent and are advanced phase by phase in turn, so one group's
// dependent gathers and loop latencies overlap another group's work (the round is otherwise
// latency-bound). A split plays both hands in the vector path; a lane that would split again
// (~0.3% of rounds) is rewound to the start of the round and replayed by the scalar reference
// Sim::play_round(). Reshuffles are done per lane by the scalar Shoe::shuffle(). Since the
// shoes are the same and every rule is applied identically, results are bit-identical to
// running each lane's Sim on its own.
#pragma once
#include <immintrin.h>

#include <array>

#include "bj.hpp"

namespace bj {

constexpr int VLANES = 16;

// Strategy matrix flattened for gathers: base | alt << 8 | k << 16 (k sign-extended).
struct FlatTable {
    int32_t e[NCLASS * 11 * NFLAGS];
    explicit FlatTable(const Table& t) {
        for (int c = 0; c < NCLASS; c++)
            for (int u = 0; u < 11; u++)
                for (int f = 0; f < NFLAGS; f++) {
                    const Entry& x = t.e[c][u][f];
                    e[(c * 11 + u) * NFLAGS + f] = x.base | x.alt << 8 | (int32_t)((uint32_t)(int32_t)x.k << 16);
                }
    }
};

inline const FlatTable& flat_table(bool counting) {
    static const FlatTable hilo(strategy_table(true)), basic(strategy_table(false));
    return counting ? hilo : basic;
}

// Hit-or-stand rule for hands of 3+ cards (no double, split or surrender), derived from the
// strategy matrix: stand iff value >= S, where for hard hands S = hard[up] minus one for each
// breakpoint b1[up], b2[up] with tc >= b, and for soft hands S = soft[up]. The builder checks the
// matrix really has this shape at every true count from -40 to +40. In the engine, tc >= b is
// kept as D_b = rc * 52 - b * unseen >= 0, updated by adds on every draw, so a hit-loop decision
// needs no gather and no multiply.
struct HitRule {
    alignas(64) int32_t hard[16] = {}, soft[16] = {}, b1[16] = {}, b2[16] = {};
    static constexpr int NEVER = 1000;  // breakpoint no count reaches
    explicit HitRule(const Table& t) {
        auto stands = [&](int cls, int up, int tc) {
            const Entry& e = t.e[cls][up][0];
            return (tc >= e.k ? e.alt : e.base) == STAND;
        };
        // Smallest value from which every higher value stands (and every lower one hits).
        auto threshold = [&](int first_cls, int lo, int hi, int up, int tc) {
            int S = hi + 1;
            for (int v = hi; v >= lo && stands(first_cls + v - lo, up, tc); v--) S = v;
            for (int v = lo; v < S; v++)
                if (stands(first_cls + v - lo, up, tc)) abort();  // not a threshold rule
            return S;
        };
        for (int up = 1; up <= 10; up++) {
            int prev = threshold(4, 4, 21, up, -40), steps = 0;
            hard[up] = prev;
            b1[up] = b2[up] = NEVER;
            int sft = threshold(22, 12, 21, up, -40);  // soft classes 22..31 = soft 12..21
            soft[up] = sft;
            for (int tc = -39; tc <= 40; tc++) {
                if (threshold(22, 12, 21, up, tc) != sft) abort();  // soft play must not depend on the count
                int S = threshold(4, 4, 21, up, tc);
                if (S > prev) abort();  // a higher count must never mean standing less
                for (; prev > S; prev--, steps++) (steps == 0 ? b1 : b2)[up] = tc;
                if (steps > 2) abort();
            }
        }
    }
};

inline const HitRule& hit_rule(bool counting) {
    static const HitRule hilo(strategy_table(true)), basic(strategy_table(false));
    return counting ? hilo : basic;
}

#ifdef __AVX512F__

template <int G>
struct Engine {
    static constexpr int LANES = G * VLANES;
    Sim* lanes;  // LANES sims, contiguous
    bool counting;
    const int32_t* table;

    // Vector-path counters, per lane, flushed into lanes[l].st every FLUSH rounds (int32 safe).
    static constexpr int FLUSH = 1 << 12, NACC = 19;
    enum { ROUNDS, HANDS, NET, INIT, ACTION, NETSQ, FLAT, FLATSQ, WINS, LOSSES, PUSHES, PBJ, DBJ, DOUBLES, SURR,
           BUSTS, INS, INSW, SPLITS };
    alignas(64) int32_t acc[G][NACC][VLANES] = {};
    // True-count histogram per lane and bucket, packed: rounds << 19 plus the signed flat-net sum
    // (|sum| < 2^18 and rounds < 2^12 between flushes), so one gather + scatter updates both.
    static constexpr int HIST_SHIFT = 19;
    alignas(64) int32_t hist[LANES * TC_BUCKETS] = {};

    Engine(Sim* l, bool c) : lanes(l), counting(c), table(flat_table(c).e) {}

    static __m512i splat(int x) { return _mm512_set1_epi32(x); }

    void flush() {
        for (int l = 0; l < LANES; l++) {
            Stats& st = lanes[l].st;
            const auto& a = acc[l / VLANES];
            int k = l % VLANES;
            st.rounds += (uint32_t)a[ROUNDS][k]; st.hands += (uint32_t)a[HANDS][k]; st.net += a[NET][k];
            st.initial_bet += (uint32_t)a[INIT][k]; st.action += (uint32_t)a[ACTION][k]; st.net_sq += (uint32_t)a[NETSQ][k];
            st.flat_net += a[FLAT][k]; st.flat_sq += (uint32_t)a[FLATSQ][k]; st.wins += (uint32_t)a[WINS][k];
            st.losses += (uint32_t)a[LOSSES][k]; st.pushes += (uint32_t)a[PUSHES][k]; st.player_bj += (uint32_t)a[PBJ][k];
            st.dealer_bj += (uint32_t)a[DBJ][k]; st.doubles += (uint32_t)a[DOUBLES][k]; st.surrenders += (uint32_t)a[SURR][k];
            st.busts += (uint32_t)a[BUSTS][k]; st.ins_taken += (uint32_t)a[INS][k]; st.ins_won += (uint32_t)a[INSW][k];
            st.splits += (uint32_t)a[SPLITS][k];
            for (int b = 0; b < TC_BUCKETS; b++) {
                int32_t v = hist[l * TC_BUCKETS + b];
                int32_t flat = ((v + (1 << (HIST_SHIFT - 1))) & ((1 << HIST_SHIFT) - 1)) - (1 << (HIST_SHIFT - 1));
                st.tc_rounds[b] += (uint32_t)(v - flat) >> HIST_SHIFT;
                st.tc_flat_net[b] += flat;
                hist[l * TC_BUCKETS + b] = 0;
            }
        }
        memset(acc, 0, sizeof acc);
    }

    void run(uint64_t rounds_per_lane) {
        const uint8_t* base = lanes[0].shoe.cards;
        const __m512i HILO_V = _mm512_setr_epi32(0, -1, 1, 1, 1, 1, 1, 0, 0, 0, -1, 0, 0, 0, 0, 0);
        const __m512i RAMP_V = _mm512_setr_epi32(1, 1, 2, 4, 6, 8, 10, 12, 0, 0, 0, 0, 0, 0, 0, 0);
        const __m512i LANE_IDX = _mm512_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
        const __m512i ZERO = _mm512_setzero_si512(), ONE = splat(1), BYTE = splat(0xFF);

        // ---- per-group state (arrays indexed by compile-time group numbers)
        __m512i off[G], pos[G], rc[G], win1[G], win2[G], left[G];
        auto store_lanes = [&](int g, __mmask16 m) {  // vector pos/rc -> the Sims in m
            alignas(64) int32_t p[VLANES], r[VLANES];
            _mm512_store_si512(p, pos[g]); _mm512_store_si512(r, rc[g]);
            for (; m; m &= m - 1) {
                int k = __builtin_ctz(m);
                lanes[g * VLANES + k].shoe.pos = p[k]; lanes[g * VLANES + k].rc = r[k];
            }
        };
        auto load_lanes = [&](int g, __mmask16 m) {  // the Sims in m -> vector pos/rc
            alignas(64) int32_t p[VLANES], r[VLANES];
            for (int k = 0; k < VLANES; k++) { p[k] = lanes[g * VLANES + k].shoe.pos; r[k] = lanes[g * VLANES + k].rc; }
            pos[g] = _mm512_mask_load_epi32(pos[g], m, p); rc[g] = _mm512_mask_load_epi32(rc[g], m, r);
        };
        for (int g = 0; g < G; g++) {
            alignas(64) int32_t o[VLANES];
            for (int k = 0; k < VLANES; k++) o[k] = (int32_t)((const char*)lanes[g * VLANES + k].shoe.cards - (const char*)base);
            off[g] = _mm512_load_si512(o);
            pos[g] = rc[g] = win1[g] = win2[g] = left[g] = ZERO;
            load_lanes(g, 0xFFFF);
        }

        auto hilo = [&](__m512i c) { return _mm512_permutexvar_epi32(c, HILO_V); };
        // Exact true-count test rc / (unseen / 52) >= k, as rc * 52 >= k * unseen. During play
        // the hole card is still unseen.
        auto tc_ge = [&](int g, __m512i k) {
            __m512i unseen = _mm512_sub_epi32(splat(CARDS + 1), pos[g]);
            return _mm512_cmpge_epi32_mask(_mm512_mullo_epi32(rc[g], splat(52)), _mm512_mullo_epi32(k, unseen));
        };
        auto lookup = [&](int g, __mmask16 m, __m512i cls, __m512i up, int flags) {
            __m512i idx = _mm512_add_epi32(_mm512_slli_epi32(_mm512_add_epi32(_mm512_mullo_epi32(cls, splat(11)), up), 3), splat(flags));
            __m512i e = _mm512_mask_i32gather_epi32(ZERO, m, idx, table, 4);
            __mmask16 ge = tc_ge(g, _mm512_srai_epi32(e, 16));
            return _mm512_mask_blend_epi32(ge, _mm512_and_si512(e, BYTE), _mm512_and_si512(_mm512_srli_epi32(e, 8), BYTE));
        };
        // Hit rule vectors (indexed by upcard) and per-lane breakpoint distances D_b.
        const HitRule& hr = hit_rule(counting);
        const __m512i HR_HARD = _mm512_load_si512(hr.hard), HR_SOFT = _mm512_load_si512(hr.soft);
        const __m512i HR_B1 = _mm512_load_si512(hr.b1), HR_B2 = _mm512_load_si512(hr.b2);
        const __m512i HILO52 = _mm512_mullo_epi32(HILO_V, splat(52));
        __m512i b1[G], b2[G], d1[G], d2[G], stand_hard[G], stand_soft[G];
        // Masked draw of one card per lane in m, from the card window while it has cards left.
        auto draw = [&](int g, __mmask16 m) {
            __m512i c = _mm512_and_si512(win1[g], BYTE);
            if (__mmask16 past = m & _mm512_cmple_epi32_mask(left[g], ZERO))
                c = _mm512_and_si512(_mm512_mask_i32gather_epi32(c, past, _mm512_add_epi32(off[g], pos[g]), base, 1), BYTE);
            win1[g] = _mm512_mask_mov_epi32(win1[g], m, _mm512_or_si512(_mm512_srli_epi32(win1[g], 8), _mm512_slli_epi32(win2[g], 24)));
            win2[g] = _mm512_mask_srli_epi32(win2[g], m, win2[g], 8);
            left[g] = _mm512_mask_sub_epi32(left[g], m, left[g], ONE);
            pos[g] = _mm512_mask_add_epi32(pos[g], m, pos[g], ONE);
            rc[g] = _mm512_mask_add_epi32(rc[g], m, rc[g], hilo(c));
            // D_b = rc*52 - b*unseen: a draw adds 52*hilo(c) to rc*52 and takes 1 from unseen.
            __m512i h52 = _mm512_permutexvar_epi32(c, HILO52);
            d1[g] = _mm512_mask_add_epi32(d1[g], m, d1[g], _mm512_add_epi32(h52, b1[g]));
            d2[g] = _mm512_mask_add_epi32(d2[g], m, d2[g], _mm512_add_epi32(h52, b2[g]));
            return c;
        };
        auto value = [&](__m512i sum, __mmask16 ace) {
            return _mm512_mask_add_epi32(sum, ace & _mm512_cmple_epi32_mask(sum, splat(11)), sum, splat(10));
        };
        auto hand_cls = [&](__m512i sum, __mmask16 ace, __mmask16 pair, __m512i first) {
            __mmask16 sft = ace & _mm512_cmple_epi32_mask(sum, splat(11));
            __m512i cl = _mm512_mask_blend_epi32(sft, sum, _mm512_add_epi32(sum, splat(20)));
            return _mm512_mask_blend_epi32(pair, cl, _mm512_add_epi32(first, splat(31)));
        };
        auto add = [&](int g, int which, __mmask16 m, __m512i v) {
            int32_t* a = acc[g][which];
            _mm512_store_si512(a, _mm512_mask_add_epi32(_mm512_load_si512(a), m, _mm512_load_si512(a), v));
        };
        auto inc = [&](int g, int which, __mmask16 m) { add(g, which, m, ONE); };

        // ---- per-round temporaries
        __m512i pos0[G], rc0[G], units[G], B[G], halfB[G], bucket[G], up[G], hole[G], c1[G], net[G], action[G];
        __m512i psum[G], h1sum[G], dsum[G], dval[G], act0[G], act1[G];
        __mmask16 pace[G], h1ace[G], dace[G], up_ace[G], dbj[G], pbj[G], ins[G], act[G], split[G], surr[G];
        __mmask16 resplit[G], sp_aces[G], m0[G], m1[G], dbl0[G], dbl1[G], more[G], live[G], live1[G], bust[G], bust1[G];

        // Hand restarted by a split: (c1, new card). Its first decision may double or split again
        // (no surrender); a second split marks the lane for scalar replay. Split aces stand.
        auto split_hand = [&](int g, __mmask16 m, __m512i& sum, __mmask16& ace, __m512i& a) {
            __m512i c = draw(g, m);
            sum = _mm512_mask_add_epi32(sum, m, c1[g], c);
            ace = (ace & ~m) | (m & (_mm512_cmpeq_epi32_mask(c1[g], ONE) | _mm512_cmpeq_epi32_mask(c, ONE)));
            __mmask16 dec = m & ~sp_aces[g];
            __m512i a2 = lookup(g, dec, hand_cls(sum, ace, _mm512_cmpeq_epi32_mask(c, c1[g]), c1[g]), up[g], 3);
            resplit[g] |= dec & _mm512_cmpeq_epi32_mask(a2, splat(SPLIT));
            a = _mm512_mask_blend_epi32(dec, a, a2);
            a = _mm512_mask_mov_epi32(a, m & sp_aces[g], splat(STAND));
        };
        // Start playing hands in m whose next action is a: a double takes one card; a hit takes
        // one and leaves the lane in more[g] for the hit loop. Returns the doubles.
        auto play_start = [&](int g, __mmask16 m, __m512i a, __m512i& sum, __mmask16& ace) {
            __mmask16 dbl = m & _mm512_cmpeq_epi32_mask(a, splat(DOUBLE));
            more[g] = m & _mm512_cmpeq_epi32_mask(a, splat(HIT));
            __mmask16 dr = dbl | more[g];
            __m512i c = draw(g, dr);
            sum = _mm512_mask_add_epi32(sum, dr, sum, c);
            ace |= dr & _mm512_cmpeq_epi32_mask(c, ONE);
            return dbl;
        };
        // One hit-loop step for multi-card hands (they can only hit or stand): see HitRule.
        auto hit_step = [&](int g, __m512i& sum, __mmask16& ace) {
            __mmask16 sft = ace & _mm512_cmple_epi32_mask(sum, splat(11));
            __m512i val = _mm512_mask_add_epi32(sum, sft, sum, splat(10));
            __m512i S = _mm512_mask_sub_epi32(stand_hard[g], _mm512_cmpge_epi32_mask(d1[g], ZERO), stand_hard[g], ONE);
            S = _mm512_mask_sub_epi32(S, _mm512_cmpge_epi32_mask(d2[g], ZERO), S, ONE);
            S = _mm512_mask_mov_epi32(S, sft, stand_soft[g]);
            __mmask16 m = more[g] & _mm512_cmplt_epi32_mask(val, splat(21)) & _mm512_cmplt_epi32_mask(val, S);
            if (m) {
                __m512i c = draw(g, m);
                sum = _mm512_mask_add_epi32(sum, m, sum, c);
                ace |= m & _mm512_cmpeq_epi32_mask(c, ONE);
            }
            more[g] = m;
        };
        auto any_more = [&]() { __mmask16 x = 0; for (int g = 0; g < G; g++) x |= more[g]; return x != 0; };

        for (uint64_t r = 0; r < rounds_per_lane; r++) {
            // ---- reshuffles (scalar, ~1 in 48 rounds per lane), bet, deal, blackjacks, first decision
            for (int g = 0; g < G; g++) {
                if (__mmask16 m = _mm512_cmpge_epi32_mask(pos[g], splat(CUT))) {
                    for (__mmask16 b = m; b; b &= b - 1) {
                        Sim& s = lanes[g * VLANES + __builtin_ctz(b)];
                        s.shoe.shuffle(); s.st.shuffles++;
                    }
                    pos[g] = _mm512_mask_mov_epi32(pos[g], m, ZERO);
                    rc[g] = _mm512_mask_mov_epi32(rc[g], m, ZERO);
                }
                pos0[g] = pos[g]; rc0[g] = rc[g];

                // Bet from the floored true count. Float division is exact enough: |rc*52| <= 6240
                // and unseen <= 312, so a non-integer quotient is >= 1/312 from any integer.
                __m512i unseen = _mm512_sub_epi32(splat(CARDS), pos[g]);
                __m512 q = _mm512_div_ps(_mm512_cvtepi32_ps(_mm512_mullo_epi32(rc[g], splat(52))), _mm512_cvtepi32_ps(unseen));
                __m512i tc = _mm512_cvtps_epi32(_mm512_roundscale_ps(q, _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC));
                units[g] = counting ? _mm512_permutexvar_epi32(_mm512_min_epi32(_mm512_max_epi32(tc, ONE), splat(7)), RAMP_V) : ONE;
                B[g] = _mm512_add_epi32(units[g], units[g]);
                halfB[g] = units[g];
                bucket[g] = _mm512_add_epi32(_mm512_min_epi32(_mm512_max_epi32(tc, splat(TC_MIN)), splat(TC_MAX)), splat(-TC_MIN));

                // Deal p1, up, p2, hole (one gather) and load the 8-card window (two more).
                __m512i at = _mm512_add_epi32(off[g], pos[g]);
                __m512i w = _mm512_i32gather_epi32(at, base, 1);
                win1[g] = _mm512_i32gather_epi32(_mm512_add_epi32(at, splat(4)), base, 1);
                win2[g] = _mm512_i32gather_epi32(_mm512_add_epi32(at, splat(8)), base, 1);
                left[g] = splat(8);
                b1[g] = _mm512_permutexvar_epi32(_mm512_and_si512(_mm512_srli_epi32(w, 8), BYTE), HR_B1);
                b2[g] = _mm512_permutexvar_epi32(_mm512_and_si512(_mm512_srli_epi32(w, 8), BYTE), HR_B2);
                stand_hard[g] = _mm512_permutexvar_epi32(_mm512_and_si512(_mm512_srli_epi32(w, 8), BYTE), HR_HARD);
                stand_soft[g] = _mm512_permutexvar_epi32(_mm512_and_si512(_mm512_srli_epi32(w, 8), BYTE), HR_SOFT);
                c1[g] = _mm512_and_si512(w, BYTE);
                up[g] = _mm512_and_si512(_mm512_srli_epi32(w, 8), BYTE);
                __m512i c2 = _mm512_and_si512(_mm512_srli_epi32(w, 16), BYTE);
                hole[g] = _mm512_srli_epi32(w, 24);
                pos[g] = _mm512_add_epi32(pos[g], splat(4));
                rc[g] = _mm512_add_epi32(rc[g], _mm512_add_epi32(hilo(c1[g]), _mm512_add_epi32(hilo(up[g]), hilo(c2))));

                psum[g] = _mm512_add_epi32(c1[g], c2);
                pace[g] = _mm512_cmpeq_epi32_mask(c1[g], ONE) | _mm512_cmpeq_epi32_mask(c2, ONE);
                up_ace[g] = _mm512_cmpeq_epi32_mask(up[g], ONE);
                __mmask16 up_ten = _mm512_cmpeq_epi32_mask(up[g], splat(10));
                dbj[g] = (up_ace[g] & _mm512_cmpeq_epi32_mask(hole[g], splat(10))) | (up_ten & _mm512_cmpeq_epi32_mask(hole[g], ONE));
                pbj[g] = pace[g] & _mm512_cmpeq_epi32_mask(psum[g], splat(11));
                net[g] = ZERO;
                action[g] = B[g];

                {  // D_b after the deal (hole card unseen); draw() keeps it current
                    __m512i rc52 = _mm512_mullo_epi32(rc[g], splat(52));
                    __m512i unseen_now = _mm512_sub_epi32(splat(CARDS + 1), pos[g]);
                    d1[g] = _mm512_sub_epi32(rc52, _mm512_mullo_epi32(b1[g], unseen_now));
                    d2[g] = _mm512_sub_epi32(rc52, _mm512_mullo_epi32(b2[g], unseen_now));
                }
                ins[g] = counting ? (up_ace[g] & tc_ge(g, splat(3))) : 0;
                net[g] = _mm512_mask_add_epi32(net[g], ins[g] & dbj[g], net[g], B[g]);
                net[g] = _mm512_mask_sub_epi32(net[g], ins[g] & ~dbj[g], net[g], halfB[g]);
                action[g] = _mm512_mask_add_epi32(action[g], ins[g], action[g], halfB[g]);

                // Blackjacks settle immediately.
                __mmask16 done = dbj[g] | pbj[g];
                net[g] = _mm512_mask_sub_epi32(net[g], dbj[g] & ~pbj[g], net[g], B[g]);
                net[g] = _mm512_mask_add_epi32(net[g], pbj[g] & ~dbj[g], net[g], _mm512_add_epi32(B[g], halfB[g]));

                // First decision: double, split and surrender all allowed.
                act[g] = ~done;
                act0[g] = lookup(g, act[g], hand_cls(psum[g], pace[g], _mm512_cmpeq_epi32_mask(c1[g], c2), c1[g]), up[g], 7);
                split[g] = act[g] & _mm512_cmpeq_epi32_mask(act0[g], splat(SPLIT));
                surr[g] = act[g] & _mm512_cmpeq_epi32_mask(act0[g], splat(SURRENDER));
                resplit[g] = 0;
                sp_aces[g] = split[g] & _mm512_cmpeq_epi32_mask(c1[g], ONE);
                h1sum[g] = ZERO; h1ace[g] = 0; m1[g] = 0; dbl1[g] = 0;
                if (split[g]) split_hand(g, split[g], psum[g], pace[g], act0[g]);
                m0[g] = act[g] & ~surr[g] & ~resplit[g];
                dbl0[g] = play_start(g, m0[g], act0[g], psum[g], pace[g]);
            }
            // ---- hand 0 hit loop, groups interleaved
            while (any_more())
                for (int g = 0; g < G; g++) if (more[g]) hit_step(g, psum[g], pace[g]);
            // ---- hand 1 of split lanes
            for (int g = 0; g < G; g++) {
                more[g] = 0;
                if (__mmask16 s1 = split[g] & ~resplit[g]) {
                    act1[g] = ZERO;
                    split_hand(g, s1, h1sum[g], h1ace[g], act1[g]);
                    m1[g] = s1 & ~resplit[g];
                    dbl1[g] = play_start(g, m1[g], act1[g], h1sum[g], h1ace[g]);
                }
            }
            while (any_more())
                for (int g = 0; g < G; g++) if (more[g]) hit_step(g, h1sum[g], h1ace[g]);
            // ---- dealer: hole card revealed, then draws to 17 if any hand is live
            for (int g = 0; g < G; g++) {
                rc[g] = _mm512_add_epi32(rc[g], hilo(hole[g]));
                bust[g] = m0[g] & _mm512_cmpgt_epi32_mask(value(psum[g], pace[g]), splat(21));
                bust1[g] = m1[g] & _mm512_cmpgt_epi32_mask(value(h1sum[g], h1ace[g]), splat(21));
                live[g] = m0[g] & ~bust[g];
                live1[g] = m1[g] & ~bust1[g];
                dsum[g] = _mm512_add_epi32(up[g], hole[g]);
                dace[g] = up_ace[g] | _mm512_cmpeq_epi32_mask(hole[g], ONE);
                dval[g] = value(dsum[g], dace[g]);
                more[g] = (live[g] | live1[g]) & _mm512_cmplt_epi32_mask(dval[g], splat(17));
            }
            while (any_more())
                for (int g = 0; g < G; g++)
                    if (more[g]) {
                        __m512i c = draw(g, more[g]);
                        dsum[g] = _mm512_mask_add_epi32(dsum[g], more[g], dsum[g], c);
                        dace[g] |= more[g] & _mm512_cmpeq_epi32_mask(c, ONE);
                        dval[g] = value(dsum[g], dace[g]);
                        more[g] &= _mm512_cmplt_epi32_mask(dval[g], splat(17));
                    }
            // ---- settle and account
            for (int g = 0; g < G; g++) {
                __mmask16 dbust = _mm512_cmpgt_epi32_mask(dval[g], splat(21));
                auto settle = [&](__mmask16 m, __mmask16 lv, __m512i pv, __mmask16 dbl) {
                    __mmask16 win = lv & (dbust | _mm512_cmpgt_epi32_mask(pv, dval[g]));
                    __mmask16 push = lv & ~dbust & _mm512_cmpeq_epi32_mask(pv, dval[g]);
                    __mmask16 loss = m & ~win & ~push;
                    __m512i wager = _mm512_mask_add_epi32(B[g], dbl, B[g], B[g]);
                    net[g] = _mm512_mask_add_epi32(net[g], win, net[g], wager);
                    net[g] = _mm512_mask_sub_epi32(net[g], loss, net[g], wager);
                    action[g] = _mm512_mask_add_epi32(action[g], dbl, action[g], B[g]);
                    return std::array<__mmask16, 3>{win, loss, push};
                };
                auto r0 = settle(m0[g], live[g], value(psum[g], pace[g]), dbl0[g]);
                auto r1 = settle(m1[g], live1[g], value(h1sum[g], h1ace[g]), dbl1[g]);
                net[g] = _mm512_mask_sub_epi32(net[g], surr[g], net[g], halfB[g]);
                action[g] = _mm512_mask_add_epi32(action[g], split[g], action[g], B[g]);

                // Lanes being replayed (resplit) are left out; the scalar replay counts them.
                __mmask16 vec = (__mmask16)~resplit[g];
                inc(g, ROUNDS, vec); inc(g, HANDS, vec); inc(g, HANDS, m1[g]); inc(g, SPLITS, split[g] & vec);
                add(g, NET, vec, net[g]);
                add(g, INIT, vec, B[g]);
                add(g, ACTION, vec, action[g]);
                add(g, NETSQ, vec, _mm512_mullo_epi32(net[g], net[g]));
                // net is an exact multiple of units, so float division is exact.
                __m512i flat = _mm512_cvtps_epi32(_mm512_div_ps(_mm512_cvtepi32_ps(net[g]), _mm512_cvtepi32_ps(units[g])));
                add(g, FLAT, vec, flat);
                add(g, FLATSQ, vec, _mm512_mullo_epi32(flat, flat));
                inc(g, WINS, (r0[0] & vec) | (pbj[g] & ~dbj[g])); inc(g, WINS, r1[0]);  // a lane can win both hands
                inc(g, LOSSES, (r0[1] & vec) | surr[g] | (dbj[g] & ~pbj[g])); inc(g, LOSSES, r1[1]);
                inc(g, PUSHES, (r0[2] & vec) | (dbj[g] & pbj[g])); inc(g, PUSHES, r1[2]);
                inc(g, PBJ, pbj[g]); inc(g, DBJ, dbj[g]);
                inc(g, DOUBLES, dbl0[g] & vec); inc(g, DOUBLES, dbl1[g]);
                inc(g, SURR, surr[g]); inc(g, BUSTS, bust[g] & vec); inc(g, BUSTS, bust1[g]);
                inc(g, INS, ins[g] & vec); inc(g, INSW, ins[g] & dbj[g]);
                // Per-lane histogram: indices are distinct per lane, so no scatter conflicts.
                __m512i hidx = _mm512_add_epi32(_mm512_mullo_epi32(_mm512_add_epi32(LANE_IDX, splat(g * VLANES)), splat(TC_BUCKETS)), bucket[g]);
                __m512i hv = _mm512_mask_i32gather_epi32(ZERO, vec, hidx, hist, 4);
                hv = _mm512_add_epi32(hv, _mm512_add_epi32(flat, splat(1 << HIST_SHIFT)));
                _mm512_mask_i32scatter_epi32(hist, vec, hidx, hv, 4);

                // Re-split lanes: rewind to the start of the round and replay on the scalar engine.
                if (resplit[g]) {
                    pos[g] = _mm512_mask_mov_epi32(pos[g], resplit[g], pos0[g]);
                    rc[g] = _mm512_mask_mov_epi32(rc[g], resplit[g], rc0[g]);
                    store_lanes(g, resplit[g]);
                    for (__mmask16 b = resplit[g]; b; b &= b - 1) lanes[g * VLANES + __builtin_ctz(b)].play_round();
                    load_lanes(g, resplit[g]);
                }
            }
            if ((r & (FLUSH - 1)) == FLUSH - 1) flush();
        }
        flush();
        for (int g = 0; g < G; g++) store_lanes(g, 0xFFFF);
    }
};

#endif  // __AVX512F__

}  // namespace bj
