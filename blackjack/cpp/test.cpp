// Unit tests: build with `make test`.
#include <cstdio>
#include <cstdlib>
#include <vector>

#include "bj.hpp"

using namespace bj;

static int failures = 0;
#define CHECK(cond)                                                    \
    do {                                                               \
        if (!(cond)) {                                                 \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                \
        }                                                              \
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

int main() {
    // Shoe composition and Hi-Lo balance.
    {
        Shoe sh;
        int counts[11] = {}, sum = 0;
        for (int i = 0; i < CARDS; i++) { counts[sh.cards[i]]++; sum += HILO[sh.cards[i]]; }
        for (int r = 1; r <= 9; r++) CHECK(counts[r] == 24);
        CHECK(counts[10] == 96);
        CHECK(sum == 0);
    }
    // Lazy shuffle keeps a permutation through many reshuffles, and deals every card once per shoe.
    {
        Shoe sh;
        Rng rng(7);
        for (int k = 0; k < 1000; k++) {
            sh.pos = 0;
            int counts[11] = {};
            for (int i = 0; i < CARDS; i++) counts[sh.draw(rng)]++;
            for (int r = 1; r <= 9; r++) CHECK(counts[r] == 24);
            CHECK(counts[10] == 96);
        }
    }
    // Bounded RNG is roughly uniform.
    {
        Rng rng(3);
        int b[13] = {};
        for (int i = 0; i < 1300000; i++) b[rng.below(13)]++;
        for (int x : b) CHECK(x > 99000 && x < 101000);
    }
    // Hand values.
    CHECK(mk({1, 6}).value() == 17 && mk({1, 6}).soft());
    CHECK(mk({1, 6, 10}).value() == 17 && !mk({1, 6, 10}).soft());
    CHECK(mk({1, 1, 9}).value() == 21);
    CHECK(mk({10, 1}).value() == 21);
    CHECK(floor_div(-1, 4) == -1 && floor_div(-4, 4) == -1 && floor_div(3, 4) == 0);
    CHECK(bet_units(1) == 1 && bet_units(2) == 2 && bet_units(4) == 6 && bet_units(9) == 12);

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
    // Basic mode ignores the count.
    {
        Sim s(1, false);
        s.shoe.pos = CARDS - 52;
        s.rc = 20;
        CHECK(s.decide(mk({10, 10}), 6, true, true, true) == STAND);
        CHECK(s.decide(mk({10, 6}), 10, true, true, false) == HIT);
    }
    // Every round conserves the shoe and the count stays consistent with dealt cards.
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

    if (failures) {
        fprintf(stderr, "%d failure(s)\n", failures);
        return 1;
    }
    printf("all C++ tests passed\n");
}
