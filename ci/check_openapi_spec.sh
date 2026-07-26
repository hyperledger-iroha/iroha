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
SIGNATURE_VERIFY_POLICY_ARGS=(--allow-unsigned=2025-q2)
if [[ "${REQUIRE_SIGNED}" == "0" ]]; then
  XTASK_VERIFY_POLICY_ARGS+=(--allow-unsigned)
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
    --locked \
    --offline \
    -p xtask \
    --bin xtask \
    -- \
    "${args[@]}"
}

require_clean_checkout() {
  if [[ -n "$(git -C "${REPO_ROOT}" status --porcelain=v1 --untracked-files=all)" ]]; then
    echo "error: Torii OpenAPI release generation requires a clean checkout." >&2
    echo "Commit or remove every tracked and untracked source change, then rerun from the pinned commit." >&2
    exit 1
  fi
}

EXPECTED_GENERATOR_COMMIT="$(
  git -C "${REPO_ROOT}" rev-parse --verify 'HEAD^{commit}'
)"
if [[ ! "${EXPECTED_GENERATOR_COMMIT}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "error: Torii OpenAPI release generation requires a lowercase 40-hex Git commit." >&2
  exit 1
fi

GENERATED_SPEC_FIRST="${TMP_DIR}/torii-first.json"
GENERATED_SPEC_SECOND="${TMP_DIR}/torii-second.json"

print_refresh_help() {
  cat >&2 <<'EOF'
Refresh the canonical manifest before syncing snapshots:
  development: cargo run --locked --offline -p xtask --bin xtask -- openapi --unsigned-manifest
               (cd docs/portal && npm run sync-openapi -- --allow-unsigned)
  release payload:
               cargo run --locked --offline -p xtask --bin xtask -- openapi \
                 --unsigned-manifest --signing-payload <operator-staging>/openapi-manifest-v2.payload
  release attach after the Ed25519 HSM signs those exact bytes:
               cargo run --locked --offline -p xtask --bin xtask -- openapi \
                 --signature-envelope <operator-staging>/openapi-manifest-v2.signature.json
               (cd docs/portal && npm run sync-openapi -- --allowed-signers=<operator-allowlist-path>)
Local private-key signing is intentionally unavailable; release signing is detached-only.
For an operator release, set OPENAPI_REQUIRE_SIGNED=1 and
OPENAPI_ALLOWED_SIGNERS_FILE=<operator-allowlist-path> when running this gate.
The checked-in allowlist is intentionally empty. The immutable 2025-q2
development snapshot remains explicitly unsigned in either mode; signed mode
requires the mutable root/latest/current release artifacts to be signed.
This gate always requires a clean checkout and clean mutable generator
provenance bound to the exact checked-out commit. --allow-unsigned relaxes only
the detached-signature requirement; it never permits generator_dirty.
EOF
}

require_clean_checkout

if ! diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" >/dev/null; then
  diff -u "${MANIFEST_PATH}" "${CURRENT_MANIFEST_PATH}" || true
  echo "error: checked-in latest/current OpenAPI manifests are not byte-identical." >&2
  print_refresh_help
  exit 1
fi

(
  cd "${REPO_ROOT}"
  node docs/portal/scripts/verify-openapi-versions.mjs \
    --expected-generator-commit="${EXPECTED_GENERATOR_COMMIT}"
)

(
  cd "${REPO_ROOT}"
  run_xtask openapi --output "${GENERATED_SPEC_FIRST}"
  run_xtask openapi --output "${GENERATED_SPEC_SECOND}"
)

if ! diff -u "${GENERATED_SPEC_FIRST}" "${GENERATED_SPEC_SECOND}" >/dev/null; then
  diff -u "${GENERATED_SPEC_FIRST}" "${GENERATED_SPEC_SECOND}" || true
  echo "error: two Torii OpenAPI generation passes produced different bytes." >&2
  exit 1
fi

if ! diff -u "${SPEC_PATH}" "${GENERATED_SPEC_FIRST}" >/dev/null; then
  diff -u "${SPEC_PATH}" "${GENERATED_SPEC_FIRST}" || true
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

require_clean_checkout

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
    --expected-generator-commit="${EXPECTED_GENERATOR_COMMIT}"
  node docs/portal/scripts/check-openapi-signatures.mjs \
    --allowed-signers="${ALLOWED_SIGNERS_PATH}" \
    "${SIGNATURE_VERIFY_POLICY_ARGS[@]}"
)

require_clean_checkout
