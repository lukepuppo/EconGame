#!/usr/bin/env bash
# Performance test: runs 100M rounds on all cores N times and fails unless EVERY run finishes in
# under the limit. Usage: ./perf_test.sh BINARY [limit_seconds=0.5] [runs=10]
set -euo pipefail
BIN=$1; LIMIT=${2:-0.5}; RUNS=${3:-10}
times=$(for ((i = 0; i < RUNS; i++)); do "$BIN" 100000000 "$(nproc)" 1 | sed -n 's/.* \([0-9.]*\) s,.*/\1/p'; done | sort -n)
worst=$(echo "$times" | tail -1)
echo "100M rounds on $(nproc) threads, $RUNS runs: $(echo $times | tr '\n' ' ')s  worst ${worst}s (limit ${LIMIT}s)"
awk -v w="$worst" -v l="$LIMIT" 'BEGIN {exit !(w < l)}' && echo "PASS" || { echo "FAIL"; exit 1; }
