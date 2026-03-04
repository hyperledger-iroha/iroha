#!/usr/bin/env bash
set -euo pipefail

# List direct serde_json usage candidates with suggested Norito replacements.
# This is a non-failing helper to assist migrations; see deny_serde_json.sh for CI enforcement.

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) not found; install ripgrep to use this tool" >&2
  exit 1
fi

# Exclusions similar to the deny script but non-failing and broader: include benches/tests separately
EXCLUDES=(
  "-g" "!target" \
  "-g" "!crates/norito/**" \
  "-g" "!crates/**/tests/**" \
  "-g" "!crates/**/benches/**" \
  "-g" "!crates/**/examples/**"
)

matches=""
for search_root in "$ROOT_DIR/crates" "$ROOT_DIR/tools"; do
  if [ -d "$search_root" ]; then
    root_matches=$(rg -n "\\bserde_json\\b" "${EXCLUDES[@]}" "$search_root" || true)
    if [ -n "$root_matches" ]; then
      matches+="${root_matches}"$'\n'
    fi
  fi
done

if [[ -z "$matches" ]]; then
  echo "No direct serde_json usage found outside norito/tests/benches/examples."
  exit 0
fi

echo "serde_json usage candidates (outside norito/tests/benches/examples):"
echo "$matches"
echo
cat <<'SUG'
Common replacements:
  serde_json::Value        -> norito::json::Value
  serde_json::from_str     -> norito::json::from_str
  serde_json::to_string    -> norito::json::to_json
  serde_json::to_string_pretty -> norito::json::to_json_pretty
  serde_json::from_slice   -> norito::json::from_slice
  serde_json::to_vec       -> norito::json::to_vec
  serde_json::from_value   -> norito::json::from_value
  serde_json::to_value     -> norito::json::to_value
  serde_json::json!(...)   -> norito::json::json!(...)

Notes:
  - Norito's `json` feature uses the native parser/writer exclusively; serde adapters are no longer provided.
  - Use `norito::json::to_json`/`from_json` for typed roundtrips and `norito::json::Parser` for streaming decode.
SUG
