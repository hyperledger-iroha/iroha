#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_JS_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${PRIVACY_JS_SDK_NODE_BIN:-}"
PYTHON_BIN="${PRIVACY_JS_SDK_PYTHON_BIN:-python3}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FROZEN_CARGO_LOCK_SHA256="ccf4acebfe63ad981193b87afd559c195d8a67642d9536b8082f77bbf24a11f0"
TRACKED_ROOT_CARGO_LOCK_SHA256="ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7"
ABI22_CHECKER="${ROOT_DIR}/scripts/check_native_sdk_abi22_artifact.py"
NATIVE_BUILD_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/iroha-privacy-js-native.XXXXXX")"

# Preserve the tracked root source authority independently from the distinct
# frozen privacy-release lock selected for the native build.
# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"
WORKSPACE_CARGO_LOCKFILE="${ROOT_DIR}/Cargo.lock"
WORKSPACE_CARGO_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${WORKSPACE_CARGO_LOCKFILE}" \
    "workspace Cargo.lock" \
    "${PYTHON_BIN}"
)"
PRIVACY_RELEASE_CARGO_LOCK_SEAL=""

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
  if [[ -n "${PRIVACY_RELEASE_CARGO_LOCK_SEAL}" ]] && \
    ! privacy_sdk_assert_file_seal \
      "${PRIVACY_RELEASE_CARGO_LOCK}" \
      "${PRIVACY_RELEASE_CARGO_LOCK_SEAL}" \
      "privacy JavaScript external Cargo.lock" \
      "${PYTHON_BIN}"; then
    status=1
  fi
  rm -rf -- "${NATIVE_BUILD_ROOT}"
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

sha256_file() {
  "${PYTHON_BIN}" -I -S - "$1" <<'PY'
import hashlib
import pathlib
import sys

digest = hashlib.sha256()
with pathlib.Path(sys.argv[1]).open("rb") as source:
    while chunk := source.read(1024 * 1024):
        digest.update(chunk)
print(digest.hexdigest())
PY
}

[[ -f "${WORKSPACE_CARGO_LOCKFILE}" && ! -L "${WORKSPACE_CARGO_LOCKFILE}" ]] \
  || { echo "error: privacy JavaScript native execution requires Cargo.lock" >&2; exit 1; }
[[ "$(sha256_file "${WORKSPACE_CARGO_LOCKFILE}")" == "${TRACKED_ROOT_CARGO_LOCK_SHA256}" ]] \
  || { echo "error: privacy JavaScript tracked root Cargo.lock authority changed" >&2; exit 1; }
PRIVACY_RELEASE_CARGO_LOCK="${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH:-}"
[[ -f "${PRIVACY_RELEASE_CARGO_LOCK}" && ! -L "${PRIVACY_RELEASE_CARGO_LOCK}" && \
  "${PRIVACY_RELEASE_CARGO_LOCK}" != "${WORKSPACE_CARGO_LOCKFILE}" ]] \
  || { echo "error: privacy JavaScript requires a distinct external release Cargo.lock" >&2; exit 1; }
[[ "$(sha256_file "${PRIVACY_RELEASE_CARGO_LOCK}")" == "${FROZEN_CARGO_LOCK_SHA256}" ]] \
  || { echo "error: privacy JavaScript external Cargo.lock is not the frozen release lock" >&2; exit 1; }
PRIVACY_RELEASE_CARGO_LOCK_SEAL="$(
  privacy_sdk_file_seal "${PRIVACY_RELEASE_CARGO_LOCK}" "${PYTHON_BIN}"
)" || {
  echo "error: privacy JavaScript external Cargo.lock cannot be identity-sealed" >&2
  exit 1
}

assert_privacy_release_cargo_lock() {
  privacy_sdk_assert_file_seal \
    "${PRIVACY_RELEASE_CARGO_LOCK}" \
    "${PRIVACY_RELEASE_CARGO_LOCK_SEAL}" \
    "privacy JavaScript external Cargo.lock" \
    "${PYTHON_BIN}"
}

RUSTUP_BIN="${PRIVACY_JS_SDK_RUSTUP_BIN:-$(command -v rustup)}"
IROHA_JS_CARGO_PATH="$("${RUSTUP_BIN}" which --toolchain 1.93.1 cargo)"
RUSTC="$("${RUSTUP_BIN}" which --toolchain 1.93.1 rustc)"
RUSTDOC="$("${RUSTUP_BIN}" which --toolchain 1.93.1 rustdoc)"
[[ "$("${RUSTC}" --version)" == rustc\ 1.93.1\ * ]] \
  || { echo "error: privacy JavaScript native execution requires exact rustc 1.93.1" >&2; exit 1; }
export IROHA_JS_CARGO_PATH RUSTC RUSTDOC
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0
export CARGO_NET_OFFLINE=true
export CARGO_TARGET_DIR="${NATIVE_BUILD_ROOT}/target"
export IROHA_JS_CARGO_LOCKFILE_PATH="${PRIVACY_RELEASE_CARGO_LOCK}"
export IROHA_JS_NATIVE_DIR="${NATIVE_BUILD_ROOT}/native"
export NORITO_SKIP_BINDINGS_SYNC=1
export RUSTC_BOOTSTRAP=1

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

"${NODE_BIN}" scripts/build-native.mjs
"${NODE_BIN}" scripts/copy-native.mjs
NATIVE_ARTIFACT="${IROHA_JS_NATIVE_DIR}/iroha_js_host.node"
NATIVE_MANIFEST="${NATIVE_BUILD_ROOT}/node-native-abi22.json"
NATIVE_TARGET="$("${NODE_BIN}" --eval 'process.stdout.write(`${process.platform}-${process.arch}-node${process.versions.node.split(".")[0]}`)')"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" record \
  --artifact "${NATIVE_ARTIFACT}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --node "${NODE_BIN}" \
  --sdk node \
  --target "${NATIVE_TARGET}"
"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_ARTIFACT}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --node "${NODE_BIN}"

"${NODE_BIN}" scripts/build-dist.mjs
"${NODE_BIN}" --test test/privacyNative.integration.test.js
"${NODE_BIN}" --test \
  test/privacyFfiContractParity.test.js \
  test/privacyCatalogParity.test.js \
  test/privacyNative.test.js \
  test/privacyCapabilities.test.js
"${NODE_BIN}" --test --test-name-pattern "verifier ids use|complete ProofBox|exact canonical base64|commitments and envelope|support lane privacy|reject empty lane|malformed and impossible|non-canonical lane|invalid ids and extra tails" \
  test/instructionBuilders.test.js
"${NODE_BIN}" --test test/proofAttachmentParity.test.js
"${NODE_BIN}" --test test/privacyExact12Network.test.js
"${NODE_BIN}" --test test/privacyExact12FixtureBundle.test.js
"${NODE_BIN}" --test --test-name-pattern "package dist entrypoint exports only the canonical privacy compiled-profile catalog bridge|package declarations expose readonly snapshot metadata without retired privacy types" \
  test/package_dist.test.js
"${NODE_BIN}" --test --test-name-pattern "strict NodeNext resolves the root and every public subpath from a packed layout" \
  test/packageTypes.test.js
"${NODE_BIN}" --test --test-name-pattern "browser crypto exposes only the privacy compiled-profile catalog bridge as a safe stub" \
  test/crypto.browser.test.js

"${PYTHON_BIN}" -I -S "${ABI22_CHECKER}" verify \
  --artifact "${NATIVE_ARTIFACT}" \
  --manifest "${NATIVE_MANIFEST}" \
  --source-root "${ROOT_DIR}" \
  --node "${NODE_BIN}"
[[ "$(sha256_file "${WORKSPACE_CARGO_LOCKFILE}")" == "${TRACKED_ROOT_CARGO_LOCK_SHA256}" ]] \
  || { echo "error: tracked root Cargo.lock changed during privacy JavaScript native execution" >&2; exit 1; }
privacy_sdk_assert_optional_file_state \
  "${WORKSPACE_CARGO_LOCKFILE}" \
  "${WORKSPACE_CARGO_LOCK_STATE}" \
  "workspace Cargo.lock" \
  "${PYTHON_BIN}"
assert_privacy_release_cargo_lock
