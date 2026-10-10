#!/usr/bin/env bash
# Build both, verify they produce identical results for the same seed, then time them.
# Usage: ./compare.sh [rounds=100000000] [threads=all] [seed=1]
set -euo pipefail
cd "$(dirname "$0")"
ROUNDS=${1:-100000000}; THREADS=${2:-$(nproc)}; SEED=${3:-1}
make -s -C cpp
(cd rust && cargo build --release -q)
./cpp/bj "$ROUNDS" "$THREADS" "$SEED" > cpp_out.txt
./rust/target/release/bj "$ROUNDS" "$THREADS" "$SEED" > rust_out.txt
if diff <(grep -v '^time:' cpp_out.txt) <(grep -v '^time:' rust_out.txt); then
  echo "C++ and Rust results are identical"
else
  echo "MISMATCH between C++ and Rust" >&2; exit 1
fi
grep -v '^time:' cpp_out.txt
grep '^time:' cpp_out.txt rust_out.txt | cut -d: -f2-
