#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_POLICY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_policy.sh [--self-test]" >&2
  exit 2
fi

if [[ "${MODE}" == "--self-test" ]]; then
  "${ROOT_DIR}/ci/check_kagemusha_production_readiness.sh" candidate --self-test
  "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh" --self-test
else
  "${ROOT_DIR}/ci/check_kagemusha_production_readiness.sh" candidate
  "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh"
fi

"${ROOT_DIR}/ci/check_kagemusha_recursive_spend_payload_bench.sh"
