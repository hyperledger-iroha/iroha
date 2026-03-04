#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
ALLOWED_TOML="$ROOT/crates/norito/Cargo.toml"

violations=()
for dir in crates integration_tests tools xtask python fuzz; do
  base="$ROOT/$dir"
  [[ -d "$base" ]] || continue
  while IFS= read -r manifest; do
    [[ -z "$manifest" ]] && continue
    if [[ "$manifest" == "$ALLOWED_TOML" ]]; then
      continue
    fi
    if rg -q 'parity[-_]scale' "$manifest"; then
      violations+=("$manifest")
    fi
  done < <(find "$base" -name Cargo.toml)
done

if [[ ${#violations[@]} -ne 0 ]]; then
  echo "SCALE dependency detected outside the Norito benches in:" >&2
  printf '  %s\n' "${violations[@]}" >&2
  exit 1
fi

echo "No SCALE dependencies found outside crates/norito." 
