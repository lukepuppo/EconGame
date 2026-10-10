// Usage: bj [rounds=100000000] [threads=all] [seed=1] [hilo|basic] [vec|ref]
// Games run in lanes (independent games with their own shoes), GROUPS x 16 lanes per set. `vec`
// runs them on the AVX-512 lockstep engine, `ref` on the scalar reference engine; results are
// identical, and the same for any thread count.
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

#include "engine.hpp"

using namespace bj;

#ifndef GROUPS
#define GROUPS 3  // 16-lane groups interleaved per thread (3 measured fastest)
#endif
constexpr int NLANES = GROUPS * VLANES;
constexpr int SETS = 128;  // independent lane sets: the unit of work handed to threads

static void report(const Stats& s) {
    double n = (double)s.rounds;
    double net = s.net / 2.0, init = s.initial_bet / 2.0, act = s.action / 2.0;
    double mean = net / n, var = s.net_sq / 4.0 / n - mean * mean, sd = std::sqrt(var);
    double fmean = s.flat_net / 2.0 / n, fsd = std::sqrt(s.flat_sq / 4.0 / n - fmean * fmean);
    double avg_bet = init / n;
    printf("rounds              %llu\n", (unsigned long long)s.rounds);
    printf("hands               %llu\n", (unsigned long long)s.hands);
    printf("shoes               %llu\n", (unsigned long long)s.shuffles);
    printf("avg initial bet     %.4f units\n", avg_bet);
    printf("total action        %.1f units\n", act);
    printf("net result          %+.1f units\n", net);
    printf("EV per round        %+.5f units (SD %.4f)\n", mean, sd);
    printf("edge vs initial bet %+.4f%% +/- %.4f%% (95%% CI)\n", 100 * mean / avg_bet,
           100 * 1.96 * sd / std::sqrt(n) / avg_bet);
    printf("edge vs total action%+.4f%%\n", 100 * net / act);
    printf("flat-bet edge       %+.4f%% +/- %.4f%% (play only, 1 unit every round)\n", 100 * fmean,
           100 * 1.96 * fsd / std::sqrt(n));
    printf("win/loss/push       %.4f / %.4f / %.4f per hand\n", (double)s.wins / s.hands,
           (double)s.losses / s.hands, (double)s.pushes / s.hands);
    printf("blackjacks          player %.4f%%  dealer %.4f%%\n", 100.0 * s.player_bj / n,
           100.0 * s.dealer_bj / n);
    printf("doubles/splits/surr %llu / %llu / %llu\n", (unsigned long long)s.doubles,
           (unsigned long long)s.splits, (unsigned long long)s.surrenders);
    printf("insurance           taken %llu  won %llu\n", (unsigned long long)s.ins_taken,
           (unsigned long long)s.ins_won);
    printf("true count  %%rounds   flat EV%%\n");
    for (int i = 0; i < TC_BUCKETS; i++) {
        if (!s.tc_rounds[i]) continue;
        int tc = i + TC_MIN;
        printf("%s%+3d    %8.4f  %+8.3f\n", tc == TC_MIN ? "<=" : tc == TC_MAX ? ">=" : "  ", tc,
               100.0 * s.tc_rounds[i] / n, 100.0 * s.tc_flat_net[i] / 2.0 / s.tc_rounds[i]);
    }
}

int main(int argc, char** argv) {
    uint64_t rounds = argc > 1 ? strtoull(argv[1], nullptr, 10) : 100000000ull;
    unsigned threads = argc > 2 ? (unsigned)atoi(argv[2]) : std::thread::hardware_concurrency();
    uint64_t seed = argc > 3 ? strtoull(argv[3], nullptr, 10) : 1;
    bool counting = !(argc > 4 && strcmp(argv[4], "basic") == 0);
    bool reference = argc > 5 && strcmp(argv[5], "ref") == 0;
#ifndef __AVX512F__
    reference = true;  // no AVX-512: scalar engine only
#endif
    if (threads == 0) threads = 1;

    // The work is split into SETS independent lane sets with fixed seeds and round counts, handed
    // out to threads dynamically, so a core slowed by other load doesn't hold up the run. Each
    // set's result doesn't depend on which thread runs it, so output is the same for any thread
    // count.
    std::vector<Stats> results(SETS);
    std::vector<std::thread> pool;
    std::atomic<int> next{0};
    auto t0 = std::chrono::steady_clock::now();
    for (unsigned t = 0; t < threads; t++) {
        pool.emplace_back([&] {
            std::vector<Sim> lanes;
            lanes.reserve(NLANES);
            for (int set; (set = next.fetch_add(1)) < SETS;) {
                uint64_t n = rounds / SETS + ((uint64_t)set < rounds % SETS);
                uint64_t seeder = set_seed(seed, (uint64_t)set);
                lanes.clear();
                for (int l = 0; l < NLANES; l++) lanes.emplace_back(splitmix64(seeder), counting);
                uint64_t per = n / NLANES, extra = n % NLANES;
                if (reference) {
                    for (int l = 0; l < NLANES; l++)
                        for (uint64_t i = 0; i < per + (l < (int)extra); i++) lanes[l].play_round();
                } else {
#ifdef __AVX512F__
                    Engine<GROUPS>(lanes.data(), counting).run(per);
#endif
                    for (uint64_t l = 0; l < extra; l++) lanes[l].play_round();
                }
                for (auto& sim : lanes) results[set].merge(sim.st);
            }
        });
    }
    for (auto& th : pool) th.join();
    double secs = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();

    Stats total;
    for (auto& r : results) total.merge(r);
    printf("mode                %s\n", counting ? "hi-lo (I18 + Fab 4, 1-12 spread)" : "basic strategy, flat bet");
    report(total);
    printf("time: C++ %s, %u thread(s) %.3f s, %.1f M rounds/s, %zu bytes/lane\n",
           reference ? "scalar" : "avx512", threads, secs, rounds / secs / 1e6, sizeof(Sim));
}
