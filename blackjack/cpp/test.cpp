// Test suite: build and run with `make test` (AddressSanitizer + UBSan, AVX-512 when available).
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>

#include "engine.hpp"

using namespace bj;

static int failures = 0;
#define CHECK(cond)                                                         \
    do {                                                                    \
        if (!(cond)) {                                                      \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                     \
        }                                                                   \
    } while (0)

static Hand mk(const std::vector<int>& cs) {
    Hand h;
    for (int c : cs) h.add((uint8_t)c);
    return h;
}

// Sim with a forced count: rc and remaining cards chosen to give an exact true count.
static Sim at_tc(int tc) {
    Sim s(1);
    s.shoe.pos = CARDS - 104;  // two decks unseen
    s.rc = tc * 2;
    return s;
}

static bool same_stats(const Stats& a, const Stats& b) { return memcmp(&a, &b, sizeof(Stats)) == 0; }
static bool same_rng(const Rng& a, const Rng& b) {
    return memcmp(a.s0, b.s0, sizeof a.s0) == 0 && memcmp(a.s1, b.s1, sizeof a.s1) == 0 &&
           memcmp(a.s2, b.s2, sizeof a.s2) == 0 && memcmp(a.s3, b.s3, sizeof a.s3) == 0 &&
           a.idx == b.idx && memcmp(a.buf + a.idx, b.buf + b.idx, (RNG_BUF - a.idx) * 4) == 0;
}

// Reference definition of a shuffle: textbook Fisher-Yates, one bounded draw per position.
static void reference_shuffle(Shoe& s) {
    for (int i = 0; i < SHUF_EAGER; i++) s.fy_step(i);
    s.shuffled = SHUF_EAGER;
    s.pos = 0;
}

#ifdef __AVX512F__
// Runs `rounds` rounds per lane on Engine<G> and on independent scalar Sims; every lane's
// stats, shoe position, count and RNG state must match exactly.
template <int G>
static void check_engine(uint64_t seed, bool counting, uint64_t rounds, int extra) {
    constexpr int N = G * VLANES;
    std::vector<Sim> vec, ref;
    vec.reserve(N);
    ref.reserve(N);
    for (int l = 0; l < N; l++) {
        uint64_t s = splitmix64(seed);
        vec.emplace_back(s, counting);
        ref.emplace_back(s, counting);
    }
    Engine<G>(vec.data(), counting).run(rounds);
    for (int l = 0; l < extra; l++) vec[l].play_round();  // the scalar tail main.cpp runs
    for (int l = 0; l < N; l++)
        for (uint64_t i = 0; i < rounds + (l < extra); i++) ref[l].play_round();
    bool ok = true;
    for (int l = 0; l < N; l++) {
        ok &= same_stats(vec[l].st, ref[l].st);
        ok &= vec[l].shoe.pos == ref[l].shoe.pos && vec[l].rc == ref[l].rc && vec[l].shoe.shuffled == ref[l].shoe.shuffled;
        ok &= memcmp(vec[l].shoe.cards, ref[l].shoe.cards, SHOE_BYTES) == 0 && same_rng(vec[l].shoe.rng, ref[l].shoe.rng);
    }
    if (!ok) fprintf(stderr, "  engine G=%d seed=%llu counting=%d rounds=%llu differs from reference\n", G,
                     (unsigned long long)seed, counting, (unsigned long long)rounds);
    CHECK(ok);
    uint64_t splits = 0;
    for (auto& s : ref) splits += s.st.splits;
    CHECK(splits > 0);  // the comparison exercised the split paths
}
#endif

int main() {
    // ---------------------------------------------------------------- RNG
    // The AVX-512 refill produces exactly the scalar xoshiro256++ streams, in buffer order.
    {
        Rng r(42), ref(42);
        for (int k = 0; k < 10; k++) {
            r.refill();
            for (int step = 0; step < RNG_STEPS; step++)
                for (int l = 0; l < RNG_LANES; l++) {
                    uint64_t& s0 = ref.s0[l], &s1 = ref.s1[l], &s2 = ref.s2[l], &s3 = ref.s3[l];
                    uint64_t out = Rng::rotl(s0 + s3, 23) + s0, t = s1 << 17;
                    s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t; s3 = Rng::rotl(s3, 45);
                    CHECK(r.buf[step * 16 + l] == (uint32_t)(out >> 32));
                    CHECK(r.buf[step * 16 + 8 + l] == (uint32_t)out);
                }
        }
    }
    // Lane seeds never repeat across lane sets (overlapping splitmix chains once replayed games).
    {
        std::vector<uint64_t> seeds;
        for (uint64_t seed : {1, 2})
            for (uint64_t set = 0; set < 128; set++) {
                uint64_t seeder = set_seed(seed, set);
                for (int l = 0; l < 48; l++) seeds.push_back(splitmix64(seeder));
            }
        std::sort(seeds.begin(), seeds.end());
        CHECK(std::adjacent_find(seeds.begin(), seeds.end()) == seeds.end());
    }
    // Bounded draws are uniform.
    {
        Rng rng(3);
        int b[13] = {};
        for (int i = 0; i < 1300000; i++) b[rng.below(13)]++;
        for (int x : b) CHECK(x > 99000 && x < 101000);
    }

    // ---------------------------------------------------------------- Shoe
    // Composition and Hi-Lo balance after many reshuffles; a reshuffle is a permutation.
    {
        Shoe sh(9);
        for (int k = 0; k < 1000; k++) {
            int counts[11] = {}, sum = 0;
            for (int i = 0; i < CARDS; i++) { counts[sh.cards[i]]++; sum += HILO[sh.cards[i]]; }
            for (int r = 1; r <= 9; r++) CHECK(counts[r] == 24);
            CHECK(counts[10] == 96);
            CHECK(sum == 0);
            sh.shuffle();
        }
    }
    // The vectorized shuffle equals textbook Fisher-Yates from any RNG position, including
    // misaligned starts (this caught an aligned store to an unaligned address) and blocks that
    // take the exact rejection path.
    for (int skew = 0; skew < 40; skew++) {
        Shoe a(100 + skew);
        for (int k = 0; k < skew; k++) a.rng.next32();
        if (skew >= 20) for (int k = a.rng.idx; k < RNG_BUF; k++) a.rng.buf[k] &= 0xFF;  // force lo < range
        Shoe b = a;
        a.shuffle();
        reference_shuffle(b);
        CHECK(memcmp(a.cards, b.cards, SHOE_BYTES) == 0);
        CHECK(same_rng(a.rng, b.rng));
    }
    // Extending past the shuffled prefix is the same Fisher-Yates continued.
    {
        Shoe a(5), b(5);
        for (int i = SHUF_EAGER; i < CARDS; i++) b.fy_step(i);
        a.pos = SHUF_EAGER;
        for (int i = SHUF_EAGER; i < CARDS; i++) CHECK(a.draw() == b.cards[i]);
    }
    // Every position is uniformly distributed over ranks (chi-square, 9 degrees of freedom; the
    // 99.99% critical value is 33.7).
    {
        const int SHOES = 60000, POSN[] = {0, 1, 100, 259, 287};
        for (int p : POSN) {
            Shoe sh(77 + p);
            long counts[11] = {};
            for (int k = 0; k < SHOES; k++) { sh.shuffle(); counts[sh.cards[p]]++; }
            double chi = 0;
            for (int r = 1; r <= 10; r++) {
                double e = SHOES * (r == 10 ? 96.0 : 24.0) / CARDS;
                chi += (counts[r] - e) * (counts[r] - e) / e;
            }
            if (chi >= 33.7) fprintf(stderr, "  position %d chi-square %.1f\n", p, chi);
            CHECK(chi < 33.7);
        }
    }

    // ---------------------------------------------------------------- Hands and helpers
    CHECK(mk({1, 6}).value() == 17 && mk({1, 6}).soft());
    CHECK(mk({1, 6, 10}).value() == 17 && !mk({1, 6, 10}).soft());
    CHECK(mk({1, 1, 9}).value() == 21);
    CHECK(mk({10, 1}).value() == 21);
    CHECK(floor_div(-1, 4) == -1 && floor_div(-4, 4) == -1 && floor_div(3, 4) == 0);
    CHECK(bet_units(1) == 1 && bet_units(2) == 2 && bet_units(4) == 6 && bet_units(9) == 12);

    // ---------------------------------------------------------------- Strategy
    // Basic strategy spot checks at TC 0.
    {
        Sim s = at_tc(0);
        CHECK(s.decide(mk({10, 6}), 10, true, true, true) == SURRENDER);
        CHECK(s.decide(mk({8, 8}), 10, true, true, true) == SPLIT);
        CHECK(s.decide(mk({1, 1}), 1, true, true, true) == SPLIT);
        CHECK(s.decide(mk({5, 5}), 6, true, true, true) == DOUBLE);
        CHECK(s.decide(mk({9, 9}), 7, true, true, true) == STAND);
        CHECK(s.decide(mk({9, 9}), 8, true, true, true) == SPLIT);
        CHECK(s.decide(mk({6, 5}), 1, true, true, true) == HIT);
        CHECK(s.decide(mk({1, 7}), 9, true, true, true) == HIT);
        CHECK(s.decide(mk({1, 7}), 6, true, true, true) == DOUBLE);
        CHECK(s.decide(mk({1, 7}), 6, false, true, true) == STAND);
        CHECK(s.decide(mk({1, 6}), 2, true, true, true) == HIT);
        CHECK(s.decide(mk({10, 2}), 4, true, true, true) == STAND);
        CHECK(s.decide(mk({10, 3}), 2, true, true, true) == STAND);
        CHECK(s.decide(mk({10, 6}), 7, true, true, true) == HIT);
        CHECK(s.decide(mk({5, 4, 7}), 10, false, false, false) == STAND);  // 16v10 at TC 0
    }
    // Illustrious 18 / Fab 4 thresholds: one below the index, then at the index.
    struct Dev { std::vector<int> cards; int up, index; Action below, at; };
    Dev devs[] = {
        {{10, 5}, 10, 4, SURRENDER, SURRENDER},  // surrender 15v10 at >=0 wins over the stand index
        {{4, 3, 2, 6}, 10, 4, HIT, STAND},       // 15v10 (multi-card, no surrender)
        {{10, 10}, 5, 5, STAND, SPLIT},
        {{10, 10}, 6, 4, STAND, SPLIT},
        {{6, 4}, 10, 4, HIT, DOUBLE},
        {{10, 2}, 3, 2, HIT, STAND},
        {{10, 2}, 2, 3, HIT, STAND},
        {{6, 5}, 1, 1, HIT, DOUBLE},
        {{5, 4}, 2, 1, HIT, DOUBLE},
        {{6, 4}, 1, 4, HIT, DOUBLE},
        {{5, 4}, 7, 3, HIT, DOUBLE},
        {{3, 4, 9}, 9, 5, HIT, STAND},           // 16v9 (multi-card, no surrender)
        {{10, 3}, 2, -1, HIT, STAND},
        {{10, 2}, 4, 0, HIT, STAND},
        {{10, 2}, 5, -2, HIT, STAND},
        {{10, 2}, 6, -1, HIT, STAND},
        {{10, 3}, 3, -2, HIT, STAND},
        {{10, 4}, 10, 3, HIT, SURRENDER},
        {{10, 5}, 9, 2, HIT, SURRENDER},
        {{10, 5}, 1, 1, HIT, SURRENDER},
        {{9, 7}, 10, 0, HIT, STAND},             // 16v10 stand index, surrender unavailable
    };
    for (auto& d : devs) {
        Hand h = mk(d.cards);
        bool surr = h.ncards == 2 && d.at != STAND;  // stand indices tested without surrender
        Sim lo = at_tc(d.index - 1), hi = at_tc(d.index);
        CHECK(lo.decide(h, d.up, h.ncards == 2, true, surr) == d.below);
        CHECK(hi.decide(h, d.up, h.ncards == 2, true, surr) == d.at);
    }
    {
        Sim lo = at_tc(-1);  // 15v10 surrender index is 0
        CHECK(lo.decide(mk({10, 5}), 10, true, true, true) == HIT);
    }
    // The strategy matrix agrees with decide() at every flag combination, over counts seen in play.
    for (bool counting : {true, false}) {
        Sim s(5, counting);
        Rng r(6);
        for (int i = 0; i < 20000; i++) {
            s.play_round();
            int up = (int)r.below(10) + 1;
            Hand h = mk({(int)r.below(10) + 1, (int)r.below(10) + 1});
            for (int f = 0; f < 8; f++)
                CHECK(s.lookup(h, up, f & 1, f & 2, f & 4) == s.decide(h, up, f & 1, f & 2, f & 4));
        }
    }
    // Basic mode ignores the count.
    {
        Sim s(1, false);
        s.shoe.pos = CARDS - 52;
        s.rc = 20;
        CHECK(s.decide(mk({10, 10}), 6, true, true, true) == STAND);
        CHECK(s.decide(mk({10, 6}), 10, true, true, false) == HIT);
    }

    // ---------------------------------------------------------------- Scalar engine
    // The running count always matches the cards dealt.
    {
        Sim s(11);
        for (int i = 0; i < 200000; i++) {
            s.play_round();
            int rc = 0;
            for (int k = 0; k < s.shoe.pos; k++) rc += HILO[s.shoe.cards[k]];
            CHECK(rc == s.rc);
            if (rc != s.rc) break;
        }
        CHECK(s.st.rounds == 200000);
    }

    // ---------------------------------------------------------------- AVX-512 engine
#ifdef __AVX512F__
    // Bit-identical to the scalar reference, per lane: 1 and 3 interleaved groups, both modes,
    // with and without a scalar tail. Enough rounds for splits, re-splits and many reshuffles.
    check_engine<1>(1, true, 20000, 0);
    check_engine<1>(2, false, 20000, 7);
    check_engine<3>(3, true, 6000, 13);
    check_engine<3>(4, false, 6000, 0);
    // Run twice on the same lanes: state carries over between run() calls.
    {
        std::vector<Sim> vec, ref;
        vec.reserve(16); ref.reserve(16);
        uint64_t seed = 8;
        for (int l = 0; l < 16; l++) { uint64_t s = splitmix64(seed); vec.emplace_back(s); ref.emplace_back(s); }
        Engine<1> e(vec.data(), true);
        e.run(3000);
        e.run(4000);
        bool ok = true;
        for (int l = 0; l < 16; l++) {
            for (int i = 0; i < 7000; i++) ref[l].play_round();
            ok &= same_stats(vec[l].st, ref[l].st);
        }
        CHECK(ok);
    }
    // Statistical sanity: flat-bet basic strategy loses close to the published house edge for
    // these rules (~0.35-0.40%), and the Hi-Lo spread player wins. 20M rounds: CI about +/-0.05%.
    {
        auto run = [](bool counting) {
            std::vector<Sim> lanes;
            lanes.reserve(48);
            uint64_t seed = counting ? 21 : 22;
            for (int l = 0; l < 48; l++) lanes.emplace_back(splitmix64(seed), counting);
            Engine<3>(lanes.data(), counting).run(20000000 / 48);
            Stats t;
            for (auto& s : lanes) t.merge(s.st);
            return 100.0 * (double)t.net / (double)t.initial_bet;
        };
        double basic = run(false), hilo = run(true);
        printf("  basic strategy edge %+.3f%%, hi-lo edge %+.3f%%\n", basic, hilo);
        CHECK(basic > -0.50 && basic < -0.25);
        CHECK(hilo > 0.8 && hilo < 1.6);
    }
#else
    printf("  (no AVX-512: vector engine tests skipped)\n");
#endif

    if (failures) {
        fprintf(stderr, "%d failure(s)\n", failures);
        return 1;
    }
    printf("all C++ tests passed\n");
}
