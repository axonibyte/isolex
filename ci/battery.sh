#!/usr/bin/env bash
# The engine battery as reaper's run verb (container execution, same
# toolchain image as the build). Owns its exit status; results land in
# $REAPER_OUT.
set -euo pipefail

export CARGO_HOME="${REAPER_CACHE_CARGO:-$HOME/.cargo}"
export CARGO_TARGET_DIR="${REAPER_CACHE_TARGET:-target}"

OUT="${REAPER_OUT:-out}"
mkdir -p "$OUT"
rm -rf "${OUT:?}"/*

cargo build --release --locked --bin isolex --bin battery

if "$CARGO_TARGET_DIR/release/battery" \
    --isolex "$CARGO_TARGET_DIR/release/isolex" \
    --rounds "${ROUNDS:-5}" \
    > "$OUT/battery.log" 2>&1; then
  status=0
else
  status=$?
fi
tail -5 "$OUT/battery.log"
exit $status
