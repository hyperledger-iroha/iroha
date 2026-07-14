#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SPEC_PATH="${REPO_ROOT}/docs/portal/static/openapi/torii.json"
CURRENT_SPEC_PATH="${REPO_ROOT}/docs/portal/static/openapi/versions/current/torii.json"
MANIFEST_PATH="${REPO_ROOT}/docs/portal/static/openapi/manifest.json"
CURRENT_MANIFEST_PATH="${REPO_ROOT}/docs/portal/static/openapi/versions/current/manifest.json"
ALLOWED_SIGNERS_PATH="${OPENAPI_ALLOWED_SIGNERS_FILE:-${REPO_ROOT}/docs/portal/static/openapi/allowed_signers.json}"
REQUIRE_SIGNED="${OPENAPI_REQUIRE_SIGNED:-0}"

case "${REQUIRE_SIGNED}" in
  0|1) ;;
  *)
    echo "error: OPENAPI_REQUIRE_SIGNED must be 0 or 1." >&2
    exit 2
    ;;
esac

XTASK_VERIFY_POLICY_ARGS=()
VERSION_VERIFY_POLICY_ARGS=()
SIGNATURE_VERIFY_POLICY_ARGS=(--allow-unsigned=2025-q2)
if [[ "${REQUIRE_SIGNED}" == "0" ]]; then
  XTASK_VERIFY_POLICY_ARGS+=(--allow-unsigned)
  VERSION_VERIFY_POLICY_ARGS+=(--allow-unsigned)
  SIGNATURE_VERIFY_POLICY_ARGS+=(--allow-unsigned=latest --allow-unsigned=current)
fi

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

run_xtask() {
  local -a args=("$@")
  NORITO_SKIP_BINDINGS_SYNC=1 cargo run \
    -p xtask \
    --bin xtask \
    -- \
    "${args[@]}"
}

GENERATED_SPEC="${TMP_DIR}/torii.json"

print_refresh_help() {
  cat >&2 <<'EOF'
Refresh the canonical manifest before syncing snapshots:
  development: cargo run -p xtask --bin xtask -- openapi --unsigned-manifest
               (cd docs/portal && npm run sync-openapi -- --allow-unsigned)
  release:     cargo run -p xtask --bin xtask -- openapi --sign <key>
               (cd docs/portal && npm run sync-openapi -- --allowed-signers=<operator-allowlist-path>)
An operator signature envelope may be supplied instead of --sign for the release path.
For an operator release, set OPENAPI_REQUIRE_SIGNED=1 and
OPENAPI_ALLOWED_SIGNERS_FILE=<operator-allowlist-path> when running this gate.
The checked-in allowlist is intentionally empty. The immutable 2025-q2
development snapshot remains explicitly unsigned in either mode; signed mode
requires the mutable root/latest/current release artifacts to be signed.
EOF
}

(
  cd "${REPO_ROOT}"
  run_xtask openapi --output "${GENERATED_SPEC}"
)

if ! diff -u "${SPEC_PATH}" "${GENERATED_SPEC}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${GENERATED_SPEC}" || true
  echo "error: docs/portal/static/openapi/torii.json is stale." >&2
  print_refresh_help
  exit 1
fi

if ! diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${CURRENT_SPEC_PATH}" || true
  echo "error: docs/portal/static/openapi/versions/current/torii.json is out of sync with the latest spec." >&2
  print_refresh_help
  exit 1
fi

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${SPEC_PATH}" \
    --manifest "${MANIFEST_PATH}" \
    --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
    "${XTASK_VERIFY_POLICY_ARGS[@]}"
)

(
  cd "${REPO_ROOT}"
  run_xtask openapi-verify \
    --spec "${CURRENT_SPEC_PATH}" \
    --manifest "${CURRENT_MANIFEST_PATH}" \
    --allowed-signers "${ALLOWED_SIGNERS_PATH}" \
    "${XTASK_VERIFY_POLICY_ARGS[@]}"
)

(
  cd "${REPO_ROOT}"
  node docs/portal/scripts/verify-openapi-versions.mjs \
    "${VERSION_VERIFY_POLICY_ARGS[@]}"
  node docs/portal/scripts/check-openapi-signatures.mjs \
    --allowed-signers="${ALLOWED_SIGNERS_PATH}" \
    "${SIGNATURE_VERIFY_POLICY_ARGS[@]}"
)
