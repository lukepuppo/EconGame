// Blackjack simulator core: 6-deck shoe, 5 decks dealt, Hi-Lo with Illustrious 18 + Fab 4.
// Rules: S17, DAS, late surrender, dealer peeks, split to 4 hands, split aces once
// (one card each, no resplit), 3:2 blackjack, insurance offered.
//
// The Rust port (../rust/src/lib.rs) mirrors this file step for step, including the RNG
// and the order cards are drawn, so both produce identical results for the same seed.
#pragma once
#include <cstdint>
#include <cstring>

namespace bj {

constexpr int DECKS = 6;
constexpr int CARDS = DECKS * 52;  // 312
constexpr int CUT = 5 * 52;        // reshuffle once 260 cards (5 decks) are dealt
constexpr int MAX_HANDS = 4;
constexpr int TC_MIN = -10, TC_MAX = 10, TC_BUCKETS = TC_MAX - TC_MIN + 1;

// Card ranks are 1..10 (1 = ace, 10 = any ten-value card).
constexpr int8_t HILO[11] = {0, -1, 1, 1, 1, 1, 1, 0, 0, 0, -1};

// ---------------------------------------------------------------- RNG
// xoshiro256** seeded by splitmix64; Lemire's unbiased bounded integers.
inline uint64_t splitmix64(uint64_t& s) {
    uint64_t z = (s += 0x9E3779B97F4A7C15ull);
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ull;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBull;
    return z ^ (z >> 31);
}

struct Rng {
    uint64_t s[4];
    explicit Rng(uint64_t seed) {
        for (auto& x : s) x = splitmix64(seed);
    }
    static uint64_t rotl(uint64_t x, int k) { return (x << k) | (x >> (64 - k)); }
    uint64_t next() {
        uint64_t r = rotl(s[1] * 5, 7) * 9;
        uint64_t t = s[1] << 17;
        s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
        s[2] ^= t;
        s[3] = rotl(s[3], 45);
        return r;
    }
    // Uniform in [0, range).
    uint32_t below(uint32_t range) {
        uint64_t x = next() >> 32;
        uint64_t m = x * range;
        uint32_t l = (uint32_t)m;
        if (l < range) {
            uint32_t t = (uint32_t)(-range) % range;
            while (l < t) {
                x = next() >> 32;
                m = x * range;
                l = (uint32_t)m;
            }
        }
        return (uint32_t)(m >> 32);
    }
};

// ---------------------------------------------------------------- Shoe
// 312 bytes. Shuffling is lazy Fisher-Yates: each draw swaps a random undealt card into
// position `pos`. The array always stays a permutation of the full shoe, so a reshuffle is
// just `pos = 0` and only the 260 cards actually dealt are ever shuffled.
struct Shoe {
    uint8_t cards[CARDS];
    int pos = 0;
    Shoe() {
        int i = 0;
        for (int d = 0; d < DECKS * 4; d++)
            for (int r = 1; r <= 13; r++) cards[i++] = (uint8_t)(r > 10 ? 10 : r);
    }
    uint8_t draw(Rng& rng) {
        int j = pos + (int)rng.below((uint32_t)(CARDS - pos));
        uint8_t c = cards[j];
        cards[j] = cards[pos];
        cards[pos++] = c;
        return c;
    }
};

// ---------------------------------------------------------------- Hand
struct Hand {
    uint8_t sum = 0;       // aces counted as 1
    uint8_t ncards = 0;
    uint8_t first = 0;     // first card rank (pair detection)
    uint8_t bet_mult = 1;  // 1, or 2 after doubling
    bool ace = false;
    bool split_aces = false;
    bool surrendered = false;

    void add(uint8_t c) {
        if (ncards == 0) first = c;
        sum += c;
        ace |= (c == 1);
        ncards++;
    }
    bool soft() const { return ace && sum <= 11; }
    int value() const { return soft() ? sum + 10 : sum; }
};

enum Action : uint8_t { HIT, STAND, DOUBLE, SPLIT, SURRENDER };

// ---------------------------------------------------------------- Stats
// Money is tracked in half-units so 3:2, insurance and surrender stay integral.
struct Stats {
    uint64_t rounds = 0, hands = 0, shuffles = 0;
    int64_t net = 0;              // spread-bet result, half-units
    uint64_t initial_bet = 0;     // sum of initial bets, half-units
    uint64_t action = 0;          // all money put at risk incl. doubles/splits/insurance
    uint64_t net_sq = 0;          // sum of squared per-round net (half-units^2)
    int64_t flat_net = 0;         // same play with a 1-unit bet every round
    uint64_t flat_sq = 0;
    uint64_t wins = 0, losses = 0, pushes = 0, player_bj = 0, dealer_bj = 0;
    uint64_t doubles = 0, splits = 0, surrenders = 0, busts = 0;
    uint64_t ins_taken = 0, ins_won = 0;
    uint64_t tc_rounds[TC_BUCKETS] = {};
    int64_t tc_flat_net[TC_BUCKETS] = {};

    void merge(const Stats& o) {
        rounds += o.rounds; hands += o.hands; shuffles += o.shuffles;
        net += o.net; initial_bet += o.initial_bet; action += o.action; net_sq += o.net_sq;
        flat_net += o.flat_net; flat_sq += o.flat_sq;
        wins += o.wins; losses += o.losses; pushes += o.pushes;
        player_bj += o.player_bj; dealer_bj += o.dealer_bj;
        doubles += o.doubles; splits += o.splits; surrenders += o.surrenders; busts += o.busts;
        ins_taken += o.ins_taken; ins_won += o.ins_won;
        for (int i = 0; i < TC_BUCKETS; i++) {
            tc_rounds[i] += o.tc_rounds[i];
            tc_flat_net[i] += o.tc_flat_net[i];
        }
    }
};

inline int floor_div(int a, int b) {  // b > 0
    int q = a / b;
    return (a % b != 0 && a < 0) ? q - 1 : q;
}

// Bet ramp in units, keyed on floored true count: 1-12 spread.
inline int bet_units(int tc) {
    if (tc < 2) return 1;
    int u = 2 * (tc - 1);
    return u > 12 ? 12 : u;
}

// ---------------------------------------------------------------- Player
struct Sim {
    Rng rng;
    Shoe shoe;
    int rc = 0;             // Hi-Lo running count
    int unseen_extra = 0;   // 1 while the dealer's hole card is drawn but not yet seen
    bool counting = true;   // false = flat-bet basic strategy (for validation)
    Stats st;

    explicit Sim(uint64_t seed, bool counting_ = true) : rng(seed), counting(counting_) {}

    int unseen() const { return CARDS - shoe.pos + unseen_extra; }

    // Exact true-count comparison: rc / (unseen/52) >= k, with no division or rounding.
    bool tc_ge(int k) const { return rc * 52 >= k * unseen(); }
    // Index plays fire only when counting; otherwise fall back to basic strategy.
    bool idx(int k, bool basic) const { return counting ? tc_ge(k) : basic; }

    uint8_t draw_seen() {
        uint8_t c = shoe.draw(rng);
        rc += HILO[c];
        return c;
    }

    Action decide(const Hand& h, int up, bool can_double, bool can_split, bool can_surrender) const {
        int v = h.value();
        bool pair = h.ncards == 2 && h.sum == 2 * h.first;

        if (can_surrender && !h.soft() && !(pair && h.first == 8)) {
            if (v == 16 && (up == 9 || up == 10 || up == 1)) return SURRENDER;
            if (v == 15 && up == 10 && idx(0, true)) return SURRENDER;
            if (v == 15 && up == 9 && idx(2, false)) return SURRENDER;
            if (v == 15 && up == 1 && idx(1, false)) return SURRENDER;
            if (v == 14 && up == 10 && idx(3, false)) return SURRENDER;
        }

        if (pair && can_split) {
            switch (h.first) {
                case 1: case 8: return SPLIT;
                case 10:
                    if (up == 5 && idx(5, false)) return SPLIT;
                    if (up == 6 && idx(4, false)) return SPLIT;
                    break;
                case 9: if ((up >= 2 && up <= 6) || up == 8 || up == 9) return SPLIT; break;
                case 7: if (up >= 2 && up <= 7) return SPLIT; break;
                case 6: if (up >= 2 && up <= 6) return SPLIT; break;
                case 4: if (up == 5 || up == 6) return SPLIT; break;
                case 2: case 3: if (up >= 2 && up <= 7) return SPLIT; break;
                default: break;  // 5,5 plays as hard 10
            }
        }

        if (h.soft()) {
            if (v >= 19) return STAND;
            if (v == 18) {
                if (up >= 3 && up <= 6) return can_double ? DOUBLE : STAND;
                if (up == 2 || up == 7 || up == 8) return STAND;
                return HIT;
            }
            bool dbl = (v == 17 && up >= 3 && up <= 6) ||
                       ((v == 15 || v == 16) && up >= 4 && up <= 6) ||
                       ((v == 13 || v == 14) && up >= 5 && up <= 6);
            return (dbl && can_double) ? DOUBLE : HIT;
        }

        if (v >= 17) return STAND;
        if (v >= 13) {
            if (v == 16 && up == 10) return idx(0, false) ? STAND : HIT;
            if (v == 15 && up == 10) return idx(4, false) ? STAND : HIT;
            if (v == 16 && up == 9) return idx(5, false) ? STAND : HIT;
            if (v == 13 && up == 2) return idx(-1, true) ? STAND : HIT;
            if (v == 13 && up == 3) return idx(-2, true) ? STAND : HIT;
            return (up >= 2 && up <= 6) ? STAND : HIT;
        }
        if (v == 12) {
            bool stand;
            switch (up) {
                case 2: stand = idx(3, false); break;
                case 3: stand = idx(2, false); break;
                case 4: stand = idx(0, true); break;
                case 5: stand = idx(-2, true); break;
                case 6: stand = idx(-1, true); break;
                default: stand = false;
            }
            return stand ? STAND : HIT;
        }
        bool dbl = false;
        if (v == 11) dbl = (up != 1) || idx(1, false);
        else if (v == 10) dbl = (up >= 2 && up <= 9) || ((up == 10 || up == 1) && idx(4, false));
        else if (v == 9) dbl = (up >= 3 && up <= 6) || (up == 2 && idx(1, false)) || (up == 7 && idx(3, false));
        return (dbl && can_double) ? DOUBLE : HIT;
    }

    void play_round() {
        if (shoe.pos >= CUT) {
            shoe.pos = 0;
            rc = 0;
            st.shuffles++;
        }
        int tc = floor_div(rc * 52, CARDS - shoe.pos);
        int units = counting ? bet_units(tc) : 1;
        int64_t B = 2 * units;  // initial bet, half-units
        int64_t net = 0;
        st.rounds++;
        st.initial_bet += (uint64_t)B;
        st.action += (uint64_t)B;

        Hand hands[MAX_HANDS];
        int nh = 1;
        hands[0].add(draw_seen());
        uint8_t up = draw_seen();
        hands[0].add(draw_seen());
        uint8_t hole = shoe.draw(rng);
        unseen_extra = 1;

        bool dealer_bj = (up == 1 && hole == 10) || (up == 10 && hole == 1);
        bool player_bj = hands[0].value() == 21;

        if (up == 1 && counting && tc_ge(3)) {
            st.ins_taken++;
            st.action += (uint64_t)(B / 2);
            if (dealer_bj) { net += B; st.ins_won++; }
            else net -= B / 2;
        }

        if (dealer_bj || player_bj) {
            rc += HILO[hole];
            unseen_extra = 0;
            st.hands++;
            if (dealer_bj) st.dealer_bj++;
            if (player_bj) st.player_bj++;
            if (dealer_bj && player_bj) st.pushes++;
            else if (dealer_bj) { net -= B; st.losses++; }
            else { net += 3 * B / 2; st.wins++; }
            finish(net, units, tc);
            return;
        }

        bool any_live = false;
        for (int i = 0; i < nh; i++) {
            Hand& h = hands[i];
            if (h.ncards == 1) {  // second card for a split hand
                h.add(draw_seen());
                if (h.split_aces) { any_live = true; continue; }
            }
            for (;;) {
                if (h.value() >= 21) break;
                bool two = h.ncards == 2;
                // Split aces never reach here, so no resplitting aces falls out naturally.
                Action a = decide(h, up, two, two && nh < MAX_HANDS, two && nh == 1);
                if (a == SURRENDER) { h.surrendered = true; break; }
                if (a == STAND) break;
                if (a == HIT) { h.add(draw_seen()); continue; }
                if (a == DOUBLE) {
                    h.bet_mult = 2;
                    st.doubles++;
                    st.action += (uint64_t)B;
                    h.add(draw_seen());
                    break;
                }
                // SPLIT
                st.splits++;
                st.action += (uint64_t)B;
                uint8_t c = h.first;
                Hand nhand;
                nhand.add(c);
                h = Hand();
                h.add(c);
                if (c == 1) { h.split_aces = true; nhand.split_aces = true; }
                hands[nh++] = nhand;
                h.add(draw_seen());
                if (h.split_aces) break;
            }
            if (!h.surrendered && h.value() <= 21) any_live = true;
        }

        rc += HILO[hole];
        unseen_extra = 0;
        Hand d;
        d.add(up);
        d.add(hole);
        if (any_live)
            while (d.value() < 17) d.add(draw_seen());
        int dv = d.value();

        for (int i = 0; i < nh; i++) {
            const Hand& h = hands[i];
            int64_t w = B * h.bet_mult;
            int pv = h.value();
            st.hands++;
            if (h.surrendered) { net -= B / 2; st.surrenders++; st.losses++; }
            else if (pv > 21) { net -= w; st.busts++; st.losses++; }
            else if (dv > 21 || pv > dv) { net += w; st.wins++; }
            else if (pv < dv) { net -= w; st.losses++; }
            else st.pushes++;
        }
        finish(net, units, tc);
    }

    void finish(int64_t net, int units, int tc) {
        st.net += net;
        st.net_sq += (uint64_t)(net * net);
        int64_t flat = net / units;  // every amount scales with the bet, so this is exact
        st.flat_net += flat;
        st.flat_sq += (uint64_t)(flat * flat);
        int b = tc < TC_MIN ? TC_MIN : (tc > TC_MAX ? TC_MAX : tc);
        st.tc_rounds[b - TC_MIN]++;
        st.tc_flat_net[b - TC_MIN] += flat;
    }
};

}  // namespace bj
