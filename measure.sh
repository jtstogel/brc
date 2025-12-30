#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <runs> <command...>" >&2
  exit 1
fi

runs="$1"
shift
cmd=( "$@" )

min=""
max=""
sum=0

for ((i=1; i<=runs; i++)); do
  start=$(date +%s%N)          # nanoseconds since epoch
  "${cmd[@]}" >/dev/null
  end=$(date +%s%N)

  elapsed_ns=$((end - start))

  # init min/max on first iteration
  if [[ -z "$min" || $elapsed_ns -lt $min ]]; then min=$elapsed_ns; fi
  if [[ -z "$max" || $elapsed_ns -gt $max ]]; then max=$elapsed_ns; fi

  sum=$((sum + elapsed_ns))
done

avg=$((sum / runs))

to_sec () {
  awk -v ns="$1" 'BEGIN { printf "%.6f", ns / 1000000000 }'
}

echo "Runs: $runs"
echo "Min:  $(to_sec "$min")s"
echo "Max:  $(to_sec "$max")s"
echo "Avg:  $(to_sec "$avg")s"
