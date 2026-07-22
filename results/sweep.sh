#!/usr/bin/env bash
# Hyperparameter robustness sweep.
#
# Answers the reviewer's two obvious objections:
#   1. "You handicapped UCB1 with a bad exploration constant."   → sweep MURMURATION_UCB1_C
#   2. "Q-routing only wins at one lucky (alpha, epsilon)."       → sweep BENCH_QALPHA / BENCH_QEPS
#
# Both sweeps run the same fixed condition and print delivery% (mean over seeds)
# for the arms that matter, so the comparison is read off one column.
#
# Usage:  bash results/sweep.sh
# Writes: results/sweep.csv   (one row per configuration)
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=(cargo run --release --quiet --bin benchmark)
# Fixed evaluation point. Concentrated traffic (s=1.2) is where Q-routing is
# active; 5 seeds and 40k messages keep the full sweep to a few minutes.
COMMON=(BENCH_NODES=50 BENCH_SEEDS=5 BENCH_MESSAGES=40000 BENCH_ZIPF=1.2)
OUT=results/sweep.csv

# Pull the p=0.20 delivery% for one strategy out of a benchmark run's stdout.
field() {  # $1 = logfile, $2 = strategy
  awk -v s="$2" '$1==s && $3=="0.20" {gsub("%","",$4); print $4; exit}' "$1"
}

echo "sweep,param,value,ucb1,q_routing,agnostic_limit,oracle" > "$OUT"
tmp=$(mktemp)

echo "── UCB1 exploration constant (Q at defaults) ──"
printf "%-8s %8s %10s %10s %8s\n" "C" "ucb1" "q_routing" "ceiling" "oracle"
for C in 0.25 0.5 1.0 2.0 4.0 8.0; do
  env "${COMMON[@]}" MURMURATION_UCB1_C=$C BENCH_TAG=sweep "${BIN[@]}" > "$tmp" 2>/dev/null
  u=$(field "$tmp" ucb1); q=$(field "$tmp" q_routing)
  a=$(field "$tmp" agnostic_limit); o=$(field "$tmp" oracle)
  printf "%-8s %8s %10s %10s %8s\n" "$C" "$u" "$q" "$a" "$o"
  echo "ucb1_c,MURMURATION_UCB1_C,$C,$u,$q,$a,$o" >> "$OUT"
done

echo
echo "── Q-routing learning rate alpha (epsilon=0.05) ──"
printf "%-8s %8s %10s %10s %8s\n" "alpha" "ucb1" "q_routing" "ceiling" "oracle"
for A in 0.05 0.10 0.15 0.30 0.50; do
  env "${COMMON[@]}" BENCH_QALPHA=$A BENCH_TAG=sweep "${BIN[@]}" > "$tmp" 2>/dev/null
  u=$(field "$tmp" ucb1); q=$(field "$tmp" q_routing)
  a=$(field "$tmp" agnostic_limit); o=$(field "$tmp" oracle)
  printf "%-8s %8s %10s %10s %8s\n" "$A" "$u" "$q" "$a" "$o"
  echo "q_alpha,BENCH_QALPHA,$A,$u,$q,$a,$o" >> "$OUT"
done

echo
echo "── Q-routing exploration epsilon (alpha=0.15) ──"
printf "%-8s %8s %10s %10s %8s\n" "eps" "ucb1" "q_routing" "ceiling" "oracle"
for E in 0.01 0.02 0.05 0.10 0.20; do
  env "${COMMON[@]}" BENCH_QEPS=$E BENCH_TAG=sweep "${BIN[@]}" > "$tmp" 2>/dev/null
  u=$(field "$tmp" ucb1); q=$(field "$tmp" q_routing)
  a=$(field "$tmp" agnostic_limit); o=$(field "$tmp" oracle)
  printf "%-8s %8s %10s %10s %8s\n" "$E" "$u" "$q" "$a" "$o"
  echo "q_eps,BENCH_QEPS,$E,$u,$q,$a,$o" >> "$OUT"
done

rm -f "$tmp" results/benchmark_sweep.csv results/learning_curve_sweep.csv
echo
echo "Wrote $OUT"
