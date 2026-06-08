#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
violations=()
retired_codec_pattern='parity[-_]'"scale"
if rg -q "$retired_codec_pattern" "$ROOT/Cargo.toml"; then
  violations+=("$ROOT/Cargo.toml")
fi

for dir in crates integration_tests tools xtask python fuzz; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  while IFS= read -r manifest; do
    [[ -z "$manifest" ]] && continue
    if rg -q "$retired_codec_pattern" "$manifest"; then
      violations+=("$manifest")
    fi
  done < <(find "$base" -name Cargo.toml)
done

if [[ ${#violations[@]} -ne 0 ]]; then
  echo "retired codec dependency detected in:" >&2
  printf '  %s\n' "${violations[@]}" >&2
  exit 1
fi

echo "No retired codec dependencies found."
