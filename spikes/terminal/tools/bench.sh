#!/usr/bin/env bash
# usage: bench.sh <spike-binary> <data-dir>
BIN=$1; D=$2
for variant in gui forcedraw headless; do
  for e in none alacritty ghostty; do
    for f in plain ansi; do
      for i in 1 2 3; do
        extra=(); envs=()
        [ $variant = headless ] && extra=(--headless)
        [ $variant = forcedraw ] && envs=(SPIKE_FORCE_DRAW=1)
        out=$(env "${envs[@]}" /usr/bin/time -l "$BIN" --engine $e --bench "$D/$f-50mb.txt" "${extra[@]}" 2>&1)
        secs=$(echo "$out" | sed -n 's/.*seconds=\([0-9.]*\).*/\1/p')
        rss=$(echo "$out" | awk '/maximum resident/{printf "%.1f", $1/1048576}')
        foot=$(echo "$out" | awk '/peak memory footprint/{printf "%.1f", $1/1048576}')
        ren=$(echo "$out" | sed -n 's/renders=//p')
        echo "$variant $e $f run$i secs=$secs maxrss_mb=$rss footprint_mb=$foot renders=${ren:--}"
      done
    done
  done
done
