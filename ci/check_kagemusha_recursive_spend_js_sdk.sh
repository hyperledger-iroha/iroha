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

"${NODE_BIN}" --test --test-name-pattern "Kagemusha recursive spend|Kagemusha record-backed|Kagemusha .* SDK runner|browser crypto exposes native-only helpers as safe stubs|buildKagemusha|privacy native availability probes build and verify with Norito request archives|privacy native wrappers require binary Norito request archives|privacy algorithm JS catalogs reject malformed internal review evidence|fromAccount rejects control and Unicode-confusable curve algorithm aliases|offline cash configuration snapshot requires cached issuer key and ABI|canonical request signing: rejects padded auth fields|streamEvents rejects unsupported production backend event filters before fetch|streamEvents rejects malformed verifying key event names before fetch|streamEvents rejects malformed proof event hashes before fetch|ZK-ACE verifier-key references reject padded selector metadata|privacy proof envelopes preserve pending production backend tags|verifyIdentifierResolutionReceipt rejects adversarial receipt mutations|encodeIdentifierResolutionReceiptPayload rejects non-exact execution tags|encodeIdentifierResolutionReceiptAttestation rejects padded proof backend|verifyIdentifierResolutionReceipt matches shared receipt vectors|NexusAppClient rejects non-Ed25519 wallet signatures|NexusAppClient accepts exact numeric and string Ed25519 signature algorithm tags|ToriiClient attaches canonical signing headers for app endpoints|ToriiClient canonical auth uses raw Node transport for UTF-8 account headers|ToriiClient canonical auth rejects UTF-8 account headers when no supported transport is available|ToriiClient canonical auth rejects non-byte private key arrays|subscription plan and create endpoints send normalized payloads|subscription action endpoints send normalized payloads|getSubscription returns null on 404|buildConnectWebSocketUrl rejects token query parameters|buildConnectWebSocketUrl rejects endpoint host overrides|buildConnectWebSocketUrl rejects endpoint protocol mismatches|openConnectWebSocket injects Sec-WebSocket-Protocol when headers are unavailable|openConnectWebSocket emits telemetry when allowInsecure is used|resolveAliasByIndex enforces non-negative indices before issuing requests|resolveAlias attaches canonical auth when provided|lookupAliasesByAccount validates options before issuing requests|generateConnectSid|createConnectSessionPreview|connectErrorFrom returns existing ConnectError|connect queue overflow maps to queueOverflow category|connect queue expiration maps to timeout category|http status errors derive authorization category|network socket failures map to transport category|tls failures map to authorization category|timeout detection handles timeouts codes and names|syntax errors surface codec category|http timeout status maps to timeout category|http rate limit status maps to retryable transport category|http 4xx client errors no longer map to authorization by default|connect retry|memory journal|indexeddb journal|connect journal|connect queue diagnostics|connect queue root resolves config before env and gates env usage|Connect session vector fixture matches browser crypto helpers|Connect browser wallet signature encoder validates algorithm labels before byte encoding|buildConnectWebSocketUrl switches schemes for secure and insecure Torii urls|registerConnectSession posts sid and node directly to Torii|deleteConnectSession tolerates missing sessions and uses DELETE|resolveConnectLaunchUri prefers canonical session deeplinks|rewriteConnectUriProtocol swaps the scheme without changing the session payload|resolveConnectLaunchUriForProtocol rewrites the selected launch URI|openConnectWebSocket sends the connect token as the first subprotocol|createConnectAppSession|bootstrapConnectPreviewSession|ConnectJournalRecord|fromCiphertext applies retention automatically|decode accepts array-like payloads|decode rejects|decode accepts header padding" \
  test/address.test.js \
  test/canonicalRequest.test.js \
  test/connect.browser.test.js \
  test/connectError.test.js \
  test/connectJournalRecord.test.js \
  test/connectPreviewFlow.test.js \
  test/connectQueueDiagnostics.test.js \
  test/connectQueueJournal.test.js \
  test/connectRetryPolicy.test.js \
  test/connectSession.test.js \
  test/connectWebSocket.test.js \
  test/crypto.browser.test.js \
  test/instructionBuilders.test.js \
  test/kagemushaFfiContractParity.test.js \
  test/kagemushaRecursiveSpend.test.js \
  test/nexusAppClient.test.js \
  test/offlineCashLifecycle.test.js \
  test/package_dist.test.js \
  test/privacyCatalogParity.test.js \
  test/privacyNative.test.js \
  test/toriiCanonicalAuth.test.js \
  test/toriiClient.identifier.test.js \
  test/toriiClient.isoAlias.test.js \
  test/toriiClient.test.js \
  test/toriiSubscriptions.test.js \
  test/transactionBuilder.test.js
