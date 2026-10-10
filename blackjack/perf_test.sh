#!/usr/bin/env bash
# Performance test: runs 100M rounds on all cores N times and fails unless the median wall time is
# under the limit. Usage: ./perf_test.sh BINARY [limit_seconds=0.5] [runs=5]
set -euo pipefail
BIN=$1; LIMIT=${2:-0.5}; RUNS=${3:-5}
times=$(for ((i = 0; i < RUNS; i++)); do "$BIN" 100000000 "$(nproc)" 1 | sed -n 's/.* \([0-9.]*\) s,.*/\1/p'; done | sort -n)
median=$(echo "$times" | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')
echo "100M rounds on $(nproc) threads: $(echo $times | tr '\n' ' ')s  median ${median}s (limit ${LIMIT}s)"
awk -v m="$median" -v l="$LIMIT" 'BEGIN {exit !(m < l)}' && echo "PASS" || { echo "FAIL"; exit 1; }
