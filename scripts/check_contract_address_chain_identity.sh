#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

readonly forbidden_pattern='ContractAddress::derive\([[:space:]]*(?:(?:iroha_data_model|crate)::account::address::)?chain_discriminant\(\)|ContractAddress::derive\([[:space:]]*iroha_config::parameters::defaults::common::chain_discriminant\(\)'

if rg --pcre2 --multiline --line-number "$forbidden_pattern" crates/iroha_core/src; then
    echo "ledger-side contract-address derivation must use the authenticated State ChainId" >&2
    exit 1
fi

echo "contract-address ChainId guard passed"
