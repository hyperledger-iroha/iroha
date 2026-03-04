#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SPEC_PATH="${REPO_ROOT}/docs/portal/static/openapi/torii.json"
CURRENT_SPEC_PATH="${REPO_ROOT}/docs/portal/static/openapi/versions/current/torii.json"
MANIFEST_PATH="${REPO_ROOT}/docs/portal/static/openapi/manifest.json"
CURRENT_MANIFEST_PATH="${REPO_ROOT}/docs/portal/static/openapi/versions/current/manifest.json"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

run_xtask() {
  local -a args=("$@")
  if NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask "${args[@]}"; then
    return 0
  fi
  echo "cargo xtask unavailable; falling back to cargo run -p xtask --bin xtask -- ${args[*]}" >&2
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run -p xtask --bin xtask -- "${args[@]}"
}

GENERATED_SPEC="${TMP_DIR}/torii.json"

(
  cd "${REPO_ROOT}"
  run_xtask openapi --output "${GENERATED_SPEC}"
)

if ! diff -u "${SPEC_PATH}" "${GENERATED_SPEC}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${GENERATED_SPEC}" || true
  echo "error: docs/portal/static/openapi/torii.json is stale. Run 'npm run sync-openapi -- --latest' from docs/portal/" >&2
  exit 1
fi

if ! diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" || true
  echo "error: docs/portal/static/openapi/versions/current/torii.json is out of sync with the latest spec. Run 'npm run sync-openapi -- --latest' from docs/portal/." >&2
  exit 1
fi

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${SPEC_PATH}" \
    --manifest "${MANIFEST_PATH}" \
    --allowed-signers "${REPO_ROOT}/docs/portal/static/openapi/allowed_signers.json"
)

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${CURRENT_SPEC_PATH}" \
    --manifest "${CURRENT_MANIFEST_PATH}" \
    --allowed-signers "${REPO_ROOT}/docs/portal/static/openapi/allowed_signers.json"
)

(
  cd "${REPO_ROOT}"
  node docs/portal/scripts/verify-openapi-versions.mjs
)
