#!/usr/bin/sh

# usage: ./profile.sh <binary> [args...] > onoro.svg

set -e

cargo b --profile profiled # --features=profiled
rm -f perf.data
cp brc.svg brc-prev.svg
perf buildid-cache --add ./target/profiled/brc
perf record -g -F 9999 -e branches,branch-misses -e LLC-load-misses:pp -e cycles:pp -e cache-misses:pp --call-graph dwarf ./target/profiled/brc -- $@ >/dev/null
perf script  --demangle \
    | rustfilt \
    | stackcollapse-perf.pl \
    | flamegraph.pl  \
        --width 8000 \
        --fontsize 10 \
        --minwidth 0  \
        > brc.svg
