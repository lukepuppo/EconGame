#!/usr/bin/env bash
# Compare simulator binaries fairly on a noisy machine: runs them alternately N times and prints
# the min and median wall time of 100M rounds on all cores. Usage: ./bench.sh N bin1 [bin2 ...]
set -euo pipefail
N=$1; shift
declare -A T
for ((i = 0; i < N; i++)); do
  for b in "$@"; do T[$b]+="$("$b" 100000000 "$(nproc)" 1 | sed -n 's/.* \([0-9.]*\) s,.*/\1/p') "; done
done
for b in "$@"; do
  echo "$b: $(echo ${T[$b]} | tr ' ' '\n' | sort -n | awk '{a[NR]=$1} END {printf "min %.3f s  median %.3f s  (n=%d)", a[1], a[int((NR+1)/2)], NR}')"
done
