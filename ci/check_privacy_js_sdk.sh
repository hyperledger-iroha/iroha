#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_JS_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${PRIVACY_JS_SDK_NODE_BIN:-}"
PYTHON_BIN="${PRIVACY_JS_SDK_PYTHON_BIN:-python3}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# The JavaScript checks do not invoke Cargo, but they share the privacy SDK
# guard boundary. Preserve a developer's workspace lock when one exists and,
# on a clean checkout, fail if any test creates the ignored root Cargo.lock.
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"
WORKSPACE_CARGO_LOCKFILE="${ROOT_DIR}/Cargo.lock"
WORKSPACE_CARGO_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${WORKSPACE_CARGO_LOCKFILE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"
)"

cleanup_privacy_js_sdk_lock_state() {
  local status=$?
  trap - EXIT HUP INT TERM
  if ! privacy_sdk_assert_optional_file_state \
    "${WORKSPACE_CARGO_LOCKFILE}" \
    "${WORKSPACE_CARGO_LOCK_STATE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"; then
    status=1
  fi
  exit "${status}"
}
trap cleanup_privacy_js_sdk_lock_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

node_candidate_path() {
  local candidate="$1"
  if command -v "${candidate}" >/dev/null 2>&1; then
    command -v "${candidate}"
    return 0
  fi
  if [[ -x "${candidate}" ]]; then
    printf '%s\n' "${candidate}"
    return 0
  fi
  return 1
}

is_node_20_bin() {
  local candidate="$1"
  local version
  version="$("${candidate}" --version 2>/dev/null || true)"
  [[ "${version}" == v20.* ]]
}

resolve_node_20_bin() {
  if [[ -n "${NODE_OVERRIDE}" ]]; then
    printf '%s\n' "${NODE_OVERRIDE}"
    return 0
  fi

  local candidate path
  for candidate in \
    node20 \
    node20.20 \
    /opt/homebrew/opt/node@20/bin/node \
    /usr/local/opt/node@20/bin/node \
    /opt/homebrew/Cellar/node@20/*/bin/node \
    /usr/local/Cellar/node@20/*/bin/node \
    "${HOME}"/.npm/_npx/*/node_modules/node/bin/node \
    node; do
    path="$(node_candidate_path "${candidate}" || true)"
    if [[ -n "${path}" ]] && is_node_20_bin "${path}"; then
      printf '%s\n' "${path}"
      return 0
    fi
  done

  printf '%s\n' "node"
}

NODE_BIN="$(resolve_node_20_bin)"

cd "${ROOT_DIR}/javascript/iroha_js"
NODE_VERSION="$("${NODE_BIN}" --version)"
printf '%s\n' "${NODE_VERSION}"
case "${NODE_VERSION}" in
  v20.*) ;;
  *)
    echo "error: privacy JavaScript SDK tests require Node 20; got ${NODE_VERSION}" >&2
    exit 1
    ;;
esac

export PYTHONDONTWRITEBYTECODE=1

"${NODE_BIN}" --test \
  test/privacyFfiContractParity.test.js \
  test/privacyCatalogParity.test.js \
  test/privacyNative.test.js
"${NODE_BIN}" --test --test-name-pattern "package dist entrypoint exports only the canonical privacy capability bridge" \
  test/package_dist.test.js
"${NODE_BIN}" --test --test-name-pattern "browser crypto exposes only the privacy capability bridge as a safe stub" \
  test/crypto.browser.test.js
