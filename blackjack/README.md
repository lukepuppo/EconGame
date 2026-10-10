# Blackjack Hi-Lo simulator (C++ vs Rust)

Two equivalent implementations of the same simulator. They use the same RNG and draw cards in the
same order, so a given `rounds threads seed` produces **byte-identical results** in both languages.
`compare.sh` checks that before it prints any timings.

```
./compare.sh [rounds=100000000] [threads=nproc] [seed=1]
make -C cpp test           # C++ unit tests (ASan + UBSan)
(cd rust && cargo test)    # Rust unit tests
./cpp/bj 100000000 4 1 basic   # flat-bet basic strategy baseline
```

## Rules
6 decks, reshuffle after 5 decks dealt (83% penetration). Dealer stands on soft 17 and peeks for
blackjack. 3:2 blackjack. Double on any two cards, double after split. Split up to 4 hands; split
aces get one card each and can't be resplit. Late surrender. Insurance is offered.

## Player
- **Basic strategy:** 4+ deck S17/DAS/LS chart.
- **Hi-Lo count:** 2–6 = +1, 7–9 = 0, T/A = −1. The dealer's hole card is counted when it's turned
  over. True count = running count ÷ unseen decks, compared exactly with integers (no rounding).
- **Deviations:** Illustrious 18 (Schlesinger), including insurance at TC ≥ +3, plus the Fab 4
  surrenders (14v10 +3, 15v10 0, 15v9 +2, 15vA +1).
- **Bet ramp:** 1-12 units on the floored true count: ≤+1 → 1, +2 → 2, +3 → 4, +4 → 6, +5 → 8,
  +6 → 10, ≥+7 → 12.

## Memory and speed choices
- **Small state per thread:** a `Sim` is about 860 bytes: a 312-byte shoe of `u8` ranks, the RNG,
  the count and the stats. The hot loop never allocates. Hands live in a fixed `[Hand; 4]` on the
  stack, 7 bytes each, holding a running total rather than a list of cards.
- **Lazy Fisher-Yates shuffle:** each draw swaps a random undealt card into place. The shoe stays a
  permutation of itself, so reshuffling is just `pos = 0`. Only the 260 cards that get dealt are
  ever shuffled, never the full 312.
- **Fast RNG:** xoshiro256** with Lemire's unbiased bounded integers. No modulo bias and no
  division on the fast path.
- **Exact integer money:** amounts are kept in half-units, so 3:2 payouts, insurance and surrender
  stay exact.
- **Threads share nothing:** each thread keeps its own stats, and they're merged once at the end.
  No atomics, locks or false sharing.
- **Build flags:** C++ uses `-O3 -march=native -flto`. Rust uses `opt-level=3`, fat LTO,
  `codegen-units=1`, `panic=abort` and `target-cpu=native`.

## Results: 100,000,000 rounds, seed 1 (identical in both languages)

| | Hi-Lo + I18/Fab 4, 1-12 spread | Basic strategy, flat bet |
|---|---|---|
| Player edge (vs initial bet) | **+1.187% ± 0.035%** | −0.376% ± 0.022% |
| Edge vs total action | +1.029% | −0.332% |
| Average initial bet | 1.656 units | 1 unit |
| EV per round | +0.0197 units (SD 2.935) | −0.0038 units (SD 1.141) |

With the count running but a flat 1-unit bet, the edge is −0.221%. So the deviations alone are
worth about 0.155%, and the bet spread is what turns the game positive. The −0.376% baseline is
close to the published house edge for these rules (about 0.35–0.40%).

Flat-bet EV by true count (in the counting run): TC 0 → −0.11%, +1 → +0.59%, +2 → +1.21%,
+3 → +1.76%, +4 → +2.46%, +5 → +3.22%, ≥+10 → +7.85%.

## Performance (4-vCPU cloud container, 100M rounds)

| | 1 thread (best of 3) | 4 threads |
|---|---|---|
| C++ (g++ 13) | 9.11 s (11.0 M rounds/s) | 2.28 s (43.8 M/s) |
| C++ (clang 18) | 8.32 s (12.0 M rounds/s) | — |
| Rust 1.97 | 8.61 s (11.6 M rounds/s) | 2.20 s (45.5 M/s) |

All three are within about 10% of each other. The gap follows the compiler backend (LLVM vs GCC)
more than the language: clang-built C++ and Rust both use LLVM and land within a few percent.
