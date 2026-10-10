// Usage: bj [rounds=100000000] [threads=all] [seed=1] [hilo|basic]
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <thread>
#include <vector>

#include "bj.hpp"

using namespace bj;

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
    if (threads == 0) threads = 1;

    std::vector<Stats> results(threads);
    std::vector<std::thread> pool;
    uint64_t seeder = seed;
    auto t0 = std::chrono::steady_clock::now();
    for (unsigned t = 0; t < threads; t++) {
        uint64_t n = rounds / threads + (t < rounds % threads ? 1 : 0);
        uint64_t s = splitmix64(seeder);
        pool.emplace_back([&results, t, n, s, counting] {
            Sim sim(s, counting);
            for (uint64_t i = 0; i < n; i++) sim.play_round();
            results[t] = sim.st;
        });
    }
    for (auto& th : pool) th.join();
    double secs = std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count();

    Stats total;
    for (auto& r : results) total.merge(r);
    printf("mode                %s\n", counting ? "hi-lo (I18 + Fab 4, 1-12 spread)" : "basic strategy, flat bet");
    report(total);
    printf("time: C++ %u thread(s) %.3f s, %.1f M rounds/s, sim state %zu bytes/thread\n", threads, secs,
           rounds / secs / 1e6, sizeof(Sim));
}
