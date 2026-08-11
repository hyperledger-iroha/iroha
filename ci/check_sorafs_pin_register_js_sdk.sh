#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${SORAFS_PIN_REGISTER_JS_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${SORAFS_PIN_REGISTER_JS_SDK_NODE_BIN:-}"

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

is_node_24_bin() {
  local candidate="$1"
  local version
  version="$("${candidate}" --version 2>/dev/null || true)"
  [[ "${version}" == v24.* ]]
}

resolve_node_24_bin() {
  if [[ -n "${NODE_OVERRIDE}" ]]; then
    printf '%s\n' "${NODE_OVERRIDE}"
    return 0
  fi

  local candidate path
  for candidate in \
    node24 \
    node24.0 \
    /opt/homebrew/opt/node@24/bin/node \
    /usr/local/opt/node@24/bin/node \
    /opt/homebrew/Cellar/node@24/*/bin/node \
    /usr/local/Cellar/node@24/*/bin/node \
    "${HOME}"/.npm/_npx/*/node_modules/node/bin/node \
    node; do
    path="$(node_candidate_path "${candidate}" || true)"
    if [[ -n "${path}" ]] && is_node_24_bin "${path}"; then
      printf '%s\n' "${path}"
      return 0
    fi
  done

  printf '%s\n' "node"
}

NODE_BIN="$(resolve_node_24_bin)"

cd "${ROOT_DIR}/javascript/iroha_js"
NODE_VERSION="$("${NODE_BIN}" --version)"
printf '%s\n' "${NODE_VERSION}"
case "${NODE_VERSION}" in
  v24.*) ;;
  *)
    echo "error: SoraFS pin-register JavaScript SDK tests require Node 24; got ${NODE_VERSION}" >&2
    exit 1
    ;;
esac

"${NODE_BIN}" --test --test-name-pattern "registerSorafsPinManifest|buildRegisterPinManifestInstruction|rejects a retired submitted epoch|SoraFS pin-register SDK guard|SoraFS .* SDK runner" \
  test/toriiClient.test.js \
  test/transactionBuilder.test.js \
  test/sorafsPinRegisterSdkGuard.test.js
