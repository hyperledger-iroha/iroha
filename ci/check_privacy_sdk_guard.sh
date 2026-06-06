#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SDK_GUARD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
NODE_OVERRIDE="${PRIVACY_SDK_GUARD_NODE_BIN:-}"
PYTHON_BIN="${PRIVACY_SDK_GUARD_PYTHON_BIN:-python3}"
SDK_PYTHON_OVERRIDE="${PRIVACY_PYTHON_SDK_PYTHON_BIN:-}"
VENV_DIR="${PRIVACY_SDK_GUARD_VENV:-${TMPDIR:-/tmp}/iroha-privacy-sdk-guard-venv}"
MODE="${1:-}"

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

resolve_python_311_bin() {
  if [[ -n "${SDK_PYTHON_OVERRIDE}" ]]; then
    printf '%s\n' "${SDK_PYTHON_OVERRIDE}"
    return 0
  fi

  local candidate
  for candidate in python3.11 /opt/homebrew/bin/python3.11 /usr/local/bin/python3.11 python3; do
    if command -v "${candidate}" >/dev/null 2>&1; then
      command -v "${candidate}"
      return 0
    fi
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done

  printf '%s\n' "python3"
}

"${PYTHON_BIN}" - "${ROOT_DIR}" "${MODE}" <<'PY'
import re
import subprocess
import sys
from fnmatch import fnmatchcase
from pathlib import Path

root = Path(sys.argv[1])
mode = sys.argv[2]
text_overrides = {}
workflow_path = ".github/workflows/pr_privacy_sdk_guard.yml"
bridge_header_command = "ci/check_connect_norito_bridge_header.sh"
bridge_header_drift_commands = (
    (
        "missing privacy C header declaration negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-privacy-header",
    ),
    (
        "bad privacy C header signature negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-privacy-signature",
    ),
    (
        "missing Rust privacy export negative control",
        "ci/check_connect_norito_bridge_header.sh --negative-control-missing-privacy-rust-export",
    ),
)
bytecode_command = "bash ci/check_no_tracked_python_bytecode.sh"
main_command = "ci/check_privacy_sdk_guard.sh"
native_bridge_command = "cargo test -p connect_norito_bridge privacy_ --lib -- --test-threads=1"
native_bridge_job = "privacy_native_bridge_tests"
csharp_sdk_command = "ci/check_privacy_csharp_sdk.sh"
csharp_sdk_job = "privacy_csharp_sdk_tests"
js_sdk_install_command = "npm ci --prefix javascript/iroha_js"
js_sdk_command = "ci/check_privacy_js_sdk.sh"
js_sdk_job = "privacy_javascript_sdk_tests"
jvm_sdk_command = "ci/check_privacy_jvm_sdk.sh"
jvm_sdk_job = "privacy_jvm_sdk_tests"
python_sdk_command = "ci/check_privacy_python_sdk.sh"
python_sdk_job = "privacy_python_sdk_tests"
swift_sdk_command = "ci/check_privacy_swift_sdk.sh"
swift_sdk_job = "privacy_swift_sdk_parse"
main_job = "privacy-sdk-guard"
main_job_needs_line = (
    "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, "
    "privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, "
    "privacy_javascript_sdk_tests, privacy_python_sdk_tests]"
)
negative_control_commands = (
    (
        "bridge header workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-workflow-path",
    ),
    (
        "bridge header command workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-command-workflow",
    ),
    (
        "bridge header negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bridge-header-negative-controls-workflow",
    ),
    (
        "tracked Python bytecode workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-bytecode-workflow",
    ),
    (
        "workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-path",
    ),
    (
        "workflow command negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-command",
    ),
    (
        "backend-tag workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-backend-tag-workflow-path",
    ),
    (
        "browser test workflow path negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-test-workflow-path",
    ),
    (
        "negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-workflow",
    ),
    (
        "commented negative-control workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-comment-workflow",
    ),
    (
        "negative-control ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-order-workflow",
    ),
    (
        "negative-control inventory parity test negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-negative-controls-inventory-parity",
    ),
    (
        "README boundary negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-boundary",
    ),
    (
        "README API negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-api",
    ),
    (
        "ZK-ACE proof-builder coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-zk-ace-proof-builder-coverage",
    ),
    (
        "ZK-ACE production-gate fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-zk-ace-production-gate-fail-closed-coverage",
    ),
    (
        "Python ZK-ACE production-gate fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-zk-ace-production-gate-fail-closed-coverage",
    ),
    (
        "Python direct ZK-ACE production-gate fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-direct-zk-ace-production-gate-fail-closed-coverage",
    ),
    (
        "Python ZK-ACE capability production-gate fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-zk-ace-capability-production-gate-fail-closed-coverage",
    ),
    (
        "JS ZK-ACE capability production-gate fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-zk-ace-capability-production-gate-fail-closed-coverage",
    ),
    (
        "public privacy required production plan rows coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-rows-coverage",
    ),
    (
        "public privacy required production plan exact row coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-exact-row-coverage",
    ),
    (
        "public privacy required production plan row order coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-row-order-coverage",
    ),
    (
        "public privacy required production plan display-text coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-display-text-coverage",
    ),
    (
        "public privacy required production plan category coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-category-coverage",
    ),
    (
        "public privacy required production plan maturity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-maturity-coverage",
    ),
    (
        "public privacy required production plan recommendedFor coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-recommended-for-coverage",
    ),
    (
        "public privacy required production plan covered-criteria coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-covered-criteria-coverage",
    ),
    (
        "public privacy required production plan proof-family coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-proof-family-coverage",
    ),
    (
        "public privacy required production plan public-input schema coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-public-input-schema-coverage",
    ),
    (
        "public privacy required production plan verifier-key coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-verifier-key-coverage",
    ),
    (
        "public privacy required production plan state-token coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-state-token-coverage",
    ),
    (
        "public privacy required production plan failure-mode coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-failure-mode-coverage",
    ),
    (
        "public privacy required production plan exact failure-modes coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-failure-modes-coverage",
    ),
    (
        "public privacy required production plan security-notes coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-security-notes-coverage",
    ),
    (
        "public privacy required production plan source-reference coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-source-reference-coverage",
    ),
    (
        "public privacy required production plan exact source-reference coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-source-reference-exact-coverage",
    ),
    (
        "public privacy required production plan SDK entrypoint coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-sdk-entrypoint-coverage",
    ),
    (
        "public privacy required production plan planned SDK entrypoint coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-planned-sdk-entrypoint-coverage",
    ),
    (
        "public privacy required production plan PQ-layer coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-pq-layer-coverage",
    ),
    (
        "public privacy required production plan chain-requirement coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-chain-requirement-coverage",
    ),
    (
        "public privacy required production plan required-state coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-required-state-coverage",
    ),
    (
        "public privacy required production plan setup-step coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-setup-step-coverage",
    ),
    (
        "public privacy required production plan execution-step coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-required-production-plan-execution-step-coverage",
    ),
    (
        "Python catalog bytecode guard negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-catalog-bytecode-guard",
    ),
    (
        "Python FFI catalog bytecode guard negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-ffi-catalog-bytecode-guard",
    ),
    (
        "source-reference obfuscated IPv4 coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-source-reference-obfuscated-ipv4-coverage",
    ),
    (
        "source-reference audit-readiness URL coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-source-reference-audit-readiness-url-coverage",
    ),
    (
        "source-reference encoded-host URL coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-source-reference-encoded-host-url-coverage",
    ),
    (
        "DevFixture entrypoint fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-dev-fixture-entrypoint-coverage",
    ),
    (
        "privacy catalog defensive-copy coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-catalog-defensive-copy-coverage",
    ),
    (
        "planned privacy entrypoint quarantine coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-planned-entrypoint-quarantine-coverage",
    ),
    (
        "native privacy catalog parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-catalog-parity-coverage",
    ),
    (
        "native privacy planned entrypoint dispatch coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-planned-entrypoint-dispatch-coverage",
    ),
    (
        "native privacy planned entrypoint rejection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-planned-entrypoint-rejection-coverage",
    ),
    (
        "native privacy catalog structure coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-catalog-structure-coverage",
    ),
    (
        "native privacy required production plan rows coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-required-production-plan-rows-coverage",
    ),
    (
        "native privacy required production plan row completeness coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-required-production-plan-row-completeness-coverage",
    ),
    (
        "native privacy required production plan duplicate row coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-required-production-plan-duplicate-row-coverage",
    ),
    (
        "native privacy required production plan public parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-required-production-plan-public-parity-coverage",
    ),
    (
        "native privacy required production allowlist profile coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-required-production-allowlist-profile-coverage",
    ),
    (
        "native privacy verifier-key registration coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-verifier-key-registration-coverage",
    ),
    (
        "native privacy public catalog parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-public-catalog-parity-coverage",
    ),
    (
        "native privacy component proof-only coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-component-proof-only-coverage",
    ),
    (
        "native privacy planned ledger-mutation proof-builder coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-planned-ledger-mutation-proof-builder-coverage",
    ),
    (
        "native privacy ledger-mutation proof-pairing coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-ledger-mutation-proof-pairing-coverage",
    ),
    (
        "native privacy capability fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-capability-fail-closed-coverage",
    ),
    (
        "native privacy ZK-ACE capability fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-zk-ace-capability-fail-closed-coverage",
    ),
    (
        "native privacy capability production-claim quarantine coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-capability-claim-quarantine-coverage",
    ),
    (
        "native privacy capability archive invariant coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-capability-archive-invariant-coverage",
    ),
    (
        "privacy FFI adversarial fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-adversarial-fail-closed-coverage",
    ),
    (
        "privacy FFI verify empty public inputs coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-verify-empty-public-inputs-coverage",
    ),
    (
        "privacy FFI operation-confusion fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-operation-confusion-fail-closed-coverage",
    ),
    (
        "privacy FFI operation required-material coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-operation-required-material-coverage",
    ),
    (
        "privacy FFI non-proof entrypoint rejection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-non-proof-entrypoint-rejection-coverage",
    ),
    (
        "privacy FFI verifier-key backend binding coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-vk-ref-backend-binding-coverage",
    ),
    (
        "privacy FFI verifier-key shape hardening coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-vk-ref-shape-hardening-coverage",
    ),
    (
        "privacy FFI verifier-key name binding coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-vk-ref-name-binding-coverage",
    ),
    (
        "privacy FFI production-disabled build gate-message coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-production-disabled-gate-message-coverage",
    ),
    (
        "privacy FFI production-disabled verify gate-message coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-production-disabled-verify-gate-message-coverage",
    ),
    (
        "privacy FFI production-disabled message constant coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-production-disabled-message-constant-coverage",
    ),
    (
        "privacy FFI ZK-ACE production-disabled request coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-zk-ace-production-disabled-request-coverage",
    ),
    (
        "privacy FFI failure-result invariant coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-failure-result-invariant-coverage",
    ),
    (
        "privacy FFI witness helper non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-witness-helper-nonreflection-coverage",
    ),
    (
        "privacy FFI witness non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-witness-nonreflection-coverage",
    ),
    (
        "privacy FFI proof non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-proof-nonreflection-coverage",
    ),
    (
        "privacy FFI request non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-nonreflection-coverage",
    ),
    (
        "privacy FFI request text-field enumerator coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-text-field-enumerator-coverage",
    ),
    (
        "privacy FFI oversized request text-field non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-oversized-request-field-nonreflection-coverage",
    ),
    (
        "privacy FFI oversized request byte-payload non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-oversized-request-payload-nonreflection-coverage",
    ),
    (
        "privacy FFI oversized public-input non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-oversized-public-inputs-nonreflection-coverage",
    ),
    (
        "privacy FFI oversized proof non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-oversized-proof-nonreflection-coverage",
    ),
    (
        "privacy FFI control-character request text-field non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-control-request-field-nonreflection-coverage",
    ),
    (
        "privacy FFI control-character vk_ref non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-control-vk-ref-nonreflection-coverage",
    ),
    (
        "privacy FFI non-ASCII request text-field non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-non-ascii-request-field-nonreflection-coverage",
    ),
    (
        "privacy FFI non-ASCII vk_ref non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-non-ascii-vk-ref-nonreflection-coverage",
    ),
    (
        "privacy FFI unportable request text-field non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-unportable-request-field-nonreflection-coverage",
    ),
    (
        "privacy FFI unportable vk_ref non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-unportable-vk-ref-nonreflection-coverage",
    ),
    (
        "privacy FFI required request fields non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-required-request-fields-nonreflection-coverage",
    ),
    (
        "privacy FFI required vk_ref non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-required-vk-ref-nonreflection-coverage",
    ),
    (
        "privacy FFI request catalog-shape non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-catalog-shape-nonreflection-coverage",
    ),
    (
        "privacy FFI request entrypoint catalog-shape non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-entrypoint-catalog-shape-nonreflection-coverage",
    ),
    (
        "privacy FFI request production-claim non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-production-claim-nonreflection-coverage",
    ),
    (
        "privacy FFI request vk_ref production-claim non-reflection coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-request-vk-ref-production-claim-nonreflection-coverage",
    ),
    (
        "privacy FFI public operation schema coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-public-operation-schema-coverage",
    ),
    (
        "cross-SDK privacy operation schema coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-cross-sdk-operation-schema-coverage",
    ),
    (
        "privacy native availability output hardening coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-availability-output-hardening-coverage",
    ),
    (
        "privacy native availability probe gating coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-availability-probe-gating-coverage",
    ),
    (
        "cross-SDK privacy request boundary coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-cross-sdk-request-boundary-coverage",
    ),
    (
        "cross-SDK privacy sliced byte-view boundary coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-cross-sdk-sliced-view-boundary-coverage",
    ),
    (
        "cross-SDK privacy native output boundary coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-cross-sdk-native-output-boundary-coverage",
    ),
    (
        "privacy Norito request schema hardening coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-norito-request-schema-hardening-coverage",
    ),
    (
        "privacy Norito request field-bitset coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-norito-request-field-bitset-coverage",
    ),
    (
        "privacy Norito wrong-schema request coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-norito-wrong-schema-request-coverage",
    ),
    (
        "privacy request decoder bounds coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-request-decoder-bounds-coverage",
    ),
    (
        "privacy C bridge output-buffer precedence coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-c-bridge-output-buffer-precedence-coverage",
    ),
    (
        "privacy native adversarial request frame coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-adversarial-request-frame-coverage",
    ),
    (
        "privacy request copy isolation coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-request-copy-isolation-coverage",
    ),
    (
        "privacy request copy zeroization coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-request-copy-zeroization-coverage",
    ),
    (
        "Python privacy native method-surface coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-native-method-surface-coverage",
    ),
    (
        "privacy native ABI probe bounds coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-privacy-abi-probe-bounds-coverage",
    ),
    (
        "ZK-ACE public proof-builder native-error sanitizer negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-zk-ace-public-proof-builder-native-error-sanitizer-coverage",
    ),
    (
        "privacy malformed request no-dispatch coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-malformed-request-no-dispatch-coverage",
    ),
    (
        "privacy public wrapper isolation coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-wrapper-isolation-coverage",
    ),
    (
        "privacy public archive wrapper Norito coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-public-archive-wrapper-norito-coverage",
    ),
    (
        "privacy backend alias fail-closed coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-backend-alias-fail-closed-coverage",
    ),
    (
        "required pending backend parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-required-pending-backend-parity-coverage",
    ),
    (
        "privacy chain backend allowlist coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-chain-backend-allowlist-coverage",
    ),
    (
        "required production allowlist backend coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-required-production-allowlist-backend-coverage",
    ),
    (
        "required production allowlist Rust backend mapping negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-required-production-allowlist-rust-backend-mapping",
    ),
    (
        "required production allowlist public backend labels negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-required-production-allowlist-public-backends",
    ),
    (
        "required production allowlist row scope negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-required-production-allowlist-row-scope",
    ),
    (
        "privacy FFI ABI surface coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-abi-surface-coverage",
    ),
    (
        "privacy FFI binding loader surface coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-binding-loader-surface-coverage",
    ),
    (
        "privacy FFI error contract parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-ffi-error-contract-parity-coverage",
    ),
    (
        "privacy native archive max parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-archive-max-parity-coverage",
    ),
    (
        "privacy SDK bridge method surface coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-sdk-bridge-method-surface-coverage",
    ),
    (
        "privacy binary-only Norito FFI coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-binary-only-norito-ffi-coverage",
    ),
    (
        "privacy native host Norito-only FFI coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-host-norito-only-ffi-coverage",
    ),
    (
        "privacy production gate missing-reason parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-production-gate-missing-reason-parity-coverage",
    ),
    (
        "privacy native production gate missing-reason parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-production-gate-missing-reason-parity-coverage",
    ),
    (
        "privacy Norito schema operation parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-norito-schema-operation-parity-coverage",
    ),
    (
        "privacy Norito proof operation variant parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-norito-operation-variant-parity-coverage",
    ),
    (
        "privacy capability metadata parity coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-capability-metadata-parity-coverage",
    ),
    (
        "privacy mobile ZK-ACE capability quarantine coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-mobile-zk-ace-capability-quarantine-coverage",
    ),
    (
        "privacy capability fail-closed metadata coverage negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-capability-fail-closed-metadata-coverage",
    ),
    (
        "README error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
    ),
    (
        "browser source error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-error-code",
    ),
    (
        "browser dist error-code negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-browser-dist-error-code",
    ),
    (
        "workflow cancellation negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-workflow-cancel-in-progress",
    ),
    (
        "native bridge workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-job-workflow",
    ),
    (
        "native bridge runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-runner-workflow",
    ),
    (
        "native bridge test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-test-workflow",
    ),
    (
        "native bridge dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-native-bridge-needs-workflow",
    ),
    (
        "Swift SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-job-workflow",
    ),
    (
        "Swift SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-runner-workflow",
    ),
    (
        "Swift SDK parse workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-parse-workflow",
    ),
    (
        "Swift SDK version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-version-script",
    ),
    (
        "Swift SDK compiler override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-override-script",
    ),
    (
        "Swift SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-swift-sdk-needs-workflow",
    ),
    (
        "JVM SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-job-workflow",
    ),
    (
        "JVM SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-setup-workflow",
    ),
    (
        "JVM SDK Java distribution workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-distribution-workflow",
    ),
    (
        "JVM SDK Java version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-version-workflow",
    ),
    (
        "JVM SDK Java setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-setup-order-workflow",
    ),
    (
        "JVM SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-test-workflow",
    ),
    (
        "JVM SDK JDK 21 script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-jdk21-script",
    ),
    (
        "JVM SDK Java home override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-home-override-script",
    ),
    (
        "JVM SDK inherited Java home rejection script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-java-home-reject-script",
    ),
    (
        "JVM SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-jvm-sdk-needs-workflow",
    ),
    (
        "C# SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-job-workflow",
    ),
    (
        "C# SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-setup-workflow",
    ),
    (
        "C# SDK dotnet version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-version-workflow",
    ),
    (
        "C# SDK setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-setup-order-workflow",
    ),
    (
        "C# SDK dotnet version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-version-script",
    ),
    (
        "C# SDK dotnet override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-override-script",
    ),
    (
        "C# SDK dotnet major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-dotnet-major-script",
    ),
    (
        "C# SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-test-workflow",
    ),
    (
        "C# SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-csharp-sdk-needs-workflow",
    ),
    (
        "JavaScript SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-job-workflow",
    ),
    (
        "JavaScript SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-runner-workflow",
    ),
    (
        "JavaScript SDK Node setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-setup-workflow",
    ),
    (
        "JavaScript SDK Node version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-version-workflow",
    ),
    (
        "JavaScript SDK Node version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-version-script",
    ),
    (
        "JavaScript SDK Node override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-override-script",
    ),
    (
        "JavaScript SDK Node resolver script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-resolver-script",
    ),
    (
        "JavaScript SDK Node major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-major-script",
    ),
    (
        "JavaScript SDK Python bytecode script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-python-bytecode-script",
    ),
    (
        "JavaScript SDK Node cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-cache-workflow",
    ),
    (
        "JavaScript SDK Node setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-node-setup-order-workflow",
    ),
    (
        "JavaScript SDK install workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-install-workflow",
    ),
    (
        "JavaScript SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-test-workflow",
    ),
    (
        "JavaScript SDK install ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-install-order-workflow",
    ),
    (
        "JavaScript SDK test ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-test-order-workflow",
    ),
    (
        "JavaScript SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-js-sdk-needs-workflow",
    ),
    (
        "Python SDK workflow job negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-job-workflow",
    ),
    (
        "Python SDK runner workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-runner-workflow",
    ),
    (
        "Python SDK setup workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-setup-workflow",
    ),
    (
        "Python SDK version workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-version-workflow",
    ),
    (
        "Python SDK setup ordering workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-setup-order-workflow",
    ),
    (
        "Python SDK Rust cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-rust-cache-workflow",
    ),
    (
        "Python SDK timeout workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-timeout-workflow",
    ),
    (
        "Python SDK version script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-version-script",
    ),
    (
        "Python SDK override script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-override-script",
    ),
    (
        "Python SDK resolver script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-resolver-script",
    ),
    (
        "Python SDK major script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-major-script",
    ),
    (
        "Python SDK stale venv rebuild script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-venv-rebuild-script",
    ),
    (
        "Python SDK native build script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-native-build-script",
    ),
    (
        "Python SDK venv activation script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-venv-activation-script",
    ),
    (
        "Python SDK bytecode script negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-bytecode-script",
    ),
    (
        "Python SDK test workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-test-workflow",
    ),
    (
        "Python SDK dependency workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-python-sdk-needs-workflow",
    ),
    (
        "main guard Rust cache workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-main-rust-cache-workflow",
    ),
    (
        "main guard timeout workflow negative control",
        "ci/check_privacy_sdk_guard.sh --negative-control-main-timeout-workflow",
    ),
)
required_paths = (
    workflow_path,
    "ci/check_connect_norito_bridge_header.sh",
    "ci/check_no_tracked_python_bytecode.sh",
    "ci/check_privacy_csharp_sdk.sh",
    "ci/check_privacy_js_sdk.sh",
    "ci/check_privacy_jvm_sdk.sh",
    "ci/check_privacy_python_sdk.sh",
    "ci/check_privacy_sdk_guard.sh",
    "ci/check_privacy_swift_sdk.sh",
    "crates/connect_norito_bridge/Cargo.toml",
    "crates/connect_norito_bridge/include/connect_norito_bridge.h",
    "crates/connect_norito_bridge/include/NoritoBridge.h",
    "crates/connect_norito_bridge/src/lib.rs",
    "crates/iroha_js_host/Cargo.toml",
    "crates/iroha_js_host/src/lib.rs",
    "crates/iroha_data_model/src/zk.rs",
    "python/iroha_python/iroha_python_rs/Cargo.toml",
    "python/iroha_python/iroha_python_rs/src/lib.rs",
    "javascript/iroha_js/src/crypto.js",
    "javascript/iroha_js/src/crypto.browser.js",
    "javascript/iroha_js/src/index.js",
    "javascript/iroha_js/src/instructionBuilders.js",
    "javascript/iroha_js/src/norito.js",
    "javascript/iroha_js/src/privacyAlgorithms.js",
    "javascript/iroha_js/dist/crypto.js",
    "javascript/iroha_js/dist/crypto.browser.js",
    "javascript/iroha_js/dist/index.js",
    "javascript/iroha_js/dist/instructionBuilders.js",
    "javascript/iroha_js/dist/norito.js",
    "javascript/iroha_js/dist/privacyAlgorithms.js",
    "javascript/iroha_js/index.d.ts",
    "javascript/iroha_js/README.md",
    "javascript/iroha_js/test/privacyFfiContractParity.test.js",
    "javascript/iroha_js/test/privacyCatalogParity.test.js",
    "javascript/iroha_js/test/privacyNative.test.js",
    "javascript/iroha_js/test/instructionBuilders.test.js",
    "javascript/iroha_js/test/package_dist.test.js",
    "javascript/iroha_js/test/crypto.browser.test.js",
    "python/iroha_python/src/iroha_python/__init__.py",
    "python/iroha_python/src/iroha_python/crypto.py",
    "python/iroha_python/src/iroha_python/privacy_catalog.py",
    "python/iroha_python/src/iroha_python/anonymous_pgc.py",
    "python/iroha_python/src/iroha_python/jindo.py",
    "python/iroha_python/src/iroha_python/silent_threshold.py",
    "python/iroha_python/src/iroha_python/sis_hints.py",
    "python/iroha_python/src/iroha_python/vega.py",
    "python/iroha_python/src/iroha_python/verange.py",
    "python/iroha_python/src/iroha_python/zk_ams.py",
    "python/iroha_python/src/iroha_python/zk_x509.py",
    "python/iroha_python/src/iroha_python/zkat.py",
    "python/iroha_python/tests/privacy_catalog_test.py",
    "python/iroha_python/tests/package_import_fallback_test.py",
    "python/iroha_python/tests/privacy_native_registry_test.py",
    "python/iroha_python/tests/crypto_algorithms_test.py",
    "python/iroha_python/tests/anonymous_pgc_test.py",
    "python/iroha_python/tests/jindo_test.py",
    "python/iroha_python/tests/silent_threshold_test.py",
    "python/iroha_python/tests/sis_hints_test.py",
    "python/iroha_python/tests/vega_test.py",
    "python/iroha_python/tests/verange_test.py",
    "python/iroha_python/tests/zk_ams_test.py",
    "python/iroha_python/tests/zk_x509_test.py",
    "python/iroha_python/tests/zkat_test.py",
    "python/iroha_python/README.md",
    "IrohaSwift/README.md",
    "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
    "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift",
    "IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift",
    "java/iroha_android/README.md",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RegisterPinManifestInstruction.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtils.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyBackendTag.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyRecordDescription.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/zk/VerifyingKeyStatus.java",
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/model/instructions/VerifyingKeyInstructionUtilsTests.java",
    "java/iroha_android/src/test/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridgeTest.java",
    "kotlin/README.md",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/RegisterPinManifestInstruction.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTag.kt",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/core/model/zk/VerifyingKeyBackendTagTest.kt",
    "kotlin/core-jvm/src/test/kotlin/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridgeTest.kt",
    "csharp/README.md",
    "csharp/src/Hyperledger.Iroha.Sdk/Hyperledger.Iroha.Sdk.csproj",
    "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
    "csharp/src/Hyperledger.Iroha.Sdk/Zk/VerifyingKeyBackendTag.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
    "csharp/tests/Hyperledger.Iroha.Sdk.Tests/VerifyingKeyBackendTagTests.cs",
    "docs/source/offline_kagemusha.md",
    "roadmap.md",
    "status.md",
)

readme_required = {
    "IrohaSwift/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesV1()",
        "buildProofV1(requestArchive:)",
        "verifyProofV1(requestArchive:)",
        "productionReady = false",
        "ffiStatusError",
        "ffiErrorNullPointer",
        "ffiErrorMalformedNorito",
        "ffiErrorUnsupportedAlgorithm",
        "ffiErrorProductionDisabled",
        "ffiErrorInvalidRequest",
    ),
    "java/iroha_android/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesArchive()",
        "buildProof(requestArchive)",
        "verifyProof(requestArchive)",
        "productionReady = false",
        "STATUS_ERROR",
        "ERROR_NULL_POINTER",
        "ERROR_MALFORMED_NORITO",
        "ERROR_UNSUPPORTED_ALGORITHM",
        "ERROR_PRODUCTION_DISABLED",
        "ERROR_INVALID_REQUEST",
    ),
    "kotlin/README.md": (
        "PrivacyNativeBridge",
        "capabilitiesArchive()",
        "buildProof(requestArchive)",
        "verifyProof(requestArchive)",
        "productionReady = false",
        "STATUS_ERROR",
        "ERROR_NULL_POINTER",
        "ERROR_MALFORMED_NORITO",
        "ERROR_UNSUPPORTED_ALGORITHM",
        "ERROR_PRODUCTION_DISABLED",
        "ERROR_INVALID_REQUEST",
    ),
    "csharp/README.md": (
        "Hyperledger.Iroha.Privacy.PrivacyNative",
        "CapabilitiesV1()",
        "BuildProofV1(requestArchive)",
        "VerifyProofV1(requestArchive)",
        "ProductionReady = false",
        "StatusError",
        "ErrorNullPointer",
        "ErrorMalformedNorito",
        "ErrorUnsupportedAlgorithm",
        "ErrorProductionDisabled",
        "ErrorInvalidRequest",
    ),
    "javascript/iroha_js/README.md": (
        "isPrivacyNativeAvailable()",
        "privacyCapabilitiesV1()",
        "privacyBuildProofV1(requestArchive)",
        "privacyVerifyProofV1(requestArchive)",
        "productionReady = false",
        "PRIVACY_FFI_STATUS_ERROR",
        "PRIVACY_FFI_ERROR_NULL_POINTER",
        "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    ),
    "python/iroha_python/README.md": (
        "privacy_capabilities_v1()",
        "privacy_build_proof_v1(request_archive)",
        "privacy_verify_proof_v1(request_archive)",
        "build_zk_ace_authorization_proof_v1()",
        "zk_ace_build_transfer_authorization_v1()",
        "production_ready = False",
        "PRIVACY_FFI_STATUS_ERROR",
        "PRIVACY_FFI_ERROR_NULL_POINTER",
        "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
    ),
}

common_readme_required = (
    "raw Norito archives",
    "64 MiB native size cap",
    "operation-specific result schema",
    "privacy-production-gate-v1",
    "fail-closed",
    "capabilities",
    "build",
    "verify",
    "deterministic privacy FFI status/error-code contract",
    "status_error = 1",
    "null_pointer = 1",
    "malformed_norito = 2",
    "unsupported_algorithm = 3",
    "production_disabled = 4",
    "invalid_request = 5",
    "sanitized status metadata, not proof success",
)


class PrivacyGuardError(RuntimeError):
    pass


def read(relative):
    if relative in text_overrides:
        return text_overrides[relative]
    return (root / relative).read_text(encoding="utf-8")


def require(condition, message, errors):
    if not condition:
        errors.append(message)


def workflow_trigger_paths():
    paths = []
    in_paths = False
    for line in read(workflow_path).splitlines():
        if line.strip() == "paths:":
            in_paths = True
            continue
        if not in_paths:
            continue
        match = re.match(r'\s+-\s+"([^"]+)"\s*$', line)
        if match is not None:
            paths.append(match.group(1))
            continue
        if line and not line.startswith("      "):
            break
    return paths


def workflow_command_match(workflow, command):
    return re.search(rf"(?m)^\s+(?:run:\s+)?{re.escape(command)}\s*$", workflow)


def workflow_job_block(workflow, job_name):
    match = re.search(
        rf"(?ms)^  {re.escape(job_name)}:\s*\n(?P<block>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)",
        workflow,
    )
    return None if match is None else match.group("block")


def workflow_job_needs(workflow, job_name):
    block = workflow_job_block(workflow, job_name)
    if block is None:
        return set()
    match = re.search(r"(?m)^\s+needs:\s*(?P<needs>.+?)\s*$", block)
    if match is None:
        return set()
    needs = match.group("needs").strip()
    if needs.startswith("[") and needs.endswith("]"):
        return {item.strip() for item in needs[1:-1].split(",") if item.strip()}
    return {needs}


def check_readmes(errors):
    for relative, language_needles in readme_required.items():
        text = " ".join(read(relative).split())
        missing_needles = [
            needle for needle in (*common_readme_required, *language_needles) if needle not in text
        ]
        require(
            not missing_needles,
            (
                f"{relative} is missing privacy native bridge documentation tokens: "
                + ", ".join(missing_needles)
            ),
            errors,
        )


def check_zk_ace_proof_builder_coverage(errors):
    js_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    js_instruction_builder = read("javascript/iroha_js/src/instructionBuilders.js")
    js_dist_instruction_builder = read("javascript/iroha_js/dist/instructionBuilders.js")
    js_instruction_tests = read("javascript/iroha_js/test/instructionBuilders.test.js")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")
    python_crypto = read("python/iroha_python/src/iroha_python/crypto.py")
    python_tx = read("python/iroha_python/src/iroha_python/tx.py")
    python_client = read("python/iroha_python/src/iroha_python/client.py")
    python_package_root = read("python/iroha_python/src/iroha_python/__init__.py")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_client_tests = read("python/iroha_python/tests/client_ledger_helpers_test.py")
    python_readme = read("python/iroha_python/README.md")

    require(
        "function assertPythonZkAceProofBuilderCoverage()" in js_parity,
        "Privacy catalog parity tests must define the Python ZK-ACE proof-builder coverage guard",
        errors,
    )
    require(
        re.search(r"(?m)^\s*assertPythonZkAceProofBuilderCoverage\(\);\s*$", js_parity)
        is not None,
        "Privacy catalog parity tests must invoke the Python ZK-ACE proof-builder coverage guard",
        errors,
    )
    require(
        "function assertZkAceJsBuilderAmountCoverage()" in js_parity,
        "Privacy catalog parity tests must define the JS ZK-ACE positive-amount coverage guard",
        errors,
    )
    require(
        re.search(r"(?m)^\s*assertZkAceJsBuilderAmountCoverage\(\);\s*$", js_parity)
        is not None,
        "Privacy catalog parity tests must invoke the JS ZK-ACE positive-amount coverage guard",
        errors,
    )
    require(
        "function assertZkAcePythonTransactionAmountCoverage()" in js_parity,
        "Privacy catalog parity tests must define the Python ZK-ACE transaction amount guard",
        errors,
    )
    require(
        re.search(r"(?m)^\s*assertZkAcePythonTransactionAmountCoverage\(\);\s*$", js_parity)
        is not None,
        "Privacy catalog parity tests must invoke the Python ZK-ACE transaction amount guard",
        errors,
    )
    for snippet in (
        "Python privacy capabilities must require both ZK-ACE proof-builder names",
        "Python catalog-named ZK-ACE proof builder must delegate to the native-backed builder",
        "Python tests must cover ZK-ACE alias delegation, missing-native propagation, and malformed native prover payloads",
        "JS ZK-ACE positive-amount coverage missing",
        "must require positive ZK-ACE proof and transfer amounts",
        "Python transaction draft must require positive decimal u128 ZK-ACE transfer amounts",
        "Python client ZK-ACE helper must expose the strict positive-u128 amount contract",
        "privacy algorithm catalogs pin executable ZK-ACE proof-builder descriptor shape",
        "assertZkAceExecutableDescriptorShape",
        "assertZkAceCapabilitySurfaceFailClosed",
        "pythonDescriptorToJsShape",
        "python ZK-ACE descriptor must exist",
        "ZK-ACE capability descriptor must exist",
        "ZK-ACE capability must stay fail-closed through getPrivacyCapabilities",
        "ZK-ACE capability audit references must be frozen",
        "ZK-ACE descriptor must expose the concrete STARK/FRI SHA-256 Goldilocks verifier profile",
        "buildShieldedZkAceAuthorizedTransferInstruction",
        "planned shielded SDK entrypoints",
        "ZK-ACE production gate must not claim audit references before signoff",
        "ZK-ACE production gate must keep every required gate false",
        "ZK-ACE production gate must not inherit verifier-backend allowlist admission",
        "ZK-ACE production gate must stay fail-closed despite the STARK/FRI verifier profile allowlist",
    ):
        require(
            snippet in js_parity,
            f"Privacy catalog parity tests are missing ZK-ACE proof-builder assertion: {snippet}",
            errors,
        )
    for label, text in (
        ("JS source instruction builder", js_instruction_builder),
        ("JS dist instruction builder", js_dist_instruction_builder),
    ):
        for snippet in (
            "function asPositiveU128JsonNumber(value, name)",
            "const amount = asU128JsonNumber(value, name)",
            "amount <= 0",
            "must be greater than zero",
            "function normalizeZkAcePublicInputs",
            "asPositiveU128JsonNumber(source.amount, `${name}.amount`)",
            "function buildZkAceAuthorizedTransferInstruction",
            'asPositiveU128JsonNumber(source.amount, "zkAceAuthorizedTransfer.amount")',
        ):
            require(
                snippet in text,
                f"{label} is missing ZK-ACE positive-amount builder coverage: {snippet}",
                errors,
            )
    for snippet in (
        'descriptorTest("ZK-ACE builders reject malformed proof and replay inputs"',
        "ZK-ACE builders reject malformed proof and replay inputs",
        "must be greater than zero",
        "Number.MAX_SAFE_INTEGER + 1",
        "BigInt(Number.MAX_SAFE_INTEGER) + 1n",
        "Number.NaN",
        "{ toString: () => \"17\" }",
        "canonicalAmountTransfer.amount, 17",
        "buildZkAceAuthorizationProofV1({",
        "buildZkAceAuthorizedTransferInstruction({",
    ):
        require(
            snippet in js_instruction_tests,
            f"JS ZK-ACE instruction-builder adversarial amount coverage is missing {snippet}",
            errors,
        )
    for snippet in (
        "PositiveU128Like = Union[str, int]",
        "_U128_MAX = (1 << 128) - 1",
        "def _normalize_positive_u128_literal(quantity: Any, context: str) -> str:",
        "isinstance(quantity, bool)",
        "text.isdecimal()",
        "value <= 0 or value > _U128_MAX",
        "amount: PositiveU128Like",
        '_normalize_positive_u128_literal(amount, "amount")',
    ):
        require(
            snippet in python_tx,
            f"Python transaction builder is missing ZK-ACE positive amount boundary: {snippet}",
            errors,
        )
    require(
        re.search(
            r"def zk_ace_authorized_transfer_and_wait[\s\S]*amount: Union\[str, int\]",
            python_client,
        )
        is not None,
        "Python client ZK-ACE helper must expose the strict int/string amount contract",
        errors,
    )
    for snippet in (
        "test_zk_ace_transaction_amount_normalizer_matches_proof_builder_boundary",
        "_normalize_positive_u128_literal",
        '"00017"',
        "str((1 << 128) - 1)",
        'Decimal("1")',
        '"1e3"',
        "str(1 << 128)",
        'id="zk-ace-transfer-zero-amount"',
        "positive decimal u128",
    ):
        require(
            snippet in python_client_tests,
            f"Python ZK-ACE transaction amount adversarial coverage is missing {snippet}",
            errors,
        )
    require(
        re.search(
            r"(?m)^\s*assertZkAceCapabilitySurfaceFailClosed\(label, capabilities\);\s*$",
            js_parity,
        )
        is not None,
        "Privacy catalog parity tests must invoke the JS ZK-ACE capability fail-closed guard",
        errors,
    )
    require(
        re.search(
            r'zk_ace_prover\s*=\s*_callable_on_crypto\(\s*"build_zk_ace_authorization_proof_v1"\s*\)\s*and\s*_callable_on_crypto\("zk_ace_build_transfer_authorization_v1"\)',
            python_catalog,
        )
        is not None,
        "Python privacy capabilities must fail closed unless both ZK-ACE proof-builder names are callable",
        errors,
    )
    require(
        re.search(
            r"def\s+build_zk_ace_authorization_proof_v1\(\*\*kwargs:\s*Any\)\s*->\s*Dict\[str,\s*Any\]:[\s\S]*return\s+zk_ace_build_transfer_authorization_v1\(\*\*kwargs\)",
            python_crypto,
        )
        is not None,
        "Python catalog-named ZK-ACE proof builder must delegate to the native-backed alias",
        errors,
    )
    for label, text in (
        ("Python crypto exports", python_crypto),
        ("Python package root exports", python_package_root),
        ("Python README", python_readme),
    ):
        require(
            "build_zk_ace_authorization_proof_v1" in text
            and "zk_ace_build_transfer_authorization_v1" in text,
            f"{label} must keep both ZK-ACE proof-builder names",
            errors,
        )
    for snippet in (
        "def test_zk_ace_python_capabilities_require_both_proof_builder_names(",
        '"build_zk_ace_authorization_proof_v1"',
        '"zk_ace_build_transfer_authorization_v1"',
        'assert capabilities["zk_ace_authorization_proof_v1"] is False',
        'assert capabilities["zk_ace_sdk_exports_v1"] is False',
        "test_privacy_capabilities_reports_native_bridge_without_production_claims",
        "zk_ace_capability = next(",
        'assert zk_ace_capability["proof_family"] == "stark/fri/sha256-goldilocks"',
        'assert zk_ace_capability["backend_family"] == "stark-fri"',
        'assert zk_ace_capability["production_gate"]["audit_references"] == []',
        "test_privacy_catalog_enforces_execution_and_metadata_invariants",
        'assert zk_ace["proof_family"] == "stark/fri/sha256-goldilocks"',
        'assert zk_ace["backend_family"] == "stark-fri"',
        'assert zk_ace["production_gate"]["audit_references"] == []',
        'assert all(ready is False for ready in zk_ace["production_gate"]["gates"].values())',
        "Iroha production allowlist is not enabled for this audited row",
        "test_zk_ace_python_catalog_named_proof_builder_delegates",
        "test_zk_ace_python_catalog_named_proof_builder_propagates_native_errors",
        "test_zk_ace_python_transfer_authorization_rejects_non_object_native_payload",
    ):
        require(
            snippet in python_tests,
            f"Python ZK-ACE proof-builder adversarial coverage is missing {snippet}",
            errors,
        )


def check_public_privacy_required_production_plan_rows_coverage(errors):
    sources = (
        (
            "JS catalog parity tests",
            read("javascript/iroha_js/test/privacyCatalogParity.test.js"),
            r"const REQUIRED_PRIVACY_PLAN_ROWS = Object\.freeze\(\[([\s\S]*?)\]\);",
        ),
        (
            "JS source privacy catalog",
            read("javascript/iroha_js/src/privacyAlgorithms.js"),
            r"const REQUIRED_PRIVACY_PLAN_ROWS = Object\.freeze\(\[([\s\S]*?)\]\);",
        ),
        (
            "JS dist privacy catalog",
            read("javascript/iroha_js/dist/privacyAlgorithms.js"),
            r"const REQUIRED_PRIVACY_PLAN_ROWS = Object\.freeze\(\[([\s\S]*?)\]\);",
        ),
        (
            "Python privacy catalog",
            read("python/iroha_python/src/iroha_python/privacy_catalog.py"),
            r"REQUIRED_PRIVACY_PLAN_ROWS = \(([\s\S]*?)\n\)\nREQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID",
        ),
    )
    display_text_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID",
        ),
    )
    category_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID",
        ),
    )
    maturity_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID",
        ),
    )
    recommended_for_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID",
        ),
    )
    covered_criteria_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID",
        ),
    )
    proof_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID",
        ),
    )
    public_input_schema_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID",
        ),
    )
    verifier_key_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID",
        ),
    )
    state_token_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS",
        ),
    )
    failure_mode_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = Object\.freeze\(\[([\s\S]*?)\]\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = Object\.freeze\(\[([\s\S]*?)\]\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = Object\.freeze\(\[([\s\S]*?)\]\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\}\);\nconst REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_COMMON_FAILURE_MODE_TOKENS = \(([\s\S]*?)\n\)\nREQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
        ),
    )
    exact_failure_mode_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID",
        ),
    )
    security_note_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID",
        ),
    )
    source_reference_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
    )
    sdk_entrypoint_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
        ),
    )
    planned_sdk_entrypoint_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID",
        ),
    )
    pq_layer_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID",
        ),
    )
    chain_requirement_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
        ),
    )
    required_state_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID",
        ),
    )
    setup_step_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\nREQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID",
        ),
    )
    execution_step_sources = (
        (
            "JS catalog parity tests",
            sources[0][1],
            r"const REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\nconst BRIDGE_MISSING_REASON_SOURCES",
        ),
        (
            "JS source privacy catalog",
            sources[1][1],
            r"const REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\n\nfunction isPlainObject",
        ),
        (
            "JS dist privacy catalog",
            sources[2][1],
            r"const REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = Object\.freeze\(\{([\s\S]*?)\n\}\);\n\nfunction isPlainObject",
        ),
        (
            "Python privacy catalog",
            sources[3][1],
            r"REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID = \{([\s\S]*?)\n\}\n\n_RAW_PRIVACY_ALGORITHM_DESCRIPTORS_JSON",
        ),
    )
    expected_rows = (
        ("anonymous-pgc-k-out-of-n-v1", "sdk-builder", "anonymous-pgc"),
        ("verange-transparent-range-v1", "component", "verange"),
        ("zkat-policy-private-auth-v1", "sdk-builder", "zkat"),
        (
            "zk-ams-recursive-admission-v0",
            "sdk-builder",
            "recursive-anonymous-admission",
        ),
        (
            "vega-existing-credential-zk-v0",
            "sdk-builder",
            "vega-existing-credential-zk",
        ),
        (
            "silent-threshold-anoncred-v0",
            "sdk-builder",
            "silent-threshold-anoncred",
        ),
        ("zk-x509-onchain-identity-v0", "sdk-builder", "zk-x509"),
        ("jindo-lattice-pcs-zk-v0", "sdk-builder", "lattice-pcs-sis"),
        ("sis-hints-anoncred-pq-v0", "sdk-builder", "sis-with-hints"),
        ("zk-ace-pq-authorization-v0", "chain-executable", "stark-fri"),
        ("orchard-halo2-actions-v1", "research-target-as-of-2026-05", "halo2-ipa-orchard"),
        ("penumbra-masp-v1", "research-target-as-of-2026-05", "groth16-bls12-377"),
        (
            "monero-fcmp-plus-plus-v1",
            "research-target-as-of-2026-05",
            "fcmp-plus-plus-curve-tree",
        ),
        ("miden-stark-note-v1", "research-target-as-of-2026-05", "miden-stark"),
        (
            "aztec-private-rollup-v1",
            "research-target-as-of-2026-05",
            "aztec-plonkish-private-kernel",
        ),
        ("pq-masp-stark-v0", "research-target-as-of-2026-05", "pq-masp-stark-fri"),
    )
    expected_display_text = (
        ("anonymous-pgc-k-out-of-n-v1", "Anonymous PGC k-out-of-n payments v1", "Anonymous PGC", "Account-based anonymous confidential payment target with hidden sender, hidden amount, receiver privacy, and k-out-of-n receiver-set proofs."),
        ("verange-transparent-range-v1", "VeRange transparent range proofs v1", "VeRange", "Verification-efficient transparent range-proof component for confidential amounts, solvency proofs, and numeric credential predicates."),
        ("zkat-policy-private-auth-v1", "zkAt policy-private authorization v1", "zkAt policy auth", "Policy-private blockchain authenticator that hides threshold rules, signer sets, and account authorization logic."),
        ("zk-ams-recursive-admission-v0", "ZK-AMS recursive anonymous admission v0", "ZK-AMS admission", "Research target for recursively aggregated anonymous admission from real-world personhood or eligibility credentials into anonymous on-chain accounts."),
        ("vega-existing-credential-zk-v0", "Vega existing-credential ZK proofs v0", "Vega credentials", "Low-latency zero-knowledge proof target for proving predicates over existing credentials without revealing the full credential."),
        ("silent-threshold-anoncred-v0", "Silent threshold anonymous credentials v0", "Silent threshold cred", "Research target for threshold-issued anonymous credentials with silent setup, issuer hiding, constant-size showings, and dynamic verifier policies."),
        ("zk-x509-onchain-identity-v0", "ZK-X.509 on-chain identity v0", "ZK-X.509 identity", "ZK proof target for X.509 certificate validity, ownership, revocation status, and wallet-address binding."),
        ("jindo-lattice-pcs-zk-v0", "Jindo lattice polynomial commitment ZK v0", "Jindo lattice PCS", "2026 lattice-based polynomial commitment candidate for post-quantum zero-knowledge proof systems."),
        ("sis-hints-anoncred-pq-v0", "SIS-with-hints PQ anonymous credentials v0", "SIS hints anoncred", "PKC 2026 research foundation for lattice/SIS-with-hints anonymous credentials and post-quantum credential proofs."),
        ("zk-ace-pq-authorization-v0", "ZK-ACE post-quantum authorization v0", "ZK-ACE PQ auth", "STARK/FRI-backed source-account authorization for transparent asset transfers."),
        ("orchard-halo2-actions-v1", "Orchard-style Halo2 action bundle v1", "Orchard Halo2", "Zcash Orchard-style action bundle with note commitments, nullifiers, and one aggregated Halo2 proof over spend/output actions."),
        ("penumbra-masp-v1", "Penumbra-style multi-asset shielded pool v1", "Penumbra MASP", "Single multi-asset shielded pool using typed notes, note commitments, nullifiers, and spend/output proofs for private IBC-style assets."),
        ("monero-fcmp-plus-plus-v1", "Monero FCMP++ RingCT-style transfer v1", "FCMP++", "Full-chain membership proof target that replaces small decoy rings with a full-output-set spend proof while retaining hidden amounts and one-time receivers."),
        ("miden-stark-note-v1", "Miden-style STARK private note transaction v1", "Miden STARK", "Client-side STARK-proved account transition using private notes whose data stays off-chain while note hashes/nullifiers anchor correctness."),
        ("aztec-private-rollup-v1", "Aztec-style programmable private transaction v1", "Aztec private", "Programmable private-state transaction using client-side private execution, note hashes, nullifiers, encrypted logs, and recursive private-kernel proofs."),
        ("pq-masp-stark-v0", "Post-quantum MASP STARK v0", "PQ MASP v0", "Target end-to-end post-quantum MASP using STARK/FRI proofs, ML-DSA authorization, and ML-KEM note encryption."),
    )
    expected_categories = (
        ("anonymous-pgc-k-out-of-n-v1", "payment"),
        ("verange-transparent-range-v1", "proof_backend"),
        ("zkat-policy-private-auth-v1", "authorization"),
        ("zk-ams-recursive-admission-v0", "admission"),
        ("vega-existing-credential-zk-v0", "credential"),
        ("silent-threshold-anoncred-v0", "credential"),
        ("zk-x509-onchain-identity-v0", "identity"),
        ("jindo-lattice-pcs-zk-v0", "proof_backend"),
        ("sis-hints-anoncred-pq-v0", "credential"),
        ("zk-ace-pq-authorization-v0", "authorization"),
        ("orchard-halo2-actions-v1", "payment"),
        ("penumbra-masp-v1", "payment"),
        ("monero-fcmp-plus-plus-v1", "payment"),
        ("miden-stark-note-v1", "payment"),
        ("aztec-private-rollup-v1", "payment"),
        ("pq-masp-stark-v0", "payment"),
    )
    expected_maturities = (
        ("anonymous-pgc-k-out-of-n-v1", "accepted_conference"),
        ("verange-transparent-range-v1", "accepted_conference"),
        ("zkat-policy-private-auth-v1", "accepted_conference"),
        ("zk-ams-recursive-admission-v0", "arxiv_preprint"),
        ("vega-existing-credential-zk-v0", "technical_report"),
        ("silent-threshold-anoncred-v0", "technical_report"),
        ("zk-x509-onchain-identity-v0", "arxiv_preprint"),
        ("jindo-lattice-pcs-zk-v0", "technical_report"),
        ("sis-hints-anoncred-pq-v0", "accepted_conference"),
        ("zk-ace-pq-authorization-v0", "arxiv_preprint"),
        ("orchard-halo2-actions-v1", "specification"),
        ("penumbra-masp-v1", "specification"),
        ("monero-fcmp-plus-plus-v1", "specification"),
        ("miden-stark-note-v1", "specification"),
        ("aztec-private-rollup-v1", "specification"),
        ("pq-masp-stark-v0", "specification"),
    )
    expected_recommended_for = (
        ("anonymous-pgc-k-out-of-n-v1", "account-based private payments", "multi-receiver confidential transfers", "payment privacy without a note-based shielded pool UX"),
        ("verange-transparent-range-v1", "confidential amount range proofs", "reserve or solvency proofs", "numeric credential predicates"),
        ("zkat-policy-private-auth-v1", "institutional wallet policy privacy", "hidden threshold authorization", "authorization-policy migration without revealing signer topology"),
        ("zk-ams-recursive-admission-v0", "anonymous onboarding", "Sybil-resistant wallet issuance", "credential-gated CBDC pilots"),
        ("vega-existing-credential-zk-v0", "legacy credential bridges", "private eligibility checks", "attribute predicates for wallet enrollment"),
        ("silent-threshold-anoncred-v0", "multi-authority regulated credentials", "issuer-hiding eligibility proofs", "central-bank or supervisor issued wallet credentials"),
        ("zk-x509-onchain-identity-v0", "institutional wallet identity", "legal-entity account binding", "private PKI-based eligibility checks"),
        ("jindo-lattice-pcs-zk-v0", "post-quantum proof-system research", "future PQ verifier backend evaluation", "lattice PCS benchmarking"),
        ("sis-hints-anoncred-pq-v0", "post-quantum anonymous credential research", "future PQ KYC or eligibility proofs", "assumption tracking for lattice credential designs"),
        ("zk-ace-pq-authorization-v0", "post-quantum transaction authorization migration", "identity-private source-account authorization", "authorization envelopes for transparent asset transfers"),
        ("orchard-halo2-actions-v1", "single-asset private transfers", "mature note/nullifier wallet design", "compact client proofs without Groth16 ceremonies"),
        ("penumbra-masp-v1", "multi-asset shielded pools", "IBC-style asset privacy", "asset-id hiding with typed-value notes"),
        ("monero-fcmp-plus-plus-v1", "maximal sender anonymity sets", "decoy-ring replacement research", "account-independent UTXO spend privacy"),
        ("miden-stark-note-v1", "client-side proving", "private programmable note workflows", "parallel account-local transaction execution"),
        ("aztec-private-rollup-v1", "programmable private payments", "hybrid public/private contract workflows", "wallet-side private execution with encrypted note discovery"),
        ("pq-masp-stark-v0", "end-to-end post-quantum privacy target", "long-horizon central-bank pilot research", "strict PQ proof, authorization, and note-encryption experiments"),
    )
    expected_covered_criteria = (
        ("anonymous-pgc-k-out-of-n-v1", "hide_amount", "hide_sender", "hide_receiver"),
        ("verange-transparent-range-v1", "hide_amount"),
        ("zkat-policy-private-auth-v1",),
        ("zk-ams-recursive-admission-v0",),
        ("vega-existing-credential-zk-v0",),
        ("silent-threshold-anoncred-v0",),
        ("zk-x509-onchain-identity-v0",),
        ("jindo-lattice-pcs-zk-v0",),
        ("sis-hints-anoncred-pq-v0",),
        ("zk-ace-pq-authorization-v0",),
        ("orchard-halo2-actions-v1", "hide_amount", "hide_sender", "hide_receiver"),
        ("penumbra-masp-v1", "hide_amount", "hide_sender", "hide_receiver", "hide_asset_type"),
        ("monero-fcmp-plus-plus-v1", "hide_amount", "hide_sender", "hide_receiver"),
        ("miden-stark-note-v1", "hide_amount", "hide_receiver", "hide_asset_type"),
        ("aztec-private-rollup-v1", "hide_amount", "hide_sender", "hide_receiver"),
        ("pq-masp-stark-v0", "hide_amount", "hide_sender", "hide_receiver", "hide_asset_type", "post_quantum"),
    )
    expected_proof_families = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous-pgc-k-out-of-n"),
        ("verange-transparent-range-v1", "verange-transparent-range"),
        ("zkat-policy-private-auth-v1", "zkat-policy-private-authenticator"),
        ("zk-ams-recursive-admission-v0", "recursive-anonymous-admission"),
        ("vega-existing-credential-zk-v0", "existing-credential-zk"),
        ("silent-threshold-anoncred-v0", "threshold-anonymous-credentials"),
        ("zk-x509-onchain-identity-v0", "zkvm-x509-identity"),
        ("jindo-lattice-pcs-zk-v0", "lattice-polynomial-commitment"),
        ("sis-hints-anoncred-pq-v0", "lattice-anonymous-credentials"),
        ("zk-ace-pq-authorization-v0", "stark/fri/sha256-goldilocks"),
        ("orchard-halo2-actions-v1", "halo2-pasta-action-bundle"),
        ("penumbra-masp-v1", "groth16-bls12-377-decaf377"),
        ("monero-fcmp-plus-plus-v1", "fcmp-plus-plus-curve-trees-bulletproofs"),
        ("miden-stark-note-v1", "stark-vm-note-transaction"),
        ("aztec-private-rollup-v1", "plonkish-private-kernel-rollup"),
        ("pq-masp-stark-v0", "stark-fri"),
    )
    expected_public_input_schemas = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymity_set_root", "receiver_ciphertext_commitments", "domain_separator"),
        ("verange-transparent-range-v1", "commitments", "range_parameters", "payload_digest"),
        ("zkat-policy-private-auth-v1", "policy_commitment", "action_class", "policy_epoch"),
        ("zk-ams-recursive-admission-v0", "issuer_root", "admission_nullifiers", "recursive_admission_digest"),
        ("vega-existing-credential-zk-v0", "issuer_commitment", "predicate_commitment", "expiration_epoch"),
        ("silent-threshold-anoncred-v0", "issuer_set_commitment", "showing_nullifier", "verifier_policy_hash"),
        ("zk-x509-onchain-identity-v0", "ca_root_commitment", "revocation_root", "address_binding"),
        ("jindo-lattice-pcs-zk-v0", "commitment", "opening_claim", "parameter_hash"),
        ("sis-hints-anoncred-pq-v0", "issuer_commitment", "showing_policy_hash", "parameter_hash"),
        ("zk-ace-pq-authorization-v0", "identity_commitment", "replay_nullifier", "verifier_key_id"),
        ("orchard-halo2-actions-v1", "anchor", "nullifiers", "binding_signature"),
        ("penumbra-masp-v1", "state_commitment_anchor", "note_commitments", "asset_id_commitment"),
        ("monero-fcmp-plus-plus-v1", "membership_root", "key_image_or_link_tag", "chain_tag"),
        ("miden-stark-note-v1", "initial_account_commitment", "input_note_nullifiers", "reference_block"),
        ("aztec-private-rollup-v1", "note_hashes", "private_kernel_commitment", "rollup_state_roots"),
        ("pq-masp-stark-v0", "pool_id", "asset_set_root", "pq_policy_hash"),
    )
    expected_verifier_key_ids = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous_pgc_k_out_of_n_v1"),
        ("verange-transparent-range-v1", "verange_transparent_range_v1"),
        ("zkat-policy-private-auth-v1", "zkat_policy_private_auth_v1"),
        ("zk-ams-recursive-admission-v0", "zk_ams_recursive_admission_v0"),
        ("vega-existing-credential-zk-v0", "vega_existing_credential_zk_v0"),
        ("silent-threshold-anoncred-v0", "silent_threshold_anoncred_v0"),
        ("zk-x509-onchain-identity-v0", "zk_x509_onchain_identity_v0"),
        ("jindo-lattice-pcs-zk-v0", "jindo_lattice_pcs_zk_v0"),
        ("sis-hints-anoncred-pq-v0", "sis_hints_anoncred_pq_v0"),
        ("zk-ace-pq-authorization-v0", "zk_ace_pq_authorization_v0"),
        ("orchard-halo2-actions-v1", "orchard_halo2_action_bundle_v1"),
        ("penumbra-masp-v1", "penumbra_masp_v1"),
        ("monero-fcmp-plus-plus-v1", "monero_fcmp_plus_plus_v1"),
        ("miden-stark-note-v1", "miden_stark_note_v1"),
        ("aztec-private-rollup-v1", "aztec_private_kernel_v1"),
        ("pq-masp-stark-v0", "pq_masp_stark_v0"),
    )
    expected_state_tokens = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous account commitment", "spent link-tag"),
        ("verange-transparent-range-v1", "range-proof verifier parameters", "range commitment"),
        ("zkat-policy-private-auth-v1", "policy commitment registry", "authorization replay"),
        ("zk-ams-recursive-admission-v0", "issuer root registry", "admission nullifier set"),
        ("vega-existing-credential-zk-v0", "credential schema registry", "revocation or expiration policy"),
        ("silent-threshold-anoncred-v0", "threshold issuer registry", "credential showing nullifier policy"),
        ("zk-x509-onchain-identity-v0", "trusted ca root registry", "revocation root registry"),
        ("jindo-lattice-pcs-zk-v0", "lattice pcs parameter registry", "lattice pcs verifier key registry"),
        ("sis-hints-anoncred-pq-v0", "lattice credential parameter registry", "credential showing verifier"),
        ("zk-ace-pq-authorization-v0", "active identity commitment registry", "replay nullifier set"),
        ("orchard-halo2-actions-v1", "orchard note commitment tree", "orchard nullifier set"),
        ("penumbra-masp-v1", "multi-asset state commitment tree", "typed nullifier set"),
        ("monero-fcmp-plus-plus-v1", "full-output-set commitment accumulator", "spent link-tag set"),
        ("miden-stark-note-v1", "private note hash database", "input note nullifier set"),
        ("aztec-private-rollup-v1", "private note-hash tree", "nullifier tree"),
        ("pq-masp-stark-v0", "pq masp asset-set commitment root", "pq nullifier set"),
    )
    expected_common_failure_mode_tokens = (
        "malformed proof bytes",
        "wrong verifier key",
        "public input mismatch",
    )
    expected_failure_mode_tokens = (
        ("anonymous-pgc-k-out-of-n-v1", "stale or unknown anonymity-set root", "duplicate link tag"),
        ("verange-transparent-range-v1", "wrong bit length", "commitment substitution"),
        ("zkat-policy-private-auth-v1", "policy-root substitution", "authorization replay"),
        ("zk-ams-recursive-admission-v0", "duplicate credential admission", "wrong issuer root"),
        ("vega-existing-credential-zk-v0", "expired credential", "wallet-binding replay"),
        ("silent-threshold-anoncred-v0", "insufficient issuer threshold", "credential showing replay"),
        ("zk-x509-onchain-identity-v0", "expired certificate", "stale revocation root"),
        ("jindo-lattice-pcs-zk-v0", "parameter mismatch", "opening claim substitution"),
        ("sis-hints-anoncred-pq-v0", "wrong parameter set", "credential showing replay"),
        ("zk-ace-pq-authorization-v0", "transaction digest substitution", "replayed nullifier"),
        ("orchard-halo2-actions-v1", "stale anchor", "duplicate nullifier"),
        ("penumbra-masp-v1", "stale state commitment anchor", "duplicate nullifier"),
        ("monero-fcmp-plus-plus-v1", "stale membership root", "duplicate link tag"),
        ("miden-stark-note-v1", "stale reference block", "duplicate input note nullifier"),
        ("aztec-private-rollup-v1", "stale rollup state root", "duplicate nullifier"),
        ("pq-masp-stark-v0", "stale asset-set root", "duplicate pq nullifier"),
    )
    expected_failure_modes = (
        ("anonymous-pgc-k-out-of-n-v1", "stale or unknown anonymity-set root", "duplicate link tag", "receiver-set substitution", "range commitment mismatch", "authorization envelope mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("verange-transparent-range-v1", "wrong bit length", "commitment substitution", "verifier-parameter mismatch", "oversized aggregation", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("zkat-policy-private-auth-v1", "policy-root substitution", "stale policy epoch", "unauthorized signer witness", "transaction digest mismatch", "authorization replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("zk-ams-recursive-admission-v0", "duplicate credential admission", "wrong issuer root", "batch omission or account commitment substitution", "recursive proof parameter mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("vega-existing-credential-zk-v0", "expired credential", "wrong issuer", "predicate mismatch", "wallet-binding replay", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("silent-threshold-anoncred-v0", "insufficient issuer threshold", "issuer-set substitution", "credential showing replay", "verifier-policy mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("zk-x509-onchain-identity-v0", "expired certificate", "revoked certificate", "unknown CA root", "wrong wallet address binding", "stale revocation root", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("jindo-lattice-pcs-zk-v0", "parameter mismatch", "opening claim substitution", "unsupported query set", "backend misclassified as production-ready", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("sis-hints-anoncred-pq-v0", "wrong parameter set", "issuer parameter substitution", "credential showing replay", "overclaiming production readiness from assumption research", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("zk-ace-pq-authorization-v0", "transaction digest substitution", "chain-id or domain-separator mismatch", "replayed nullifier", "revoked identity commitment", "policy hash mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("orchard-halo2-actions-v1", "stale anchor", "duplicate nullifier", "invalid action-bundle proof", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("penumbra-masp-v1", "stale state commitment anchor", "duplicate nullifier", "asset balance commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("monero-fcmp-plus-plus-v1", "stale membership root", "duplicate link tag", "amount commitment mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("miden-stark-note-v1", "stale reference block", "duplicate input note nullifier", "account commitment transition mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("aztec-private-rollup-v1", "stale rollup state root", "duplicate nullifier", "private-kernel public input mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
        ("pq-masp-stark-v0", "stale asset-set root", "duplicate PQ nullifier", "ML-DSA or ML-KEM domain mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"),
    )
    expected_security_notes = (
        ("anonymous-pgc-k-out-of-n-v1", "Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("verange-transparent-range-v1", "This is a component, not a complete payment protocol.", "Range parameters must be bound to the transaction payload and verifier key.", "Aggregated proof limits must be enforced by validators.", "Local verification is limited to deterministic dev fixtures; the production VeRange prover remains unavailable.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("zkat-policy-private-auth-v1", "Hides authorization policy, not payment fields.", "Policy commitments require explicit epoch, replay, and rotation semantics.", "Combining with ZK-ACE requires both proofs to bind the same transaction digest.", "The SDK dev fixture verifies deterministic binding only; chain policy state and production zkAt proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("zk-ams-recursive-admission-v0", "Admission privacy is separate from later payment privacy.", "Duplicate admission prevention depends on issuer-scoped nullifiers.", "Recursive batching must bind every admitted account commitment.", "The SDK dev fixture verifies deterministic binding only; chain admission state and production recursive proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("vega-existing-credential-zk-v0", "Credential schema parsing must be deterministic and versioned.", "Proofs must bind to wallet or identity commitments to prevent credential replay.", "Issuer trust and revocation semantics remain external policy inputs.", "The SDK dev fixture verifies deterministic binding only; chain credential policy state and production Vega proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("silent-threshold-anoncred-v0", "Credential issuance and revocation governance are as important as proof verification.", "Issuer-set commitments need rotation and downgrade protections.", "This is a credential layer, not a private payment protocol.", "The SDK dev fixture verifies deterministic binding only; chain credential state and production silent-threshold proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("zk-x509-onchain-identity-v0", "Legacy X.509 trust roots are usually not post-quantum.", "Revocation root freshness must be explicit in the public inputs.", "Address binding must prevent proof replay across wallets and chains.", "The SDK dev fixture verifies deterministic public-input binding only; chain trust-root, revocation, policy state, and production ZK-X.509 proofs remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("jindo-lattice-pcs-zk-v0", "This is a proof backend candidate, not a transaction algorithm.", "PQ proof coverage alone does not imply PQ authorization or note encryption.", "Parameter selection and implementation security require independent review.", "The SDK dev fixture verifies deterministic public-input binding only; production Jindo lattice proving and verifier backends remain unavailable.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("sis-hints-anoncred-pq-v0", "This is a credential foundation, not an immediately deployable wallet protocol.", "PQ credential proof coverage does not make a payment flow end-to-end post-quantum.", "Parameter choices and reduction assumptions need explicit governance.", "The SDK dev fixture verifies deterministic public-input binding only; production SIS-with-hints credential proving and verifier backends remain unavailable.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("zk-ace-pq-authorization-v0", "Authorization is only one PQ layer; proof backend and note encryption must also be PQ before a payment flow is end-to-end post-quantum.", "Replay nullifiers must be chain-domain separated and irreversible after acceptance.", "A dev verifier must never be accepted under a production verifier key id.", "Native AIR openings are blinded so sampled rows do not recover identity or replay witness limbs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("orchard-halo2-actions-v1", "Orchard actions require circuit-compatible note/nullifier semantics and domain-separated action hashes.", "Viewing-key and outgoing-viewing metadata must remain wallet-local.", "Production readiness requires audited Halo2 parameters and note-encryption review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("penumbra-masp-v1", "Typed asset values must bind asset identifiers to balance commitments.", "Groth16 parameter registration must distinguish spend and output circuits.", "Wallet note plaintexts and position metadata must not be exposed through public APIs.", "Production MASP use requires audited parameter governance and chain-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("monero-fcmp-plus-plus-v1", "Full-chain membership roots must be canonical and replay protected.", "Link tags/key images must be unique without revealing owned outputs.", "Range-proof and amount-commitment parameters require production verifier review.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("miden-stark-note-v1", "Private note data and off-chain delivery metadata must stay wallet-local.", "Account-local transition proofs must bind initial and final account commitments.", "Reference blocks must prevent replay against stale account state.", "Production Miden note transactions require audited STARK parameters and account-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("aztec-private-rollup-v1", "Private-kernel proofs must bind note hashes, nullifiers, encrypted logs, and public calls.", "Encrypted log delivery metadata must not leak wallet note ownership.", "Recursive verifier registration must distinguish private-kernel versions and rollup state roots.", "Production private-rollup use requires audited private-kernel parameters and rollup-state integration review.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
        ("pq-masp-stark-v0", "PQ MASP combines experimental STARK/FRI proving with production PQ authorization and note encryption requirements.", "ML-DSA domains and ML-KEM ciphertext formats must be bound to verifier keys and pool identifiers.", "Post-quantum readiness still requires parameter review, parser fuzzing, and external audit.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Any chain roots, nullifiers, revocation data, or replay guards for this flow must persist across node restarts before admitting ledger mutations.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."),
    )
    expected_source_references = (
        ("anonymous-pgc-k-out-of-n-v1", "Anonymous PGC with k-out-of-n Proofs", "https://eprint.iacr.org/2025/884"),
        ("verange-transparent-range-v1", "VeRange: Verification-efficient Zero-knowledge Range Arguments", "https://eprint.iacr.org/2025/528"),
        ("zkat-policy-private-auth-v1", "zkAt: Zero-Knowledge Authenticator for Blockchain", "https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.AFT.2025.2"),
        ("zk-ams-recursive-admission-v0", "ZK-AMS recursive anonymous admission", "https://arxiv.org/abs/2602.16130"),
        ("vega-existing-credential-zk-v0", "Vega: Low-Latency Zero-Knowledge Proofs over Existing Credentials", "https://www.microsoft.com/en-us/research/publication/vega-low-latency-zero-knowledge-proofs-over-existing-credentials/"),
        ("silent-threshold-anoncred-v0", "Anonymous Credentials with Issuer-Hiding, Threshold Issuance, and Silent Setup", "https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-124.html"),
        ("zk-x509-onchain-identity-v0", "ZK-X.509 on-chain identity", "https://arxiv.org/abs/2603.25190"),
        ("jindo-lattice-pcs-zk-v0", "Jindo lattice-based polynomial commitment", "https://eprint.iacr.org.cn/2026/044"),
        ("sis-hints-anoncred-pq-v0", "Tight Reductions for SIS-with-Hints Assumptions with Applications", "https://kclpure.kcl.ac.uk/portal/en/publications/tight-reductions-for-sis-with-hints-assumptions-with-applications/"),
        ("zk-ace-pq-authorization-v0", "ZK-ACE: Practical Post-Quantum Authorization for Blockchain", "https://arxiv.org/abs/2603.07974"),
        ("orchard-halo2-actions-v1", "ZIP 224 Orchard Shielded Protocol", "https://zips.z.cash/zip-0224"),
        ("orchard-halo2-actions-v1", "Zcash Protocol Specification", "https://zips.z.cash/protocol/protocol.pdf"),
        ("penumbra-masp-v1", "Penumbra Multi-Asset Shielded Pool", "https://protocol.penumbra.zone/main/shielded_pool.html"),
        ("penumbra-masp-v1", "Penumbra Cryptographic Primitives", "https://protocol.penumbra.zone/main/crypto.html"),
        ("monero-fcmp-plus-plus-v1", "Monero FCMP++ Development", "https://web.getmonero.org/2024/04/27/fcmps.html"),
        ("miden-stark-note-v1", "Miden Transaction Model", "https://docs.miden.xyz/core-concepts/miden-base/transaction/"),
        ("miden-stark-note-v1", "Miden Notes", "https://docs.miden.xyz/core-concepts/miden-base/note/"),
        ("aztec-private-rollup-v1", "Aztec State Management", "https://docs.aztec.network/developers/docs/foundational-topics/state_management"),
        ("aztec-private-rollup-v1", "Aztec Private Kernel Circuit", "https://docs.aztec.network/developers/nightly/docs/foundational-topics/advanced/circuits/private_kernel"),
        ("pq-masp-stark-v0", "NIST Post-Quantum Standards", "https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards"),
        ("pq-masp-stark-v0", "FIPS 203 ML-KEM", "https://csrc.nist.gov/pubs/fips/203/final"),
        ("pq-masp-stark-v0", "FIPS 204 ML-DSA", "https://csrc.nist.gov/pubs/fips/204/final"),
        ("pq-masp-stark-v0", "FIPS 205 SLH-DSA", "https://csrc.nist.gov/pubs/fips/205/final"),
    )
    expected_sdk_entrypoints = (
        ("anonymous-pgc-k-out-of-n-v1", "buildAnonymousPgcReceiverSet", "buildAnonymousPgcDevProofFixture", "verifyAnonymousPgcDevProofLocally"),
        ("verange-transparent-range-v1", "buildRangeCommitment", "buildVeRangeDevProofFixture", "buildVeRangeProofEnvelope", "verifyVeRangeProofLocally"),
        ("zkat-policy-private-auth-v1", "buildZkAtPolicyCommitment", "buildZkAtAuthenticatorEnvelope", "buildZkAtDevProofFixture", "verifyZkAtAuthenticatorLocally"),
        ("zk-ams-recursive-admission-v0", "buildZkAmsAdmissionBatch", "buildZkAmsAdmissionProofEnvelope", "buildZkAmsAdmissionDevProofFixture", "verifyZkAmsAdmissionProofLocally"),
        ("vega-existing-credential-zk-v0", "buildVegaCredentialPredicateCommitment", "buildVegaCredentialProofEnvelope", "buildVegaCredentialDevProofFixture", "verifyVegaCredentialProofLocally"),
        ("silent-threshold-anoncred-v0", "buildSilentThresholdCredentialCommitments", "buildSilentThresholdCredentialEnvelope", "buildSilentThresholdCredentialDevProofFixture", "verifySilentThresholdCredentialProofLocally"),
        ("zk-x509-onchain-identity-v0", "buildZkX509IdentityCommitments", "buildZkX509IdentityEnvelope", "buildZkX509IdentityDevProofFixture", "verifyZkX509IdentityProofLocally"),
        ("jindo-lattice-pcs-zk-v0", "buildJindoLatticePublicInputs", "buildJindoLatticeProofEnvelope", "buildJindoLatticeDevProofFixture", "verifyJindoLatticeProofLocally"),
        ("sis-hints-anoncred-pq-v0", "buildSisHintsCredentialCommitments", "buildSisHintsCredentialEnvelope", "buildSisHintsCredentialDevProofFixture", "verifySisHintsCredentialProofLocally"),
        ("zk-ace-pq-authorization-v0", "buildRegisterZkAceIdentityCommitmentInstruction", "buildRotateZkAceIdentityCommitmentInstruction", "buildRevokeZkAceIdentityCommitmentInstruction", "buildZkAceAuthorizedTransferInstruction", "buildZkAceAuthorizationProofV1"),
        ("orchard-halo2-actions-v1",),
        ("penumbra-masp-v1",),
        ("monero-fcmp-plus-plus-v1",),
        ("miden-stark-note-v1",),
        ("aztec-private-rollup-v1",),
        ("pq-masp-stark-v0",),
    )
    expected_planned_sdk_entrypoints = (
        ("anonymous-pgc-k-out-of-n-v1", "buildAnonymousPgcAccountCommitmentInstruction", "buildAnonymousPgcKOutOfNProofV1", "buildAnonymousPgcTransferInstruction"),
        ("verange-transparent-range-v1", "buildVeRangeProofV1"),
        ("zkat-policy-private-auth-v1", "buildZkAtPolicyCommitmentInstruction", "buildZkAtPolicyProofV1", "buildZkAtAuthorizedTransaction"),
        ("zk-ams-recursive-admission-v0", "buildZkAmsAdmissionBatchProofV0", "buildSubmitZkAmsAdmissionBatchInstruction"),
        ("vega-existing-credential-zk-v0", "buildVegaCredentialPredicateProofV0", "buildSubmitVegaCredentialProofInstruction"),
        ("silent-threshold-anoncred-v0", "buildSilentThresholdCredentialShowingProofV0", "buildSubmitSilentThresholdCredentialProofInstruction"),
        ("zk-x509-onchain-identity-v0", "buildZkX509IdentityProofV0", "buildSubmitZkX509IdentityProofInstruction"),
        ("jindo-lattice-pcs-zk-v0", "buildJindoLatticeProofV0", "verifyJindoPolynomialCommitmentV0"),
        ("sis-hints-anoncred-pq-v0", "buildSisHintsAnonymousCredentialProofV0", "buildSubmitSisHintsCredentialProofInstruction"),
        ("zk-ace-pq-authorization-v0", "buildShieldedZkAceAuthorizationProofV1", "buildShieldedZkAceAuthorizedTransferInstruction"),
        ("orchard-halo2-actions-v1", "buildOrchardActionBundleProofV1", "buildOrchardActionBundleInstruction"),
        ("penumbra-masp-v1", "buildPenumbraSpendProofV1", "buildPenumbraOutputProofV1", "buildPenumbraShieldedPoolTransaction"),
        ("monero-fcmp-plus-plus-v1", "buildFcmpPlusPlusMembershipProofV1", "buildFcmpPlusPlusTransferInstruction"),
        ("miden-stark-note-v1", "buildMidenStarkTransactionProofV1", "buildMidenNoteTransactionInstruction"),
        ("aztec-private-rollup-v1", "buildAztecPrivateKernelProofV1", "buildAztecPrivateRollupTransactionInstruction"),
        ("pq-masp-stark-v0", "buildPqMaspStarkTransferProofV0", "buildPqMaspStarkRegisterPoolInstruction", "buildPqMaspStarkTransferInstruction", "generateMlDsaKeyPair", "encapsulateMlKem"),
    )
    expected_pq_layers = (
        ("anonymous-pgc-k-out-of-n-v1", False, False, False),
        ("verange-transparent-range-v1", False, False, False),
        ("zkat-policy-private-auth-v1", False, False, False),
        ("zk-ams-recursive-admission-v0", False, False, False),
        ("vega-existing-credential-zk-v0", False, False, False),
        ("silent-threshold-anoncred-v0", False, False, False),
        ("zk-x509-onchain-identity-v0", False, False, False),
        ("jindo-lattice-pcs-zk-v0", True, False, False),
        ("sis-hints-anoncred-pq-v0", True, False, False),
        ("zk-ace-pq-authorization-v0", True, True, False),
        ("orchard-halo2-actions-v1", False, False, False),
        ("penumbra-masp-v1", False, False, False),
        ("monero-fcmp-plus-plus-v1", False, False, False),
        ("miden-stark-note-v1", True, False, False),
        ("aztec-private-rollup-v1", False, False, False),
        ("pq-masp-stark-v0", True, True, True),
    )
    expected_chain_requirements = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous account commitment accumulator", "spent link-tag set", "Anonymous PGC verifier", "range-proof component verifier", "typed zk::RegisterAnonymousPgcAccountCommitment instruction", "typed zk::SubmitAnonymousPgcTransfer instruction"),
        ("verange-transparent-range-v1", "VeRange verifier registry entry", "range commitment binding rules", "dependent payment or credential verifier"),
        ("zkat-policy-private-auth-v1", "zkAt policy commitment registry", "zkAt verifier", "account policy epoch state", "account policy replay protection", "typed zk::RegisterZkAtPolicyCommitment instruction", "typed zk::SubmitZkAtAuthorizedTransaction admission"),
        ("zk-ams-recursive-admission-v0", "issuer root registry", "admission nullifier set", "recursive admission verifier", "typed ZK-AMS admission batch instruction"),
        ("vega-existing-credential-zk-v0", "credential schema registry", "issuer registry", "credential predicate verifier", "typed Vega credential proof instruction"),
        ("silent-threshold-anoncred-v0", "threshold issuer registry", "anonymous credential verifier", "credential showing replay policy", "typed silent-threshold credential proof instruction"),
        ("zk-x509-onchain-identity-v0", "trusted CA root registry", "revocation root registry", "ZK-X.509 verifier", "typed ZK-X.509 identity proof instruction"),
        ("jindo-lattice-pcs-zk-v0", "Jindo verifier backend", "lattice PCS parameter registry", "dependent circuit integration"),
        ("sis-hints-anoncred-pq-v0", "lattice anonymous credential verifier", "credential parameter registry", "issuer parameter registry", "typed SIS-with-hints credential proof instruction"),
        ("zk-ace-pq-authorization-v0", "zk::RegisterZkAceIdentityCommitment", "zk::RotateZkAceIdentityCommitment", "zk::RevokeZkAceIdentityCommitment", "zk::SubmitZkAceAuthorizedTransfer", "active stark/fri/sha256-goldilocks ZK-ACE verifier key", "ZK-ACE identity source-account allowlist"),
        ("orchard-halo2-actions-v1", "Orchard note commitment tree", "Orchard nullifier set", "Halo2 action-bundle verifier", "wallet Orchard witness store", "typed Orchard action-bundle instruction"),
        ("penumbra-masp-v1", "multi-asset state commitment tree", "typed note commitment and nullifier state", "Groth16 verifier registry", "wallet multi-asset witness store", "typed Penumbra shielded-pool transaction admission"),
        ("monero-fcmp-plus-plus-v1", "full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier", "wallet scanning and ownership recovery", "typed FCMP++ transfer instruction"),
        ("miden-stark-note-v1", "STARK VM verifier", "private note hash and nullifier database", "account commitment state", "wallet private-note delivery store", "typed Miden note transaction instruction"),
        ("aztec-private-rollup-v1", "private note-hash tree", "nullifier tree", "encrypted log store", "private-kernel verifier", "wallet private execution environment", "typed Aztec private-rollup transaction instruction"),
        ("pq-masp-stark-v0", "STARK/FRI verifier enabled", "ML-DSA transaction authorization", "ML-KEM note payload encryption", "zk::RegisterAssetHiddenZkPool", "zk::AssetHiddenZkTransfer", "active PQ MASP verifier key"),
    )
    expected_required_state = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous account commitment set", "recent anonymity-set roots", "spent link-tag set", "range-proof verifier parameters", "wallet account blinding and receiver recovery metadata"),
        ("verange-transparent-range-v1", "range-proof verifier parameters", "VeRange verifier key registry", "range commitment domain separators", "maximum aggregation policy"),
        ("zkat-policy-private-auth-v1", "policy commitment registry", "policy epoch state", "authorization replay guard", "authorization verifier registry", "wallet policy witness store"),
        ("zk-ams-recursive-admission-v0", "issuer root registry", "admission nullifier set", "anonymous account commitment registry", "recursive verifier parameters", "recursive admission verifier key registry", "wallet admission witness store"),
        ("vega-existing-credential-zk-v0", "credential issuer registry", "supported credential schema registry", "predicate registry", "revocation or expiration policy", "wallet credential predicate witness store", "credential predicate commitment registry", "credential predicate verifier key registry"),
        ("silent-threshold-anoncred-v0", "threshold issuer registry", "credential parameter registry", "verifier policy registry", "credential showing nullifier policy", "wallet credential showing witness store", "credential showing commitment registry", "anonymous credential verifier key registry"),
        ("zk-x509-onchain-identity-v0", "trusted CA root registry", "certificate policy registry", "revocation root registry", "identity proof verifier", "wallet certificate witness store", "certificate subject commitment registry", "ZK-X.509 verifier key registry"),
        ("jindo-lattice-pcs-zk-v0", "lattice PCS parameter registry", "backend verifier implementation", "lattice PCS verifier key registry", "benchmark fixtures"),
        ("sis-hints-anoncred-pq-v0", "lattice credential parameter registry", "issuer parameter registry", "credential showing verifier", "wallet lattice credential witness store", "lattice credential commitment registry", "lattice credential verifier key registry"),
        ("zk-ace-pq-authorization-v0", "active identity commitment registry", "replay nullifier set", "authorization verifier registry", "wallet identity witness and replay-secret store"),
        ("orchard-halo2-actions-v1", "Orchard note commitment tree", "Orchard nullifier set", "Orchard action-bundle verifier key registry", "wallet Orchard witness store"),
        ("penumbra-masp-v1", "multi-asset state commitment tree", "typed nullifier set", "Groth16 spend/output verifier key registry", "wallet asset metadata witness store"),
        ("monero-fcmp-plus-plus-v1", "full-output-set commitment accumulator", "spent link-tag set", "FCMP++ verifier key registry", "wallet output ownership scan state"),
        ("miden-stark-note-v1", "private note hash database", "input note nullifier set", "account commitment state", "STARK VM verifier key registry", "wallet private note witness store"),
        ("aztec-private-rollup-v1", "private note-hash tree", "nullifier tree", "encrypted log delivery store", "private-kernel verifier key registry", "wallet private execution witness store"),
        ("pq-masp-stark-v0", "PQ MASP asset-set commitment root", "PQ nullifier set", "ML-KEM encrypted note payload store", "wallet PQ note witness store"),
    )
    expected_setup_steps = (
        ("anonymous-pgc-k-out-of-n-v1", "Register anonymous account commitments and anonymity-set accumulator state.", "Register the k-out-of-n payment verifier key and range-proof parameters.", "Persist wallet blinding, balance-opening, and receiver recovery witnesses."),
        ("verange-transparent-range-v1", "Register VeRange verifier parameters and allowed bit lengths.", "Define the commitment scheme and domain separators used by dependent algorithms."),
        ("zkat-policy-private-auth-v1", "Register a hidden policy commitment and verifier key.", "Bind the policy to account action classes and epoch rules."),
        ("zk-ams-recursive-admission-v0", "Register credential issuer roots and recursive verifier parameters.", "Define anonymous account commitment format and admission-nullifier derivation."),
        ("vega-existing-credential-zk-v0", "Register supported credential schemas, issuers, and predicates.", "Bind credential proof subjects to wallet or ZK-ACE identity commitments."),
        ("silent-threshold-anoncred-v0", "Register issuer sets, threshold policies, and credential parameters.", "Define showing-nullifier and verifier-policy binding rules."),
        ("zk-x509-onchain-identity-v0", "Register trusted CA roots, certificate policies, and revocation-root feeds.", "Define wallet address binding and domain-separation rules."),
        ("jindo-lattice-pcs-zk-v0", "Track lattice PCS parameter sets and verifier API shape.", "Benchmark prover, verifier, and proof-size behavior before integration."),
        ("sis-hints-anoncred-pq-v0", "Track supported SIS-with-hints parameter sets and issuer parameters.", "Define how future PQ credential showings bind to wallet or authorization contexts."),
        ("zk-ace-pq-authorization-v0", "Register a ZK-ACE identity commitment, source-account allowlist, and verifier key.", "Initialize replay-state tracking for the authorizing wallet.", "Bind authorization policy hash to the allowed transaction action classes."),
        ("orchard-halo2-actions-v1", "Add Orchard-compatible note, nullifier, action, and anchor data model types.", "Register Orchard Halo2 verifier parameters and action-bundle public input layout.", "Persist wallet note plaintexts, diversifiers, Merkle witnesses, and outgoing viewing data."),
        ("penumbra-masp-v1", "Add typed-value notes, asset identifiers, state commitments, and nullifier state.", "Register Groth16/BLS12-377 verifier parameters for spend and output proofs.", "Persist wallet note plaintexts, asset metadata, state commitment positions, and nullifier keys."),
        ("monero-fcmp-plus-plus-v1", "Add output commitment accumulator state suitable for full-chain membership proofs.", "Define link tags/key images and spent-output rejection for Iroha assets.", "Implement wallet scanning, ownership recovery, and amount commitment witness storage."),
        ("miden-stark-note-v1", "Add private note hash/nullifier state and account-local transition verification.", "Register a STARK VM verifier and public-input commitment layout.", "Persist private note data and off-chain delivery metadata in the wallet note store."),
        ("aztec-private-rollup-v1", "Add private note-hash and nullifier trees plus encrypted log delivery metadata.", "Register a private-kernel verifier and public-input layout for private contract side effects.", "Persist wallet PXE-style note discovery, private call witnesses, and app-scoped nullifier keys."),
        ("pq-masp-stark-v0", "Register STARK/FRI verifier parameters and PQ MASP public input layout.", "Define ML-DSA authorization domains and ML-KEM note-encryption payload formats.", "Persist wallet PQ note witnesses, nullifier positions, and encapsulation metadata."),
    )
    expected_execution_steps = (
        ("anonymous-pgc-k-out-of-n-v1", "Select an anonymity-set root and receiver set.", "Create balance commitments, receiver ciphertext commitments, and link tag.", "Generate the Anonymous PGC proof and submit the transfer instruction."),
        ("verange-transparent-range-v1", "Build amount commitments.", "Generate a range proof bound to the transaction payload.", "Attach the range-proof envelope to the dependent confidential algorithm."),
        ("zkat-policy-private-auth-v1", "Generate a policy-private authenticator proof.", "Attach the authenticator envelope to the transaction authorization path."),
        ("zk-ams-recursive-admission-v0", "Collect admitted account commitments into a batch.", "Generate or import a recursive admission proof.", "Submit the batch proof and admission nullifiers."),
        ("vega-existing-credential-zk-v0", "Parse the credential under a registered schema.", "Generate a predicate proof and bind it to the wallet context.", "Submit the proof envelope to the admission or authorization flow."),
        ("silent-threshold-anoncred-v0", "Generate a credential showing proof under the verifier policy.", "Submit the proof as an admission or authorization component."),
        ("zk-x509-onchain-identity-v0", "Generate a proof of certificate validity, ownership, and revocation status.", "Bind the proof to an institution wallet or ZK-ACE identity commitment."),
        ("jindo-lattice-pcs-zk-v0", "Use as a candidate backend for future PQ circuits only after concrete circuit integration."),
        ("sis-hints-anoncred-pq-v0", "Use as a future PQ credential backend after a concrete credential protocol is selected."),
        ("zk-ace-pq-authorization-v0", "Hash the transaction payload and chain/domain context.", "Derive a fresh replay nullifier.", "Generate a ZK-ACE authorization proof and submit a protected transparent transfer."),
        ("orchard-halo2-actions-v1", "Select spend notes and anchors from the wallet witness store.", "Create output notes and value commitments.", "Generate one Halo2 proof over the action bundle and submit nullifiers plus commitments."),
        ("penumbra-masp-v1", "Select positioned notes and derive nullifiers.", "Create typed output notes and balance commitments.", "Submit spend/output actions with proofs against the shielded pool state commitment tree."),
        ("monero-fcmp-plus-plus-v1", "Select owned outputs from the wallet scan state.", "Generate full-chain membership and amount-conservation proofs.", "Submit link tag, output commitments, range proof, and spend authorization."),
        ("miden-stark-note-v1", "Execute the account-local transition against private note witnesses.", "Produce a STARK proof for the transaction script and account state delta.", "Submit note nullifiers, output note hashes, account commitments, and proof."),
        ("aztec-private-rollup-v1", "Execute private contract calls locally against wallet notes.", "Accumulate note hashes, nullifiers, encrypted logs, and public-call requests in the private kernel.", "Submit the recursive private-kernel proof and side-effect commitments for validator verification."),
        ("pq-masp-stark-v0", "Select PQ MASP input notes and derive nullifiers.", "Generate STARK/FRI transfer proofs with ML-DSA authorization and ML-KEM output-note encryption.", "Submit nullifiers, output commitments, PQ policy hash, and proof for verifier admission."),
    )

    source_texts = {label: text for label, text, _ in sources}
    for label, text, pattern in sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan row inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        if label == "Python privacy catalog":
            row_ids = re.findall(
                r'\(\s*"([^"]+)"\s*,\s*"[^"]+"\s*,\s*"[^"]+"\s*,?\s*\)',
                block,
            )
        else:
            row_ids = re.findall(
                r'Object\.freeze\(\[\s*"([^"]+)"\s*,\s*"[^"]+"\s*,\s*"[^"]+",?\s*\]\)',
                block,
            )
        require(
            row_ids == [algorithm_id for algorithm_id, _, _ in expected_rows],
            f"{label} must keep exact required production privacy plan row order and cardinality",
            errors,
        )
        if label == "Python privacy catalog":
            row_backend_pairs = re.findall(
                r'\(\s*"([^"]+)"\s*,\s*"[^"]+"\s*,\s*"([^"]+)"\s*,?\s*\)',
                block,
            )
        else:
            row_backend_pairs = re.findall(
                r'Object\.freeze\(\[\s*"([^"]+)"\s*,\s*"[^"]+"\s*,\s*"([^"]+)",?\s*\]\)',
                block,
            )
        production_allowlist_rows = [
            algorithm_id
            for algorithm_id, backend_family in row_backend_pairs
            if backend_family == "stark-fri"
        ]
        require(
            production_allowlist_rows == ["zk-ace-pq-authorization-v0"],
            f"{label} must keep stark-fri production allowlist backend scoped to ZK-ACE only",
            errors,
        )
        for algorithm_id, implementation_stage, backend_family in expected_rows:
            for snippet in (algorithm_id, implementation_stage, backend_family):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan row component {snippet}",
                    errors,
                )
            if label == "Python privacy catalog":
                row_pattern = (
                    r"\(\s*"
                    + re.escape(f'"{algorithm_id}"')
                    + r",\s*"
                    + re.escape(f'"{implementation_stage}"')
                    + r",\s*"
                    + re.escape(f'"{backend_family}"')
                    + r",?\s*\)"
                )
            else:
                row_pattern = (
                    r"Object\.freeze\(\[\s*"
                    + re.escape(f'"{algorithm_id}"')
                    + r",\s*"
                    + re.escape(f'"{implementation_stage}"')
                    + r",\s*"
                    + re.escape(f'"{backend_family}"')
                    + r",?\s*\]\)"
                )
            require(
                re.search(row_pattern, block) is not None,
                f"{label} must keep exact required production privacy plan row ({algorithm_id}, {implementation_stage}, {backend_family})",
                errors,
            )
    for label, text, pattern in display_text_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan display-text inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, name, short_name, summary in expected_display_text:
            display_literal = ", ".join(
                f'"{value}"' for value in (name, short_name, summary)
            )
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({display_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{display_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan display-text row {row_snippet}",
                errors,
            )
    for label, text, pattern in category_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan category inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, category in expected_categories:
            row_snippet = f'"{algorithm_id}": "{category}"'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan category row {row_snippet}",
                errors,
            )
    for label, text, pattern in maturity_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan maturity inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, maturity in expected_maturities:
            row_snippet = f'"{algorithm_id}": "{maturity}"'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan maturity row {row_snippet}",
                errors,
            )
    for label, text, pattern in recommended_for_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan recommendedFor inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *recommendations in expected_recommended_for:
            recommendations_literal = ", ".join(f'"{recommendation}"' for recommendation in recommendations)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({recommendations_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{recommendations_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan recommendedFor row {row_snippet}",
                errors,
            )
    for label, text, pattern in covered_criteria_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan covered-criteria inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *criteria in expected_covered_criteria:
            criteria_literal = ", ".join(f'"{criterion}"' for criterion in criteria)
            if label == "Python privacy catalog":
                if not criteria:
                    row_snippet = f'"{algorithm_id}": ()'
                elif len(criteria) == 1:
                    row_snippet = f'"{algorithm_id}": ("{criteria[0]}",)'
                else:
                    row_snippet = f'"{algorithm_id}": ({criteria_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{criteria_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan covered-criteria row {row_snippet}",
                errors,
            )
    for label, text, pattern in proof_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan proof-family inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, proof_family in expected_proof_families:
            for snippet in (algorithm_id, proof_family):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan proof-family component {snippet}",
                    errors,
                )
    for label, text, pattern in public_input_schema_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan public-input schema inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *schema_tokens in expected_public_input_schemas:
            for snippet in (algorithm_id, *schema_tokens):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan public-input schema component {snippet}",
                    errors,
                )
    for label, text, pattern in verifier_key_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan verifier-key inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, verifier_key_id in expected_verifier_key_ids:
            for snippet in (algorithm_id, verifier_key_id):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan verifier-key component {snippet}",
                    errors,
                )
    for label, text, pattern in state_token_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan state-token inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *state_tokens in expected_state_tokens:
            for snippet in (algorithm_id, *state_tokens):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan state-token component {snippet}",
                    errors,
                )
    for label, text, pattern in failure_mode_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan failure-mode inventory",
            errors,
        )
        common_block = block_match.group(1) if block_match else ""
        algorithm_block = block_match.group(2) if block_match else ""
        for snippet in expected_common_failure_mode_tokens:
            require(
                snippet in common_block,
                f"{label} must keep required production privacy plan common failure-mode component {snippet}",
                errors,
            )
        for algorithm_id, *failure_tokens in expected_failure_mode_tokens:
            for snippet in (algorithm_id, *failure_tokens):
                require(
                    snippet in algorithm_block,
                    f"{label} must keep required production privacy plan failure-mode component {snippet}",
                    errors,
                )
    for label, text, pattern in exact_failure_mode_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan exact failure-mode inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *failure_modes in expected_failure_modes:
            failure_modes_literal = ", ".join(f'"{failure_mode}"' for failure_mode in failure_modes)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({failure_modes_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{failure_modes_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan exact failure-mode row {row_snippet}",
                errors,
            )
    for label, text, pattern in security_note_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan security-note inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *security_notes in expected_security_notes:
            security_notes_literal = ", ".join(f'"{security_note}"' for security_note in security_notes)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({security_notes_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{security_notes_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan security-note row {row_snippet}",
                errors,
            )
    for label, text, pattern in source_reference_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan source-reference inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, reference_label, reference_url in expected_source_references:
            for snippet in (algorithm_id, reference_label, reference_url):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan source-reference component {snippet}",
                    errors,
                )
        expected_counts = {}
        for algorithm_id, _reference_label, _reference_url in expected_source_references:
            expected_counts[algorithm_id] = expected_counts.get(algorithm_id, 0) + 1
        for algorithm_id, expected_count in expected_counts.items():
            row_pattern = (
                rf'"{re.escape(algorithm_id)}": \(([\s\S]*?)\n    \),'
                if label == "Python privacy catalog"
                else rf'"{re.escape(algorithm_id)}": Object\.freeze\(\[([\s\S]*?)\n  \]\),'
            )
            row_match = re.search(row_pattern, block)
            row_block = row_match.group(1) if row_match else ""
            observed_count = (
                row_block.count("https://")
                if label == "Python privacy catalog"
                else row_block.count("url:")
            )
            require(
                row_match is not None and observed_count == expected_count,
                f"{label} must keep required production privacy plan exact source-reference count for {algorithm_id}",
                errors,
            )
    for label, text, pattern in sdk_entrypoint_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan SDK entrypoint inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *entrypoints in expected_sdk_entrypoints:
            entrypoints_literal = ", ".join(f'"{entrypoint}"' for entrypoint in entrypoints)
            if label == "Python privacy catalog":
                if not entrypoints:
                    row_snippet = f'"{algorithm_id}": ()'
                elif len(entrypoints) == 1:
                    row_snippet = f'"{algorithm_id}": ({entrypoints_literal},)'
                else:
                    row_snippet = f'"{algorithm_id}": ({entrypoints_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{entrypoints_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan SDK entrypoint row {row_snippet}",
                errors,
            )
    for label, text, pattern in planned_sdk_entrypoint_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan planned SDK entrypoint inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *entrypoints in expected_planned_sdk_entrypoints:
            for snippet in (algorithm_id, *entrypoints):
                require(
                    snippet in block,
                    f"{label} must keep required production privacy plan planned SDK entrypoint component {snippet}",
                    errors,
                )
    for label, text, pattern in pq_layer_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan PQ-layer inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, proof, authorization, note_encryption in expected_pq_layers:
            if label in ("JS source privacy catalog", "JS dist privacy catalog"):
                row_snippet = (
                    f'"{algorithm_id}": Object.freeze({{ proof: {str(proof).lower()}, '
                    f"authorization: {str(authorization).lower()}, "
                    f"noteEncryption: {str(note_encryption).lower()} }})"
                )
            elif label == "JS catalog parity tests":
                row_snippet = (
                    f'"{algorithm_id}": Object.freeze({{ proof: {str(proof).lower()}, '
                    f"authorization: {str(authorization).lower()}, "
                    f"note_encryption: {str(note_encryption).lower()} }})"
                )
            else:
                row_snippet = (
                    f'"{algorithm_id}": {{"proof": {proof}, "authorization": {authorization}, '
                    f'"note_encryption": {note_encryption}}}'
                )
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan PQ-layer row {row_snippet}",
                errors,
            )
    for label, text, pattern in chain_requirement_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan chain-requirement inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *requirements in expected_chain_requirements:
            requirements_literal = ", ".join(f'"{requirement}"' for requirement in requirements)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({requirements_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{requirements_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan chain-requirement row {row_snippet}",
                errors,
            )
    for label, text, pattern in required_state_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan required-state inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *state_items in expected_required_state:
            state_literal = ", ".join(f'"{state_item}"' for state_item in state_items)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({state_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{state_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan required-state row {row_snippet}",
                errors,
            )
    for label, text, pattern in setup_step_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan setup-step inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *setup_steps in expected_setup_steps:
            setup_literal = ", ".join(f'"{setup_step}"' for setup_step in setup_steps)
            if label == "Python privacy catalog":
                row_snippet = f'"{algorithm_id}": ({setup_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{setup_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan setup-step row {row_snippet}",
                errors,
            )
    for label, text, pattern in execution_step_sources:
        block_match = re.search(pattern, text)
        require(
            block_match is not None,
            f"{label} must keep public required production privacy plan execution-step inventory",
            errors,
        )
        block = block_match.group(1) if block_match else ""
        for algorithm_id, *execution_steps in expected_execution_steps:
            execution_literal = ", ".join(f'"{execution_step}"' for execution_step in execution_steps)
            if label == "Python privacy catalog":
                if len(execution_steps) == 1:
                    row_snippet = f'"{algorithm_id}": ({execution_literal},)'
                else:
                    row_snippet = f'"{algorithm_id}": ({execution_literal})'
            else:
                row_snippet = f'"{algorithm_id}": Object.freeze([{execution_literal}])'
            require(
                row_snippet in block,
                f"{label} must keep required production privacy plan execution-step row {row_snippet}",
                errors,
            )
    for label, text in (
        ("JS source privacy catalog", source_texts["JS source privacy catalog"]),
        ("JS dist privacy catalog", source_texts["JS dist privacy catalog"]),
        ("Python privacy catalog", source_texts["Python privacy catalog"]),
    ):
        for snippet in (
            "validateRequiredPrivacyPlanRows" if label.startswith("JS") else "_validate_required_privacy_plan_rows",
            "REQUIRED_PRIVACY_PLAN_DISPLAY_TEXT_BY_ALGORITHM_ID",
            "must keep display text",
            "REQUIRED_PRIVACY_PLAN_CATEGORY_BY_ALGORITHM_ID",
            "must keep category",
            "REQUIRED_PRIVACY_PLAN_MATURITY_BY_ALGORITHM_ID",
            "must keep maturity",
            "REQUIRED_PRIVACY_PLAN_RECOMMENDED_FOR_BY_ALGORITHM_ID",
            "must keep recommendedFor",
            "REQUIRED_PRIVACY_PLAN_COVERED_CRITERIA_BY_ALGORITHM_ID",
            "must keep covered criteria",
            "REQUIRED_PRIVACY_PLAN_PROOF_FAMILY_BY_ALGORITHM_ID",
            "must keep proof family",
            "REQUIRED_PRIVACY_PLAN_PUBLIC_INPUT_SCHEMA_BY_ALGORITHM_ID",
            "must keep public inputs schema",
            "REQUIRED_PRIVACY_PLAN_VERIFIER_KEY_ID_BY_ALGORITHM_ID",
            "must keep verifier key id",
            "REQUIRED_PRIVACY_PLAN_PQ_LAYERS_BY_ALGORITHM_ID",
            "must keep PQ layer",
            "REQUIRED_PRIVACY_PLAN_CHAIN_REQUIREMENTS_BY_ALGORITHM_ID",
            "must keep chain requirements",
            "REQUIRED_PRIVACY_PLAN_REQUIRED_STATE_BY_ALGORITHM_ID",
            "must keep required state",
            "REQUIRED_PRIVACY_PLAN_SETUP_STEPS_BY_ALGORITHM_ID",
            "must keep setup steps",
            "REQUIRED_PRIVACY_PLAN_EXECUTION_STEPS_BY_ALGORITHM_ID",
            "must keep execution steps",
            "REQUIRED_PRIVACY_PLAN_FAILURE_MODES_BY_ALGORITHM_ID",
            "must keep failure modes",
            "REQUIRED_PRIVACY_PLAN_SECURITY_NOTES_BY_ALGORITHM_ID",
            "must keep security notes",
            "REQUIRED_PRIVACY_PLAN_STATE_TOKENS_BY_ALGORITHM_ID",
            "must retain required state token"
            if label.startswith("JS")
            else "must retain required state",
            "REQUIRED_PRIVACY_PLAN_FAILURE_TOKENS_BY_ALGORITHM_ID",
            "must retain required failure-mode token"
            if label.startswith("JS")
            else "failure-mode token",
            "REQUIRED_PRIVACY_PLAN_SOURCE_REFERENCES_BY_ALGORITHM_ID",
            "must retain source reference",
            "must keep source references",
            "REQUIRED_PRIVACY_PLAN_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
            "must keep SDK entrypoints",
            "REQUIRED_PRIVACY_PLAN_PLANNED_SDK_ENTRYPOINTS_BY_ALGORITHM_ID",
            "must keep planned SDK entrypoints",
            "must retain a planned production proof builder until production gates pass"
            if label.startswith("JS")
            else "must retain a planned production proof",
        ):
            require(
                snippet in text,
                f"{label} must validate required production privacy plan rows with proof-builder coverage",
                errors,
            )
    for snippet in (
        "required production privacy plan display text drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_display_text_drift",
        "Account-based private payment pilot.",
        "required production privacy plan proof family drifted",
        "required production privacy plan category drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_category_drift",
        'descriptor["category"] = "authorization"',
        "required production privacy plan maturity drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_maturity_drift",
        'descriptor["maturity"] = "specification"',
        "required production privacy plan recommendedFor drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_recommended_for_drift",
        "claimed production rollout",
        "required production privacy plan covered criteria drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_covered_criteria_drift",
        'descriptor["covered_criteria"].append("hide_asset_type")',
        "test_privacy_catalog_rejects_required_production_privacy_plan_proof_family_drift",
        "forged-proof-family",
        "required production privacy plan public input schema drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_public_input_schema_drift",
        "forged_public_input",
        "required production privacy plan verifier-key id drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_verifier_key_drift",
        "forged_verifier_key",
        "required production privacy plan PQ layer drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_pq_layer_drift",
        'descriptor["pq_layers"]["proof"] = True',
        "required production privacy plan chain requirements drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_chain_requirement_drift",
        "typed zk::SubmitAnonymousPgcProofOnly instruction",
        "required production privacy plan required state drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_required_state_drift",
        "forged wallet recovery placeholder",
        "required production privacy plan setup steps drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_setup_step_drift",
        "Register forged Anonymous PGC verifier setup.",
        "required production privacy plan execution steps drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_execution_step_drift",
        "Submit forged Anonymous PGC proof-only envelope.",
        "required production privacy plan failure modes drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_failure_modes_drift",
        "accept forged replay tag",
        "required production privacy plan security notes drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_security_note_drift",
        "latency gates",
        "required production privacy plan state token drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_state_token_drift",
        "forged state placeholder",
        "required production privacy plan failure-mode token drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_failure_mode_drift",
        "forged failure placeholder",
        "required production privacy plan source reference drifted",
        "required production privacy plan source references drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_drift",
        "https://example.com/forged-source",
        "test_privacy_catalog_rejects_required_production_privacy_plan_source_reference_extra",
        "https://example.com/forged-extra-source",
        "required production privacy plan SDK entrypoints drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_sdk_entrypoint_drift",
        "buildForgedAnonymousPgcProductionProof",
        "required production privacy plan planned SDK entrypoints drifted",
        "test_privacy_catalog_rejects_required_production_privacy_plan_planned_sdk_entrypoint_drift",
        "buildForgedAnonymousPgcProofV1",
    ):
        require(
            snippet in source_texts["JS catalog parity tests"]
            or snippet in read("python/iroha_python/tests/privacy_catalog_test.py"),
            f"Privacy catalog tests must keep required production plan drift coverage for {snippet}",
            errors,
        )


def check_python_catalog_loader_bytecode_guards(errors):
    bytecode_env = 'env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" }'
    for relative in (
        "javascript/iroha_js/test/privacyCatalogParity.test.js",
        "javascript/iroha_js/test/privacyFfiContractParity.test.js",
    ):
        require(
            bytecode_env in read(relative),
            f"{relative} must suppress Python bytecode when loading the Python catalog",
            errors,
        )


def check_negative_control_inventory_parity_test(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    require(
        "function negativeControlModesFromInventory(text, startMarker, endMarker)" in ffi_parity,
        "Privacy FFI parity tests must keep the guard negative-control inventory parser",
        errors,
    )
    require(
        re.search(
            r'const\s+privacySdkGuardNegativeControlModes\s*=\s*negativeControlModesFromInventory\(\s*guard,\s*"negative_control_commands = \(",\s*"required_paths = \(",\s*\);',
            ffi_parity,
        )
        is not None,
        "Privacy FFI parity tests must derive SDK guard negative-control modes from the guard inventory",
        errors,
    )
    require(
        re.search(
            r'assertWorkflowRunsNegativeControlModes\(\s*workflow,\s*"ci/check_privacy_sdk_guard\.sh",\s*privacySdkGuardNegativeControlModes,\s*"Privacy SDK guard",\s*\);',
            ffi_parity,
        )
        is not None,
        "Privacy FFI parity tests must pass the dynamic SDK guard negative-control inventory to the workflow assertion",
        errors,
    )
    for snippet in (
        "for (const mode of privacySdkGuardNegativeControlModes)",
        'guard.includes(`if mode == "${mode}":`)',
        'new RegExp(`^\\\\s+ci/check_privacy_sdk_guard\\\\.sh ${mode}$`, "m")',
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests are missing dynamic SDK guard negative-control assertion: {snippet}",
            errors,
        )


def check_source_reference_obfuscated_ipv4_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    js_catalog = read("javascript/iroha_js/src/privacyAlgorithms.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")

    require(
        "function isNonGlobalIpv4Address(hostname)" in js_catalog,
        "JS privacy catalog must keep source-reference IPv4 private-range validation",
        errors,
    )
    require(
        "_parse_obfuscated_ipv4_source_reference_address" in python_catalog,
        "Python privacy catalog must keep obfuscated IPv4 source-reference validation",
        errors,
    )
    for url in (
        "https://2130706433/source",
        "https://0x7f000001/source",
        "https://017700000001/source",
        "https://127.1/source",
        "https://192.168.257/source",
    ):
        require(
            url in js_catalog_parity,
            f"JS privacy catalog parity tests must reject obfuscated source-reference IPv4 URL {url}",
            errors,
        )
    for url in (
        "https://2130706433/shape-source",
        "https://0x7f000001/shape-source",
        "https://017700000001/shape-source",
        "https://127.1/shape-source",
        "https://192.168.257/shape-source",
    ):
        require(
            url in python_tests,
            f"Python privacy catalog tests must reject obfuscated source-reference IPv4 URL {url}",
            errors,
        )


def check_source_reference_audit_readiness_url_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    js_catalog = read("javascript/iroha_js/src/privacyAlgorithms.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")

    require(
        "function sourceReferenceUrlClaimsAuditOrReadiness(value)" in js_catalog,
        "JS privacy catalog must keep source-reference audit/readiness URL claim detection",
        errors,
    )
    require(
        "def _source_reference_url_claims_audit_or_readiness(value: str) -> bool" in python_catalog,
        "Python privacy catalog must keep source-reference audit/readiness URL claim detection",
        errors,
    )
    for url in (
        "https://zips.z.cash/zip-0224#external-audit-complete",
        "https://zips.z.cash/zip-0224?production=ready",
        "https://zips.z.cash/zip-0224?evidence=audit%3Dcomplete",
        "https://zips.z.cash/zip-0224?evidence=production%253Dready",
        "https://zips.z.cash/zip-0224?evidence=mainnet%2520claim",
        "https://zips.z.cash/zip-0224#external-%2561udit-complete",
        "https://zips.z.cash/zip-0224?evidence=production%2525253Dready",
    ):
        require(
            url in js_catalog_parity,
            f"JS privacy catalog parity tests must reject audit/readiness source-reference URL {url}",
            errors,
        )
        require(
            url in python_tests,
            f"Python privacy catalog tests must reject audit/readiness source-reference URL {url}",
            errors,
        )


def check_source_reference_encoded_host_url_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    js_catalog = read("javascript/iroha_js/src/privacyAlgorithms.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")

    for snippet, message in (
        (
            "sourceReferenceUrlAuthority(value).includes(\"%\")",
            "JS privacy catalog must reject percent-encoded source-reference authorities",
        ),
        (
            "/%(?![0-9a-fA-F]{2})/.test(value)",
            "JS privacy catalog must reject malformed source-reference percent escapes",
        ),
        (
            "function sourceReferenceHostnameUsesIdna(hostname)",
            "JS privacy catalog must reject IDNA source-reference hostnames",
        ),
    ):
        require(snippet in js_catalog, message, errors)
    for snippet, message in (
        (
            "def _has_malformed_percent_escape(value: str) -> bool",
            "Python privacy catalog must reject malformed source-reference percent escapes",
        ),
        (
            "def _hostname_has_invalid_ipv4_literal_shape(hostname: str) -> bool",
            "Python privacy catalog must reject invalid source-reference IPv4 literal shapes",
        ),
        (
            "def _source_reference_hostname_uses_idna(hostname: str) -> bool",
            "Python privacy catalog must reject IDNA source-reference hostnames",
        ),
    ):
        require(snippet in python_catalog, message, errors)
    for url in (
        "https://127%2e0%2e0%2e1/source",
        "https://localhost%2elocaltest%2eme/source",
        "https://256.256.256.256/source",
        "https://zips.z.cash/zip-0224?section=notes%ZZappendix",
    ):
        require(
            url in js_catalog_parity,
            f"JS privacy catalog parity tests must reject malformed source-reference URL {url}",
            errors,
        )
        require(
            url in python_tests,
            f"Python privacy catalog tests must reject malformed source-reference URL {url}",
            errors,
        )


def check_dev_fixture_entrypoint_fail_closed_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    js_catalog = read("javascript/iroha_js/src/privacyAlgorithms.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_catalog = read("python/iroha_python/src/iroha_python/privacy_catalog.py")

    for snippet, message in (
        (
            "function entrypointIsDevFixture(entrypoint)",
            "JS privacy catalog must keep DevFixture entrypoint classification",
        ),
        (
            "function entrypointIsExplicitDevFixture(entrypoint)",
            "JS privacy catalog must keep explicit DevFixture entrypoint classification",
        ),
        (
            "function entrypointIsLocalVerifier(entrypoint)",
            "JS privacy catalog must keep local-verifier entrypoint classification",
        ),
        (
            "function hasDevFixtureNonProductionWarning(notes)",
            "JS privacy catalog must require DevFixture non-production warnings",
        ),
    ):
        require(snippet in js_catalog, message, errors)
    for snippet, message in (
        (
            "def _entrypoint_is_dev_fixture(entrypoint: str) -> bool",
            "Python privacy catalog must keep DevFixture entrypoint classification",
        ),
        (
            "def _entrypoint_is_explicit_dev_fixture(entrypoint: str) -> bool",
            "Python privacy catalog must keep explicit DevFixture entrypoint classification",
        ),
        (
            "def _entrypoint_is_local_verifier(entrypoint: str) -> bool",
            "Python privacy catalog must keep local-verifier entrypoint classification",
        ),
        (
            "def _has_dev_fixture_non_production_warning(notes: list[str]) -> bool",
            "Python privacy catalog must require DevFixture non-production warnings",
        ),
    ):
        require(snippet in python_catalog, message, errors)
    for snippet in (
        "buildShapeDev.Proof.Fixture",
        "verifyShapeProofLocalVerifier",
        "The SDK dev fixture is deterministic only; production Shape proofs remain unavailable.",
        "buildShapeProductionProof",
    ):
        require(
            snippet in js_catalog_parity,
            f"JS privacy catalog parity tests must keep DevFixture/local-verifier fail-closed coverage for {snippet}",
            errors,
        )
        require(
            snippet in python_tests,
            f"Python privacy catalog tests must keep DevFixture/local-verifier fail-closed coverage for {snippet}",
            errors,
        )


def check_privacy_catalog_defensive_copy_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")

    for snippet in (
        'assertPythonCatalogDefensiveCopyCoverage();',
        'test("privacy algorithm JS getters return immutable fail-closed production metadata"',
        "Object.isFrozen(frozenDescriptor.productionGate)",
        "Object.isFrozen(frozenDescriptor.productionGate.gates)",
        "Object.isFrozen(frozenDescriptor.productionGate.missing)",
        "Object.isFrozen(frozenDescriptor.productionGate.auditReferences)",
        "frozenDescriptor.productionGate.auditReferences.push({",
        "frozenDescriptor.plannedSdkEntrypoints.length = 0;",
        "frozenDescriptor.sourceReferences.push({",
        "capabilities.privacyAlgorithms.length = 0;",
        "fresh.productionGate.gates.external_audit",
    ):
        require(
            snippet in js_catalog_parity,
            f"JS privacy catalog parity tests must keep immutable fail-closed metadata coverage for {snippet}",
            errors,
        )

    for snippet in (
        "def test_privacy_catalog_returns_defensive_copies() -> None:",
        "def test_privacy_capabilities_returns_defensive_copies() -> None:",
        'descriptors[0]["production_ready"] = True',
        'descriptors[0]["production_gate"]["ready"] = True',
        'descriptors[0]["production_gate"]["gates"]["external_audit"] = True',
        'descriptors[0]["production_gate"]["missing"].clear()',
        'descriptors[0]["production_gate"]["audit_references"].append(',
        'planned["planned_sdk_entrypoints"].clear()',
        'source_descriptor["source_references"][0]["url"] = "https://audit.example/forged"',
        'source_descriptor["source_references"].append(',
        'capabilities["privacy_algorithms"][0]["production_gate"]',
        'fresh["privacy_algorithms"][0]["production_gate"]',
    ):
        require(
            snippet in python_tests,
            f"Python privacy catalog tests must keep defensive-copy fail-closed metadata coverage for {snippet}",
            errors,
        )


def check_planned_privacy_entrypoint_quarantine_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    python_tests = read("python/iroha_python/tests/privacy_catalog_test.py")

    for snippet in (
        'test("planned privacy SDK entrypoints remain unexported until production gates pass"',
        "publicApiNameVariants(entrypoint)",
        "sourceCapabilityKeys.has(capabilityKey)",
        "distCapabilityKeys.has(capabilityKey)",
        'assertExecutableEntrypointsExported("JS src package"',
        "assertExecutableEntrypointsDeclared(",
        "PUBLIC_PRIVACY_API_DECLARATION_SURFACES",
        "publicPrivacyApiSourceTexts()",
        'descriptor.productionGate.missing.includes("planned SDK entrypoints remain")',
    ):
        require(
            snippet in js_catalog_parity,
            f"JS privacy catalog parity tests must keep planned-entrypoint quarantine coverage for {snippet}",
            errors,
        )

    for snippet in (
        "def test_planned_privacy_sdk_entrypoints_remain_unexported_and_fail_closed() -> None:",
        "def test_planned_privacy_sdk_entrypoints_have_no_public_python_definitions() -> None:",
        "def test_privacy_capabilities_do_not_advertise_planned_production_entrypoints() -> None:",
        "EXPECTED_PRIVACY_CAPABILITY_KEYS = frozenset(",
        "planned_name_variants.isdisjoint(package_exports)",
        "planned_name_variants.isdisjoint(crypto_exports)",
        "assert not hasattr(module, entrypoint)",
        'for source_path in sorted(source_root.rglob("*.py")):',
        "forbidden_status_keys = {",
        '"asset_hidden_transfer_proof_v1"',
        '"ml_kem_note_encryption"',
        'assert capabilities.get(key, False) is False',
        "assert set(capabilities) == EXPECTED_PRIVACY_CAPABILITY_KEYS",
    ):
        require(
            snippet in python_tests,
            f"Python privacy catalog tests must keep planned-entrypoint quarantine coverage for {snippet}",
            errors,
        )
    require(
        re.search(
            r"forbidden_status_keys\s*=\s*\{[\s\S]*\"asset_hidden_transfer_proof_v1\"[\s\S]*\"shielded_zk_ace_authorized_transfer_instruction\"[\s\S]*\"anonymous_pgc_k_out_of_n_proof_v1\"[\s\S]*\"pq_masp_stark_transfer_proof_v0\"[\s\S]*\"ml_kem_note_encryption\"",
            python_tests,
        )
        is not None,
        "Python privacy catalog tests must keep broad planned-production capability denylist coverage",
        errors,
    )


def check_native_privacy_catalog_parity_coverage(errors):
    js_catalog_parity = read("javascript/iroha_js/test/privacyCatalogParity.test.js")
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "const RUST_PRIVACY_ALGORITHM_SOURCES = Object.freeze([",
        'path: "crates/connect_norito_bridge/src/lib.rs"',
        'path: "crates/iroha_js_host/src/lib.rs"',
        'path: "python/iroha_python/iroha_python_rs/src/lib.rs"',
        "function extractRustPrivacyAlgorithmEntries(text, label)",
        'assert.ok(entries.length > 0, `${label} native privacy capability catalog is empty`)',
        '`${label} native privacy capability catalog has duplicate algorithm ids`',
        "function assertRustNativeProductionGateParity(pythonCatalog)",
        "function assertRustNativeCatalogParity(pythonCatalog)",
        "const actual = extractRustPrivacyAlgorithmEntries(fileText(source.path), source.label)",
        "assert.deepEqual(",
        "`${source.label} native privacy capability catalog drifted from SDK catalog`",
        "assertRustNativeProductionGateParity(pythonCatalog);",
        "assertRustNativeCatalogParity(pythonCatalog);",
        '"production_gate: privacy_production_gate()"',
    ):
        require(
            snippet in js_catalog_parity,
            f"JS privacy catalog parity tests must keep native catalog parity coverage for {snippet}",
            errors,
        )

    for snippet in (
        "privacy_production_gate_key_is_required",
        "privacy_production_gate_missing_reason_is_required",
        "privacy_production_gate_invariants_hold",
        "privacy_capability_invariants_hold",
        "!capability\\.production_ready",
        "privacy_production_gate_invariants_hold",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native production-gate invariant coverage for {snippet}",
            errors,
        )


def check_native_privacy_executable_entrypoint_allowlist_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep planned entrypoints non-executable"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_entrypoint_supported",
        "sdk_entrypoints",
        "executable entrypoint allowlist must use SDK entrypoints",
        "executable entrypoint allowlist must not include planned entrypoints",
        "planned_entrypoints",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native executable-entrypoint allowlist coverage for {snippet}",
            errors,
        )


def check_native_privacy_planned_entrypoint_rejection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep planned entrypoints non-executable"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_entrypoint_planned",
        "planned_entrypoints",
        "privacy_entrypoint_planned\\(entry,\\s*&request\\.entrypoint\\)",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
        "planned but not executable",
        "privacy_build_proof_rejects_planned_entrypoint_before_request_validation",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native planned-entrypoint rejection coverage for {snippet}",
            errors,
        )


def check_native_privacy_catalog_identifier_structure_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalogs keep algorithm rows unique and portable"',
        'test("native privacy FFI catalogs keep dev fixtures explicit and non-production"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_proof_family_is_portable",
        "privacy_vk_ref_backend_family_is_portable",
        "privacy_vk_ref_name_is_portable",
        "privacy_algorithm_id_is_portable",
        "privacy_sdk_entrypoint_is_portable",
        "privacy_algorithm_catalog_entries_are_valid",
        "privacy_algorithm_catalog_rejects_adversarial_duplicates_and_unportable_labels",
        "privacy_entrypoint_is_dev_fixture",
        "privacy_entrypoint_is_explicit_dev_fixture",
        "privacy_entrypoint_is_local_verifier",
        "privacy_entrypoint_is_proof_helper",
        "privacy_entrypoint_is_production_proof_builder",
        "privacy_algorithm_catalog_rejects_adversarial_fixture_and_local_verifier_entrypoints",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native catalog identifier/fixture structure coverage for {snippet}",
            errors,
        )


def check_native_privacy_required_production_plan_rows_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    expected_rows = (
        ("anonymous-pgc-k-out-of-n-v1", "anonymous-pgc-k-out-of-n", "anonymous-pgc"),
        ("verange-transparent-range-v1", "verange-transparent-range", "verange"),
        ("zkat-policy-private-auth-v1", "zkat-policy-private-authenticator", "zkat"),
        (
            "zk-ams-recursive-admission-v0",
            "recursive-anonymous-admission",
            "recursive-anonymous-admission",
        ),
        (
            "vega-existing-credential-zk-v0",
            "existing-credential-zk",
            "vega-existing-credential-zk",
        ),
        (
            "silent-threshold-anoncred-v0",
            "threshold-anonymous-credentials",
            "silent-threshold-anoncred",
        ),
        ("zk-x509-onchain-identity-v0", "zkvm-x509-identity", "zk-x509"),
        ("jindo-lattice-pcs-zk-v0", "lattice-polynomial-commitment", "lattice-pcs-sis"),
        ("sis-hints-anoncred-pq-v0", "lattice-anonymous-credentials", "sis-with-hints"),
        ("zk-ace-pq-authorization-v0", "stark/fri/sha256-goldilocks", "stark-fri"),
        ("orchard-halo2-actions-v1", "halo2-pasta-action-bundle", "halo2-ipa-orchard"),
        ("penumbra-masp-v1", "groth16-bls12-377-decaf377", "groth16-bls12-377"),
        (
            "monero-fcmp-plus-plus-v1",
            "fcmp-plus-plus-curve-trees-bulletproofs",
            "fcmp-plus-plus-curve-tree",
        ),
        ("miden-stark-note-v1", "stark-vm-note-transaction", "miden-stark"),
        (
            "aztec-private-rollup-v1",
            "plonkish-private-kernel-rollup",
            "aztec-plonkish-private-kernel",
        ),
        ("pq-masp-stark-v0", "stark-fri", "pq-masp-stark-fri"),
    )

    for snippet in (
        'test("native privacy FFI catalogs pin required production plan rows"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "EXPECTED_NATIVE_PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS",
        "EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_RUST_BACKEND_LABELS",
        "extractNativeRequiredProductionPlanRows",
        "extractPublicRequiredPrivacyPlanRows",
        "publicRequiredPrivacyPlanNativeRows",
        "native required production plan rows must match public required privacy plan rows",
        "native required production-allowlisted rows must stay scoped to public ZK-ACE allowlist rows",
        "must use a concrete Rust verifier profile",
        "PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS",
        "privacy_required_production_plan_rows_are_present",
        "privacy_algorithm_catalog_rejects_missing_or_misregistered_required_plan_rows",
        "matching_rows\\.next\\(\\)",
        "duplicate required production plan rows must be rejected",
        "deriveOrchardWitness",
        "wrong-backend",
        "wrong-proof",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native required production plan rows coverage for {snippet}",
            errors,
        )
    expected_block_match = re.search(
        r"const EXPECTED_NATIVE_PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS = Object\.freeze\(\[([\s\S]*?)\]\);",
        ffi_parity,
    )
    require(
        expected_block_match is not None,
        "Privacy FFI parity tests must keep exact expected native required production plan rows",
        errors,
    )
    expected_block = expected_block_match.group(1) if expected_block_match else ""
    expected_native_rows = re.findall(
        r'Object\.freeze\(\[\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,\s*"([^"]+)",?\s*\]\)',
        expected_block,
    )
    require(
        expected_native_rows == list(expected_rows),
        "Privacy FFI parity tests must keep exact native required production plan tuple order and cardinality",
        errors,
    )
    native_production_allowlist_rows = [
        (algorithm_id, proof_family, backend_family)
        for algorithm_id, proof_family, backend_family in expected_native_rows
        if backend_family == "stark-fri"
    ]
    require(
        native_production_allowlist_rows
        == [("zk-ace-pq-authorization-v0", "stark/fri/sha256-goldilocks", "stark-fri")],
        "Privacy FFI parity tests must keep native production allowlist row scoped to ZK-ACE sha256-goldilocks",
        errors,
    )
    for algorithm_id, proof_family, backend_family in expected_rows:
        for snippet in (algorithm_id, proof_family, backend_family):
            require(
                snippet in expected_block,
                f"Privacy FFI parity tests must keep required production plan row component {snippet}",
                errors,
            )


def check_native_privacy_verifier_key_registration_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalogs require explicit verifier-key name maps"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_catalog_vk_ref_name_is_registered",
        "privacy_algorithm_catalog_vk_ref_names_have_duplicates",
        "unmapped-mainnet-privacy-row-v1",
        "unmapped verifier-key names must fail catalog admission",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native verifier-key registration coverage for {snippet}",
            errors,
        )


def check_native_privacy_public_catalog_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalog rows match public SDK catalogs"',
        'test("native privacy FFI verifier-key maps match public SDK catalogs"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "publicPrivacyCatalogNativeRows",
        "publicProofedVerifierKeyEntries",
        "extractNativePrivacyCatalogRows",
        "extractNativePrivacyVerifierKeyNameMap",
        "native privacy catalog rows must match public SDK row ids",
        "native verifier-key map must match public SDK verifierKeyId values",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native public catalog parity coverage for {snippet}",
            errors,
        )


def check_native_privacy_component_rows_proof_only_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalogs keep component rows proof-only"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PRIVACY_COMPONENT_ALGORITHM_IDS",
        "verange-transparent-range-v1",
        "privacy_entrypoint_is_ledger_mutation",
        "privacy_algorithm_entry_is_component",
        "privacy_algorithm_catalog_rejects_component_ledger_mutation_entrypoints",
        "buildVeRangeInstruction",
        "buildVeRangeTransaction",
        "buildSubmitVeRangeProof",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native component proof-only coverage for {snippet}",
            errors,
        )


def check_native_privacy_planned_ledger_mutation_proof_builder_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalogs pair planned ledger mutations with production proof builders"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_entrypoints_include_production_proof_builder",
        "privacy_entrypoint_is_production_proof_builder",
        "privacy_entrypoint_is_instruction_builder",
        "has_planned_ledger_mutation",
        "has_production_proof_builder",
        "privacy_algorithm_catalog_rejects_planned_ledger_mutation_without_production_proof_builder",
        "buildShapeTransferInstruction",
        "buildShapeAuthorizedTransaction",
        "buildSubmitShapeProof",
        "buildShapeProofV1",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native planned ledger-mutation proof-builder coverage for {snippet}",
            errors,
        )


def check_native_privacy_proofed_sdk_ledger_mutation_pairing_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI catalogs keep proofed SDK ledger mutations typed and proof-paired"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_entrypoint_is_generic_ledger_mutation",
        "privacy_entrypoint_is_untyped_ledger_mutation",
        "privacy_algorithm_entry_is_proofed_privacy",
        "proofed_privacy_row",
        "has_generic_ledger_mutation",
        "has_untyped_ledger_mutation",
        "privacy_algorithm_catalog_rejects_unpaired_or_generic_sdk_ledger_mutations",
        "transparent-transfer",
        "confidential-transfer-v2",
        "SDK_TYPED_INSTRUCTION_WITH_PROOF",
        "Iroha\\.Privacy\\.submitSignedTransaction",
        "proofed sdk instruction without proof",
        "proofed SDK ledger mutations typed and proof-paired",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native proofed SDK ledger-mutation pairing coverage for {snippet}",
            errors,
        )


def check_native_privacy_production_gate_state_fail_closed_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    native_sources = (
        ("C bridge privacy FFI", read("crates/connect_norito_bridge/src/lib.rs")),
        ("JS NAPI privacy FFI", read("crates/iroha_js_host/src/lib.rs")),
        ("Python PyO3 privacy FFI", read("python/iroha_python/iroha_python_rs/src/lib.rs")),
    )

    for snippet in (
        'test("native privacy FFI capabilities keep production gates fail-closed"',
        "PRIVACY_PRODUCTION_GATE_MISSING_ENGINE",
        "real protocol engine is not production-enabled",
        "PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST",
        "Iroha production allowlist is not enabled for this audited row",
        "privacy_production_gate_key_is_required",
        "privacy_production_gate_missing_reason_is_required",
        "privacy_gate_statuses_match_requirements",
        "privacy_gate_missing_reasons_match_requirements",
        "privacy_production_gate_invariants_hold",
        "!gate\\.ready",
        "audit_references\\.is_empty\\(\\)",
        "!status\\.passed",
        "ZK-ACE native capability must be advertised",
        "ZK-ACE native capability must not become production-ready only because its verifier backend is allowlisted",
        "must pin ZK-ACE native capabilities to concrete profile while fail-closed",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native production-gate state fail-closed coverage for {snippet}",
            errors,
        )
    for label, text in native_sources:
        require(
            re.search(
                r'let\s+zk_ace\s*=\s*[\s\S]*algorithm\.algorithm_id\s*==\s*"zk-ace-pq-authorization-v0"[\s\S]*assert_eq!\(zk_ace\.proof_family,\s*"stark/fri/sha256-goldilocks"\);[\s\S]*assert_eq!\(zk_ace\.backend_family,\s*"stark-fri"\);[\s\S]*ZK-ACE native capability must not become production-ready only because its verifier backend is allowlisted[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ENGINE[\s\S]*PRIVACY_PRODUCTION_GATE_MISSING_ALLOWLIST',
                text,
            )
            is not None,
            f"{label} must pin ZK-ACE native capability profile and fail-closed gate blockers",
            errors,
        )


def check_native_privacy_capability_claim_quarantine_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI capabilities keep production gates fail-closed"',
        "privacy_capability_invariants_hold",
        "!capability\\.production_ready",
        "privacy_capability_invariants_reject_forged_production_readiness",
        "uppercase proof family",
        "delimited proof family",
        "empty proof-family segment",
        "PRIVACY_EXPOSED_PRODUCTION_CLAIM_FRAGMENTS",
        "productionready",
        "productionclaim",
        "claimedproduction",
        "mainnetready",
        "auditsignoff",
        "claimedaudit",
        "securityreviewpassed",
        "privacy_exposed_label_claims_production_readiness",
        "!privacy_exposed_label_claims_production_readiness\\(entry\\.id\\)",
        "!privacy_exposed_label_claims_production_readiness\\(&capability\\.algorithm_id\\)",
        "halo2-production-ready",
        "audit-signoff-pasta",
        "buildMainnetReadyProof",
        "claimed-mainnet-row",
        "buildClaimedAuditProof",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native capability production-claim quarantine coverage for {snippet}",
            errors,
        )


def check_native_privacy_capability_archive_invariant_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI capabilities keep production gates fail-closed"',
        "privacy_capability_invariants_hold",
        "!capability\\.production_ready",
        "privacy_production_gate_invariants_hold",
        "privacy_capability_rows_match_catalog_order",
        "privacy_capabilities_invariants_hold",
        "version\\s*==\\s*PRIVACY_FFI_VERSION_V1",
        "gate_version\\s*==\\s*PRIVACY_PRODUCTION_GATE_VERSION",
        "debug_assert!\\(privacy_capabilities_invariants_hold\\(&capabilities\\)\\)",
        "privacy_capabilities_result_invariants_are_fail_closed",
        "production_ready\\s*=\\s*true",
        "production_gate\\.ready\\s*=\\s*true",
        "\\.passed\\s*=\\s*true",
        "shadow_gate",
        "shuffled production gate key order",
        "audit:\\/\\/forged",
        "external audit signoff is missing",
        "shuffled production-gate missing reasons",
        "external audit signoff passed without evidence",
        "buildShadowProductionProof",
        "privacy_capabilities_invariants_reject_bad_versions_and_duplicate_rows",
        "PRIVACY_FFI_VERSION_V1\\s*\\+\\s*1",
        "privacy-production-gate-v2",
        "shuffled algorithm capability rows",
        "duplicate algorithm capability rows",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native capability archive invariant coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_build_empty_public_inputs_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject empty public inputs before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "privacy_build_proof_rejects_empty_public_inputs_before_production_gate",
        "PrivacyProofOperationV1::Build",
        "public_inputs",
        "non-empty",
        "!result\\.verified",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep build empty public-input coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_verify_empty_public_inputs_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject empty public inputs before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_verify_proof_rejects_empty_public_inputs_before_production_gate",
        "PrivacyProofOperationV1::Verify",
        "public_inputs",
        "non-empty",
        "!result\\.verified",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep verify empty public-input coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_vk_ref_name_binding_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject verifier-key name drift before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_catalog_vk_ref_name",
        '"confidential-transfer-v2"\\s*=>\\s*"confidential_transfer_v2"',
        '"unshield"\\s*=>\\s*"confidential_unshield_v3"',
        '"zk-ace-pq-authorization-v0"\\s*=>\\s*"zk_ace_pq_authorization_v0"',
        '"aztec-private-rollup-v1"\\s*=>\\s*"aztec_private_kernel_v1"',
        '"pq-masp-stark-v0"\\s*=>\\s*"pq_masp_stark_v0"',
        "privacy_canonical_vk_ref_name",
        "privacy_catalog_vk_ref_name\\(entry\\)\\.to_owned\\(\\)",
        "privacy_vk_ref_name_matches_algorithm",
        "privacy_canonical_vk_ref_name\\(entry\\)",
        "name\\s*==\\s*expected_name\\.as_str\\(\\)",
        "privacy proof request vk_ref name must match algorithm verifier key name",
        "privacy_proof_ffi_rejects_wrong_vk_ref_name_before_production_gate",
        "generic-vk-name",
        "foreign-algorithm-vk-name",
        "legacy-vk-prefix",
        "vk_ref name",
        "algorithm verifier key",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep verifier-key name binding coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_vk_ref_shape_hardening_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject verifier-key backend drift before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_vk_ref_parts",
        "split_once\\(':'\\)",
        "privacy_vk_ref_is_well_formed",
        "privacy_vk_ref_backend_family_is_portable",
        "privacy_vk_ref_name_is_portable",
        "privacy proof request vk_ref must use backend:name with portable verifier-key components",
        "privacy_proof_ffi_rejects_malformed_vk_ref_without_reflection",
        "privacy_proof_ffi_rejects_malformed_vk_ref_before_catalog_binding_without_reflection",
        "missing-separator",
        "empty-vk-name",
        "extra-separator",
        "delimited-backend",
        "uppercase-backend",
        "leading-separator-backend",
        "trailing-separator-backend",
        "dotted-backend-alias",
        "underscored-backend-alias",
        "repeated-backend-separator",
        "uppercase-vk-name",
        "dotted-vk-name",
        "dashed-vk-name",
        "leading-underscore-vk-name",
        "trailing-underscore-vk-name",
        "repeated-underscore-vk-name",
        "vk-ref-order-never-echo",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep verifier-key shape hardening coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_vk_ref_backend_binding_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject verifier-key backend drift before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_vk_ref_matches_backend",
        "privacy_vk_ref_parts",
        "privacy proof request vk_ref backend must match algorithm backend family",
        "privacy_proof_ffi_rejects_wrong_backend_vk_ref_before_production_gate",
        "wrong-backend",
        "vk_ref backend",
        "backend family",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep verifier-key backend binding coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_operation_shadow_material_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject proof/witness operation confusion before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PrivacyProofOperationV1::Build",
        "PrivacyProofOperationV1::Verify",
        "privacy proof build request must not include proof bytes",
        "privacy proof verify request must not include witness bytes",
        "privacy_build_proof_rejects_proof_shadow_before_production_gate",
        "privacy_verify_proof_rejects_witness_shadow_before_production_gate",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep operation shadow-material coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_operation_required_material_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject proof/witness operation confusion before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PrivacyProofOperationV1::Build",
        "PrivacyProofOperationV1::Verify",
        "privacy proof build request must include witness bytes",
        "privacy proof verify request must include proof bytes",
        "privacy_build_proof_rejects_missing_witness_before_production_gate",
        "privacy_verify_proof_rejects_missing_proof_before_production_gate",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep operation required-material coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_non_proof_entrypoint_rejection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts reject proof/witness operation confusion before production gate"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy proof request entrypoint must be a production proof builder",
        "privacy_proof_ffi_rejects_non_proof_sdk_entrypoints_before_production_gate",
        "buildRangeCommitment",
        "buildVeRangeDevProofFixture",
        "verifyVeRangeProofLocally",
        "buildZkTransferInstruction",
        "production proof builder",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep non-proof entrypoint rejection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_production_disabled_build_gate_message_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    native_sources = (
        ("C bridge privacy FFI", read("crates/connect_norito_bridge/src/lib.rs")),
        ("JS NAPI privacy FFI", read("crates/iroha_js_host/src/lib.rs")),
        ("Python PyO3 privacy FFI", read("python/iroha_python/iroha_python_rs/src/lib.rs")),
    )

    for snippet in (
        'test("native privacy FFI production-disabled responses enumerate all gates"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_build_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes",
        "(?:iroha_privacy_build_proof_v1|PrivacyProofOperationV1::Build)",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        '"exact protocol implementation"',
        '"real proving"',
        '"real verification"',
        '"chain admission"',
        '"cross-SDK parity"',
        "wallet/state support",
        '"deterministic tests"',
        '"fuzzing"',
        '"performance gates"',
        '"external audit"',
        '"real protocol engine"',
        '"Iroha production allowlist"',
        'result\\.message\\.contains\\(fragment\\)[\\s\\S]*!result\\.message\\.contains\\("secret"\\)',
        "must keep ZK-ACE build requests production-disabled without witness leakage",
        "stark-fri:zk_ace_pq_authorization_v0",
        '!zk_ace_result\\.message\\.contains\\("secret-witness"\\)',
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep production-disabled build gate-message coverage for {snippet}",
            errors,
        )
    for label, text in native_sources:
        require(
            re.search(
                r'privacy_build_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes[\s\S]*"zk-ace-pq-authorization-v0"[\s\S]*"buildZkAceAuthorizationProofV1"[\s\S]*PRIVACY_FFI_ERROR_PRODUCTION_DISABLED[\s\S]*"stark-fri:zk_ace_pq_authorization_v0"[\s\S]*Iroha production allowlist[\s\S]*!zk_ace_result\.message\.contains\("secret-witness"\)',
                text,
            )
            is not None,
            f"{label} must keep ZK-ACE build requests production-disabled without witness leakage",
            errors,
        )


def check_privacy_ffi_production_disabled_verify_gate_message_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    native_sources = (
        ("C bridge privacy FFI", read("crates/connect_norito_bridge/src/lib.rs")),
        ("JS NAPI privacy FFI", read("crates/iroha_js_host/src/lib.rs")),
        ("Python PyO3 privacy FFI", read("python/iroha_python/iroha_python_rs/src/lib.rs")),
    )

    for snippet in (
        'test("native privacy FFI production-disabled responses enumerate all gates"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_verify_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes",
        "(?:iroha_privacy_verify_proof_v1|PrivacyProofOperationV1::Verify)",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        '"exact protocol implementation"',
        '"real proving"',
        '"real verification"',
        '"chain admission"',
        '"cross-SDK parity"',
        "wallet/state support",
        '"deterministic tests"',
        '"fuzzing"',
        '"performance gates"',
        '"external audit"',
        '"real protocol engine"',
        '"Iroha production allowlist"',
        'result\\.message\\.contains\\(fragment\\)[\\s\\S]*!result\\.message\\.contains\\("secret"\\)',
        "must keep ZK-ACE verify requests production-disabled without proof leakage",
        "candidate-zk-ace-proof",
        '!zk_ace_result\\.message\\.contains\\("candidate-zk-ace-proof"\\)',
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep production-disabled verify gate-message coverage for {snippet}",
            errors,
        )
    for label, text in native_sources:
        require(
            re.search(
                r'privacy_verify_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes[\s\S]*"zk-ace-pq-authorization-v0"[\s\S]*"buildZkAceAuthorizationProofV1"[\s\S]*candidate-zk-ace-proof[\s\S]*PRIVACY_FFI_ERROR_PRODUCTION_DISABLED[\s\S]*"stark-fri:zk_ace_pq_authorization_v0"[\s\S]*Iroha production allowlist[\s\S]*!zk_ace_result\.message\.contains\("candidate-zk-ace-proof"\)',
                text,
            )
            is not None,
            f"{label} must keep ZK-ACE verify requests production-disabled without proof leakage",
            errors,
        )


def check_privacy_ffi_production_disabled_message_constant_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI production-disabled responses enumerate all gates"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PRIVACY_PRODUCTION_DISABLED_MESSAGE",
        "privacy production is disabled until exact protocol implementation",
        "real protocol engine enablement",
        "Iroha production allowlist evidence all pass",
        "privacy_failure_result\\(\\s*PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "Some\\(request\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep production-disabled message constant coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_witness_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep failure results non-successful and proof-free"',
        "privacy_failure_results_never_serialize_witness_material",
        "witness-never-echo",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "groth16-bls12-377:confidential_transfer_v2",
        "halo2-ipa-pasta:vk_test",
        "wrong_vk_backend",
        "wrong_vk_name",
        "empty_public_inputs",
        "disabled_build",
        "disabled_verify",
        "witness_shadow_verify",
        "assert_privacy_result_does_not_serialize_witness",
        "PrivacyProofOperationV1::Build",
        "PrivacyProofOperationV1::Verify",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep witness material non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_witness_helper_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep failure results non-successful and proof-free"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "assert_privacy_result_does_not_serialize_witness",
        "result.proof.is_empty()",
        "failed privacy result must not carry a proof",
        "privacy result message",
        "Norito privacy result archive",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep witness helper non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_failure_result_invariant_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep failure results non-successful and proof-free"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_failure_result_invariants_hold",
        "result\\.status\\s*==\\s*PRIVACY_FFI_STATUS_ERROR",
        "result\\.error_code\\s*!=\\s*0",
        "result\\.proof\\.is_empty\\(\\)",
        "!result\\.verified",
        "debug_assert!\\(privacy_failure_result_invariants_hold\\(&result\\)\\)",
        "privacy_failure_result_invariants_hold\\(&result\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep failure-result invariant coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_proof_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts keep failure results non-successful and proof-free"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_failure_results_preserve_error_invariants_without_proof_reflection",
        "proof-never-echo",
        "build-proof-shadow",
        "disabled-verify-proof",
        "PrivacyProofOperationV1::Build",
        "PrivacyProofOperationV1::Verify",
        "privacy_failure_result_invariants_hold(&result)",
        "privacy result message",
        "Norito privacy result archive",
        "encoded privacy result",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep proof non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts bound reflected request fields before production gate"',
        "PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES",
        "PRIVACY_REQUEST_WITNESS_MAX_BYTES",
        "PRIVACY_REQUEST_PROOF_MAX_BYTES",
        "privacy proof request public_inputs exceeds maximum length",
        "privacy proof request witness exceeds maximum length",
        "privacy proof request proof exceeds maximum length",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_text_field_enumerator_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_text_fields",
        "request text-field enumerator",
        "&request.algorithm_id",
        "&request.entrypoint",
        "&request.vk_ref",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request text-field enumerator coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_oversized_request_field_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES",
        "privacy_request_has_oversized_text_field",
        "privacy proof request text fields exceed maximum length",
        "privacy_request_rejects_oversized_text_fields_without_reflection",
        "PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES + 1",
        "algorithm_id",
        "entrypoint",
        "vk_ref",
        "maximum length",
        "windows(oversized.len())",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep oversized request text-field non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_oversized_public_inputs_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_rejects_oversized_public_inputs_without_reflection",
        "PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES + 1",
        '"public_inputs"',
        '"public"',
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep oversized public-input non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_oversized_witness_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_rejects_oversized_witness_without_reflection",
        "oversized-witness-never-echo",
        "PRIVACY_REQUEST_WITNESS_MAX_BYTES + 1",
        "copy_from_slice(marker)",
        '"witness"',
        "assert_subslice_absent",
        "oversized witness marker was reflected",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep oversized witness non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_oversized_proof_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_rejects_oversized_proof_without_reflection",
        "oversized-proof-never-echo",
        "PRIVACY_REQUEST_PROOF_MAX_BYTES + 1",
        '"proof"',
        "PrivacyProofOperationV1::Verify",
        "oversized proof marker was reflected",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep oversized proof non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_control_algorithm_entrypoint_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_control_text_field",
        "privacy proof request text fields must not contain control characters",
        "privacy_request_rejects_control_text_fields_without_reflection",
        "confidential-transfer-v2\\\\nforged",
        "buildConfidentialTransferProofV2\\\\rforged",
        "control characters",
        "algorithm_id",
        "entrypoint",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep control-character algorithm/entrypoint non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_control_vk_ref_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_control_text_field",
        "privacy proof request text fields must not contain control characters",
        "privacy_request_rejects_control_text_fields_without_reflection",
        "vk:test\\\\tforged",
        "control characters",
        "vk_ref",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep control-character vk_ref non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_non_ascii_algorithm_entrypoint_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_non_ascii_text_field",
        "privacy proof request text fields must be printable ASCII",
        "privacy_request_rejects_non_ascii_text_fields_without_reflection",
        "unicode-text-never-echo",
        "confidential-transfer-v2\\{marker\\}\\\\u\\{200B\\}",
        "buildConfidentialTransferProofV2\\{marker\\}\\\\u\\{2060\\}",
        "algorithm_id",
        "entrypoint",
        "printable ASCII",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep non-ASCII algorithm/entrypoint non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_non_ascii_vk_ref_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_non_ascii_text_field",
        "privacy proof request text fields must be printable ASCII",
        "privacy_request_rejects_non_ascii_text_fields_without_reflection",
        "unicode-text-never-echo",
        "vk:test\\{marker\\}\\\\u\\{FF1A\\}spoof",
        "vk_ref",
        "printable ASCII",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep non-ASCII vk_ref non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_unportable_algorithm_entrypoint_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_text_field_is_portable_identifier",
        "is_ascii_alphanumeric\\(\\)",
        "b'-'",
        "b'_'",
        "b'\\.'",
        "b':'",
        "privacy_request_has_unportable_text_field",
        "privacy proof request text fields must use portable identifier characters",
        "privacy_request_rejects_unportable_text_fields_without_reflection",
        "punctuation-text-never-echo",
        "confidential-transfer-v2 \\{marker\\}",
        'buildConfidentialTransferProofV2\\\\"\\{marker\\}\\\\"',
        "algorithm_id",
        "entrypoint",
        "portable identifier",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep unportable algorithm/entrypoint non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_unportable_vk_ref_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_text_field_is_portable_identifier",
        "b'\\.'",
        "b':'",
        "privacy_request_has_unportable_text_field",
        "privacy proof request text fields must use portable identifier characters",
        "privacy_request_rejects_unportable_text_fields_without_reflection",
        "punctuation-text-never-echo",
        "vk:test\\/\\.\\.\\/\\{marker\\}",
        "vk_ref",
        "portable identifier",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep unportable vk_ref non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_algorithm_catalog_shape_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_invalid_catalog_shape",
        "privacy proof request algorithm_id and entrypoint must use catalog identifier shapes",
        "privacy_request_rejects_invalid_catalog_shapes_without_reflection",
        "catalog-shape-text-never-echo",
        "_confidential-transfer-v2",
        "-confidential-transfer-v2",
        "confidential-transfer-v2-\\{marker\\}_",
        "confidential-transfer-v2-\\{marker\\}-",
        "algorithm_id",
        "catalog identifier shapes",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request algorithm_id catalog-shape non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_entrypoint_catalog_shape_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_invalid_catalog_shape",
        "privacy proof request algorithm_id and entrypoint must use catalog identifier shapes",
        "privacy_request_rejects_invalid_catalog_shapes_without_reflection",
        "catalog-shape-text-never-echo",
        "buildConfidentialTransferProofV2:\\{marker\\}",
        "build-ConfidentialTransferProofV2\\{marker\\}",
        "_buildConfidentialTransferProofV2\\{marker\\}",
        "buildConfidentialTransferProofV2_\\{marker\\}",
        "Iroha\\._Privacy\\.buildConfidentialTransferProofV2\\{marker\\}",
        "Iroha\\.Privacy_\\.buildConfidentialTransferProofV2\\{marker\\}",
        "entrypoint",
        "catalog identifier shapes",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request entrypoint catalog-shape non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_required_algorithm_entrypoint_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "request\\.algorithm_id\\.trim\\(\\)\\.is_empty\\(\\)",
        "request\\.entrypoint\\.trim\\(\\)\\.is_empty\\(\\)",
        "privacy proof request must include non-empty algorithm_id and entrypoint",
        "privacy_request_rejects_empty_required_text_fields_without_reflection",
        "required-text-field-never-echo",
        "algorithm_id",
        "entrypoint",
        "empty required field failure result",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep required algorithm/entrypoint non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_required_vk_ref_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "request\\.vk_ref\\.trim\\(\\)\\.is_empty\\(\\)",
        "privacy proof request must include non-empty vk_ref",
        "privacy_request_rejects_empty_required_text_fields_without_reflection",
        "required-text-field-never-echo",
        "vk_ref",
        "empty required field failure result",
        "assert_subslice_absent",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep required vk_ref non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_algorithm_entrypoint_production_claim_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_exposed_production_claim_text_field",
        "privacy_request_text_fields\\(request\\)",
        "privacy_exposed_label_claims_production_readiness\\(field\\)",
        "privacy proof request text fields must not claim production\\/mainnet\\/audit readiness",
        "privacy_request_rejects_exposed_production_claims_without_reflection",
        "forged-mainnet-ready-algorithm",
        "claimed-mainnet-algorithm",
        "buildAuditSignoffProof",
        "buildClaimedAuditProof",
        "buildS\\.e\\.c\\.u\\.r\\.i\\.t\\.yReviewPassedProof",
        "algorithm_id",
        "entrypoint",
        "production\\/mainnet\\/audit readiness",
        "value.as_bytes()",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request algorithm/entrypoint production-claim non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_request_vk_ref_production_claim_nonreflection_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "privacy_request_has_exposed_production_claim_text_field",
        "privacy_request_text_fields\\(request\\)",
        "privacy_exposed_label_claims_production_readiness\\(field\\)",
        "privacy proof request text fields must not claim production\\/mainnet\\/audit readiness",
        "privacy_request_rejects_exposed_production_claims_without_reflection",
        "vk_ref",
        "externally-audited-confidential-transfer",
        "audit-claim-confidential-transfer",
        "production\\/mainnet\\/audit readiness",
        "value.as_bytes()",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request vk_ref production-claim non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_public_operation_schema_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI archives use public operation schema bytes"',
        "PRIVACY_CAPABILITIES_RESULT_SCHEMA_BYTE",
        "PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE",
        "PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE",
        "PRIVACY_REQUEST_SCHEMA_BYTE",
        "privacy_archive_has_repeated_schema_byte",
        "privacy_patch_archive_schema_hash",
        "privacy_patch_archive_repeated_schema_byte",
        "privacy_decode_public_request_archive",
        "<PrivacyProofRequestV1\\s+as\\s+norito::NoritoSerialize>::schema_hash",
        "privacy_result_schema_byte",
        "PrivacyProofOperationV1::Build\\s*=>\\s*PRIVACY_BUILD_PROOF_RESULT_SCHEMA_BYTE",
        "PrivacyProofOperationV1::Verify\\s*=>\\s*PRIVACY_VERIFY_PROOF_RESULT_SCHEMA_BYTE",
        "privacy_(?:ffi|native)_archives_use_public_schema_hashes",
        "privacy_public_schema_request_archives_reject_operation_confusion",
        "privacy_request_archives_reject_private_rust_schema_hashes",
        "private Rust request schema must not masquerade as the public FFI request schema",
        "assert_malformed_privacy_request_result",
        "forged-public-build-proof-shadow",
        "public_privacy_request_archive",
        "PrivacyProofOperationV1::Build",
        "forged-public-verify-witness-shadow",
        "PrivacyProofOperationV1::Verify",
        "missing_witness",
        "missing_proof",
        "write_privacy_payload",
        "read_privacy_request",
        "privacy_patch_archive_repeated_schema_byte\\(&mut bytes,\\s*schema_byte\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep public operation-schema coverage for {snippet}",
            errors,
        )


def check_cross_sdk_privacy_operation_schema_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy native availability proof probes use shared Norito request archives and reject unknown operations"',
        "privacyNativeProbeReturnsBytes",
        "privacyNativeOutputToBuffer\\(result,\\s*operation\\)",
        "assertPrivacyNoritoArchive(",
        "privacyExpectedResultSchemaByte\\(operation\\)",
        "privacy Norito frame validator must require a concrete expected schema",
        "privacy Norito frame validator must not treat missing schemas as wildcard matches",
        "privacy native output decoder must reject unknown operation schemas",
        "JS privacy native tests must pin wrong-operation result schema rejection",
        "_privacy_native_probe_returns_bytes",
        "_privacy_output_archive\\(operation,\\s*result\\)",
        "_assert_privacy_norito_archive\\(",
        "_privacy_expected_result_schema_byte\\(operation\\)",
        "Python privacy tests must pin explicit schema matching without None-schema wildcards",
        "Python privacy tests must pin unknown operation schema rejection",
        "static func isValidPrivacyNativeProbeResult",
        "expectedSchemaByte: UInt8",
        "probePrivacyProofFunction\\(",
        "privacyBuildProofResultSchemaByte",
        "privacyVerifyProofResultSchemaByte",
        "hasPrivacyNoritoSchema\\(archive,\\s*expectedSchemaByte:\\s*expectedSchemaByte\\)",
        "Swift public privacy bridge must require explicit operation schemas before dispatch",
        "Swift public privacy bridge must not expose schema-less output helper defaults",
        "Swift privacy native output decoder must require explicit operation schemas",
        "Swift privacy native output decoder must not expose schema-less output helper defaults",
        "Swift native privacy schema validator must require a concrete expected schema",
        "Swift public privacy schema validator must require a concrete expected schema",
        "Swift native privacy schema validator must not treat missing schemas as wildcard matches",
        "Swift public privacy schema validator must not treat missing schemas as wildcard matches",
        "static byte[] requireNativeOutput",
        "static boolean returnsOutputProbe",
        "static\\s+boolean\\s+returnsOutputProbe\\(\\s*final\\s+int\\s+expectedSchemaByte",
        "Java privacy schema matcher must reject missing expected schemas",
        "Java privacy tests must pin explicit schema matching without negative-schema wildcards",
        "isValidPrivacyNoritoArchive",
        "hasNonEmptyPrivacyNoritoPayload",
        "internal fun requireNativeOutput",
        "internal fun returnsOutputProbe",
        "expectedSchemaByte:\\s*Int",
        "Kotlin privacy schema matcher must require a concrete expected schema",
        "Kotlin privacy schema matcher must not treat missing schemas as wildcard matches",
        "Kotlin privacy native output decoder must not accept nullable expected schemas",
        "Kotlin privacy tests must pin explicit schema matching without negative-schema wildcards",
        "privacySchemaMatcherRequiresExplicitExpectedSchema",
        "private static int CheckedArchiveLength",
        "internal static byte[] ReadPrivacyOutput",
        "ExpectedPrivacyResultSchemas\\(symbol\\)",
        "RequireKnownPrivacyResultSymbol\\(symbol\\)",
        "RequireExplicitPrivacyResultSchemas\\(symbol,\\s*expectedSchemaBytes\\)",
        "HasNoritoSchema\\(result,\\s*schemas\\)",
        "expectedSchemaBytes\\.Length\\s*==\\s*0",
        "C# privacy schema matcher must reject missing expected schemas",
        "C# privacy tests must pin explicit schema matching without empty-schema wildcards",
        "PrivacyNativeSchemaMatcherRequiresExplicitExpectedSchemas",
        "PrivacyNativeReadOutputRequiresExplicitExpectedSchemasAndFreesPointer",
        "PrivacyNativeReadOutputRejectsMismatchedExpectedSchemaSetAndFreesPointer",
        "schema-less success output",
        "privacyNoritoFrameWithPayload\\(0x51\\)",
        "not a supported privacy native operation",
        "rejectsUnknownOperationSchemaNativeOutputs",
        "PrivacyNativeRejectsUnknownOperationSchemaBeforeNativeDispatch",
        "unsupported privacy operations must not reach native dispatch",
        "C# privacy native output decoder must reject wrong-operation result schemas",
        "C# privacy native output decoder must reject unknown operation schemas",
        "C# privacy bridge must reject unknown operations before native dispatch",
        "C# privacy availability probes must require operation-specific result schemas",
        "Swift public privacy bridge must reject wrong-operation result schemas",
        "Swift privacy availability probes must require operation-specific result schemas",
        "Java privacy bridge must reject unknown operations before native dispatch",
        "Java privacy native output decoder must reject unknown operation schemas",
        "Kotlin privacy bridge must reject unknown operations before native dispatch",
        "Kotlin privacy native output decoder must reject unknown operation schemas",
        "Python privacy native output decoder must validate Norito frame headers and operation schemas",
        "Python privacy Norito frame validator must reject empty request and result payloads",
        "JS src",
        "JS dist",
        "Python",
        "Swift",
        "Java Android",
        "Kotlin JVM",
        "C#",
        "PRIVACY_REQUEST_SCHEMA_BYTE = 0x52",
        "_PRIVACY_REQUEST_SCHEMA_BYTE",
        "privacyRequestSchemaByte",
        "PRIVACY_SCHEMA_REQUEST",
        "PrivacyRequestSchemaByte",
        "LEGACY_PRIVACY_MALFORMED_AVAILABILITY_PROBE_ARCHIVE",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep cross-SDK operation-schema coverage for {snippet}",
            errors,
        )


def check_privacy_native_availability_archive_hardening_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native availability probes reject adversarial native output archives"',
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "malformedPrivacyNativeOutputArchives(0x50)",
        "malformedPrivacyNativeOutputArchives(0x42)",
        "malformedPrivacyNativeOutputArchives(0x56)",
        "_malformed_privacy_native_output_archives(0x50)",
        "_malformed_privacy_native_output_archives(0x42)",
        "_malformed_privacy_native_output_archives(0x56)",
        "invalidPrivacyNoritoFrame(offset: 5, value: 1)",
        "invalidPrivacyNoritoDeclaredPayloadLength()",
        "invalidPrivacyNoritoOversizedPayloadLength()",
        "invalidPrivacyNoritoPayloadTamper()",
        "invalidPrivacyNoritoDeclaredPayloadLength(0x50)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x50)",
        "InvalidPrivacyNoritoDeclaredPayloadLength(0x50)",
        "InvalidPrivacyNoritoOversizedPayloadLength(0x50)",
        "InvalidPrivacyNoritoPayloadTamper()",
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "privacyNativeArchiveMaxBytes + 1",
        "PrivacyNativeArchiveMaxBytes + 1",
        "Buffer.alloc(PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1, 0x7f)",
        'monkeypatch.setattr(crypto_module, "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES", 2)',
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native availability archive hardening coverage for {snippet}",
            errors,
        )


def check_privacy_native_availability_probe_gating_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native availability probes reject adversarial native output archives"',
        'test("Swift privacy native availability requires valid Norito proof probes"',
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "testPrivacyNativeProbeResultRequiresSuccessfulNonemptyArchive",
        "nativeProbeRequiresAbiAndAllPrivacySymbols",
        "PrivacyNativeProbeRequiresSuccessfulNonemptyOutput",
        "probePrivacyNativeAvailability",
        "probePrivacyCapabilitiesFunction",
        "probePrivacyProofFunction",
        "privacyNativeProbeOk\\s*=\\s*available",
        "isPrivacyNativeAvailable",
        "isValidPrivacyNativeProbeResult",
        "status == 0",
        "outLen > 0",
        "hasPrivacyNoritoSchema",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native availability probe gating coverage for {snippet}",
            errors,
        )


def check_cross_sdk_privacy_malformed_request_dispatch_boundary_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("JS privacy native tests reject adversarial malformed request archives before dispatch"',
        'test("Python privacy native tests reject adversarial malformed request archives before dispatch"',
        'test("mobile and C# privacy native tests reject adversarial malformed request archives before dispatch"',
        "bad_magic",
        "bad_minor_version",
        "bad_excessive_padding",
        "bad_field_bitset_flags",
        "bad_checksum",
        "bad_payload",
        "empty_payload_request",
        "privacyNoritoFrame(0x52)",
        "PrivacyNoritoFrame(0x52)",
        "empty-payload request must not reach native dispatch",
        "empty-payload build request must not reach native dispatch",
        "empty-payload verify request must not reach native dispatch",
        "non-empty privacy request payload",
        "_FakePrivacyNativeMustNotDispatch\\(\\)",
        "_malformed_privacy_request_archives",
        "privacy_build_proof_v1",
        "privacy_verify_proof_v1",
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep malformed request dispatch boundary coverage for {snippet}",
            errors,
        )


def check_cross_sdk_privacy_sliced_view_boundary_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("JS and Python privacy native tests pin sliced byte-view request handling"',
        'test("JS and Python privacy native tests pin sliced byte-view native output handling"',
        "slicedPrivacyView\\(PRIVACY_REQUEST_ARCHIVE\\)",
        "_sliced_privacy_memoryview",
        "new DataView\\(",
        "buildRequest\\.every",
        "verifyRequest\\.every",
        "all\\(value == 0 for value in request\\)",
        "capabilitiesBacking\\.subarray\\(",
        "memoryview\\(self\\.capabilities_backing\\)",
        "native\\.capabilities_backing\\[native\\.prefix_len\\]",
        "assert\\.notEqual\\(capabilitiesArchive",
        "assertTrue\\(archive !== nativeOutput\\)",
        "_privacy_unsigned_byte_view",
        "view\\.format\\s*!=\\s*\"B\"[\\s\\S]*view\\.itemsize\\s*!=\\s*1",
        "request_archive must use unsigned byte elements",
        "output must use unsigned byte elements",
        "test_privacy_native_build_and_verify_reject_ambiguous_typed_request_archive",
        "test_privacy_native_wrappers_reject_ambiguous_typed_native_output",
        "_signed_byte_array",
        "_FakeTypedOutputPrivacyNative",
        'array\\("H", \\[0x5252\\] \\* 24\\)',
        'array\\("H", \\[0x4242\\] \\* 24\\)',
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep sliced byte-view boundary coverage for {snippet}",
            errors,
        )


def check_cross_sdk_privacy_native_output_boundary_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native tests defensively copy native output archives"',
        'test("SDK privacy native tests reject malformed native output archives"',
        "invalidPrivacyNoritoDeclaredPayloadLength(0x52)",
        "invalidPrivacyNoritoOversizedPayloadLength(0x52)",
        "InvalidPrivacyNoritoOversizedPayloadLength(0x52)",
        "valid Norito V1 archive",
        "empty_payload_capabilities_result",
        "empty_payload_build_result",
        "empty_payload_verify_result",
        "privacyNoritoFrame(0x50)",
        "privacyNoritoFrame(0x42)",
        "privacyNoritoFrame(0x56)",
        "PrivacyNoritoFrame(0x50)",
        "PrivacyNoritoFrame(0x42)",
        "PrivacyNoritoFrame(0x56)",
        "empty privacy result payload",
        "Marshal\\.Copy\\(FilledBytes\\(0x7f",
        "PrivacyNativeArchiveMaxBytes + 1",
        "privacyNativeArchiveMaxBytes + 1",
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1",
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native output boundary coverage for {snippet}",
            errors,
        )


def check_privacy_norito_request_padding_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native tests pin Norito header padding boundaries"',
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "privacyNoritoFrameWithPadding(0x52, 64)",
        "privacyNoritoFrameWithPadding(0x52, 65)",
        "_privacy_norito_frame_with_padding(0x52, 65)",
        "PrivacyNoritoFrameWithPadding(0x50, 65)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Norito request padding coverage for {snippet}",
            errors,
        )


def check_privacy_norito_request_field_bitset_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native tests accept complete Norito field-bitset flags"',
        "JS source privacy native tests",
        "JS package dist privacy native tests",
        "Python privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "function privacyNoritoFrameWithFlags",
        "def _privacy_norito_frame_with_flags",
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x42, 0x26)",
        "PrivacyNoritoFrameWithFlags(0x42, 0x26)",
        "acceptsCompleteFieldBitsetNoritoFlags",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Norito request field-bitset coverage for {snippet}",
            errors,
        )


def check_privacy_norito_wrong_schema_request_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native tests reject wrong-schema request archives before dispatch"',
        "JS source privacy native tests",
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "wrongSchemaPrivacyRequestArchives",
        "privacyNoritoFrameWithPayload(0x50)",
        "privacyNoritoFrameWithPayload(0x42)",
        "privacyNoritoFrameWithPayload(0x56)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42)",
        "privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "privacyNoritoFrameWithSchemaOverride(0x52, offset: 6, value: 0x42)",
        "PrivacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)",
        "requestArchive must use the privacy request schema",
        "wrong-schema request must not reach native dispatch",
        "wrong-schema build request must not reach native dispatch",
        "wrong-schema verify request must not reach native dispatch",
        "wrong-schema build request reached native dispatch",
        "wrong-schema verify request reached native dispatch",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Norito wrong-schema request coverage for {snippet}",
            errors,
        )


def check_privacy_sdk_request_decoder_bounds_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "function toPrivacyRequestArchiveBuffer",
        "function privacyNativeOutputToBuffer",
        "request\\.length\\s*>\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        'assertPrivacyNoritoArchive(request, name, "request", PRIVACY_REQUEST_SCHEMA_BYTE);',
        "return Buffer.from(request);",
        "def _privacy_request_archive",
        "def _clear_privacy_request_archive",
        "view\\.nbytes\\s*>\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "_assert_privacy_norito_archive",
        "static func withTemporaryPrivacyRequestArchive",
        "static func clearTemporaryPrivacyRequestArchive",
        "requestArchive\\.count\\s*<=\\s*Self\\.privacyNativeArchiveMaxBytes",
        "Self\\.isValidPrivacyNoritoArchive\\(requestArchive\\)",
        "Self\\.hasPrivacyNoritoSchema",
        "Self\\.hasNonEmptyPrivacyNoritoPayload\\(requestArchive\\)",
        "static func call(\\n        requestArchive: Data",
        "NoritoNativeBridge\\.isValidPrivacyNoritoArchive\\(requestArchive\\)",
        "NoritoNativeBridge\\.hasNonEmptyPrivacyNoritoPayload\\(requestArchive\\)",
        "static byte[] call(",
        "requestArchive\\.length\\s*>\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "hasPrivacyNoritoSchema\\(requestArchive,\\s*PRIVACY_SCHEMA_REQUEST\\)",
        "internal fun call(",
        "requestArchive\\.size\\s*<=\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "internal static byte[] CallProof",
        "requestArchive\\.Length\\s*>\\s*PrivacyNativeArchiveMaxBytes",
        "requestArchive\\.ToArray\\(\\)",
        "Array\\.Clear\\(request,\\s*0,\\s*request\\.Length\\)",
        "HasNoritoSchema\\(request,\\s*PrivacyRequestSchemaByte\\)",
        "HasNonEmptyPrivacyNoritoPayload\\(request\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep SDK request decoder bounds coverage for {snippet}",
            errors,
        )


def check_privacy_c_bridge_output_buffer_precedence_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "privacy_request_archive_out_of_bounds",
        "privacy_result_for_request_archive",
        "write_privacy_payload",
        "read_privacy_request",
        "iroha_privacy_process_request_v1",
        "privacy_request_archive_out_of_bounds\\(request_bytes\\.len\\(\\)\\)",
        "privacy_request_archive_out_of_bounds\\(request_len\\)",
        "clear_privacy_output",
        "ptr::null_mut\\(\\)",
        "out_len\\s*=\\s*0",
        "ERR_NULL_PTR",
        "privacy_capabilities_reject_missing_output_buffer",
        "privacy_proof_entrypoints_prioritize_missing_output_buffers_over_bad_requests",
        "without_provenance_mut::<c_uchar>\\(0x01\\)",
        "without_provenance_mut::<c_uchar>\\(0x04\\)",
        "privacy_request_archive_size_boundaries_are_fail_closed",
        "C bridge privacy proof entrypoints must prioritize missing output buffers over bad request pointers",
        "C bridge privacy payload writer must clear stale output slots before returning null-pointer errors",
        "C bridge privacy proof processor must clear stale output slots before null-output errors",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep C bridge output-buffer precedence coverage for {snippet}",
            errors,
        )


def check_privacy_native_adversarial_request_frame_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        "privacy_request_archive_out_of_bounds",
        "privacy_request_archive_size_boundaries_are_fail_closed",
        "adversarial_privacy_request_archives",
        "bad_magic\\[0\\]",
        "bad_version\\[4\\]",
        "bad_schema\\[6\\]",
        "bad_compression\\[22\\]",
        "bad_payload_length\\[30\\]",
        "bad_crc\\[31\\]",
        "bad_flags\\[39\\]",
        "payload_tamper\\[payload_last\\]",
        "privacy_proof_entrypoints_reject_adversarial_norito_frames",
        "privacy_request_archive_out_of_bounds\\(request_archive\\.len\\(\\)\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native adversarial request frame coverage for {snippet}",
            errors,
        )


def check_privacy_request_copy_zeroization_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native tests clear request copies after native failures"',
        'test("JS and Python privacy availability probes clear request copies after failures"',
        "capturedRequests",
        "request\\.every\\(\\(value\\)\\s*=>\\s*value\\s*===\\s*0\\)",
        "sanitize_native_exceptions",
        "native\\.requests",
        "Array\\.TrueForAll\\(buildRequest!",
        "didClearForTesting",
        "availability probes clear request copies after native failures",
        "probe failure after request copy",
        "throwingProbe",
        "badOutputProbe",
        "availability_probes_clear_request_copies_after_failures",
        "throwing_native\\.build_request",
        "bad_output_native\\.verify_request",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep request-copy zeroization coverage for {snippet}",
            errors,
        )


def check_python_privacy_native_method_surface_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("Python privacy native wrappers require the complete FFI method surface"',
        "def _missing_privacy_native_methods",
        "_PRIVACY_NATIVE_METHODS",
        "privacy FFI requires complete native method surface; missing",
        "missing privacy_verify_proof_v1",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Python native method-surface coverage for {snippet}",
            errors,
        )


def check_privacy_native_abi_probe_bounds_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy native ABI probes reject unsafe and out-of-range versions"',
        "PRIVACY_MAX_BRIDGE_ABI_VERSION",
        "Number\\.isSafeInteger",
        "version >= 0",
        "version <= PRIVACY_MAX_BRIDGE_ABI_VERSION",
        "Number.NaN",
        "Number.POSITIVE_INFINITY",
        "Number.MAX_SAFE_INTEGER + 1",
        "0x1_0000_0000",
        "6.5",
        "-1",
        "_PRIVACY_MAX_BRIDGE_ABI_VERSION",
        "version < 0",
        "version > _PRIVACY_MAX_BRIDGE_ABI_VERSION",
        "10**100",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native ABI probe bounds coverage for {snippet}",
            errors,
        )


def check_zk_ace_public_proof_builder_native_error_sanitizer_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    js_privacy_tests = read("javascript/iroha_js/test/privacyNative.test.js")
    python_catalog_tests = read("python/iroha_python/tests/privacy_catalog_test.py")
    python_crypto = read("python/iroha_python/src/iroha_python/crypto.py")

    for snippet in (
        'test("ZK-ACE public proof builders sanitize production-disabled native errors"',
        "ZK-ACE transfer authorization sanitizes production-disabled native errors",
        "test_zk_ace_python_proof_builder_sanitizes_production_disabled_native_errors",
        "js-zk-ace-private-secret-1234567",
        "py-zk-ace-private-secret-1234567",
        "candidate-zk-ace-proof",
        "error.__context__ is None",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep ZK-ACE public proof-builder sanitizer coverage for {snippet}",
            errors,
        )

    for label, text in (
        ("JS source crypto helper", read("javascript/iroha_js/src/crypto.js")),
        ("JS dist crypto helper", read("javascript/iroha_js/dist/crypto.js")),
    ):
        for snippet in (
            "ZK_ACE_PRODUCTION_DISABLED_MESSAGE",
            "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
            "zk-ace-pq-authorization-v0",
            "buildZkAceAuthorizationProofV1",
            "stark-fri:zk_ace_pq_authorization_v0",
            "Iroha production allowlist is not enabled for this audited row",
            "function sanitizeZkAceNativeProverError",
            "production[- ]disabled",
            "const nativeArgs = zkAceTransferAuthorizationNativeArgs(options)",
            "native.zkAceBuildTransferAuthorizationV1(...nativeArgs)",
            "throw sanitizeZkAceNativeProverError",
            "native ZK-ACE prover failed",
            "const U128_MAX = (1n << 128n) - 1n",
            "function zkAceTransferAuthorizationNativeArgs",
            'normalizePositiveU128Literal(options.amount, "amount")',
            "function normalizePositiveU128Literal",
            'typeof value === "bigint"',
            "Number.isSafeInteger(value)",
            "amount <= 0n || amount > U128_MAX",
        ):
            require(
                snippet in text,
                f"{label} must keep ZK-ACE public proof-builder sanitizer snippet {snippet}",
                errors,
            )

    for label, text in (
        ("JS source instruction builder", read("javascript/iroha_js/src/instructionBuilders.js")),
        ("JS dist instruction builder", read("javascript/iroha_js/dist/instructionBuilders.js")),
    ):
        for snippet in (
            "ZK_ACE_PRODUCTION_DISABLED_MESSAGE",
            "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
            "zk-ace-pq-authorization-v0",
            "buildZkAceAuthorizationProofV1",
            "stark-fri:zk_ace_pq_authorization_v0",
            "Iroha production allowlist is not enabled for this audited row",
            "function sanitizeZkAceNativeAuthorizationProofError",
            "production[- ]disabled",
            "sanitizeZkAceNativeAuthorizationProofError",
            "native ZK-ACE prover failed",
        ):
            require(
                snippet in text,
                f"{label} must keep ZK-ACE witness proof-builder sanitizer snippet {snippet}",
                errors,
            )

    for snippet in (
        "ZK-ACE transfer authorization sanitizes production-disabled native errors",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "zk-ace-pq-authorization-v0",
        "buildZkAceAuthorizationProofV1",
        "stark-fri:zk_ace_pq_authorization_v0",
        "Iroha production allowlist",
        "js-zk-ace-private-secret-1234567",
        "candidate-zk-ace-proof",
        'error.message.includes(secret.toString("utf8")), false',
        "error.message.includes(proof), false",
        "ZK-ACE transfer authorization rejects malformed amounts before native dispatch",
        "ZK-ACE transfer authorization canonicalizes positive u128 amounts before native dispatch",
        "hostileAmount",
        "stringified, false",
        "nativeCalls, 0",
        "1n << 128n",
        'capturedAmounts, ["17", "23", u128Max.toString(10)]',
    ):
        require(
            snippet in js_privacy_tests,
            f"JS privacy tests must keep ZK-ACE public proof-builder non-reflection coverage for {snippet}",
            errors,
        )

    for snippet in (
        "_ZK_ACE_PRODUCTION_DISABLED_MESSAGE",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "zk-ace-pq-authorization-v0",
        "buildZkAceAuthorizationProofV1",
        "stark-fri:zk_ace_pq_authorization_v0",
        "Iroha production allowlist is not enabled for this audited row",
        "def _zk_ace_sanitized_native_prover_error",
        "production disabled",
        "production-disabled",
        "native_args = (",
        "native_error: Exception | None = None",
        "_crypto.zk_ace_build_transfer_authorization_v1(*native_args)",
        "raise _zk_ace_sanitized_native_prover_error(native_error)",
        "native ZK-ACE prover failed",
        "_U128_MAX: Final[int] = (1 << 128) - 1",
        "def _normalize_positive_u128_literal",
        "isinstance(value, bool)",
        "not normalized.isdecimal()",
        'amount <= 0 or amount > _U128_MAX',
        '_normalize_positive_u128_literal(amount, "amount")',
    ):
        require(
            snippet in python_crypto,
            f"Python crypto helper must keep ZK-ACE native-error sanitizer snippet {snippet}",
            errors,
        )

    for snippet in (
        "test_zk_ace_python_proof_builder_sanitizes_production_disabled_native_errors",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "zk-ace-pq-authorization-v0",
        "buildZkAceAuthorizationProofV1",
        "stark-fri:zk_ace_pq_authorization_v0",
        "Iroha production allowlist",
        "py-zk-ace-private-secret-1234567",
        "candidate-zk-ace-proof",
        "secret.decode() not in message",
        "proof not in message",
        "error.__context__ is None",
        "test_zk_ace_python_transfer_authorization_rejects_malformed_amounts_before_native",
        "test_zk_ace_python_transfer_authorization_canonicalizes_positive_u128_amounts",
        "HostileAmount",
        "native.calls == 0",
        "hostile_amount.stringified is False",
        "1 << 128",
        'native.amounts == ["17", "23", str((1 << 128) - 1)]',
    ):
        require(
            snippet in python_catalog_tests,
            f"Python catalog tests must keep ZK-ACE public proof-builder non-reflection coverage for {snippet}",
            errors,
        )


def check_privacy_hostile_request_mutation_isolation_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("mobile, Swift, and C# privacy native tests isolate hostile native request mutation"',
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "UnsafeMutablePointer\\(mutating:\\s*buffer\\.baseAddress\\)",
        "request\\?\\[6\\]\\s*=\\s*0x7F",
        "request\\[6\\]\\s*=\\s*0x7f",
        "requestArchive\\.contentEquals\\(originalArchive\\)",
        "Assert\\.Equal\\(originalArchive,\\s*requestArchive\\)",
        "clearedArchive\\.allSatisfy\\s*\\{\\s*\\$0\\s*==\\s*0\\s*\\}",
        "assertAllZero\\(capturedRequests\\[0\\]\\)",
        "assertAllZero\\(capturedRequests\\[1\\]\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep hostile request mutation isolation coverage for {snippet}",
            errors,
        )


def check_privacy_malformed_request_no_dispatch_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("Swift privacy native tests reject malformed request archives before dispatch"',
        'test("JS privacy native tests reject adversarial malformed request archives before dispatch"',
        "invalidPrivacyNoritoWithExcessivePadding\\(\\)",
        "XCTFail\\(\"invalid request must not reach native dispatch\"\\)",
        "function malformedPrivacyRequestArchives",
        "badMagic",
        "badOversizedDeclaredPayloadLength",
        "badExcessivePadding",
        "badFieldBitsetFlags",
        "malformedPrivacyRequestArchives\\(\\)",
        "assert\\.fail\\(\"invalid build request must not reach native dispatch\"\\)",
        "assert\\.fail\\(\"invalid verify request must not reach native dispatch\"\\)",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep malformed request no-dispatch coverage for {snippet}",
            errors,
        )


def check_privacy_public_wrapper_isolation_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("mobile and C# privacy tests isolate forged production-gate mutations"',
        "Swift privacy native tests",
        "Java Android privacy native tests",
        "Kotlin/JVM privacy native tests",
        "C# privacy native tests",
        "tampered",
        "https://audit.example/forged-signoff",
        "missingProductionGates|productionGate\\.missing|ProductionGate\\.Missing",
        "auditReferences|AuditReferences",
        "forged missing reasons do not pollute fresh capabilities",
        "forged audit references do not pollute fresh capabilities",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep public-wrapper isolation coverage for {snippet}",
            errors,
        )


def check_privacy_public_archive_wrapper_norito_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("C# public privacy archive wrappers reject malformed Norito archives"',
        "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
        "csharp/tests/Hyperledger.Iroha.Sdk.Tests/PrivacyNativeTests.cs",
        "internal static class PrivacyArchiveBytes",
        "public sealed class PrivacyCapabilitiesArchive",
        "C# public privacy archive wrapper copy",
        "noritoBytes\\.Length\\s*>\\s*PrivacyNative\\.PrivacyNativeArchiveMaxBytes",
        "PrivacyNative\\.IsNoritoV1Archive\\(noritoBytes\\)",
        "PrivacyNativeArchiveWrappersRejectUnsafeNoritoBytes",
        "InvalidPrivacyRequestArchives\\(\\)",
        "new PrivacyCapabilitiesArchive\\(malformed\\)",
        "new PrivacyProofResultArchive\\(malformed\\)",
        "PrivacyNativeArchiveMaxBytes\\s*\\+\\s*1",
        "C# public privacy archive wrappers must reject oversized Norito archives",
        "C# public privacy archive wrappers must validate Norito frame shape",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep public archive wrapper Norito coverage for {snippet}",
            errors,
        )


def check_privacy_backend_alias_fail_closed_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("pending privacy backend tags stay in cross-SDK parity"',
        'test("developer-only privacy backend labels stay rejected before production allowlists"',
        'test("adversarial pending privacy backend aliases stay covered across SDK tests"',
        "EXPECTED_PENDING_PRIVACY_BACKEND_LABELS",
        "EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_BACKEND_LABELS",
        "EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_ROWS",
        "extractPublicRequiredPrivacyPlanRows",
        "public required backend families must match pending privacy backend tags",
        "public required backend families must document every production-allowlisted backend",
        "public required production-allowlisted backend rows must stay scoped to ZK-ACE",
        "EXPECTED_ADVERSARIAL_PENDING_PRIVACY_BACKEND_LABELS",
        "EXPECTED_ADVERSARIAL_DEVELOPER_BACKEND_LABELS",
        "EXPECTED_UNSTABLE_STARK_FRI_PROFILE_LABELS",
        "EXPECTED_DEVELOPER_ONLY_NATIVE_HALO2_PROFILE_LABELS",
        "halo2-ipa-orchard",
        "groth16-bls12-377",
        "fcmp-plus-plus-curve-tree",
        "lattice-pcs-sis",
        "miden-stark",
        "aztec-plonkish-private-kernel",
        "pq-masp-stark-fri",
        "halo2/ipa/orchard/dev-fixture",
        "stark/fri/miden/claimed-production",
        "anonymous-pgc-k-out-of-n-v1-production",
        "sis-hints-anoncred-pq-v0-devfixture",
        "groth16/bls12-377/../../prod",
        "post-quantum-masp/audit-claimed",
        "stark/fri/dev-fixture",
        "stark/fri/d-e-v-f-i-x-t-u-r-e",
        "stark/fri/test",
        "stark/fri/t-e-s-t",
        "stark/fri/placeholder",
        "halo2/ipa:dev-fixture",
        "halo2/ipa:dummy",
        "halo2/ipa:f-a-k-e",
        "halo2/ipa:s-a-m-p-l-e",
        "stark/fri/latest",
        "stark/fri/attestation",
        "stark/fri/contest",
        "halo2/pasta/asset-hidden-transfer-public-test",
        "halo2/ipa:asset-hidden-transfer-public-test",
        "Rust production backend unit tests",
        "JS instruction builder tests",
        "JS Torii verifier-key tests",
        "Python Torii verifier-key tests",
        "Python OpenVerify tests",
        "Kotlin verifier-key backend tag tests",
        "Android Java verifier-key instruction tests",
        "Swift backend tag tests",
        "C# backend tag tests",
        "isPendingProductionBackend|IsPendingProductionBackend",
        "parse\\(label\\)",
        "must cover unstable embedded-text STARK/FRI alias",
        "must include adversarial pending backend alias",
        "must assert fail-closed pending classification",
        "must assert adversarial labels are not canonical Norito tags",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep backend-alias fail-closed coverage for {snippet}",
            errors,
        )


def check_privacy_chain_backend_allowlist_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native chain proof admission uses explicit production verifier backend allowlist"',
        "pub fn production_verify_backend_tag",
        "production_verify_backend_label_is_portable",
        "pub fn is_production_verify_backend_label",
        "EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_BACKEND_LABELS",
        "EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_RUST_BACKEND_LABELS",
        'Object.freeze(["zk-ace-pq-authorization-v0", "stark-fri"])',
        'Object.freeze(["stark-fri", "stark/fri", "stark/fri/sha256-goldilocks"])',
        "required production-plan backend exceptions must map every public label to a Rust verifier backend label",
        "Rust production verifier backend allowlist test",
        "must be explicitly covered by the Rust production allowlist test",
        "fn ensure_production_verifying_key_backend_id",
        "fn validate_proof_attachment",
        "fn open_verify_backend_tag_matches",
        "production_verify_backend_allowlist_is_explicit",
        "production_claim_classifier_catches_readiness_and_audit_labels",
        "guardrails_reject_production_claim_backends_before_dispatch",
        "preverify_rejects_production_claim_backends_before_dedup",
        "register_vk_rejects_production_claim_backend_labels",
        "verify_proof_rejects_production_claim_backend_labels_before_registry_lookup",
        "guardrails_reject_unsupported_backends_before_dispatch",
        "register_vk_rejects_unsupported_backend_labels",
        "verifyproof_rejects_unsupported_backend_before_lookup",
        "function isProductionVerifyBackendLabel",
        "function assertProductionVerifyBackendLabel",
        "normalizePrivacyVerifierKeyIdFromOptions",
        "normalizePrivacyBackendTag",
        "normalizeVerifyingKeyRegisterPayload",
        "normalizeVerifyingKeyUpdatePayload",
        'backend.includes("+") && !PLUS_PRIVACY_BACKEND_ALIASES.has(backend)',
        "return /^[A-Za-z0-9/_.:+-]+$/u.test(backend);",
        "from \\._privacy_backends import _require_production_verify_backend_label",
        "def submit_zk_verifying_key_registration",
        "def _normalize_backend",
        "fun isProductionVerifyBackendLabel",
        "fun Map<String, String>\\.productionBackend",
        "RegisterVerifyingKeyInstruction",
        "UpdateVerifyingKeyInstruction",
        "static boolean isProductionVerifyBackendLabel",
        "requireProductionBackend",
        "trimWhitespace",
        "static bool IsProductionVerifyBackendLabel",
        "FromCatalogLabel",
        "backend\\.Trim\\(\\) != backend",
        "static func isProductionVerifyBackendLabel",
        "ToriiVerifyingKeyRequestValidation",
        "testVerifyingKeyRequestsRejectUnsupportedProductionBackendsBeforeEncoding",
        "productionready",
        "claimedproduction",
        "mainnetready",
        "auditsignoff",
        "externallyaudited",
        "securityreviewpassed",
        "halo2/ipa:production-ready",
        "halo2/ipa:mainnet-ready",
        "stark/fri/audit-signoff",
        "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
        "stark/fri/a-u-d-i-t-c-l-a-i-m",
        "halo2/ipa\\\\0",
        "halo2/ipa\\\\u0000",
        "'\\\\0'",
        '" halo2/ipa"',
        "\\\\uFF0F",
        "\\\\u200B",
        "\\\\u0430",
        "\\\\u{FF0F}",
        "\\\\u{200B}",
        "\\\\u{0430}",
        "STARK_FRI_V1_PRODUCTION_PROFILES",
        "STARK_FRI_PRODUCTION_BACKEND_LABELS",
        "_STARK_FRI_PRODUCTION_BACKEND_LABELS",
        "starkFriProductionBackends",
        "STARK_FRI_PRODUCTION_BACKENDS",
        "StarkFriProductionBackends",
        "sha256-goldilocks",
        "poseidon2-goldilocks",
        "sha256_goldilocks.v1",
        "EXPECTED_UNREGISTERED_STARK_FRI_PROFILE_LABELS",
        "EXPECTED_TOY_NATIVE_HALO2_PROFILE_LABELS",
        "EXPECTED_LEGACY_VOTE_NATIVE_HALO2_PROFILE_LABELS",
        "EXPECTED_LEGACY_ANON_TRANSFER_NATIVE_HALO2_PROFILE_LABELS",
        "halo2/pasta/tiny-add",
        "halo2/ipa:tiny-add",
        "halo2/pasta/vote-bool-commit",
        "halo2/ipa:vote-bool-commit-merkle16",
        "halo2/pasta/anon-transfer-2x2",
        "halo2/ipa:anon-transfer-2x2-merkle16",
        "must reject production-claim backend",
        "must reject NUL-suffixed production backend labels",
        "must reject Unicode-confusable production backend labels",
        "must keep STARK/FRI profiles on an explicit production allowlist",
        "must not advertise developer-only native Halo2 profile as production",
        "must reject unregistered STARK/FRI profile",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep chain backend allowlist coverage for {snippet}",
            errors,
        )
    required_allowlist_match = re.search(
        r"const\s+EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_BACKEND_LABELS\s*=\s*Object\.freeze\(\[\s*([\s\S]*?)\s*\]\);",
        ffi_parity,
    )
    if required_allowlist_match is None:
        require(
            False,
            "Privacy FFI parity tests must declare exact required production allowlist backend labels",
            errors,
        )
    else:
        required_allowlist_labels = re.findall(r'"([^"]+)"', required_allowlist_match.group(1))
        require(
            required_allowlist_labels == ["stark-fri"],
            "Privacy FFI parity tests must keep exact required production allowlist backend labels ['stark-fri']",
            errors,
        )


def check_privacy_ffi_c_symbol_surface_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI public symbol names stay stable across native bindings"',
        "EXPECTED_PRIVACY_C_FFI_SYMBOLS",
        "iroha_privacy_capabilities_v1",
        "iroha_privacy_build_proof_v1",
        "iroha_privacy_verify_proof_v1",
        "iroha_privacy_free_buffer",
        "assertRustNoMangleExport",
        "PrivacyProofOperationV1::Build",
        "PrivacyProofOperationV1::Verify",
        "connect_norito_free\\(ptr_\\)",
        "C bridge header must declare",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep C ABI symbol surface coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_binding_loader_surface_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI public symbol names stay stable across native bindings"',
        "EXPECTED_PRIVACY_C_FFI_SYMBOLS",
        "EXPECTED_PRIVACY_JNI_METHODS",
        "dlsym(handle,",
        "EntryPoint =",
        "Java_org_hyperledger_iroha_sdk_privacy_PrivacyNativeBridge_${method}",
        "Java_org_hyperledger_iroha_android_privacy_PrivacyNativeBridge_${method}",
        "nativeBridgeAbiVersion",
        "nativeCapabilities",
        "nativeBuildProof",
        "nativeVerifyProof",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep binding loader/JNI surface coverage for {snippet}",
            errors,
        )


def check_privacy_ffi_error_contract_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI ABI and deterministic error constants stay in parity"',
        "EXPECTED_CONTRACT",
        "assertContractSubset",
        "requiredBridgeAbiVersion",
        "value >= EXPECTED_CONTRACT[key]",
        "must be at least",
        "ffiVersionV1",
        "statusError",
        "errorNullPointer",
        "errorMalformedNorito",
        "errorUnsupportedAlgorithm",
        "errorProductionDisabled",
        "errorInvalidRequest",
        "JS_SRC_PRIVACY_FFI_VERSION_V1",
        "JS_BROWSER_SRC_PRIVACY_FFI_VERSION_V1",
        "JS_DIST_PRIVACY_FFI_VERSION_V1",
        "JS_BROWSER_DIST_PRIVACY_FFI_VERSION_V1",
        "PRIVACY_FFI_STATUS_ERROR",
        "PRIVACY_FFI_ERROR_NULL_POINTER",
        "PRIVACY_FFI_ERROR_MALFORMED_NORITO",
        "PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM",
        "PRIVACY_FFI_ERROR_PRODUCTION_DISABLED",
        "PRIVACY_FFI_ERROR_INVALID_REQUEST",
        "Python crypto",
        "Swift privacy bridge",
        "connect_norito_bridge privacy FFI",
        "iroha_js_host privacy FFI",
        "iroha_python_rs privacy FFI",
        "Java Android privacy bridge",
        "Kotlin JVM privacy bridge",
        "C# privacy bridge",
        "rustPrivacyFfiConstants",
        "jvmPrivacyFfiConstants",
        "csharpConst",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep deterministic error contract coverage for {snippet}",
            errors,
        )


def check_privacy_native_archive_max_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI ABI and deterministic error constants stay in parity"',
        "EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "JS_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "JS_BROWSER_SRC_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "JS_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "JS_BROWSER_DIST_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "privacyNativeArchiveMaxBytes",
        "PrivacyNativeArchiveMaxBytes",
        "Python privacy native archive cap drifted",
        "Swift privacy native archive cap drifted",
        "connect_norito_bridge privacy native archive cap drifted",
        "iroha_js_host privacy native archive cap drifted",
        "iroha_python_rs privacy native archive cap drifted",
        "Java Android privacy native archive cap drifted",
        "Kotlin JVM privacy native archive cap drifted",
        "C# privacy native archive cap drifted",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native archive max parity coverage for {snippet}",
            errors,
        )


def check_privacy_sdk_bridge_method_surface_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy native bridges expose only generic archive operations"',
        "EXPECTED_SWIFT_PRIVACY_BRIDGE_METHODS",
        "EXPECTED_JAVA_PRIVACY_BRIDGE_METHODS",
        "EXPECTED_KOTLIN_PRIVACY_BRIDGE_METHODS",
        "EXPECTED_CSHARP_PRIVACY_BRIDGE_METHODS",
        '"privacyCapabilities"',
        '"capabilitiesV1"',
        '"buildProofV1"',
        '"verifyProofV1"',
        '"capabilitiesArchive"',
        '"buildProof"',
        '"verifyProof"',
        '"GetPrivacyCapabilities"',
        '"CapabilitiesV1"',
        '"BuildProofV1"',
        '"VerifyProofV1"',
        "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift",
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java",
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt",
        "csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs",
        "public\\s+static\\s+func",
        "public\\s+static\\s+(?:[A-Za-z0-9_<>\\[\\]]+\\s+)+",
        "@JvmStatic",
        "public\\s+static\\s+[A-Za-z0-9_<>,\\[\\]\\s]+",
        "namesFromMatches",
        "assert.deepEqual",
        "Swift PrivacyNativeBridge public methods drifted",
        "Java Android PrivacyNativeBridge public methods drifted",
        "Kotlin/JVM PrivacyNativeBridge public methods drifted",
        "C# PrivacyNative public methods drifted",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep SDK bridge method-surface coverage for {snippet}",
            errors,
        )


def check_privacy_sdk_binary_only_ffi_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI SDK wrappers remain binary-only and JSON-free"',
        "JS src crypto privacy FFI",
        "JS dist crypto privacy FFI",
        "Python crypto privacy FFI",
        "Swift PrivacyNativeBridge",
        "Swift native bridge privacy FFI",
        "Java Android privacy bridge",
        "Kotlin JVM privacy bridge",
        "C# privacy bridge",
        "function hasPrivacyNativeSurface",
        "def _privacy_request_archive",
        "func privacyCapabilitiesV1",
        "Buffer|Uint8Array|ArrayBuffer|bytes",
        "bytes|bytearray|memoryview",
        "Data|UInt8|UnsafeMutablePointer",
        "byte\\[\\]",
        "ByteArray",
        "byte\\[\\]|ReadOnlySpan<byte>",
        "requestJson|resultJson|payloadJson|jsonPayload|jsonResult",
        "\\bJSON\\b|serde_json|norito::json|json::",
        "JSONSerialization",
        "System\\.Text\\.Json",
        "Gson|org\\.json",
        "json\\.(?:loads|dumps)",
        "JSON\\.(?:parse|stringify)",
        "must expose byte-oriented privacy archives",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep SDK binary-only FFI coverage for {snippet}",
            errors,
        )


def check_privacy_native_host_norito_only_ffi_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("native privacy FFI hosts remain Norito-only and JSON-free"',
        "C bridge privacy FFI",
        "JS NAPI privacy FFI",
        "Python PyO3 privacy FFI",
        "norito::decode_from_bytes",
        "norito::to_bytes",
        "write_privacy_payload",
        "slice::from_raw_parts",
        "encode_privacy_archive",
        "encode_privacy_archive_py",
        "PyBytes|&\\[u8\\]",
        "bytes\\.len\\(\\)\\s*>\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "java_privacy_capabilities_archive",
        "read_java_byte_array",
        "byte_array_from_slice",
        "must decode Norito request archives",
        "must encode Norito result archives",
        "must not parse or render JSON payloads",
        "Java privacy JNI adapter must not parse or render JSON payloads",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native host Norito-only FFI coverage for {snippet}",
            errors,
        )


def check_privacy_capability_metadata_shape_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")
    direct_algorithm_capability_pattern = re.compile(
        r"\b(?:anonymousPgc|AnonymousPgc|verange|VeRange|zkat|ZkAt|zkAce|ZkAce|"
        r"zkAms|ZkAms|vega|Vega|silentThreshold|SilentThreshold|zkX509|ZkX509|"
        r"jindo|Jindo|sisHints|SisHints|orchard|Orchard|penumbra|Penumbra|"
        r"fcmp|Fcmp|miden|Miden|aztec|Aztec|pqMasp|PqMasp|mlKem|MlKem|"
        r"assetHiddenTransferProof|AssetHiddenTransferProof|build[A-Z]|verify[A-Z])"
        r"[A-Za-z0-9_]*\b"
    )
    capability_model_sources = (
        (
            "Swift",
            read("IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"),
            r"public struct PrivacyCapabilities[\s\S]*?\{([\s\S]*?)\n\}\n\npublic enum PrivacyNativeBridge",
        ),
        (
            "Kotlin",
            read("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/privacy/PrivacyNativeBridge.kt"),
            r"class PrivacyCapabilities private constructor\(([\s\S]*?)\)\s*\{([\s\S]*?)\n    class PrivacyProductionGate",
        ),
        (
            "C#",
            read("csharp/src/Hyperledger.Iroha.Sdk/Privacy/PrivacyNative.cs"),
            r"public sealed class PrivacyCapabilities([\s\S]*?)\n}\n\npublic sealed class PrivacyProductionGate",
        ),
        (
            "Java",
            read("java/iroha_android/src/main/java/org/hyperledger/iroha/android/privacy/PrivacyNativeBridge.java"),
            r"public static final class PrivacyCapabilities \{([\s\S]*?)\n  \}\n\n  private static native int nativeBridgeAbiVersion",
        ),
    )

    for snippet in (
        'test("mobile and C# privacy capability models stay coarse and fail-closed"',
        "EXPECTED_SWIFT_PRIVACY_CAPABILITY_FIELDS",
        "EXPECTED_KOTLIN_PRIVACY_CAPABILITY_FIELDS",
        "EXPECTED_CSHARP_PRIVACY_CAPABILITY_FIELDS",
        "EXPECTED_JAVA_PRIVACY_CAPABILITY_FIELDS",
        "public struct PrivacyCapabilities",
        "class PrivacyCapabilities private constructor",
        "public sealed class PrivacyCapabilities",
        "public static final class PrivacyCapabilities",
        "productionReady",
        "productionGate",
        "assertNoDirectAlgorithmCapabilityFields",
        "zkAce|ZkAce",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep capability metadata shape parity coverage for {snippet}",
            errors,
        )
    for label, text, pattern in capability_model_sources:
        match = re.search(pattern, text)
        require(match is not None, f"{label} privacy capability model must be source-checkable", errors)
        if match is None:
            continue
        body = "\n".join(group for group in match.groups() if group is not None)
        require(
            direct_algorithm_capability_pattern.search(body) is None,
            f"{label} privacy capabilities must not expose direct algorithm capability fields",
            errors,
        )


def check_privacy_capability_fail_closed_metadata_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("mobile and C# privacy capability models stay coarse and fail-closed"',
        "productionReady",
        "productionGate",
        "productionReady\\s*=\\s*false",
        "productionGate\\s*=\\s*\\.failClosed",
        "PrivacyProductionGate\\.failClosed\\(\\)",
        "PrivacyProductionGate\\.FailClosed\\(\\)",
        "this\\.productionReady\\s*=\\s*false;",
        "this\\.missingProductionGates\\s*=\\s*PRODUCTION_GATE_MISSING;",
        "this\\.auditReferences\\s*=\\s*PRODUCTION_GATE_AUDIT_REFERENCES;",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep capability fail-closed metadata coverage for {snippet}",
            errors,
        )


def check_privacy_norito_struct_schema_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI Norito schema and proof operation set stay in parity"',
        "EXPECTED_PRIVACY_PRODUCTION_GATE_STATUS_FIELDS",
        "EXPECTED_PRIVACY_PRODUCTION_GATE_FIELDS",
        "EXPECTED_PRIVACY_CAPABILITY_FIELDS",
        "EXPECTED_PRIVACY_CAPABILITIES_FIELDS",
        "EXPECTED_PRIVACY_PROOF_REQUEST_FIELDS",
        "EXPECTED_PRIVACY_PROOF_RESULT_FIELDS",
        "assertRustStructIsNorito",
        "PrivacyProductionGateStatusV1",
        "PrivacyProductionGateV1",
        "PrivacyCapabilityV1",
        "PrivacyCapabilitiesV1",
        "PrivacyProofRequestV1",
        "PrivacyProofResultV1",
        "connect_norito_bridge",
        "iroha_js_host",
        "iroha_python_rs",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Norito struct schema parity coverage for {snippet}",
            errors,
        )


def check_privacy_norito_operation_variant_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("privacy FFI Norito schema and proof operation set stay in parity"',
        "EXPECTED_PRIVACY_OPERATION_VARIANTS",
        "PrivacyProofOperationV1",
        "connect_norito_bridge",
        "iroha_js_host",
        "iroha_python_rs",
        "rustEnumVariants",
        "Build",
        "Verify",
        "privacy proof operation set drifted",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep Norito operation variant parity coverage for {snippet}",
            errors,
        )


def check_privacy_catalog_production_gate_missing_reason_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy production gate missing reasons stay in cross-SDK parity"',
        "EXPECTED_SDK_PRIVACY_PRODUCTION_GATE_MISSING_REASONS",
        "real proving engine is not registered",
        "real verifier is not registered",
        "chain admission path is not enabled",
        "cross-SDK parity is incomplete",
        "wallet/state support is incomplete",
        "deterministic tests are incomplete",
        "fuzzing gate is incomplete",
        "performance gate is incomplete",
        "external audit signoff is missing",
        "implementation stage is not production-hardened",
        "planned SDK entrypoints remain",
        "dev fixture entrypoints are not production entrypoints",
        "Iroha production allowlist is not enabled for this audited row",
        "extractJsCatalogProductionGateMissingReasons",
        "extractPythonCatalogProductionGateMissingReasons",
        "JS source privacy catalog",
        "JS dist privacy catalog",
        "Python privacy catalog",
        "production gate missing reasons drifted",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep catalog production-gate missing-reason parity coverage for {snippet}",
            errors,
        )


def check_privacy_native_production_gate_missing_reason_parity_coverage(errors):
    ffi_parity = read("javascript/iroha_js/test/privacyFfiContractParity.test.js")

    for snippet in (
        'test("SDK privacy production gate missing reasons stay in cross-SDK parity"',
        "EXPECTED_SDK_PRIVACY_PRODUCTION_GATE_MISSING_REASONS",
        "assertProductionGateMissingReasons",
        "Swift privacy native bridge",
        "Kotlin privacy native bridge",
        "Android Java privacy native bridge",
        "C# privacy native bridge",
        "public static let missingReasons",
        "MISSING_REASONS",
        "PRODUCTION_GATE_MISSING",
        "Array\\.AsReadOnly",
        "Collections\\.unmodifiableList",
        "external audit signoff is missing",
        "Iroha production allowlist is not enabled for this audited row",
        "production gate missing reasons drifted",
    ):
        require(
            snippet in ffi_parity,
            f"Privacy FFI parity tests must keep native production-gate missing-reason parity coverage for {snippet}",
            errors,
        )


def check_workflow_paths(errors):
    paths = workflow_trigger_paths()
    missing = [
        relative
        for relative in required_paths
        if not any(fnmatchcase(relative, pattern) for pattern in paths)
    ]
    require(
        not missing,
        "Privacy SDK guard workflow paths do not cover required sources: "
        + ", ".join(missing),
        errors,
    )


def check_workflow_commands(errors):
    workflow = read(workflow_path)
    main_match = re.search(rf"(?m)^\s+run:\s+{re.escape(main_command)}\s*$", workflow)
    bridge_header_match = workflow_command_match(workflow, bridge_header_command)
    bytecode_match = workflow_command_match(workflow, bytecode_command)
    main_job_block = workflow_job_block(workflow, main_job)
    require(
        bridge_header_match is not None,
        "Privacy SDK guard workflow must run ci/check_connect_norito_bridge_header.sh",
        errors,
    )
    require(
        main_match is not None,
        "Privacy SDK guard workflow must run ci/check_privacy_sdk_guard.sh",
        errors,
    )
    require(
        bytecode_match is not None,
        "Privacy SDK guard workflow must reject tracked Python bytecode artifacts",
        errors,
    )
    if main_match is None:
        return
    require(
        main_job_block is not None,
        "Privacy SDK guard workflow must define the aggregate guard job",
        errors,
    )
    if main_job_block is not None:
        require(
            re.search(r"(?m)^\s+timeout-minutes:\s+45\s*$", main_job_block) is not None,
            "Privacy SDK guard workflow must allow 45 minutes for native Python builds",
            errors,
        )
        require(
            re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", main_job_block)
            is not None,
            "Privacy SDK guard workflow must cache Rust artifacts for aggregate native Python builds",
            errors,
        )
    if bridge_header_match is not None:
        require(
            bridge_header_match.start() < main_match.start(),
            "Privacy SDK guard workflow must run the bridge header parity check before the main guard",
            errors,
        )
    for label, command in bridge_header_drift_commands:
        command_match = workflow_command_match(workflow, command)
        require(
            command_match is not None,
            f"Privacy SDK guard workflow must run the bridge header {label}",
            errors,
        )
        if command_match is not None:
            require(
                command_match.start() < main_match.start(),
                f"Privacy SDK guard workflow must run the bridge header {label} before the main guard",
                errors,
            )
    if bytecode_match is not None:
        require(
            bytecode_match.start() < main_match.start(),
            "Privacy SDK guard workflow must reject tracked Python bytecode before the main guard",
            errors,
        )
    for label, command in negative_control_commands:
        command_match = workflow_command_match(workflow, command)
        require(
            command_match is not None,
            f"Privacy SDK guard workflow must run the {label}",
            errors,
        )
        if command_match is not None:
            require(
                command_match.start() < main_match.start(),
                f"Privacy SDK guard workflow must run the {label} before the main guard",
                errors,
            )


def check_workflow_cancellation_policy(errors):
    workflow = read(workflow_path)
    require(
        re.search(r"(?m)^\s+cancel-in-progress:\s+false\s*$", workflow) is not None,
        "Privacy SDK guard workflow must keep cancel-in-progress disabled",
        errors,
    )


def check_workflow_runs_native_bridge_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, native_bridge_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the native bridge test job",
        errors,
    )
    if job_block is None:
        return
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run native bridge tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Privacy SDK guard workflow must cache Rust artifacts for native bridge tests",
        errors,
    )
    require(
        workflow_command_match(job_block, native_bridge_command) is not None,
        "Privacy SDK guard workflow must run the native privacy bridge tests",
        errors,
    )
    require(
        native_bridge_job in main_needs,
        "Privacy SDK guard job must wait for the native bridge test job",
        errors,
    )


def check_workflow_runs_csharp_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, csharp_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the C# SDK test job",
        errors,
    )
    if job_block is None:
        return
    dotnet_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-dotnet@v4\s*$", job_block)
    dotnet_version_match = re.search(r"(?m)^\s+dotnet-version:\s+8\.0\.x\s*$", job_block)
    command_match = workflow_command_match(job_block, csharp_sdk_command)
    require(
        dotnet_setup_match is not None,
        "Privacy SDK guard workflow must set up dotnet for C# SDK tests",
        errors,
    )
    require(
        dotnet_version_match is not None,
        "Privacy SDK guard workflow must pin dotnet 8 for C# SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Privacy SDK guard workflow must run the C# privacy SDK tests",
        errors,
    )
    if dotnet_setup_match is not None and command_match is not None:
        require(
            dotnet_setup_match.start() < command_match.start(),
            "Privacy SDK guard workflow must set up dotnet before running C# SDK tests",
            errors,
        )
    require(
        csharp_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the C# SDK test job",
        errors,
    )


def check_csharp_sdk_script_prints_dotnet_version(errors):
    script = read(csharp_sdk_command)
    require(
        'DOTNET_BIN="${PRIVACY_CSHARP_DOTNET_BIN:-dotnet}"' in script,
        "Privacy C# SDK script must keep the documented dotnet override variable",
        errors,
    )
    require(
        'DOTNET_VERSION="$("${DOTNET_BIN}" --version)"' in script,
        "Privacy C# SDK script must print the selected dotnet version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${DOTNET_VERSION}"' in script,
        "Privacy C# SDK script must emit the selected dotnet version",
        errors,
    )
    require(
        "8.0.*) ;;" in script,
        "Privacy C# SDK script must reject non-.NET-8 SDK versions",
        errors,
    )


def check_workflow_runs_swift_sdk_parse(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, swift_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the Swift SDK parse job",
        errors,
    )
    if job_block is None:
        return
    require(
        re.search(r"(?m)^\s+runs-on:\s+macos-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run Swift SDK parsing on macOS",
        errors,
    )
    require(
        workflow_command_match(job_block, swift_sdk_command) is not None,
        "Privacy SDK guard workflow must run the Swift privacy SDK parse check",
        errors,
    )
    require(
        swift_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the Swift SDK parse job",
        errors,
    )


def check_swift_sdk_script_prints_swiftc_version(errors):
    script = read(swift_sdk_command)
    require(
        'SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"' in script,
        "Privacy Swift SDK script must keep the documented swiftc override variable",
        errors,
    )
    require(
        '"${SWIFTC_BIN}" --version' in script,
        "Privacy Swift SDK script must print the selected swiftc version",
        errors,
    )


def check_workflow_runs_jvm_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, jvm_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the JVM SDK test job",
        errors,
    )
    if job_block is None:
        return
    java_setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-java@v4\s*$", job_block)
    java_distribution_match = re.search(r'(?m)^\s+distribution:\s+"temurin"\s*$', job_block)
    java_version_match = re.search(r'(?m)^\s+java-version:\s+"21"\s*$', job_block)
    command_match = workflow_command_match(job_block, jvm_sdk_command)
    require(
        java_setup_match is not None,
        "Privacy SDK guard workflow must set up Java for JVM SDK tests",
        errors,
    )
    require(
        java_distribution_match is not None,
        "Privacy SDK guard workflow must pin Temurin for JVM SDK tests",
        errors,
    )
    require(
        java_version_match is not None,
        "Privacy SDK guard workflow must pin Java 21 for JVM SDK tests",
        errors,
    )
    require(
        command_match is not None,
        "Privacy SDK guard workflow must run the JVM privacy SDK tests",
        errors,
    )
    if java_setup_match is not None and command_match is not None:
        require(
            java_setup_match.start() < command_match.start(),
            "Privacy SDK guard workflow must set up Java before running JVM SDK tests",
            errors,
        )
    require(
        jvm_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the JVM SDK test job",
        errors,
    )


def check_workflow_runs_javascript_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, js_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the JavaScript SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-node@v4\s*$", job_block)
    install_match = workflow_command_match(job_block, js_sdk_install_command)
    test_match = workflow_command_match(job_block, js_sdk_command)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run JavaScript SDK tests on Ubuntu",
        errors,
    )
    require(
        setup_match is not None,
        "Privacy SDK guard workflow must set up Node for JavaScript SDK tests",
        errors,
    )
    require(
        re.search(r'(?m)^\s+node-version:\s+"20"\s*$', job_block) is not None,
        "Privacy SDK guard workflow must pin Node 20 for JavaScript SDK tests",
        errors,
    )
    require(
        re.search(
            r"(?m)^\s+cache-dependency-path:\s+javascript/iroha_js/package-lock\.json\s*$",
            job_block,
        )
        is not None,
        "Privacy SDK guard workflow must cache JavaScript SDK dependencies by package-lock",
        errors,
    )
    require(
        install_match is not None,
        "Privacy SDK guard workflow must install JavaScript SDK dependencies",
        errors,
    )
    require(
        test_match is not None,
        "Privacy SDK guard workflow must run the JavaScript privacy SDK tests",
        errors,
    )
    if setup_match is not None and install_match is not None:
        require(
            setup_match.start() < install_match.start(),
            "Privacy SDK guard workflow must set up Node before installing JavaScript SDK dependencies",
            errors,
        )
    if install_match is not None and test_match is not None:
        require(
            install_match.start() < test_match.start(),
            "Privacy SDK guard workflow must install JavaScript SDK dependencies before running JavaScript tests",
            errors,
        )
    require(
        js_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the JavaScript SDK test job",
        errors,
    )


def check_javascript_sdk_script_prints_node_version(errors):
    script = read(js_sdk_command)
    require(
        'NODE_OVERRIDE="${PRIVACY_JS_SDK_NODE_BIN:-}"' in script,
        "Privacy JavaScript SDK script must keep the documented Node override variable",
        errors,
    )
    require(
        "resolve_node_20_bin()" in script and "is_node_20_bin()" in script,
        "Privacy JavaScript SDK script must resolve Node 20 before falling back to node",
        errors,
    )
    require(
        'NODE_BIN="$(resolve_node_20_bin)"' in script,
        "Privacy JavaScript SDK script must use the Node 20 resolver",
        errors,
    )
    require(
        'NODE_VERSION="$("${NODE_BIN}" --version)"' in script,
        "Privacy JavaScript SDK script must print the selected Node version",
        errors,
    )
    require(
        'printf \'%s\\n\' "${NODE_VERSION}"' in script,
        "Privacy JavaScript SDK script must emit the selected Node version",
        errors,
    )
    require(
        "v20.*) ;;" in script,
        "Privacy JavaScript SDK script must reject non-Node-20 runtimes",
        errors,
    )
    require(
        "export PYTHONDONTWRITEBYTECODE=1" in script,
        "Privacy JavaScript SDK script must suppress Python bytecode for spawned Python catalog loaders",
        errors,
    )


def check_workflow_runs_python_sdk_tests(errors):
    workflow = read(workflow_path)
    job_block = workflow_job_block(workflow, python_sdk_job)
    main_needs = workflow_job_needs(workflow, main_job)
    require(
        job_block is not None,
        "Privacy SDK guard workflow must define the Python SDK test job",
        errors,
    )
    if job_block is None:
        return
    setup_match = re.search(r"(?m)^\s+- uses:\s+actions/setup-python@v5\s*$", job_block)
    test_match = workflow_command_match(job_block, python_sdk_command)
    require(
        re.search(r"(?m)^\s+runs-on:\s+ubuntu-latest\s*$", job_block) is not None,
        "Privacy SDK guard workflow must run Python SDK tests on Ubuntu",
        errors,
    )
    require(
        re.search(r"(?m)^\s+timeout-minutes:\s+45\s*$", job_block) is not None,
        "Privacy SDK guard workflow must allow 45 minutes for Python native builds",
        errors,
    )
    require(
        re.search(r"(?m)^\s+- uses:\s+Swatinem/rust-cache@v2\s*$", job_block) is not None,
        "Privacy SDK guard workflow must cache Rust artifacts for Python native builds",
        errors,
    )
    require(
        setup_match is not None,
        "Privacy SDK guard workflow must set up Python for Python SDK tests",
        errors,
    )
    require(
        re.search(r'(?m)^\s+python-version:\s+"3\.11"\s*$', job_block) is not None,
        "Privacy SDK guard workflow must pin Python 3.11 for Python SDK tests",
        errors,
    )
    require(
        test_match is not None,
        "Privacy SDK guard workflow must run the Python privacy SDK tests",
        errors,
    )
    if setup_match is not None and test_match is not None:
        require(
            setup_match.start() < test_match.start(),
            "Privacy SDK guard workflow must set up Python before running Python SDK tests",
            errors,
        )
    require(
        python_sdk_job in main_needs,
        "Privacy SDK guard job must wait for the Python SDK test job",
        errors,
    )


def check_python_sdk_script_prints_python_version(errors):
    script = read(python_sdk_command)
    require(
        'PYTHON_OVERRIDE="${PRIVACY_PYTHON_SDK_PYTHON_BIN:-}"' in script,
        "Privacy Python SDK script must keep the documented Python override variable",
        errors,
    )
    require(
        "resolve_python_311_bin()" in script and "python3.11" in script,
        "Privacy Python SDK script must resolve Python 3.11 before falling back to python3",
        errors,
    )
    require(
        'PYTHON_BIN="$(resolve_python_311_bin)"' in script,
        "Privacy Python SDK script must use the Python 3.11 resolver",
        errors,
    )
    require(
        'PYTHON_VERSION="$("${PYTHON_BIN}" -c' in script,
        "Privacy Python SDK script must capture the selected Python version",
        errors,
    )
    require(
        '"${PYTHON_BIN}" --version' in script,
        "Privacy Python SDK script must print the selected Python version",
        errors,
    )
    require(
        'VENV_PYTHON_VERSION="$("${VENV_DIR}/bin/python" -c' in script,
        "Privacy Python SDK script must capture the venv Python version",
        errors,
    )
    require(
        script.count('"${VENV_DIR}/bin/python" --version') >= 2,
        "Privacy Python SDK script must print the initial and rebuilt venv Python versions",
        errors,
    )
    require(
        "3.11) ;;" in script,
        "Privacy Python SDK script must reject non-Python-3.11 runtimes",
        errors,
    )
    require(
        "recreating privacy Python SDK venv" in script,
        "Privacy Python SDK script must rebuild stale non-3.11 venvs",
        errors,
    )
    require(
        'rm -rf "${VENV_DIR}"' in script,
        "Privacy Python SDK script must remove stale venvs before rebuilding",
        errors,
    )
    require(
        "'maturin>=1.5,<2'" in script,
        "Privacy Python SDK script must install maturin for native extension builds",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m pip install --no-deps' in script
        and '"${ROOT_DIR}/python/norito_py"' in script
        and '"${ROOT_DIR}/python/iroha_torii_client"' in script,
        "Privacy Python SDK script must install local workspace Python packages before maturin",
        errors,
    )
    require(
        'export VIRTUAL_ENV="${VENV_DIR}"' in script
        and 'export PATH="${VENV_DIR}/bin:${PATH}"' in script,
        "Privacy Python SDK script must activate the selected venv before maturin",
        errors,
    )
    require(
        '"${VENV_DIR}/bin/python" -m maturin develop --release' in script,
        "Privacy Python SDK script must build the native extension with the selected Python",
        errors,
    )
    require(
        "export PYTHONDONTWRITEBYTECODE=1" in script,
        "Privacy Python SDK script must not write bytecode cache files during tests",
        errors,
    )


def check_jvm_sdk_script_pins_jdk21(errors):
    script = read(jvm_sdk_command)
    require(
        'JAVA_HOME_OVERRIDE="${PRIVACY_JVM_SDK_JAVA_HOME:-}"' in script,
        "Privacy JVM SDK script must keep the documented Java home override variable",
        errors,
    )
    require(
        "PRIVACY_JVM_SDK_JAVA_HOME must point to a JDK 21 home." in script,
        "Privacy JVM SDK script must reject invalid explicit Java home overrides",
        errors,
    )
    require(
        "JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests." in script,
        "Privacy JVM SDK script must reject inherited non-JDK-21 JAVA_HOME values",
        errors,
    )
    require(
        "is_java_21_home()" in script,
        "Privacy JVM SDK script must validate Java homes as JDK 21",
        errors,
    )
    require(
        'version[[:space:]]+\\"21(\\.|\\")' in script,
        "Privacy JVM SDK script must match Java 21 version output",
        errors,
    )
    require(
        "java -version" in script,
        "Privacy JVM SDK script must print the selected Java version",
        errors,
    )


def run_checks():
    errors = []
    check_readmes(errors)
    check_zk_ace_proof_builder_coverage(errors)
    check_public_privacy_required_production_plan_rows_coverage(errors)
    check_python_catalog_loader_bytecode_guards(errors)
    check_negative_control_inventory_parity_test(errors)
    check_source_reference_obfuscated_ipv4_coverage(errors)
    check_source_reference_audit_readiness_url_coverage(errors)
    check_source_reference_encoded_host_url_coverage(errors)
    check_dev_fixture_entrypoint_fail_closed_coverage(errors)
    check_privacy_catalog_defensive_copy_coverage(errors)
    check_planned_privacy_entrypoint_quarantine_coverage(errors)
    check_native_privacy_catalog_parity_coverage(errors)
    check_native_privacy_executable_entrypoint_allowlist_coverage(errors)
    check_native_privacy_planned_entrypoint_rejection_coverage(errors)
    check_native_privacy_catalog_identifier_structure_coverage(errors)
    check_native_privacy_required_production_plan_rows_coverage(errors)
    check_native_privacy_verifier_key_registration_coverage(errors)
    check_native_privacy_public_catalog_parity_coverage(errors)
    check_native_privacy_component_rows_proof_only_coverage(errors)
    check_native_privacy_planned_ledger_mutation_proof_builder_coverage(errors)
    check_native_privacy_proofed_sdk_ledger_mutation_pairing_coverage(errors)
    check_native_privacy_production_gate_state_fail_closed_coverage(errors)
    check_native_privacy_capability_claim_quarantine_coverage(errors)
    check_native_privacy_capability_archive_invariant_coverage(errors)
    check_privacy_ffi_build_empty_public_inputs_coverage(errors)
    check_privacy_ffi_verify_empty_public_inputs_coverage(errors)
    check_privacy_ffi_operation_shadow_material_coverage(errors)
    check_privacy_ffi_operation_required_material_coverage(errors)
    check_privacy_ffi_non_proof_entrypoint_rejection_coverage(errors)
    check_privacy_ffi_vk_ref_shape_hardening_coverage(errors)
    check_privacy_ffi_vk_ref_backend_binding_coverage(errors)
    check_privacy_ffi_vk_ref_name_binding_coverage(errors)
    check_privacy_ffi_production_disabled_build_gate_message_coverage(errors)
    check_privacy_ffi_production_disabled_verify_gate_message_coverage(errors)
    check_privacy_ffi_production_disabled_message_constant_coverage(errors)
    check_privacy_ffi_failure_result_invariant_coverage(errors)
    check_privacy_ffi_witness_helper_nonreflection_coverage(errors)
    check_privacy_ffi_witness_nonreflection_coverage(errors)
    check_privacy_ffi_proof_nonreflection_coverage(errors)
    check_privacy_ffi_request_nonreflection_coverage(errors)
    check_privacy_ffi_request_text_field_enumerator_coverage(errors)
    check_privacy_ffi_oversized_request_field_nonreflection_coverage(errors)
    check_privacy_ffi_oversized_public_inputs_nonreflection_coverage(errors)
    check_privacy_ffi_oversized_witness_nonreflection_coverage(errors)
    check_privacy_ffi_oversized_proof_nonreflection_coverage(errors)
    check_privacy_ffi_control_algorithm_entrypoint_nonreflection_coverage(errors)
    check_privacy_ffi_control_vk_ref_nonreflection_coverage(errors)
    check_privacy_ffi_non_ascii_algorithm_entrypoint_nonreflection_coverage(errors)
    check_privacy_ffi_non_ascii_vk_ref_nonreflection_coverage(errors)
    check_privacy_ffi_unportable_algorithm_entrypoint_nonreflection_coverage(errors)
    check_privacy_ffi_unportable_vk_ref_nonreflection_coverage(errors)
    check_privacy_ffi_request_algorithm_catalog_shape_nonreflection_coverage(errors)
    check_privacy_ffi_request_entrypoint_catalog_shape_nonreflection_coverage(errors)
    check_privacy_ffi_required_algorithm_entrypoint_nonreflection_coverage(errors)
    check_privacy_ffi_required_vk_ref_nonreflection_coverage(errors)
    check_privacy_ffi_request_algorithm_entrypoint_production_claim_nonreflection_coverage(errors)
    check_privacy_ffi_request_vk_ref_production_claim_nonreflection_coverage(errors)
    check_privacy_ffi_public_operation_schema_coverage(errors)
    check_cross_sdk_privacy_operation_schema_coverage(errors)
    check_privacy_native_availability_archive_hardening_coverage(errors)
    check_privacy_native_availability_probe_gating_coverage(errors)
    check_cross_sdk_privacy_malformed_request_dispatch_boundary_coverage(errors)
    check_cross_sdk_privacy_sliced_view_boundary_coverage(errors)
    check_cross_sdk_privacy_native_output_boundary_coverage(errors)
    check_privacy_norito_request_padding_coverage(errors)
    check_privacy_norito_request_field_bitset_coverage(errors)
    check_privacy_norito_wrong_schema_request_coverage(errors)
    check_privacy_sdk_request_decoder_bounds_coverage(errors)
    check_privacy_c_bridge_output_buffer_precedence_coverage(errors)
    check_privacy_native_adversarial_request_frame_coverage(errors)
    check_privacy_request_copy_zeroization_coverage(errors)
    check_python_privacy_native_method_surface_coverage(errors)
    check_privacy_native_abi_probe_bounds_coverage(errors)
    check_zk_ace_public_proof_builder_native_error_sanitizer_coverage(errors)
    check_privacy_hostile_request_mutation_isolation_coverage(errors)
    check_privacy_malformed_request_no_dispatch_coverage(errors)
    check_privacy_public_wrapper_isolation_coverage(errors)
    check_privacy_public_archive_wrapper_norito_coverage(errors)
    check_privacy_backend_alias_fail_closed_coverage(errors)
    check_privacy_chain_backend_allowlist_coverage(errors)
    check_privacy_ffi_c_symbol_surface_coverage(errors)
    check_privacy_ffi_binding_loader_surface_coverage(errors)
    check_privacy_ffi_error_contract_parity_coverage(errors)
    check_privacy_native_archive_max_parity_coverage(errors)
    check_privacy_sdk_bridge_method_surface_coverage(errors)
    check_privacy_sdk_binary_only_ffi_coverage(errors)
    check_privacy_native_host_norito_only_ffi_coverage(errors)
    check_privacy_catalog_production_gate_missing_reason_parity_coverage(errors)
    check_privacy_native_production_gate_missing_reason_parity_coverage(errors)
    check_privacy_norito_struct_schema_parity_coverage(errors)
    check_privacy_norito_operation_variant_parity_coverage(errors)
    check_privacy_capability_metadata_shape_parity_coverage(errors)
    check_privacy_capability_fail_closed_metadata_coverage(errors)
    check_workflow_paths(errors)
    check_workflow_commands(errors)
    check_workflow_cancellation_policy(errors)
    check_workflow_runs_native_bridge_tests(errors)
    check_workflow_runs_swift_sdk_parse(errors)
    check_swift_sdk_script_prints_swiftc_version(errors)
    check_workflow_runs_jvm_sdk_tests(errors)
    check_workflow_runs_csharp_sdk_tests(errors)
    check_csharp_sdk_script_prints_dotnet_version(errors)
    check_workflow_runs_javascript_sdk_tests(errors)
    check_javascript_sdk_script_prints_node_version(errors)
    check_workflow_runs_python_sdk_tests(errors)
    check_jvm_sdk_script_pins_jdk21(errors)
    check_python_sdk_script_prints_python_version(errors)
    if errors:
        raise PrivacyGuardError("\n".join(errors))


if mode == "--negative-control-bridge-header-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "ci/check_connect_norito_bridge_header.sh"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header workflow path drift was not detected")

if mode == "--negative-control-bridge-header-command-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {bridge_header_command}",
        "        run: ci/check_connect_norito_bridge_header.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header workflow command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header workflow command drift was not detected")

if mode == "--negative-control-bridge-header-negative-controls-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "ci/check_connect_norito_bridge_header.sh --negative-control-bad-privacy-signature",
        "ci/check_connect_norito_bridge_header.sh --synthetic-bad-privacy-signature-check",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate bridge header negative-control workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy bridge header negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: bridge header negative-control workflow drift was not detected")

if mode == "--negative-control-bytecode-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - name: Reject tracked Python bytecode\n"
        f"        run: {bytecode_command}\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate tracked Python bytecode workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected tracked Python bytecode workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: tracked Python bytecode workflow drift was not detected")

if mode == "--negative-control-workflow-path":
    original = read(workflow_path)
    mutated = original.replace('      - "python/iroha_python/README.md"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow path drift was not detected")

if mode == "--negative-control-workflow-command":
    original = read(workflow_path)
    mutated = original.replace(
        "        run: ci/check_privacy_sdk_guard.sh",
        "        run: ci/check_privacy_sdk_guard.sh --skip-main-guard",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow guard command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow command drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow command drift was not detected")

if mode == "--negative-control-backend-tag-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate backend-tag workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy backend-tag workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: backend-tag workflow path drift was not detected")

if mode == "--negative-control-browser-test-workflow-path":
    original = read(workflow_path)
    mutated = original.replace(
        '      - "javascript/iroha_js/test/crypto.browser.test.js"\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser-test workflow path coverage")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy browser-test workflow path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser-test workflow path drift was not detected")

if mode == "--negative-control-negative-controls-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          ci/check_privacy_sdk_guard.sh --negative-control-readme-api",
        "          ci/check_privacy_sdk_guard.sh --synthetic-readme-api-check",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: negative-control workflow drift was not detected")

if mode == "--negative-control-negative-controls-comment-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
        "          # ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to comment workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected commented privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: commented negative-control workflow drift was not detected")

if mode == "--negative-control-negative-controls-order-workflow":
    original = read(workflow_path)
    command_line = "          ci/check_privacy_sdk_guard.sh --negative-control-readme-error-code\n"
    without_command = original.replace(command_line, "", 1)
    if without_command == original:
        raise SystemExit("negative control failed: unable to remove workflow negative-control command")
    mutated = without_command.replace(
        "        run: ci/check_privacy_sdk_guard.sh\n",
        "        run: ci/check_privacy_sdk_guard.sh\n" + command_line,
        1,
    )
    if mutated == without_command:
        raise SystemExit("negative control failed: unable to move workflow negative-control command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected late privacy negative-control workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: late negative-control workflow drift was not detected")

if mode == "--negative-control-negative-controls-inventory-parity":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "  const privacySdkGuardNegativeControlModes = negativeControlModesFromInventory(\n",
        "  const privacySdkGuardNegativeControlModes = [\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate negative-control inventory parity")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy negative-control inventory parity drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: negative-control inventory parity drift was not detected")

if mode == "--negative-control-readme-boundary":
    target = "javascript/iroha_js/README.md"
    original = read(target)
    mutated = original.replace("privacy-production-gate-v1", "privacy-production-open-v1", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README boundary")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README boundary drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README boundary drift was not detected")

if mode == "--negative-control-readme-api":
    target = "python/iroha_python/README.md"
    original = read(target)
    mutated = original.replace(
        "build_zk_ace_authorization_proof_v1()",
        "build_zk_ace_authorization_claim_v1()",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README API name")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README API drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README API drift was not detected")

if mode == "--negative-control-zk-ace-proof-builder-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "  assertPythonZkAceProofBuilderCoverage();",
        "  assertPythonZkAceProofBuilderCoverageSkipped();",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate ZK-ACE proof-builder coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected ZK-ACE proof-builder coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: ZK-ACE proof-builder coverage drift was not detected")

if mode == "--negative-control-zk-ace-production-gate-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "ZK-ACE production gate must stay fail-closed despite the STARK/FRI verifier profile allowlist",
        "ZK-ACE production gate may open after the STARK/FRI verifier profile allowlist",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate ZK-ACE production-gate fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected ZK-ACE production-gate fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: ZK-ACE production-gate fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-python-zk-ace-production-gate-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "python ZK-ACE descriptor must exist",
        "python ZK-ACE descriptor may be skipped",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python ZK-ACE production-gate fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python ZK-ACE production-gate fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python ZK-ACE production-gate fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-python-direct-zk-ace-production-gate-fail-closed-coverage":
    target = "python/iroha_python/tests/privacy_catalog_test.py"
    original = read(target)
    mutated = original.replace(
        'assert zk_ace["proof_family"] == "stark/fri/sha256-goldilocks"',
        'assert zk_ace["proof_family"] == "stark/fri"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python direct ZK-ACE production-gate fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python direct ZK-ACE production-gate fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python direct ZK-ACE production-gate fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-python-zk-ace-capability-production-gate-fail-closed-coverage":
    target = "python/iroha_python/tests/privacy_catalog_test.py"
    original = read(target)
    mutated = original.replace(
        'assert zk_ace_capability["proof_family"] == "stark/fri/sha256-goldilocks"',
        'assert zk_ace_capability["proof_family"] == "stark/fri"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python ZK-ACE capability production-gate fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python ZK-ACE capability production-gate fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: Python ZK-ACE capability production-gate fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-js-zk-ace-capability-production-gate-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "    assertZkAceCapabilitySurfaceFailClosed(label, capabilities);",
        "    assertZkAceCapabilitySurfaceFailClosedSkipped(label, capabilities);",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JS ZK-ACE capability production-gate fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected JS ZK-ACE capability production-gate fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: JS ZK-ACE capability production-gate fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-rows-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '  Object.freeze(["zk-ace-pq-authorization-v0", "chain-executable", "stark-fri"]),\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan rows coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan rows coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan rows coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-exact-row-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '  Object.freeze(["anonymous-pgc-k-out-of-n-v1", "sdk-builder", "anonymous-pgc"]),',
        '  Object.freeze(["anonymous-pgc-k-out-of-n-v1", "component", "anonymous-pgc"]),',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan exact row coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan exact row drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan exact row drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-row-order-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    row = '  Object.freeze(["anonymous-pgc-k-out-of-n-v1", "sdk-builder", "anonymous-pgc"]),\n'
    mutated = original.replace(row, row + row, 1)
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan row order coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan row order drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan row order drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-display-text-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"Anonymous PGC k-out-of-n payments v1"',
        '"Anonymous PGC pilot payments v1"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan display-text coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan display-text drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan display-text drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-category-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": "payment"',
        '"anonymous-pgc-k-out-of-n-v1": "authorization"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan category coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan category coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan category coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-maturity-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": "accepted_conference"',
        '"anonymous-pgc-k-out-of-n-v1": "specification"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan maturity coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan maturity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan maturity coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-recommended-for-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"account-based private payments"',
        '"claimed production rollout"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan recommendedFor coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan recommendedFor coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan recommendedFor coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-covered-criteria-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver"])',
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["hide_amount", "hide_sender", "hide_receiver", "hide_asset_type"])',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan covered-criteria coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan covered-criteria coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan covered-criteria coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-proof-family-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"stark/fri/sha256-goldilocks"',
        '"stark/fri/claimed-goldilocks"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan proof-family coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan proof-family coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan proof-family coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-public-input-schema-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "replay_nullifier",
        "forged_public_input",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan public-input schema coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan public-input schema coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan public-input schema coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-verifier-key-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"zk_ace_pq_authorization_v0"',
        '"zk_ace_claimed_authorization_v0"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan verifier-key coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan verifier-key coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan verifier-key coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-state-token-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"replay nullifier set"',
        '"forged replay state placeholder"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan state-token coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan state-token coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan state-token coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-failure-mode-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"replayed nullifier"',
        '"forged replay failure placeholder"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan failure-mode coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan failure-mode coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan failure-mode coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-failure-modes-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["stale or unknown anonymity-set root", "duplicate link tag", "receiver-set substitution", "range commitment mismatch", "authorization envelope mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"])',
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["stale or unknown anonymity-set root", "accept forged replay tag", "receiver-set substitution", "range commitment mismatch", "authorization envelope mismatch", "malformed proof bytes", "wrong verifier key", "public input mismatch"])',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan exact failure-mode coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan exact failure-mode coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan exact failure-mode coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-security-notes-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, performance gates, and external audit or verifier review."])',
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["Requires fresh anonymity-set roots and replay/link-tag state.", "Amount privacy depends on the range-proof component and commitment binding.", "Receiver ciphertext commitments must bind to the same transaction digest as the proof.", "The SDK dev fixture verifies deterministic binding only; chain execution and production Anonymous PGC proofs remain unavailable.", "Wallet witness material and private inputs must stay local and must not be exposed through SDK or chain APIs.", "Production hardening requires parser fuzzing, latency gates, and external audit or verifier review."])',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan security-note coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan security-note coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan security-note coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-source-reference-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"https://eprint.iacr.org/2025/884"',
        '"https://example.com/forged-source"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan source-reference coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan source-reference coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan source-reference coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-source-reference-exact-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '      url: "https://eprint.iacr.org/2025/884",\n    }),',
        '      url: "https://eprint.iacr.org/2025/884",\n    }),\n    Object.freeze({\n      label: "Forged extra source",\n      url: "https://example.com/forged-extra-source",\n    }),',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan exact source-reference coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan exact source-reference coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan exact source-reference coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-sdk-entrypoint-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["buildAnonymousPgcReceiverSet", "buildAnonymousPgcDevProofFixture", "verifyAnonymousPgcDevProofLocally"])',
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze(["buildAnonymousPgcReceiverSet", "buildForgedAnonymousPgcProductionProof", "verifyAnonymousPgcDevProofLocally"])',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan SDK entrypoint coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan SDK entrypoint coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan SDK entrypoint coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-planned-sdk-entrypoint-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"buildAnonymousPgcKOutOfNProofV1"',
        '"buildForgedAnonymousPgcProofV1"',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan planned SDK entrypoint coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan planned SDK entrypoint coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan planned SDK entrypoint coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-pq-layer-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze({ proof: false, authorization: false, note_encryption: false })',
        '"anonymous-pgc-k-out-of-n-v1": Object.freeze({ proof: true, authorization: false, note_encryption: false })',
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan PQ-layer coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan PQ-layer coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan PQ-layer coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-chain-requirement-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "typed zk::SubmitAnonymousPgcTransfer instruction",
        "typed zk::SubmitAnonymousPgcProofOnly instruction",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan chain-requirement coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan chain-requirement coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan chain-requirement coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-required-state-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "wallet account blinding and receiver recovery metadata",
        "forged wallet recovery placeholder",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan required-state coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan required-state coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan required-state coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-setup-step-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "Register the k-out-of-n payment verifier key and range-proof parameters.",
        "Register forged Anonymous PGC verifier setup.",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan setup-step coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan setup-step coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan setup-step coverage drift was not detected"
    )

if mode == "--negative-control-public-required-production-plan-execution-step-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "Generate the Anonymous PGC proof and submit the transfer instruction.",
        "Submit forged Anonymous PGC proof-only envelope.",
        1,
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate public privacy required production plan execution-step coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected public privacy required production plan execution-step coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: public privacy required production plan execution-step coverage drift was not detected"
    )

if mode == "--negative-control-python-catalog-bytecode-guard":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python catalog bytecode guard")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python catalog bytecode guard drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python catalog bytecode guard drift was not detected")

if mode == "--negative-control-python-ffi-catalog-bytecode-guard":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        '    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python FFI catalog bytecode guard")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python FFI catalog bytecode guard drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python FFI catalog bytecode guard drift was not detected")

if mode == "--negative-control-source-reference-obfuscated-ipv4-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        '      { sourceReferences: [{ label: "paper", url: "https://2130706433/source" }] },\n',
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate source-reference obfuscated IPv4 coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected source-reference obfuscated IPv4 coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: source-reference obfuscated IPv4 coverage drift was not detected")

if mode == "--negative-control-source-reference-audit-readiness-url-coverage":
    target = "python/iroha_python/tests/privacy_catalog_test.py"
    original = read(target)
    mutated = original.replace(
        "https://zips.z.cash/zip-0224#external-audit-complete",
        "https://zips.z.cash/zip-0224#external-review-placeholder",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate source-reference audit-readiness URL coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected source-reference audit-readiness URL coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: source-reference audit-readiness URL coverage drift was not detected")

if mode == "--negative-control-source-reference-encoded-host-url-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "https://256.256.256.256/source",
        "https://255.255.255.255/source",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate source-reference encoded-host URL coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected source-reference encoded-host URL coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: source-reference encoded-host URL coverage drift was not detected")

if mode == "--negative-control-dev-fixture-entrypoint-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "buildShapeDev.Proof.Fixture",
        "buildShapeDevProofHarness",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate DevFixture entrypoint coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected DevFixture entrypoint coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: DevFixture entrypoint coverage drift was not detected")

if mode == "--negative-control-catalog-defensive-copy-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "Object.isFrozen(frozenDescriptor.productionGate.auditReferences)",
        "Array.isArray(frozenDescriptor.productionGate.auditReferences)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy catalog defensive-copy coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy catalog defensive-copy coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy catalog defensive-copy coverage drift was not detected")

if mode == "--negative-control-planned-entrypoint-quarantine-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("planned privacy SDK entrypoints remain unexported until production gates pass"',
        'test("planned privacy SDK entrypoints remain tracked until production gates pass"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate planned-entrypoint quarantine coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected planned privacy entrypoint quarantine coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: planned privacy entrypoint quarantine coverage drift was not detected")

if mode == "--negative-control-native-catalog-parity-coverage":
    target = "javascript/iroha_js/test/privacyCatalogParity.test.js"
    original = read(target)
    mutated = original.replace(
        "assertRustNativeCatalogParity(pythonCatalog);",
        "assertRustNativeCatalogShape(pythonCatalog);",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy catalog parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy catalog parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy catalog parity coverage drift was not detected")

if mode == "--negative-control-native-planned-entrypoint-dispatch-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "sdk_entrypoints",
        "all_entrypoints",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy planned-entrypoint dispatch coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy planned-entrypoint dispatch coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy planned-entrypoint dispatch coverage drift was not detected")

if mode == "--negative-control-native-planned-entrypoint-rejection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "planned but not executable",
        "planned and executable",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy planned-entrypoint rejection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy planned-entrypoint rejection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy planned-entrypoint rejection coverage drift was not detected")

if mode == "--negative-control-native-catalog-structure-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_proof_family_is_portable",
        "privacy_proof_family_maybe_portable",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy catalog structure coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy catalog structure coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy catalog structure coverage drift was not detected")

if mode == "--negative-control-native-required-production-plan-rows-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "PRIVACY_REQUIRED_PRODUCTION_PLAN_ROWS",
        "PRIVACY_OPTIONAL_PRODUCTION_PLAN_ROWS",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy required production plan rows coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy required production plan rows coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy required production plan rows coverage drift was not detected"
    )

if mode == "--negative-control-native-required-production-plan-row-completeness-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "monero-fcmp-plus-plus-v1",
        "monero-fcmp-v1",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy required production plan row completeness coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy required production plan row completeness coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy required production plan row completeness coverage drift was not detected"
    )

if mode == "--negative-control-native-required-production-plan-duplicate-row-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "duplicate required production plan rows must be rejected",
        "duplicate required production plan rows may pass",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy required production plan duplicate row coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy required production plan duplicate row coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy required production plan duplicate row coverage drift was not detected"
    )

if mode == "--negative-control-native-required-production-plan-public-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "publicRequiredPrivacyPlanNativeRows",
        "publicOptionalPrivacyPlanNativeRows",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy required production plan public parity coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy required production plan public parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy required production plan public parity coverage drift was not detected"
    )

if mode == "--negative-control-native-required-production-allowlist-profile-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        '"zk-ace-pq-authorization-v0",\n    "stark/fri/sha256-goldilocks",\n    "stark-fri",',
        '"zk-ace-pq-authorization-v0",\n    "stark/fri",\n    "stark-fri",',
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy required production allowlist profile coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy required production allowlist profile drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy required production allowlist profile drift was not detected"
    )

if mode == "--negative-control-native-verifier-key-registration-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_catalog_vk_ref_name_is_registered",
        "privacy_catalog_vk_ref_name_is_optional",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy verifier-key registration coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy verifier-key registration coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy verifier-key registration coverage drift was not detected"
    )

if mode == "--negative-control-native-public-catalog-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "extractNativePrivacyCatalogRows",
        "extractNativePrivacyMaybeCatalogRows",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy public catalog parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy public catalog parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy public catalog parity coverage drift was not detected")

if mode == "--negative-control-native-component-proof-only-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "PRIVACY_COMPONENT_ALGORITHM_IDS",
        "PRIVACY_COMPONENT_OR_LEDGER_ALGORITHM_IDS",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy component proof-only coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy component proof-only coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy component proof-only coverage drift was not detected")

if mode == "--negative-control-native-planned-ledger-mutation-proof-builder-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "has_production_proof_builder",
        "has_optional_proof_builder",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy planned ledger-mutation proof-builder coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy planned ledger-mutation proof-builder coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy planned ledger-mutation proof-builder coverage drift was not detected"
    )

if mode == "--negative-control-native-ledger-mutation-proof-pairing-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("native privacy FFI catalogs keep proofed SDK ledger mutations typed and proof-paired"',
        'test("native privacy FFI catalogs allow proofed SDK ledger mutations generic"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy ledger-mutation proof-pairing coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy ledger-mutation proof-pairing coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy ledger-mutation proof-pairing coverage drift was not detected"
    )

if mode == "--negative-control-native-capability-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "!gate\\.ready",
        "gate\\.ready",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native privacy capability fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy capability fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native privacy capability fail-closed coverage drift was not detected")

if mode == "--negative-control-native-zk-ace-capability-fail-closed-coverage":
    target = "crates/connect_norito_bridge/src/lib.rs"
    original = read(target)
    mutated = original.replace(
        'assert_eq!(zk_ace.proof_family, "stark/fri/sha256-goldilocks");',
        'assert_eq!(zk_ace.proof_family, "stark/fri");',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native ZK-ACE capability fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native ZK-ACE capability fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native ZK-ACE capability fail-closed coverage drift was not detected"
    )

if mode == "--negative-control-native-capability-claim-quarantine-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "productionready",
        "productionalmostready",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy capability claim-quarantine coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy capability claim-quarantine coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy capability claim-quarantine coverage drift was not detected"
    )

if mode == "--negative-control-native-capability-archive-invariant-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "duplicate algorithm capability rows",
        "repeated capability archive rows",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate native privacy capability archive invariant coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected native privacy capability archive invariant coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: native privacy capability archive invariant coverage drift was not detected"
    )

if mode == "--negative-control-ffi-adversarial-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_build_proof_rejects_empty_public_inputs_before_production_gate",
        "privacy_build_proof_accepts_empty_public_inputs_before_production_gate",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI adversarial fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI adversarial fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI adversarial fail-closed coverage drift was not detected")

if mode == "--negative-control-ffi-verify-empty-public-inputs-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_verify_proof_rejects_empty_public_inputs_before_production_gate",
        "privacy_verify_proof_accepts_empty_public_inputs_before_production_gate",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI verify empty public-inputs coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI verify empty public-inputs coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI verify empty public-inputs coverage drift was not detected")

if mode == "--negative-control-ffi-vk-ref-backend-binding-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_vk_ref_matches_backend",
        "privacy_vk_ref_may_use_any_backend",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI verifier-key backend binding coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI verifier-key backend binding coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI verifier-key backend binding coverage drift was not detected")

if mode == "--negative-control-ffi-vk-ref-shape-hardening-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_vk_ref_is_well_formed",
        "privacy_vk_ref_is_maybe_well_formed",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI verifier-key shape hardening coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI verifier-key shape hardening coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI verifier-key shape hardening coverage drift was not detected")

if mode == "--negative-control-ffi-vk-ref-name-binding-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("native privacy FFI hosts reject verifier-key name drift before production gate"',
        'test("native privacy FFI hosts accept verifier-key name drift before production gate"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI verifier-key name binding coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI verifier-key name binding coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI verifier-key name binding coverage drift was not detected")

if mode == "--negative-control-ffi-operation-confusion-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy proof build request must not include proof bytes",
        "privacy proof build request may include proof bytes",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI operation-confusion fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI operation-confusion fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI operation-confusion fail-closed coverage drift was not detected")

if mode == "--negative-control-ffi-operation-required-material-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy proof build request must include witness bytes",
        "privacy proof build request may omit witness bytes",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI operation required-material coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI operation required-material coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI operation required-material coverage drift was not detected")

if mode == "--negative-control-ffi-non-proof-entrypoint-rejection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "buildRangeCommitment",
        "buildRangeProof",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI non-proof entrypoint rejection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI non-proof entrypoint rejection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI non-proof entrypoint rejection coverage drift was not detected")

if mode == "--negative-control-ffi-production-disabled-gate-message-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_build_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes",
        "privacy_build_proof_accepts_supported_algorithm_without_gate",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI production-disabled gate-message coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI production-disabled gate-message coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI production-disabled gate-message coverage drift was not detected"
    )

if mode == "--negative-control-ffi-production-disabled-verify-gate-message-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_verify_proof_rejects_supported_algorithm_until(?:_production)?_gate_passes",
        "privacy_verify_proof_accepts_supported_algorithm_without_gate",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI production-disabled verify gate-message coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI production-disabled verify gate-message coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI production-disabled verify gate-message coverage drift was not detected"
    )

if mode == "--negative-control-ffi-production-disabled-message-constant-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "real protocol engine enablement",
        "real protocol engine is optional",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI production-disabled message constant coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI production-disabled message constant coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI production-disabled message constant coverage drift was not detected"
    )

if mode == "--negative-control-ffi-zk-ace-production-disabled-request-coverage":
    target = "crates/connect_norito_bridge/src/lib.rs"
    original = read(target)
    mutated = original.replace(
        '!zk_ace_result.message.contains("secret-witness")',
        '!zk_ace_result.message.contains("public-inputs")',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate ZK-ACE production-disabled request coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected ZK-ACE production-disabled request coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: ZK-ACE production-disabled request coverage drift was not detected"
    )

if mode == "--negative-control-ffi-failure-result-invariant-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_failure_result_invariants_hold",
        "privacy_failure_result_invariants_drift",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI failure-result invariant coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI failure-result invariant coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI failure-result invariant coverage drift was not detected")

if mode == "--negative-control-ffi-witness-helper-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "assert_privacy_result_does_not_serialize_witness",
        "assert_privacy_result_may_serialize_witness",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI witness helper non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI witness helper non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI witness helper non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-witness-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_failure_results_never_serialize_witness_material",
        "privacy_failure_results_allow_witness_material_serialization",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI witness non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI witness non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI witness non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-proof-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_failure_results_preserve_error_invariants_without_proof_reflection",
        "privacy_failure_results_allow_proof_reflection",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI proof non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI proof non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI proof non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-request-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("native privacy FFI hosts bound reflected request fields before production gate"',
        'test("native privacy FFI hosts shape reflected request fields before production gate"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI request non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI request non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-request-text-field-enumerator-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "&request.vk_ref",
        "&request.entrypoint",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI request text-field enumerator coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request text-field enumerator coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI request text-field enumerator coverage drift was not detected")

if mode == "--negative-control-ffi-oversized-request-field-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_request_rejects_oversized_text_fields_without_reflection",
        "privacy_request_allows_oversized_text_fields_without_reflection",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI oversized request text-field non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI oversized request text-field non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI oversized request text-field non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-oversized-request-payload-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_request_rejects_oversized_witness_without_reflection",
        "privacy_request_allows_oversized_witness_without_reflection",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI oversized request byte-payload non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI oversized request byte-payload non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI oversized request byte-payload non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-oversized-public-inputs-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_request_rejects_oversized_public_inputs_without_reflection",
        "privacy_request_allows_oversized_public_inputs_without_reflection",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI oversized public-input non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI oversized public-input non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI oversized public-input non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-oversized-proof-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy_request_rejects_oversized_proof_without_reflection",
        "privacy_request_allows_oversized_proof_without_reflection",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI oversized proof non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI oversized proof non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI oversized proof non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-control-request-field-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "buildConfidentialTransferProofV2\\\\rforged",
        "buildConfidentialTransferProofV2-forged",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI control-character request text-field non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI control-character request text-field non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI control-character request text-field non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-control-vk-ref-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "vk:test\\\\tforged",
        "vk:test-forged",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI control-character vk_ref coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI control-character vk_ref coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI control-character vk_ref coverage drift was not detected")

if mode == "--negative-control-ffi-non-ascii-request-field-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "confidential-transfer-v2\\{marker\\}\\\\u\\{200B\\}",
        "confidential-transfer-v2\\{marker\\}",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI non-ASCII request text-field non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI non-ASCII request text-field non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI non-ASCII request text-field non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-non-ascii-vk-ref-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "vk:test\\{marker\\}\\\\u\\{FF1A\\}spoof",
        "vk:test\\{marker\\}:spoof",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI non-ASCII vk_ref coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI non-ASCII vk_ref coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI non-ASCII vk_ref coverage drift was not detected")

if mode == "--negative-control-ffi-unportable-request-field-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "confidential-transfer-v2 \\{marker\\}",
        "confidential-transfer-v2-\\{marker\\}",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI unportable request text-field non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI unportable request text-field non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI unportable request text-field non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-unportable-vk-ref-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "vk:test\\/\\.\\.\\/\\{marker\\}",
        "vk:test-\\{marker\\}",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI unportable vk_ref coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI unportable vk_ref coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI unportable vk_ref coverage drift was not detected")

if mode == "--negative-control-ffi-required-request-fields-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy proof request must include non-empty algorithm_id and entrypoint",
        "privacy proof request may omit algorithm_id and entrypoint",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI required request-field non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI required request-field non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI required request-field non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-required-vk-ref-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacy proof request must include non-empty vk_ref",
        "privacy proof request may omit vk_ref",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI required vk_ref non-reflection coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI required vk_ref non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI required vk_ref non-reflection coverage drift was not detected")

if mode == "--negative-control-ffi-request-catalog-shape-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "_confidential-transfer-v2",
        "confidential-transfer-v2",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI request catalog-shape non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request catalog-shape non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI request catalog-shape non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-request-entrypoint-catalog-shape-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "buildConfidentialTransferProofV2:\\{marker\\}",
        "buildConfidentialTransferProofV2\\{marker\\}",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI request entrypoint catalog-shape coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request entrypoint catalog-shape coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI request entrypoint catalog-shape coverage drift was not detected"
    )

if mode == "--negative-control-ffi-request-production-claim-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "buildAuditSignoffProof",
        "buildClaimedProof",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI request production-claim non-reflection coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request production-claim non-reflection coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI request production-claim non-reflection coverage drift was not detected"
    )

if mode == "--negative-control-ffi-request-vk-ref-production-claim-nonreflection-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "externally-audited-confidential-transfer",
        "confidential-transfer",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy FFI request vk_ref production-claim coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI request vk_ref production-claim coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy FFI request vk_ref production-claim coverage drift was not detected"
    )

if mode == "--negative-control-ffi-public-operation-schema-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("native privacy FFI archives use public operation schema bytes"',
        'test("native privacy FFI archives use host operation schema bytes"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI public operation-schema coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI public operation-schema coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI public operation-schema coverage drift was not detected")

if mode == "--negative-control-cross-sdk-operation-schema-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("privacy native availability proof probes use shared Norito request archives and reject unknown operations"',
        'test("privacy native availability proof probes use local request archives"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate cross-SDK privacy operation-schema coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected cross-SDK privacy operation-schema coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK privacy operation-schema coverage drift was not detected")

if mode == "--negative-control-native-availability-output-hardening-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "malformedPrivacyNativeOutputArchives(0x50)",
        "malformedPrivacyNativeOutputArchives(0x99)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy native availability output hardening coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native availability output hardening coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy native availability output hardening coverage drift was not detected"
    )

if mode == "--negative-control-native-availability-probe-gating-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacyNativeProbeOk\\s*=\\s*available",
        "privacyNativeProbeOk\\s*=\\s*true",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy native availability probe gating coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native availability probe gating coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy native availability probe gating coverage drift was not detected"
    )

if mode == "--negative-control-cross-sdk-request-boundary-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "bad_excessive_padding",
        "bad_accepted_padding",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate cross-SDK privacy request boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected cross-SDK privacy request boundary coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK privacy request boundary coverage drift was not detected")

if mode == "--negative-control-cross-sdk-sliced-view-boundary-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "_privacy_unsigned_byte_view",
        "_privacy_unchecked_byte_view",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate cross-SDK privacy sliced view boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected cross-SDK privacy sliced view boundary coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK privacy sliced view boundary coverage drift was not detected")

if mode == "--negative-control-cross-sdk-native-output-boundary-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "invalidPrivacyNoritoDeclaredPayloadLength(0x52)",
        "validPrivacyNoritoDeclaredPayloadLength(0x52)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate cross-SDK privacy native output boundary coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected cross-SDK privacy native output boundary coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: cross-SDK privacy native output boundary coverage drift was not detected")

if mode == "--negative-control-norito-request-schema-hardening-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacyNoritoFrameWithPadding(0x52, 65)",
        "privacyNoritoFrameWithPadding(0x52, 63)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy Norito request schema hardening coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Norito request schema hardening coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy Norito request schema hardening coverage drift was not detected")

if mode == "--negative-control-norito-request-field-bitset-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "privacyNoritoFrameWithFlags(0x52, 0x26)",
        "privacyNoritoFrameWithFlags(0x52, 0x20)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy Norito request field-bitset coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Norito request field-bitset coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy Norito request field-bitset coverage drift was not detected")

if mode == "--negative-control-norito-wrong-schema-request-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "wrong-schema verify request must not reach native dispatch",
        "wrong-schema verify request may reach native dispatch",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy Norito wrong-schema request coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Norito wrong-schema request coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy Norito wrong-schema request coverage drift was not detected")

if mode == "--negative-control-request-decoder-bounds-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "request\\.length\\s*>\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "request\\.length\\s*>=\\s*PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy request decoder bounds coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy request decoder bounds coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy request decoder bounds coverage drift was not detected")

if mode == "--negative-control-c-bridge-output-buffer-precedence-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "C bridge privacy proof entrypoints must prioritize missing output buffers over bad request pointers",
        "C bridge privacy proof entrypoints may validate bad requests before missing output buffers",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy C bridge output-buffer precedence coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C bridge output-buffer precedence coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy C bridge output-buffer precedence coverage drift was not detected"
    )

if mode == "--negative-control-native-adversarial-request-frame-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "payload_tamper\\[payload_last\\]",
        "payload_reflect\\[payload_last\\]",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy native adversarial request frame coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native adversarial request frame coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy native adversarial request frame coverage drift was not detected"
    )

if mode == "--negative-control-request-copy-isolation-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "requestArchive\\.contentEquals\\(originalArchive\\)",
        "requestArchive\\.contentEquals\\(mutatedArchive\\)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy request copy isolation coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy request copy isolation coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy request copy isolation coverage drift was not detected")

if mode == "--negative-control-request-copy-zeroization-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "request\\.every\\(\\(value\\)\\s*=>\\s*value\\s*===\\s*0\\)",
        "request\\.every\\(\\(value\\)\\s*=>\\s*value\\s*===\\s*1\\)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy request copy zeroization coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy request copy zeroization coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy request copy zeroization coverage drift was not detected")

if mode == "--negative-control-python-native-method-surface-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "missing privacy_verify_proof_v1",
        "missing privacy_optional_probe_v1",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python privacy native method-surface coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected Python privacy native method-surface coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python privacy native method-surface coverage drift was not detected")

if mode == "--negative-control-privacy-abi-probe-bounds-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "Number\\.isSafeInteger",
        "Number\\.isInteger",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy native ABI probe bounds coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native ABI probe bounds coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy native ABI probe bounds coverage drift was not detected")

if mode == "--negative-control-zk-ace-public-proof-builder-native-error-sanitizer-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("ZK-ACE public proof builders sanitize production-disabled native errors"',
        'test("ZK-ACE public proof builders reflect production-disabled native errors"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate ZK-ACE public proof-builder native-error sanitizer coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected ZK-ACE public proof-builder native-error sanitizer drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: ZK-ACE public proof-builder native-error sanitizer drift was not detected")

if mode == "--negative-control-malformed-request-no-dispatch-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "assert\\.fail\\(\"invalid verify request must not reach native dispatch\"\\)",
        "assert\\.fail\\(\"invalid verify request reached native dispatch\"\\)",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy malformed request no-dispatch coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy malformed request no-dispatch coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy malformed request no-dispatch coverage drift was not detected")

if mode == "--negative-control-public-wrapper-isolation-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("mobile and C# privacy tests isolate forged production-gate mutations"',
        'test("mobile and C# privacy tests allow forged production-gate mutations"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy public wrapper isolation coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy public wrapper isolation coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy public wrapper isolation coverage drift was not detected")

if mode == "--negative-control-public-archive-wrapper-norito-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("C# public privacy archive wrappers reject malformed Norito archives"',
        'test("C# public privacy archive wrappers accept malformed Norito archives"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy public archive wrapper Norito coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy public archive wrapper Norito coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy public archive wrapper Norito coverage drift was not detected")

if mode == "--negative-control-backend-alias-fail-closed-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("developer-only privacy backend labels stay rejected before production allowlists"',
        'test("developer-only privacy backend labels stay accepted before production allowlists"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy backend alias fail-closed coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy backend alias fail-closed coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy backend alias fail-closed coverage drift was not detected")

if mode == "--negative-control-required-pending-backend-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "public required backend families must match pending privacy backend tags",
        "public optional backend families may drift from pending privacy backend tags",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate required pending backend parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected required pending backend parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: required pending backend parity coverage drift was not detected")

if mode == "--negative-control-chain-backend-allowlist-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("native chain proof admission uses explicit production verifier backend allowlist"',
        'test("native chain proof admission uses implicit production verifier backend allowlist"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy chain backend allowlist coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy chain backend allowlist coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy chain backend allowlist coverage drift was not detected")

if mode == "--negative-control-required-production-allowlist-backend-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "must be explicitly covered by the Rust production allowlist test",
        "may skip the Rust production allowlist test",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate required production allowlist backend coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected required production allowlist backend coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: required production allowlist backend coverage drift was not detected")

if mode == "--negative-control-required-production-allowlist-rust-backend-mapping":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'Object.freeze(["stark-fri", "stark/fri", "stark/fri/sha256-goldilocks"])',
        'Object.freeze(["stark-fri", "stark/fri", "stark/fri/latest"])',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate required production allowlist Rust backend mapping")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected required production allowlist Rust backend mapping drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: required production allowlist Rust backend mapping drift was not detected"
    )

if mode == "--negative-control-required-production-allowlist-public-backends":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'const EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_BACKEND_LABELS = Object.freeze([\n  "stark-fri",\n]);',
        'const EXPECTED_REQUIRED_PRIVACY_PRODUCTION_ALLOWLIST_BACKEND_LABELS = Object.freeze([\n  "stark-fri",\n  "stark-fri-dev-fixture",\n]);',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate required production allowlist public backends")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected required production allowlist public backend drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: required production allowlist public backend drift was not detected"
    )

if mode == "--negative-control-required-production-allowlist-row-scope":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'Object.freeze(["zk-ace-pq-authorization-v0", "stark-fri"])',
        'Object.freeze(["pq-masp-stark-v0", "stark-fri"])',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate required production allowlist row scope")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected required production allowlist row scope drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: required production allowlist row scope drift was not detected"
    )

if mode == "--negative-control-ffi-abi-surface-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "assertRustNoMangleExport",
        "assertRustExportMayBeMangled",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI ABI surface coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI ABI surface coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI ABI surface coverage drift was not detected")

if mode == "--negative-control-ffi-binding-loader-surface-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "dlsym(handle,",
        "dlsym(optionalHandle,",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI binding loader surface coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI binding loader surface coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI binding loader surface coverage drift was not detected")

if mode == "--negative-control-ffi-error-contract-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "errorMalformedNorito",
        "errorMalformedArchive",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy FFI error contract parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy FFI error contract parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy FFI error contract parity coverage drift was not detected")

if mode == "--negative-control-native-archive-max-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "EXPECTED_PRIVACY_NATIVE_ARCHIVE_MAX_BYTES",
        "EXPECTED_PRIVACY_NATIVE_ARCHIVE_LIMIT_BYTES",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy native archive max parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native archive max parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy native archive max parity coverage drift was not detected")

if mode == "--negative-control-sdk-bridge-method-surface-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("SDK privacy native bridges expose only generic archive operations"',
        'test("SDK privacy native bridges expose algorithm-specific operations"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy SDK bridge method-surface coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy SDK bridge method-surface coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy SDK bridge method-surface coverage drift was not detected")

if mode == "--negative-control-binary-only-norito-ffi-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        'test("privacy FFI SDK wrappers remain binary-only and JSON-free"',
        'test("privacy FFI SDK wrappers may use JSON payloads"',
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy binary-only Norito FFI coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy binary-only Norito FFI coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy binary-only Norito FFI coverage drift was not detected")

if mode == "--negative-control-native-host-norito-only-ffi-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "norito::decode_from_bytes",
        "serde_json::from_slice",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy native host Norito-only FFI coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native host Norito-only FFI coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy native host Norito-only FFI coverage drift was not detected")

if mode == "--negative-control-production-gate-missing-reason-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "real proving engine is not registered",
        "real proving engine may be simulated",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy production gate missing-reason parity coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy production gate missing-reason parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy production gate missing-reason parity coverage drift was not detected"
    )

if mode == "--negative-control-native-production-gate-missing-reason-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "public static let missingReasons",
        "public static let optionalReasons",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy native production gate missing-reason parity coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native production gate missing-reason parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy native production gate missing-reason parity coverage drift was not detected"
    )

if mode == "--negative-control-norito-schema-operation-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "EXPECTED_PRIVACY_PROOF_REQUEST_FIELDS",
        "EXPECTED_PRIVACY_PROOF_WITNESS_FIELDS",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy Norito schema operation parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Norito schema operation parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy Norito schema operation parity coverage drift was not detected")

if mode == "--negative-control-norito-operation-variant-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "EXPECTED_PRIVACY_OPERATION_VARIANTS",
        "EXPECTED_PRIVACY_OPTIONAL_OPERATION_VARIANTS",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy Norito operation variant parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Norito operation variant parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy Norito operation variant parity coverage drift was not detected")

if mode == "--negative-control-capability-metadata-parity-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "EXPECTED_SWIFT_PRIVACY_CAPABILITY_FIELDS",
        "EXPECTED_SWIFT_PRIVACY_DETAILED_CAPABILITY_FIELDS",
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate privacy capability metadata parity coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy capability metadata parity coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: privacy capability metadata parity coverage drift was not detected")

if mode == "--negative-control-mobile-zk-ace-capability-quarantine-coverage":
    target = "IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift"
    original = read(target)
    mutated = original.replace(
        "    public let productionGate: PrivacyProductionGate\n",
        "    public let productionGate: PrivacyProductionGate\n    public let zkAceProductionReady: Bool\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate mobile ZK-ACE capability quarantine coverage")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected mobile ZK-ACE capability quarantine coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: mobile ZK-ACE capability quarantine coverage drift was not detected"
    )

if mode == "--negative-control-capability-fail-closed-metadata-coverage":
    target = "javascript/iroha_js/test/privacyFfiContractParity.test.js"
    original = read(target)
    mutated = original.replace(
        "productionReady\\s*=\\s*false",
        "productionReady\\s*=\\s*true",
    )
    if mutated == original:
        raise SystemExit(
            "negative control failed: unable to mutate privacy capability fail-closed metadata coverage"
        )
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy capability fail-closed metadata coverage drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit(
        "negative control failed: privacy capability fail-closed metadata coverage drift was not detected"
    )

if mode == "--negative-control-readme-error-code":
    target = "javascript/iroha_js/README.md"
    original = read(target)
    mutated = original.replace("status_error = 1", "status_error = 0", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate README error-code value")
    text_overrides[target] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy README error-code drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: README error-code drift was not detected")

if mode == "--negative-control-browser-error-code":
    target = root / "javascript/iroha_js/src/crypto.browser.js"
    original = target.read_text(encoding="utf-8")
    mutated = original.replace(
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;",
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 6;",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser error-code value")
    result = None
    try:
        target.write_text(mutated, encoding="utf-8")
        result = subprocess.run(
            [
                "node",
                "--test",
                "--test-name-pattern",
                "browser crypto exposes native-only helpers as safe stubs",
                "test/crypto.browser.test.js",
            ],
            cwd=root / "javascript/iroha_js",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
            check=False,
        )
    finally:
        target.write_text(original, encoding="utf-8")
    output = result.stdout if result is not None else ""
    if result is not None and result.returncode != 0 and (
        "browser crypto exposes native-only helpers as safe stubs" in output
        or "Expected values to be strictly equal" in output
    ):
        print("negative control rejected browser privacy FFI error-code drift")
        first_line = next((line for line in output.splitlines() if line.strip()), "")
        if first_line:
            print(first_line)
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser error-code drift was not detected")

if mode == "--negative-control-browser-dist-error-code":
    target = root / "javascript/iroha_js/dist/crypto.browser.js"
    original = target.read_text(encoding="utf-8")
    mutated = original.replace(
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 5;",
        "export const PRIVACY_FFI_ERROR_INVALID_REQUEST = 6;",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate browser dist error-code value")
    result = None
    try:
        target.write_text(mutated, encoding="utf-8")
        result = subprocess.run(
            [
                "node",
                "--test",
                "--test-name-pattern",
                "browser crypto exposes native-only helpers as safe stubs",
                "test/crypto.browser.test.js",
            ],
            cwd=root / "javascript/iroha_js",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=10,
            check=False,
        )
    finally:
        target.write_text(original, encoding="utf-8")
    output = result.stdout if result is not None else ""
    if result is not None and result.returncode != 0 and (
        "browser crypto exposes native-only helpers as safe stubs" in output
        or "Expected values to be strictly equal" in output
    ):
        print("negative control rejected browser dist privacy FFI error-code drift")
        first_line = next((line for line in output.splitlines() if line.strip()), "")
        if first_line:
            print(first_line)
        raise SystemExit(0)
    raise SystemExit("negative control failed: browser dist error-code drift was not detected")

if mode == "--negative-control-workflow-cancel-in-progress":
    original = read(workflow_path)
    mutated = original.replace(
        "  cancel-in-progress: false",
        "  cancel-in-progress: true",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate workflow cancellation policy")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy workflow cancellation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: workflow cancellation drift was not detected")

if mode == "--negative-control-native-bridge-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_native_bridge_tests:\n",
        "  privacy_native_bridge_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow job drift was not detected")

if mode == "--negative-control-native-bridge-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "    runs-on: ubuntu-latest",
        "    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow runner drift was not detected")

if mode == "--negative-control-native-bridge-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {native_bridge_command}",
        "        run: cargo test -p connect_norito_bridge --lib -- --skip privacy_",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge test command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge test workflow drift was not detected")

if mode == "--negative-control-native-bridge-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate native bridge workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy native bridge workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: native bridge workflow dependency drift was not detected")

if mode == "--negative-control-swift-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_swift_sdk_parse:\n",
        "  privacy_swift_sdk_parse_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow job drift was not detected")

if mode == "--negative-control-swift-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "    runs-on: macos-latest",
        "    runs-on: ubuntu-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow runner drift was not detected")

if mode == "--negative-control-swift-sdk-parse-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {swift_sdk_command}",
        "        run: swiftc --version",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK parse workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK parse workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK parse workflow drift was not detected")

if mode == "--negative-control-swift-sdk-version-script":
    original = read(swift_sdk_command)
    mutated = original.replace('"${SWIFTC_BIN}" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK version evidence")
    text_overrides[swift_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK version script drift was not detected")

if mode == "--negative-control-swift-sdk-override-script":
    original = read(swift_sdk_command)
    mutated = original.replace("PRIVACY_SWIFT_SDK_SWIFTC_BIN", "PRIVACY_SWIFT_SWIFTC_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK compiler override variable")
    text_overrides[swift_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK compiler override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK compiler override drift was not detected")

if mode == "--negative-control-swift-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Swift SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Swift SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Swift SDK workflow dependency drift was not detected")

if mode == "--negative-control-jvm-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_jvm_sdk_tests:\n",
        "  privacy_jvm_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow job drift was not detected")

if mode == "--negative-control-jvm-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-java@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK setup workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-distribution-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          distribution: "temurin"\n',
        '          distribution: "zulu"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java distribution")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java distribution drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java distribution drift was not detected")

if mode == "--negative-control-jvm-sdk-java-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          java-version: "21"\n',
        '          java-version: "17"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java version drift was not detected")

if mode == "--negative-control-jvm-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {jvm_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK setup order")
    insert = mutated.index("      - uses: actions/setup-java@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy JVM SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK setup ordering drift was not detected")

if mode == "--negative-control-jvm-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {jvm_sdk_command}",
        "        run: ci/check_privacy_jvm_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK test workflow drift was not detected")

if mode == "--negative-control-jvm-sdk-jdk21-script":
    original = read(jvm_sdk_command)
    mutated = original.replace("java -version\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK JDK 21 script evidence")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK JDK 21 script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK JDK 21 script drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-override-script":
    original = read(jvm_sdk_command)
    mutated = original.replace("PRIVACY_JVM_SDK_JAVA_HOME", "PRIVACY_JVM_SDK_JAVA_HOME_DISABLED", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK Java home override variable")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK Java home override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK Java home override drift was not detected")

if mode == "--negative-control-jvm-sdk-java-home-reject-script":
    original = read(jvm_sdk_command)
    mutated = original.replace(
        "JAVA_HOME must point to a JDK 21 home for privacy JVM SDK tests.",
        "JAVA_HOME is not checked before fallback.",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK inherited Java home rejection")
    text_overrides[jvm_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK inherited Java home rejection drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK inherited Java home rejection drift was not detected")

if mode == "--negative-control-jvm-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JVM SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JVM SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JVM SDK workflow dependency drift was not detected")

if mode == "--negative-control-csharp-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_csharp_sdk_tests:\n",
        "  privacy_csharp_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow job drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-dotnet@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          dotnet-version: 8.0.x\n",
        "          dotnet-version: 7.0.x\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version drift was not detected")

if mode == "--negative-control-csharp-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {csharp_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK setup order")
    insert = mutated.index("      - uses: actions/setup-dotnet@v4\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy C# SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK setup ordering drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-version-script":
    original = read(csharp_sdk_command)
    mutated = original.replace('printf \'%s\\n\' "${DOTNET_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet version evidence")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet version script drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-override-script":
    original = read(csharp_sdk_command)
    mutated = original.replace("PRIVACY_CSHARP_DOTNET_BIN", "PRIVACY_DOTNET_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet override variable")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet override drift was not detected")

if mode == "--negative-control-csharp-sdk-dotnet-major-script":
    original = read(csharp_sdk_command)
    mutated = original.replace("8.0.*) ;;", "7.0.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK dotnet major matcher")
    text_overrides[csharp_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK dotnet major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK dotnet major script drift was not detected")

if mode == "--negative-control-csharp-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {csharp_sdk_command}",
        "        run: ci/check_privacy_csharp_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK test workflow drift was not detected")

if mode == "--negative-control-csharp-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_javascript_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate C# SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy C# SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: C# SDK workflow dependency drift was not detected")

if mode == "--negative-control-js-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_javascript_sdk_tests:\n",
        "  privacy_javascript_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow job drift was not detected")

if mode == "--negative-control-js-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_javascript_sdk_tests:\n    runs-on: ubuntu-latest",
        "  privacy_javascript_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow runner drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-node@v4\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node setup drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup drift was not detected")

if mode == "--negative-control-js-sdk-node-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          node-version: "20"\n',
        '          node-version: "18"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version drift was not detected")

if mode == "--negative-control-js-sdk-node-version-script":
    original = read(js_sdk_command)
    mutated = original.replace('printf \'%s\\n\' "${NODE_VERSION}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node version evidence")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node version script drift was not detected")

if mode == "--negative-control-js-sdk-node-override-script":
    original = read(js_sdk_command)
    mutated = original.replace("PRIVACY_JS_SDK_NODE_BIN", "PRIVACY_JS_NODE_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node override variable")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node override drift was not detected")

if mode == "--negative-control-js-sdk-node-resolver-script":
    original = read(js_sdk_command)
    mutated = original.replace("resolve_node_20_bin()", "resolve_node_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node resolver")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node resolver drift was not detected")

if mode == "--negative-control-js-sdk-node-major-script":
    original = read(js_sdk_command)
    mutated = original.replace("v20.*) ;;", "v18.*) ;;", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Node major matcher")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node major script drift was not detected")

if mode == "--negative-control-js-sdk-python-bytecode-script":
    original = read(js_sdk_command)
    mutated = original.replace("export PYTHONDONTWRITEBYTECODE=1\n\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK Python bytecode guard")
    text_overrides[js_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Python bytecode guard drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Python bytecode guard drift was not detected")

if mode == "--negative-control-js-sdk-node-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "          cache-dependency-path: javascript/iroha_js/package-lock.json\n",
        "          cache-dependency-path: javascript/iroha_js/package.json\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK cache path")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK cache path drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK cache path drift was not detected")

if mode == "--negative-control-js-sdk-node-setup-order-workflow":
    original = read(workflow_path)
    install_block = (
        "      - name: Install JavaScript SDK dependencies\n"
        f"        run: {js_sdk_install_command}\n"
    )
    mutated = original.replace(install_block, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install before Node setup")
    insert = mutated.index("      - uses: actions/setup-node@v4\n")
    mutated = mutated[:insert] + install_block + mutated[insert:]
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK Node setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK Node setup ordering drift was not detected")

if mode == "--negative-control-js-sdk-install-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {js_sdk_install_command}",
        "        run: npm ci --prefix javascript/iroha_js --ignore-scripts",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK install workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK install workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install workflow drift was not detected")

if mode == "--negative-control-js-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {js_sdk_command}",
        "        run: ci/check_privacy_js_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test workflow drift was not detected")

if mode == "--negative-control-js-sdk-install-order-workflow":
    original = read(workflow_path)
    install_line = f"        run: {js_sdk_install_command}"
    test_line = f"        run: {js_sdk_command}"
    mutated = original.replace(f"{install_line}\n", "", 1)
    mutated = mutated.replace(f"{test_line}\n", f"{test_line}\n{install_line}\n", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK install after tests")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK install ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK install ordering drift was not detected")

if mode == "--negative-control-js-sdk-test-order-workflow":
    original = read(workflow_path)
    test_line = f"        run: {js_sdk_command}"
    mutated = original.replace(f"{test_line}\n", "", 1)
    mutated = mutated.replace(
        "        run: ci/check_privacy_sdk_guard.sh\n",
        "        run: ci/check_privacy_sdk_guard.sh\n" + test_line + "\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to move JavaScript SDK tests after main guard")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK test ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK test ordering drift was not detected")

if mode == "--negative-control-js-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_python_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate JavaScript SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy JavaScript SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: JavaScript SDK workflow dependency drift was not detected")

if mode == "--negative-control-python-sdk-job-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n",
        "  privacy_python_sdk_tests_disabled:\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow job")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow job drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow job drift was not detected")

if mode == "--negative-control-python-sdk-runner-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n    runs-on: ubuntu-latest",
        "  privacy_python_sdk_tests:\n    runs-on: macos-latest",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow runner")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow runner drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow runner drift was not detected")

if mode == "--negative-control-python-sdk-setup-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "      - uses: actions/setup-python@v5\n",
        "",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup workflow step")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK setup workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup workflow drift was not detected")

if mode == "--negative-control-python-sdk-version-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        '          python-version: "3.11"\n',
        '          python-version: "3.10"\n',
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK version drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version drift was not detected")

if mode == "--negative-control-python-sdk-setup-order-workflow":
    original = read(workflow_path)
    run_line = f"        run: {python_sdk_command}\n"
    mutated = original.replace(run_line, "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK setup order")
    insert = mutated.index("      - uses: actions/setup-python@v5\n")
    mutated = (
        mutated[:insert]
        + "      - name: Privacy Python SDK tests\n"
        + run_line
        + mutated[insert:]
    )
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK setup ordering drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK setup ordering drift was not detected")

if mode == "--negative-control-python-sdk-rust-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          cache-on-failure: \"true\"\n",
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK Rust cache")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK Rust cache drift was not detected")

if mode == "--negative-control-python-sdk-timeout-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n",
        "  privacy_python_sdk_tests:\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 15\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK timeout")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK timeout drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK timeout drift was not detected")

if mode == "--negative-control-python-sdk-version-script":
    original = read(python_sdk_command)
    mutated = original.replace('"${VENV_DIR}/bin/python" --version\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK version evidence")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK version script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK version script drift was not detected")

if mode == "--negative-control-python-sdk-override-script":
    original = read(python_sdk_command)
    mutated = original.replace("PRIVACY_PYTHON_SDK_PYTHON_BIN", "PRIVACY_PYTHON_BIN", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK override variable")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK override drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK override drift was not detected")

if mode == "--negative-control-python-sdk-resolver-script":
    original = read(python_sdk_command)
    mutated = original.replace("resolve_python_311_bin()", "resolve_python_bin()", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK resolver")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK resolver drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK resolver drift was not detected")

if mode == "--negative-control-python-sdk-major-script":
    original = read(python_sdk_command)
    mutated = original.replace("3.11) ;;", "3.10) ;;")
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK major matcher")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK major script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK major script drift was not detected")

if mode == "--negative-control-python-sdk-venv-rebuild-script":
    original = read(python_sdk_command)
    mutated = original.replace('  rm -rf "${VENV_DIR}"\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK stale venv rebuild")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK stale venv rebuild drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK stale venv rebuild drift was not detected")

if mode == "--negative-control-python-sdk-native-build-script":
    original = read(python_sdk_command)
    mutated = original.replace('"${VENV_DIR}/bin/python" -m maturin develop --release\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK native build step")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK native build script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK native build script drift was not detected")

if mode == "--negative-control-python-sdk-venv-activation-script":
    original = read(python_sdk_command)
    mutated = original.replace('export VIRTUAL_ENV="${VENV_DIR}"\nexport PATH="${VENV_DIR}/bin:${PATH}"\n\n', "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK venv activation")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK venv activation drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK venv activation drift was not detected")

if mode == "--negative-control-python-sdk-bytecode-script":
    original = read(python_sdk_command)
    mutated = original.replace("export PYTHONDONTWRITEBYTECODE=1\n\n", "", 1)
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK bytecode guard")
    text_overrides[python_sdk_command] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK bytecode script drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK bytecode script drift was not detected")

if mode == "--negative-control-python-sdk-test-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        f"        run: {python_sdk_command}",
        "        run: ci/check_privacy_python_sdk.sh --skip",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK test workflow command")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK test workflow drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK test workflow drift was not detected")

if mode == "--negative-control-python-sdk-needs-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        main_job_needs_line,
        "    needs: [privacy_native_bridge_tests, privacy_swift_sdk_parse, privacy_jvm_sdk_tests, privacy_csharp_sdk_tests, privacy_javascript_sdk_tests]",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate Python SDK workflow dependency")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy Python SDK workflow dependency drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: Python SDK workflow dependency drift was not detected")

if mode == "--negative-control-main-rust-cache-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n"
        "      - uses: Swatinem/rust-cache@v2\n"
        "        with:\n"
        "          cache-on-failure: \"true\"\n",
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n"
        "    steps:\n"
        "      - uses: actions/checkout@v4\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate main guard Rust cache")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy main guard Rust cache drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: main guard Rust cache drift was not detected")

if mode == "--negative-control-main-timeout-workflow":
    original = read(workflow_path)
    mutated = original.replace(
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 45\n",
        "  privacy-sdk-guard:\n"
        f"{main_job_needs_line}\n"
        "    runs-on: ubuntu-latest\n"
        "    timeout-minutes: 20\n",
        1,
    )
    if mutated == original:
        raise SystemExit("negative control failed: unable to mutate main guard timeout")
    text_overrides[workflow_path] = mutated
    try:
        run_checks()
    except PrivacyGuardError as error:
        print("negative control rejected privacy main guard timeout drift")
        print(str(error).splitlines()[0])
        raise SystemExit(0)
    raise SystemExit("negative control failed: main guard timeout drift was not detected")

if mode:
    raise SystemExit(f"unknown mode: {mode}")

run_checks()
PY

if [[ -n "${MODE}" ]]; then
  exit 0
fi

SDK_NODE_BIN="$(resolve_node_20_bin)"
SDK_PYTHON_BIN="$(resolve_python_311_bin)"

PRIVACY_JS_SDK_ROOT="${ROOT_DIR}" \
  PRIVACY_JS_SDK_NODE_BIN="${SDK_NODE_BIN}" \
  bash "${ROOT_DIR}/ci/check_privacy_js_sdk.sh"
PRIVACY_PYTHON_SDK_ROOT="${ROOT_DIR}" \
  PRIVACY_PYTHON_SDK_PYTHON_BIN="${SDK_PYTHON_BIN}" \
  PRIVACY_PYTHON_SDK_VENV="${VENV_DIR}" \
  bash "${ROOT_DIR}/ci/check_privacy_python_sdk.sh"
