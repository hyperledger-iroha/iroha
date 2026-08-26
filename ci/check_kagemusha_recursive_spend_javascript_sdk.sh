#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_JAVASCRIPT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SDK_DIR="${ROOT_DIR}/javascript/iroha_js"

node --test "${SDK_DIR}/test/kagemushaOfflineTorii.test.js"

source_file="${SDK_DIR}/src/kagemushaOffline.js"
types_file="${SDK_DIR}/index.d.ts"
test -f "${source_file}"
test -f "${types_file}"
cmp "${source_file}" "${SDK_DIR}/dist/kagemushaOffline.js"
cmp "${SDK_DIR}/src/toriiClient.js" "${SDK_DIR}/dist/toriiClient.js"
cmp "${SDK_DIR}/src/toriiBrowserClient.js" "${SDK_DIR}/dist/toriiBrowserClient.js"

grep -Fq 'KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 23' "${source_file}"
grep -Fq 'KAGEMUSHA_MANIFEST_VERSION = 4' "${source_file}"
grep -Fq 'KAGEMUSHA_MAX_HOPS = 8' "${source_file}"
grep -Fq 'KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES = 512 * 1024' "${source_file}"
grep -Fq 'KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES = 48 * 1024 * 1024' "${source_file}"
grep -Fq 'getOfflineCapability' "${types_file}"
grep -Fq 'interface OfflineStatus' "${types_file}"
grep -Fq 'submitKagemushaTopUpV4' "${types_file}"
grep -Fq 'submitKagemushaRedeemV4' "${types_file}"
grep -Fq 'getKagemushaOperationStatus' "${types_file}"

if grep -En 'getKagemushaReadinessV4|normalizeKagemushaAssetSelector|normalizeKagemushaReadinessV4|KagemushaReadinessV4' \
  "${source_file}" \
  "${SDK_DIR}/src/index.js" \
  "${SDK_DIR}/src/browser.js" \
  "${SDK_DIR}/src/toriiClient.js" \
  "${SDK_DIR}/src/toriiBrowserClient.js" \
  "${types_file}" \
  "${SDK_DIR}/browser.d.ts"; then
  echo "error: JavaScript offline capability must not expose selector-taking readiness aliases" >&2
  exit 1
fi

if grep -REni 'export[[:space:]]+(class|function|const)[[:space:]]+[^[:space:]]*Kagemusha[^[:space:]]*Prover' \
  "${SDK_DIR}/src"; then
  echo "error: JavaScript must remain a Torii DTO/transport client and must not claim a Kagemusha native prover" >&2
  exit 1
fi

echo "Kagemusha JavaScript boundary passed: ABI-21/V4 Torii DTOs are present without a native prover claim."
