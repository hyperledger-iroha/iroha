#!/usr/bin/env bash
set -euo pipefail

# Lints the workspace to forbid direct serde/serde_json usage outside the allowlist.
#
# Policy:
# - Disallow direct `serde` and `serde_json` under [dependencies] in Cargo.toml for most crates.
# - Allow in [dev-dependencies], [build-dependencies], and in allowlisted crates/paths.
# - Disallow importing `serde`/`serde_json` in non-test code (exclude tests/, benches/, examples/) outside allowlist.
# - Special-case: `crates/norito` is always allowed to depend on serde/serde_json (optionally) for wrappers.
#
# Allowlist file: scripts/serde_allowlist.txt — paths (one per line) relative to repo root, e.g., `crates/iroha_core`.

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ALLOWLIST_FILE="$ROOT_DIR/scripts/serde_allowlist.txt"

if [[ ! -f "$ALLOWLIST_FILE" ]]; then
  echo "Allowlist not found at $ALLOWLIST_FILE; create it with one crate path per line" >&2
  exit 2
fi

ALLOWED=()
while IFS= read -r line || [[ -n "$line" ]]; do
  case "$line" in
    ''|\#*)
      continue
      ;;
    *)
      ALLOWED+=("$line")
      ;;
  esac
done < "$ALLOWLIST_FILE"

is_allowed() {
  local path="$1"
  if [[ "$path" == crates/norito* ]]; then
    return 0
  fi
  for allow in "${ALLOWED[@]:-}"; do
    if [[ -n "$allow" && "$path" == "$allow"* ]]; then
      return 0
    fi
  done
  return 1
}

violations=()

for search_root in crates tools; do
  root_path="$ROOT_DIR/$search_root"
  [[ -d "$root_path" ]] || continue
  while IFS= read -r cargo_toml; do
    crate_dir="$(dirname "$cargo_toml")"
    rel_dir="${crate_dir#"$ROOT_DIR/"}"

    # Check [dependencies] section for direct serde/serde_json
    deps_hits=$(awk '
      BEGIN{section=""}
      /^\[/ {section=$0}
      section=="[dependencies]" && $0 ~ /^[[:space:]]*serde(_json)?[[:space:]]*=/{print FNR ":" $0}
    ' "$cargo_toml" || true)

    if [[ -n "$deps_hits" ]] && ! is_allowed "$rel_dir"; then
      while IFS= read -r hit; do
        [[ -z "$hit" ]] && continue
        violations+=("$rel_dir/Cargo.toml:$hit")
      done <<< "$deps_hits"
    fi

    # Scan non-test source for serde usage
    if ! is_allowed "$rel_dir"; then
      while IFS= read -r src; do
        # Grep for common serde usage patterns
        if grep -HnE '\b(use[[:space:]]+)?serde(_json)?(::|[[:space:]]*\{)' "$src" >/dev/null; then
          violations+=("$src: direct serde/serde_json import in production code")
        fi
        if grep -HnE '#\[derive\([^\]]*(Serialize|Deserialize)' "$src" >/dev/null; then
          violations+=("$src: derives Serialize/Deserialize in production code")
        fi
      done < <(find "$crate_dir/src" -type f -name '*.rs' 2>/dev/null || true)
    fi
  done < <(find "$root_path" -mindepth 2 -maxdepth 4 -name Cargo.toml | sort)
done

if ((${#violations[@]} > 0)); then
  echo "serde-guard: found forbidden serde/serde_json usage outside allowlist:" >&2
  for v in "${violations[@]}"; do
    echo "  - $v" >&2
  done
  echo >&2
  echo "To migrate: replace JSON with norito::json helpers and binary with norito::{Encode,Decode}." >&2
  echo "If a crate cannot be migrated yet, add its path to scripts/serde_allowlist.txt temporarily." >&2
  exit 1
else
  echo "serde-guard: OK (no forbidden serde/serde_json usage detected)"
fi
