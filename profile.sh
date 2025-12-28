#!/usr/bin/sh

# usage: ./profile.sh <binary> [args...] > onoro.svg

set -e

cargo b --profile profiled
rm -f perf.data
cp brc.svg brc-prev.svg
perf record -g -F 9999 --call-graph dwarf,16384 ./target/profiled/brc -- $@ >/dev/null
perf script | stackcollapse-perf.pl | flamegraph.pl > brc.svg
