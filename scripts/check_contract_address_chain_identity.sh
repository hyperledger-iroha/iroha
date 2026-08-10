#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

readonly rust_forbidden_pattern='ContractAddress::derive\(\s*[^,]{0,180}(?:\bChainId\b|\bchain_id\b|\bchain_discriminant\b|&chain\b|\.chain_id\()'

if rg --pcre2 --multiline --line-number "$rust_forbidden_pattern" crates integration_tests; then
    echo "contract-address derivation must use exact genesis-derived NetworkId, never ChainId" >&2
    exit 1
fi

if ! rg --pcre2 --multiline --quiet \
    'pub fn derive\(\s*network_id: &NetworkId,' \
    crates/iroha_data_model/src/smart_contract.rs; then
    echo "ContractAddress::derive must require typed NetworkId" >&2
    exit 1
fi

readonly js_impl='javascript/iroha_js/src/smartContractDeployment.js'
readonly js_types='javascript/iroha_js/smart-contract-deployment.d.ts'
if rg --line-number 'CHAIN_ID_PATTERN|requireCanonicalChainId' "$js_impl"; then
    echo "JavaScript contract-address derivation retains a retired ChainId path" >&2
    exit 1
fi
if ! rg --quiet 'networkIdBytes\(source\.networkId' "$js_impl" \
    || ! rg --quiet '^[[:space:]]*networkBytes,$' "$js_impl" \
    || ! rg --quiet '^[[:space:]]*chainId\?: never;' "$js_types"; then
    echo "JavaScript contract-address derivation must consume NetworkId and reject chainId" >&2
    exit 1
fi

echo "contract-address exact-NetworkId guard passed"
