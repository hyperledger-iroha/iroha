#!/usr/bin/env bash
set -euo pipefail

# Deny-list check: prevent new direct serde_json usage across the workspace.
# Roadmap requires migrating app-facing and production JSON to Norito wrappers.
#
# Scope: entire workspace sources, excluding:
#  - norito crate internals
#  - derive/proc-macro crates (codegen may intentionally reference serde_json)
#  - tests/examples/benches directories
#  - known test-only files that live under src (cfg(test))
#  - iroha_version (maintains Norito wrappers)

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)

if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) not found; skipping serde_json deny-list check" >&2
  exit 0
fi

IGNORE=(
  'crates/norito/**'
  'crates/**/_derive/**'
  'crates/iroha_kagami/samples/**'
  'crates/iroha_version/**'
  # Known test-only serde_json uses inside src (cfg(test))
  'crates/iroha_primitives/src/addr.rs'
  'crates/iroha_primitives/src/unique_vec.rs'
  'crates/iroha_primitives/src/numeric.rs'
)

# Build ripgrep glob excludes
GLOB_EXCLUDES=(
  --glob '!**/{tests,benches,examples}/**'
  --glob '!**/Cargo.toml'
  --glob '!**/*.md'
  --glob '!**/AGENTS.md'
)
for pat in "${IGNORE[@]}"; do
  GLOB_EXCLUDES+=(--glob "!${pat}")
done

# Search patterns that indicate direct serde_json usage in sources
matches=""
for search_root in "$ROOT_DIR/crates" "$ROOT_DIR/tools"; do
  if [ -d "$search_root" ]; then
    dir_matches=$(
      rg -n --pcre2 '(?:\buse\s+serde_json\b|\bserde_json\s*(::|!))' \
        "${GLOB_EXCLUDES[@]}" \
        --glob '!**/target/**' \
        "$search_root" || true
    )
    if [ -n "$dir_matches" ]; then
      matches+="${dir_matches}"$'\n'
    fi
  fi
done

if [ -n "$matches" ]; then
  echo "Found disallowed direct serde_json usage in workspace sources:" >&2
  echo "$matches" >&2
  echo >&2
  echo "Use norito::json wrappers instead (norito::json::Value, json!, from_str, to_string_pretty)." >&2
  echo "If this is test-only inside src, add the file to scripts/deny_serde_json.sh IGNORE list with a comment." >&2
  exit 1
fi

exit 0
