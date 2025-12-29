#!/usr/bin/env bash
set -euo pipefail

# Deny ad-hoc AoS row encoder/decoder implementations outside the `norito` crate.
#
# Intent: AoS helpers must live only in `crates/norito/src/aos.rs` (and friends).
# Reintroducing hand-rolled AoS encoders in other crates risks drift from the
# canonical formats, duplicate logic, and compatibility issues.
#
# Heuristic: flag Rust sources that define functions named like `encode_rows_*`
# or `decode_rows_*` outside the Norito crate and outside test/example/bench dirs.
# This keeps the check simple and low-noise while catching likely reintroductions.

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) not found; skipping AoS hand-rolled deny check" >&2
  exit 0
fi

# Build ripgrep exclude list
EXCLUDES=(
  "-g" "!target" \
  "-g" "!crates/norito/**" \
  "-g" "!crates/**/tests/**" \
  "-g" "!crates/**/benches/**" \
  "-g" "!crates/**/examples/**" \
  "-g" "!scripts/regenerate_norito_goldens/**"
)

# Match function definitions with the suspicious prefixes.
# Note: We keep it anchored to `fn` to avoid matching calls/uses.
PATTERN='^\s*(pub\s+)?fn\s+(encode_rows_|decode_rows_|view_aos_|encode_ncb_|view_ncb_)'

matches=$(rg -n -e "$PATTERN" "${EXCLUDES[@]}" "$ROOT_DIR/crates" || true)

if [[ -n "$matches" ]]; then
  echo "AoS/NCB guard: found potential hand-rolled AoS/NCB encoders/decoders/views outside crates/norito:" >&2
  echo "$matches" >&2
  echo >&2
  echo "Please remove these helpers and use norito::aos::{encode_*,decode_*} and norito::columnar::{encode_ncb_*,view_ncb_*,view_aos_*} / adaptive helpers instead." >&2
  exit 1
fi

echo "AoS guard: OK (no hand-rolled AoS helpers detected outside norito)"
