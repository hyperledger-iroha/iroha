#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${KAGEMUSHA_RECURSIVE_SPEND_JS_SDK_NODE_BIN:-}"

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
    echo "error: Kagemusha recursive spend JavaScript SDK tests require Node 20; got ${NODE_VERSION}" >&2
    exit 1
    ;;
esac

"${NODE_BIN}" --test --test-name-pattern "Kagemusha recursive spend|Kagemusha record-backed|Kagemusha .* SDK runner|browser crypto exposes native-only helpers as safe stubs|buildKagemusha|privacy native availability probes build and verify with Norito request archives|privacy native wrappers require binary Norito request archives|fromAccount rejects control and Unicode-confusable curve algorithm aliases|offline cash configuration snapshot requires cached issuer key and ABI|canonical request signing: rejects padded auth fields|streamEvents rejects unsupported production backend event filters before fetch|streamEvents rejects malformed verifying key event names before fetch|streamEvents rejects malformed proof event hashes before fetch|ZK-ACE verifier-key references reject padded selector metadata|privacy proof envelopes preserve pending production backend tags|verifyIdentifierResolutionReceipt rejects adversarial receipt mutations|encodeIdentifierResolutionReceiptPayload rejects non-exact execution tags|encodeIdentifierResolutionReceiptAttestation rejects padded proof backend|verifyIdentifierResolutionReceipt matches shared receipt vectors|NexusAppClient rejects non-Ed25519 wallet signatures|NexusAppClient accepts exact numeric and string Ed25519 signature algorithm tags" \
  test/address.test.js \
  test/canonicalRequest.test.js \
  test/crypto.browser.test.js \
  test/instructionBuilders.test.js \
  test/kagemushaFfiContractParity.test.js \
  test/kagemushaRecursiveSpend.test.js \
  test/nexusAppClient.test.js \
  test/offlineCashLifecycle.test.js \
  test/package_dist.test.js \
  test/privacyNative.test.js \
  test/toriiClient.identifier.test.js \
  test/toriiClient.test.js \
  test/transactionBuilder.test.js
