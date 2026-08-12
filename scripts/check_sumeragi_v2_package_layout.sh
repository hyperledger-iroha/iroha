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

# `cargo package` only carries files under the package root. The sole reviewed
# path override is the package-local refinement test split; its nested terminal
# cases use an identity-preserving include. Reject every other path override and
# every parent-relative include.
REFINEMENT_TESTS="$CORE_DIR/refinement_cases.rs"
if [[ ! -f "$REFINEMENT_TESTS" || -L "$REFINEMENT_TESTS" ]]; then
  echo "missing package-local regular refinement test source: $REFINEMENT_TESTS" >&2
  exit 1
fi
REFINEMENT_PIPELINE_TESTS="$CORE_DIR/refinement_cases/terminal_body_pipeline.rs"
if [[ ! -f "$REFINEMENT_PIPELINE_TESTS" || -L "$REFINEMENT_PIPELINE_TESTS" ]]; then
  echo "missing package-local regular terminal body-pipeline refinement test source: $REFINEMENT_PIPELINE_TESTS" >&2
  exit 1
fi
path_attribute_hits="$(
  rg -n '#\[path[[:space:]]*=' "$CORE_MODULE" "$CORE_DIR" || true
)"
path_attribute_count="$(
  printf '%s\n' "$path_attribute_hits" \
    | awk 'NF { count += 1 } END { print count + 0 }'
)"
reviewed_outer_path_count="$(
  rg -F -c '#[path = "refinement_cases.rs"]' "$CORE_DIR/refinement.rs" || true
)"
reviewed_pipeline_include_count="$(
  rg -F -c 'include!("refinement_cases/terminal_body_pipeline.rs");' "$REFINEMENT_TESTS" || true
)"
if [[ "$path_attribute_count" != 1 || "$reviewed_outer_path_count" != 1 \
  || "$reviewed_pipeline_include_count" != 1 ]] \
  || ! rg -U -q \
    '^#\[cfg\(test\)\]\n#\[path = "refinement_cases.rs"\]\nmod tests;$' \
    "$CORE_DIR/refinement.rs"; then
  printf '%s\n' "$path_attribute_hits" >&2
  echo "production Sumeragi v2 reducer must use only the reviewed package-local refinement test split and identity-preserving nested include" >&2
  exit 1
fi
if rg -n \
  'include(_str|_bytes)?![[:space:]]*\([[:space:]]*"\.\.' \
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

for test_module in tests; do
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
