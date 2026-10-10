#!/usr/bin/env bash
# Build both, check that the C++ and Rust AVX-512 engines and the scalar reference engine all
# produce identical results for the same seed, then time them.
# Usage: ./compare.sh [rounds=100000000] [threads=nproc] [seed=1]
set -euo pipefail
cd "$(dirname "$0")"
ROUNDS=${1:-100000000}; THREADS=${2:-$(nproc)}; SEED=${3:-1}
make -s -C cpp bj
(cd rust && cargo build --release -q)
./cpp/bj "$ROUNDS" "$THREADS" "$SEED" > cpp_out.txt
./rust/target/release/bj "$ROUNDS" "$THREADS" "$SEED" > rust_out.txt
./rust/target/release/bj "$ROUNDS" "$THREADS" "$SEED" hilo ref > ref_out.txt
if diff <(grep -v '^time:' cpp_out.txt) <(grep -v '^time:' rust_out.txt) &&
   diff <(grep -v '^time:' cpp_out.txt) <(grep -v '^time:' ref_out.txt); then
  echo "C++, Rust and the scalar reference engine give identical results"
else
  echo "MISMATCH" >&2; exit 1
fi
grep -v '^time:' cpp_out.txt
grep -h '^time:' cpp_out.txt rust_out.txt ref_out.txt | cut -d: -f2-
