#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_MODULE="$REPO_ROOT/crates/iroha_core/src/sumeragi/v2_core.rs"
CORE_DIR="$REPO_ROOT/crates/iroha_core/src/sumeragi/v2_core"

command -v rg >/dev/null 2>&1 || {
  echo "ripgrep is required to check the Sumeragi v2 package layout" >&2
  exit 1
}

if [[ ! -f "$CORE_MODULE" || -L "$CORE_MODULE" ]]; then
  echo "missing package-local regular reducer module: $CORE_MODULE" >&2
  exit 1
fi

# `cargo package` only carries files under the package root. Reject source
# indirection that could compile in the repository while disappearing from a
# published `iroha_core` archive.
if rg -n \
  '#\[path[[:space:]]*=|include(_str|_bytes)?![[:space:]]*\([[:space:]]*"\.\.' \
  "$CORE_MODULE" "$CORE_DIR"; then
  echo "production Sumeragi v2 reducer must not load source outside iroha_core" >&2
  exit 1
fi

for module in quorum refinement reducer types wal; do
  source="$CORE_DIR/$module.rs"
  if [[ ! -f "$source" || -L "$source" ]]; then
    echo "missing package-local regular reducer source: $source" >&2
    exit 1
  fi
  if ! rg -q "^[[:space:]]*mod ${module};$" "$CORE_MODULE"; then
    echo "production reducer does not declare package-local module ${module}" >&2
    exit 1
  fi
done

for test_module in tests network_simulation; do
  source="$CORE_DIR/$test_module.rs"
  if [[ ! -f "$source" || -L "$source" ]]; then
    echo "missing package-local reducer test source: $source" >&2
    exit 1
  fi
  if ! rg -q "^[[:space:]]*mod ${test_module};$" "$CORE_MODULE"; then
    echo "iroha_core test compilation does not include ${test_module}" >&2
    exit 1
  fi
done

network_test_count="$(rg -c '^#\[test\]$' "$CORE_DIR/network_simulation.rs")"
if [[ "$network_test_count" != "8" ]]; then
  echo "expected eight package-local Sumeragi v2 network simulations, found $network_test_count" >&2
  exit 1
fi
