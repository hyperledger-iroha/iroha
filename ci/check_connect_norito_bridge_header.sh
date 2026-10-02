#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIB="${ROOT_DIR}/crates/connect_norito_bridge/src/lib.rs"
PARLIAMENT_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/parliament_timed_ovn_ffi.rs"
PRIVATE_SETTLEMENT_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/private_settlement_ffi.rs"
HEADER="${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h"
UMBRELLA="${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h"
PRIVACY_MODEL="${ROOT_DIR}/crates/iroha_data_model/src/privacy/protocol.rs"
RETAIL_MODEL="${ROOT_DIR}/crates/iroha_data_model/src/validation_fee/retail.rs"
RESERVE_FINALITY_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_reserve_finality_v1.rs"
TESTNET_OBSERVATION_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_testnet_observation_v1.rs"
TESTNET_VALUE_LEDGER_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_testnet_native_value_ledger_v1.rs"
TESTNET_STARTUP_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_testnet_native_startup_v1.rs"
ORDINARY_STARTUP_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/ordinary_native_startup.rs"
ORDINARY_CURRENT_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/ordinary_current_control.rs"
ORDINARY_OUTGOING_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/ordinary_outgoing_driver.rs"
MODE="${1:-}"

SELF_TESTS=(
  --self-test-missing-domain-header-symbol
  --self-test-missing-domain-rust-symbol
  --self-test-bad-domain-length-width
  --self-test-missing-top-up-binding-header
  --self-test-missing-top-up-binding-rust
  --self-test-bad-top-up-binding-width
  --self-test-missing-top-up-binding-request
  --self-test-bad-abi
  --self-test-missing-reserve-finality-header-symbol
  --self-test-missing-reserve-finality-rust-symbol
  --self-test-bad-reserve-finality-checkpoint-signature
  --self-test-missing-reserve-finality-request-binding
  --self-test-missing-ordinary-startup-header-symbol
  --self-test-missing-ordinary-startup-rust-symbol
  --self-test-bad-ordinary-startup-signature
  --self-test-missing-ordinary-current-header-symbol
  --self-test-missing-ordinary-current-rust-symbol
  --self-test-bad-ordinary-current-signature
  --self-test-missing-ordinary-outgoing-header-symbol
  --self-test-missing-ordinary-outgoing-rust-symbol
  --self-test-bad-ordinary-outgoing-signature
  --self-test-missing-kagemusha-header-symbol
  --self-test-missing-kagemusha-close-header-symbol
  --self-test-missing-kagemusha-install-header-symbol
  --self-test-missing-kagemusha-install-rust-symbol
  --self-test-bad-kagemusha-install-signature
  --self-test-missing-kagemusha-testnet-observation-header-symbol
  --self-test-missing-kagemusha-rust-symbol
  --self-test-bad-kagemusha-signature
  --self-test-missing-kagemusha-command-binding
  --self-test-bad-kagemusha-error-code
  --self-test-missing-kagemusha-mint-stage-header-symbol
  --self-test-bad-kagemusha-mint-stage-signature
  --self-test-missing-finalized-mint-header-symbol
  --self-test-missing-finalized-mint-rust-symbol
  --self-test-missing-testnet-value-header-symbol
  --self-test-missing-testnet-value-rust-symbol
  --self-test-missing-testnet-credit-header-symbol
  --self-test-missing-testnet-credit-rust-symbol
  --self-test-missing-testnet-startup-contract-header-symbol
  --self-test-missing-testnet-startup-contract-rust-symbol
  --self-test-missing-testnet-startup-activate-header-symbol
  --self-test-missing-testnet-startup-activate-rust-symbol
  --self-test-bad-testnet-startup-contract-signature
  --self-test-bad-testnet-startup-activate-signature
  --self-test-missing-privacy-header-symbol
  --self-test-bad-privacy-signature
  --self-test-missing-privacy-rust-symbol
  --self-test-extra-privacy-symbol
  --self-test-missing-parliament-header-symbol
  --self-test-bad-parliament-page-rust-width
  --self-test-bad-parliament-page-header-width
  --self-test-retired-parliament-page-rust-name
  --self-test-retired-parliament-page-header-name
  --self-test-missing-retail-header-symbol
  --self-test-bad-retail-signature
  --self-test-bad-retail-marker-bound
  --self-test-bad-retail-intent-bound
  --self-test-bad-retail-assessment-bound
  --self-test-retired-fee-header-symbol
  --self-test-missing-sorafs-reference-header-symbol
  --self-test-missing-sorafs-reference-rust-symbol
  --self-test-bad-sorafs-reference-bundle-signature
  --self-test-bad-sorafs-reference-bundle-layout
  --self-test-bad-sorafs-reference-bundle-limit
  --self-test-missing-generated-transaction-signer
  --self-test-missing-conviction-update-signer-header-symbol
  --self-test-bad-generated-transaction-signer-signature
  --self-test-forbidden-retired-transaction-signer
  --self-test-bad-deallocator-signature
  --self-test-umbrella-drift
  --self-test-umbrella-comment-growth
)

usage() {
  echo "usage: ci/check_connect_norito_bridge_header.sh [--self-test|--self-test-*]" >&2
}

run_contract_check() {
  local rust_lib="$1"
  local header="$2"
  local umbrella="$3"
  local privacy_model="$4"
  local parliament_rust="$5"
  local retail_model="$6"
  local private_settlement_rust="$7"
  local reserve_finality_rust="$8"
  local testnet_observation_rust="$9"
  local testnet_value_ledger_rust="${10}"
  local testnet_startup_rust="${11}"
  local ordinary_startup_rust="${12}"
  local ordinary_current_rust="${13}"
  local ordinary_outgoing_rust="${14}"

  python3 - \
    "${rust_lib}" \
    "${header}" \
    "${umbrella}" \
    "${privacy_model}" \
    "${parliament_rust}" \
    "${retail_model}" \
    "${private_settlement_rust}" \
    "${reserve_finality_rust}" \
    "${testnet_observation_rust}" \
    "${testnet_value_ledger_rust}" \
    "${testnet_startup_rust}" \
    "${ordinary_startup_rust}" \
    "${ordinary_current_rust}" \
    "${ordinary_outgoing_rust}" <<'PY'
from pathlib import Path
import re
import sys

rust = Path(sys.argv[1]).read_text(encoding="utf-8")
header = Path(sys.argv[2]).read_text(encoding="utf-8")
umbrella = Path(sys.argv[3]).read_text(encoding="utf-8")
privacy = Path(sys.argv[4]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[5]).read_text(encoding="utf-8")
retail_model = Path(sys.argv[6]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[7]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[8]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[9]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[10]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[11]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[12]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[13]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[14]).read_text(encoding="utf-8")


def require(pattern: str, text: str, label: str) -> None:
    if re.search(pattern, text, re.S) is None:
        raise SystemExit(f"[connect-norito-header] missing or invalid {label}")


KAGEMUSHA_EXPORTS = {
    "connect_norito_kagemusha_v1_payment_request_validate",
    "connect_norito_kagemusha_v1_payment_validate",
    "connect_norito_kagemusha_v1_acknowledgement_validate",
    "connect_norito_kagemusha_v1_complete_exchange_validate",
    "connect_norito_kagemusha_v1_mint_authorization_validate",
    "connect_norito_kagemusha_v1_mint_credit_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_validate",
    "connect_norito_kagemusha_v1_payment_request_text_validate",
    "connect_norito_kagemusha_v1_payment_text_validate",
    "connect_norito_kagemusha_v1_acknowledgement_text_validate",
    "connect_norito_kagemusha_v1_complete_exchange_text_validate",
    "connect_norito_kagemusha_v1_mint_authorization_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_text_validate",
    "connect_norito_kagemusha_v1_mint_credit_against_authorization_text_validate",
    "connect_norito_kagemusha_v1_redemption_voucher_text_validate",
    "connect_norito_kagemusha_contract_vector_v1",
    "connect_norito_kagemusha_core_coordinator_contract_v1",
    "connect_norito_kagemusha_core_coordinator_install_v1",
    "connect_norito_kagemusha_core_coordinator_open_v1",
    "connect_norito_kagemusha_core_coordinator_invoke_v1",
    "connect_norito_kagemusha_core_coordinator_close_v1",
    "connect_norito_kagemusha_ordinary_runtime_startup_v1",
    "connect_norito_kagemusha_ordinary_current_control_v1",
    "connect_norito_kagemusha_ordinary_outgoing_v1",
    "connect_norito_kagemusha_testnet_state_proof_observe_v1",
    "connect_norito_kagemusha_testnet_finalized_mint_observe_v1",
    "connect_norito_kagemusha_testnet_value_admit_v1",
    "connect_norito_kagemusha_testnet_value_credit_v1",
    "connect_norito_kagemusha_testnet_native_startup_contract_v1",
    "connect_norito_kagemusha_testnet_native_startup_activate_v1",
    "connect_norito_kagemusha_device_capabilities_v1",
    "connect_norito_kagemusha_device_execute_v1",
    "connect_norito_kagemusha_device_command_response_v1_verify",
    "connect_norito_kagemusha_reserve_finality_hint_v1",
    "connect_norito_kagemusha_reserve_finality_verify_v1",
    "connect_norito_kagemusha_top_up_signed_request_validate_v1",
    "connect_norito_kagemusha_device_mint_stage_command_v1_validate",
    "connect_norito_kagemusha_device_mint_stage_result_v1_validate",
}
PRIVACY_EXPORTS = {
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_capability_manifest_v1",
    "iroha_privacy_free_buffer",
}
SORAFS_REFERENCE_EXPORTS = {
    "connect_norito_sorafs_reference_build_signed_orderbook_order_cancel",
    "connect_norito_sorafs_reference_build_signed_orderbook_order_request",
    "connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt",
    "connect_norito_sorafs_reference_derive_orderbook_order_id",
    "connect_norito_sorafs_reference_sign_orderbook_payload",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
    "connect_norito_sorafs_reference_validate_bundle_json",
    "connect_norito_sorafs_reference_validate_governance_json",
    "connect_norito_sorafs_reference_validate_hedging_json",
    "connect_norito_sorafs_reference_validate_governance_dag_block_json",
    "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
    "connect_norito_sorafs_reference_validate_orderbook_json",
    "connect_norito_sorafs_reference_validate_pdp_bundle_json",
    "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json",
    "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json",
    "connect_norito_sorafs_reference_validate_pdp_payload_json",
    "connect_norito_sorafs_reference_validate_pop_json",
}
DETACHED_EXPORTS = {
    "connect_norito_canonical_json_blake3_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
}
PARLIAMENT_EXPORTS = {
    "connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1",
    "connect_norito_parliament_timed_ovn_verify_casting_proof_v1",
    "connect_norito_parliament_timed_ovn_ballot_from_proof_v1",
    "connect_norito_parliament_timed_ovn_registration_from_proof_v1",
}
RETAIL_EXPORTS = {
    "connect_norito_retail_fee_intent_hash_v1",
    "connect_norito_retail_fee_assessment_marker_v1",
    "connect_norito_retail_fee_assessment_decode_v1",
}
PRIVATE_SETTLEMENT_EXPORTS = {
    "connect_norito_private_settlement_committee_proof_response_verify_v1",
    "connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1",
    "connect_norito_private_settlement_audit_approval_response_verify_v1",
}
TRANSACTION_SIGNER_BASE_EXPORTS = {
    "connect_norito_encode_account_read_permission_multisig_signed_transaction",
    "connect_norito_encode_burn_signed_transaction",
    "connect_norito_encode_claim_identifier_signed_transaction",
    "connect_norito_encode_governance_cast_plain_ballot_signed_transaction",
    "connect_norito_encode_governance_cast_zk_ballot_signed_transaction",
    "connect_norito_encode_governance_propose_deploy_v1_signed_transaction",
    "connect_norito_encode_mint_signed_transaction",
    "connect_norito_encode_multisig_register_signed_transaction",
    "connect_norito_encode_register_zk_asset_signed_transaction",
    "connect_norito_encode_remove_key_value_signed_transaction",
    "connect_norito_encode_set_key_value_signed_transaction",
    "connect_norito_encode_transfer_signed_transaction",
}
TRANSACTION_SIGNER_ALGORITHM_ONLY_EXPORTS = {
    "connect_norito_encode_governance_update_plain_conviction_signed_transaction_alg",
}
TRANSACTION_SIGNER_EXPORTS = TRANSACTION_SIGNER_BASE_EXPORTS | {
    f"{name}_alg" for name in TRANSACTION_SIGNER_BASE_EXPORTS
} | TRANSACTION_SIGNER_ALGORITHM_ONLY_EXPORTS


def split_parameters(value: str) -> list[str]:
    value = value.strip()
    if not value or value == "void":
        return []
    return [part.strip() for part in value.split(",") if part.strip()]


def header_exports(prefix: str) -> set[str]:
    return set(re.findall(
        rf'(?:int32_t|uint32_t|void)\s+({re.escape(prefix)}[a-z0-9_]+)\s*\(',
        header,
    ))


def exact(label: str, expected: set[str], actual: set[str]) -> None:
    if actual != expected:
        raise SystemExit(
            f"[connect-norito-header] {label} inventory mismatch: "
            f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )


def signer_template_suffix(template_name: str) -> tuple[str, list[str]]:
    match = re.search(
        rf'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
        rf'{re.escape(f"${template_name}")}\s*\(\s*'
        r'\$\(\s*\$argument\s*:\s*\$argument_type\s*,\s*\)\*\s*'
        rf'(.*?)\)\s*->\s*([^\s{{]+)\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse generated signer template: ${template_name}")
    return match.group(2), split_parameters(match.group(1))


GENERATED_RUST_SIGNATURES: dict[str, tuple[str, list[str]]] = {}


def register_generated_signature(name: str, return_type: str, parameters: list[str]) -> None:
    if name in GENERATED_RUST_SIGNATURES:
        raise SystemExit(f"duplicate generated Rust FFI export: {name}")
    GENERATED_RUST_SIGNATURES[name] = (return_type, parameters)


signer_default_return, signer_default_suffix = signer_template_suffix("default")
signer_algorithm_return, signer_algorithm_suffix = signer_template_suffix("with_algorithm")
signer_invocation_pattern = re.compile(
    r'define_ed25519_signed_transaction_wrapper!\s*\{\s*'
    r'(?P<default>connect_norito_encode_[a-z0-9_]+_signed_transaction)\s*=>\s*'
    r'(?P<algorithm>connect_norito_encode_[a-z0-9_]+_signed_transaction_alg)\s*'
    r'\((?P<arguments>.*?)\)\s*'
    r'identifiers:\s*\(\s*'
    r'(?P<algorithm_code>[A-Za-z_][A-Za-z0-9_]*)\s*,\s*'
    r'(?P<signed_bytes>[A-Za-z_][A-Za-z0-9_]*)\s*,\s*'
    r'(?P<hash_bytes>[A-Za-z_][A-Za-z0-9_]*)\s*\)\s*;',
    re.S,
)
for match in signer_invocation_pattern.finditer(rust):
    default_name = match.group("default")
    algorithm_name = match.group("algorithm")
    if algorithm_name != f"{default_name}_alg":
        raise SystemExit(f"generated signer algorithm export must pair with {default_name}")
    arguments = split_parameters(match.group("arguments"))
    register_generated_signature(
        default_name,
        signer_default_return,
        arguments + signer_default_suffix,
    )
    register_generated_signature(
        algorithm_name,
        signer_algorithm_return,
        arguments + [
            parameter.replace("$algorithm_code", match.group("algorithm_code"))
            for parameter in signer_algorithm_suffix
        ],
    )

DIRECT_RUST_EXPORTS = set(re.findall(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(',
    rust,
))
if set(GENERATED_RUST_SIGNATURES) & DIRECT_RUST_EXPORTS:
    raise SystemExit("Rust FFI exports overlap direct and generated definitions")


def rust_exports(prefix: str) -> set[str]:
    return {
        name
        for name in DIRECT_RUST_EXPORTS | set(GENERATED_RUST_SIGNATURES)
        if name.startswith(prefix)
    }


def canonical_rust_type(value: str) -> str:
    value = " ".join(value.strip().split())
    if value.startswith("*const "):
        return "const" + canonical_rust_type(value.removeprefix("*const ")) + "*"
    if value.startswith("*mut "):
        return canonical_rust_type(value.removeprefix("*mut ")) + "*"
    mapping = {
        "()": "void",
        "c_char": "char",
        "c_int": "int32_t",
        "std::ffi::c_int": "int32_t",
        "i32": "int32_t",
        "c_uchar": "uint8_t",
        "c_ulong": "unsignedlong",
        "usize": "size_t",
        "ConnectNoritoSorafsReferenceBundlePayload": "ConnectNoritoSorafsReferenceBundlePayload",
        "ConnectNoritoSorafsReferenceInput": "ConnectNoritoSorafsReferenceInput",
        "u8": "uint8_t",
        "u16": "uint16_t",
        "u32": "uint32_t",
        "u64": "uint64_t",
    }
    try:
        return mapping[value]
    except KeyError as error:
        raise SystemExit(f"unsupported Rust FFI type: {value}") from error


def canonical_c_type(value: str) -> str:
    match = re.fullmatch(r"(.+?)([A-Za-z_][A-Za-z0-9_]*)", value.strip(), re.S)
    if match is None:
        raise SystemExit(f"cannot parse C FFI parameter: {value}")
    return "".join(match.group(1).split())


def rust_signature(name: str) -> tuple[str, list[str]]:
    generated = GENERATED_RUST_SIGNATURES.get(name)
    if generated is not None:
        return (
            canonical_rust_type(generated[0]),
            [canonical_rust_type(value.split(":", 1)[1]) for value in generated[1]],
        )
    match = re.search(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(name)}\s*'
        rf'\((.*?)\)\s*(?:->\s*([^\s{{]+))?\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse Rust FFI signature: {name}")
    return (
        canonical_rust_type(match.group(2) or "()"),
        [canonical_rust_type(value.split(":", 1)[1]) for value in split_parameters(match.group(1))],
    )


def c_signature(name: str) -> tuple[str, list[str]]:
    match = re.search(
        rf'(int32_t|uint32_t|void)\s+{re.escape(name)}\s*\((.*?)\)\s*;',
        header,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse C FFI signature: {name}")
    return match.group(1), [canonical_c_type(value) for value in split_parameters(match.group(2))]


def require_signature_parity(names: set[str]) -> None:
    for name in sorted(names):
        rust_value = rust_signature(name)
        c_value = c_signature(name)
        if rust_value != c_value:
            raise SystemExit(
                f"Rust/C FFI signature mismatch for {name}: rust={rust_value}, c={c_value}"
            )


def parameter_names(parameters: str, rust_parameters: bool) -> list[str]:
    names = []
    for parameter in split_parameters(parameters):
        if rust_parameters:
            names.append(parameter.split(":", 1)[0].strip())
        else:
            match = re.search(r'([A-Za-z_][A-Za-z0-9_]*)\s*$', parameter)
            if match is None:
                raise SystemExit(f"cannot parse C parameter: {parameter}")
            names.append(match.group(1))
    return names


def rust_parameter_names(name: str) -> list[str]:
    generated = GENERATED_RUST_SIGNATURES.get(name)
    if generated is not None:
        return parameter_names(",".join(generated[1]), True)
    match = re.search(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(name)}\s*\((.*?)\)\s*'
        rf'(?:->\s*[^\s{{]+)?\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse Rust FFI parameters: {name}")
    return parameter_names(match.group(1), True)


def c_parameter_names(name: str) -> list[str]:
    match = re.search(
        rf'(?:int32_t|uint32_t|void)\s+{re.escape(name)}\s*\((.*?)\)\s*;',
        header,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse C FFI parameters: {name}")
    return parameter_names(match.group(1), False)


exact("Rust KAGEMUSHA", KAGEMUSHA_EXPORTS, rust_exports("connect_norito_kagemusha_"))
exact("C KAGEMUSHA", KAGEMUSHA_EXPORTS, header_exports("connect_norito_kagemusha_"))
exact("Rust privacy", PRIVACY_EXPORTS, rust_exports("iroha_privacy_"))
exact("C privacy", PRIVACY_EXPORTS, header_exports("iroha_privacy_"))
exact(
    "Rust SoraFS reference",
    SORAFS_REFERENCE_EXPORTS,
    rust_exports("connect_norito_sorafs_reference_"),
)
exact(
    "C SoraFS reference",
    SORAFS_REFERENCE_EXPORTS,
    header_exports("connect_norito_sorafs_reference_"),
)
exact(
    "Rust detached transaction",
    DETACHED_EXPORTS,
    rust_exports("connect_norito_detached_transaction_")
    | rust_exports("connect_norito_canonical_json_"),
)
exact(
    "C detached transaction",
    DETACHED_EXPORTS,
    header_exports("connect_norito_detached_transaction_")
    | header_exports("connect_norito_canonical_json_"),
)
exact("Rust Parliament timed-OVN", PARLIAMENT_EXPORTS, rust_exports("connect_norito_parliament_timed_ovn_"))
exact("C Parliament timed-OVN", PARLIAMENT_EXPORTS, header_exports("connect_norito_parliament_timed_ovn_"))
exact("Rust retail fee", RETAIL_EXPORTS, rust_exports("connect_norito_retail_fee_"))
exact("C retail fee", RETAIL_EXPORTS, header_exports("connect_norito_retail_fee_"))
# First-release artifacts cannot retain a second, retired fee protocol.
exact("retired Rust Hijiri", set(), rust_exports("connect_norito_validation_fee_hijiri_quote_"))
exact("retired C Hijiri", set(), header_exports("connect_norito_validation_fee_hijiri_quote_"))
exact("Rust private settlement", PRIVATE_SETTLEMENT_EXPORTS, rust_exports("connect_norito_private_settlement_"))
exact("C private settlement", PRIVATE_SETTLEMENT_EXPORTS, header_exports("connect_norito_private_settlement_"))

signer_name = re.compile(r"^connect_norito_encode_[a-z0-9_]+_signed_transaction(?:_alg)?$")
rust_transaction_signers = {
    name for name in rust_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
header_transaction_signers = {
    name for name in header_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
exact("Rust transaction signer", TRANSACTION_SIGNER_EXPORTS, rust_transaction_signers)
exact("C transaction signer", TRANSACTION_SIGNER_EXPORTS, header_transaction_signers)
for name in sorted(rust_transaction_signers):
    rust_names = rust_parameter_names(name)
    header_names = c_parameter_names(name)
    if rust_names[:2] != ["network_id_ptr", "network_id_len"]:
        raise SystemExit(f"Rust signer {name} must start with exact NetworkId pointer/length")
    if header_names[:2] != ["network_id", "network_id_len"]:
        raise SystemExit(f"C signer {name} must start with exact NetworkId pointer/length")
    rust_fee_index = rust_names.index("fee_payment_json_ptr")
    header_fee_index = header_names.index("fee_payment_json")
    if rust_names[rust_fee_index:rust_fee_index + 4] != [
        "fee_payment_json_ptr", "fee_payment_json_len", "private_key_ptr", "private_key_len"
    ]:
        raise SystemExit(f"Rust signer {name} fee/private-key argument ordering drift")
    if header_names[header_fee_index:header_fee_index + 4] != [
        "fee_payment_json", "fee_payment_json_len", "private_key", "private_key_len"
    ]:
        raise SystemExit(f"C signer {name} fee/private-key argument ordering drift")

require_signature_parity(
    KAGEMUSHA_EXPORTS
    | PRIVACY_EXPORTS
    | SORAFS_REFERENCE_EXPORTS
    | DETACHED_EXPORTS
    | PARLIAMENT_EXPORTS
    | RETAIL_EXPORTS
    | PRIVATE_SETTLEMENT_EXPORTS
    | rust_transaction_signers
    | {"connect_norito_bridge_abi_version", "connect_norito_free", "connect_norito_domain_id_validate_v1"}
)

require(r"#define\s+CONNECT_NORITO_BRIDGE_ABI_VERSION\s+25\b", header, "C bridge ABI version")
require(r"pub\s+const\s+PRIVACY_BRIDGE_ABI_VERSION_V1:\s*u32\s*=\s*25\s*;", privacy, "Rust bridge ABI version")
require(
    r"const\s+CONNECT_NORITO_BRIDGE_ABI_VERSION:\s*u32\s*=\s*PRIVACY_BRIDGE_ABI_VERSION_V1\s*;",
    rust,
    "bridge ABI binding",
)
for rust_name, value, header_name in (
    ("ERR_KAGEMUSHA_V1", "-311", "CONNECT_NORITO_ERR_KAGEMUSHA_V1"),
    (
        "ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1",
        "-312",
        "CONNECT_NORITO_ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1",
    ),
    ("ERR_PARLIAMENT_TIMED_OVN", "-505", "CONNECT_NORITO_ERR_PARLIAMENT_TIMED_OVN"),
    (
        "ERR_RETAIL_FEE_ASSESSMENT",
        "-506",
        "CONNECT_NORITO_ERR_RETAIL_FEE_ASSESSMENT",
    ),
):
    require(rf"const\s+{rust_name}\s*:\s*c_int\s*=\s*{value}\s*;", rust, rust_name)
    require(rf"#define\s+{header_name}\s+{value}(?:\s|$)", header, header_name)

for name, expected in {
    "CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1": "64",
    "CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_TOTAL_BYTES_V1": "67108864",
    "CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1": "64",
    "CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1": "32",
    "CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1": "67108864",
    "CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1": "1024",
}.items():
    require(rf"pub\s+const\s+{name}\s*:\s*u32\s*=\s*{expected}\s*;", rust, f"Rust {name}")
    require(rf"#define\s+{name}\s+{expected}\b", header, f"C {name}")

require(
    r"typedef\s+struct\s+ConnectNoritoSorafsReferenceInput\s*\{\s*"
    r"const\s+uint8_t\s*\*\s*bytes_ptr\s*;\s*size_t\s+bytes_len\s*;\s*"
    r"const\s+uint8_t\s*\*\s*label_ptr\s*;\s*size_t\s+label_len\s*;\s*"
    r"\}\s*ConnectNoritoSorafsReferenceInput\s*;",
    header,
    "SoraFS governance descriptor layout",
)
require(
    r"typedef\s+struct\s+ConnectNoritoSorafsReferenceBundlePayload\s*\{\s*"
    r"uint32_t\s+kind\s*;\s*const\s+uint8_t\s*\*\s*bytes_ptr\s*;\s*"
    r"size_t\s+bytes_len\s*;\s*const\s+uint8_t\s*\*\s*label_ptr\s*;\s*"
    r"size_t\s+label_len\s*;\s*\}\s*ConnectNoritoSorafsReferenceBundlePayload\s*;",
    header,
    "SoraFS bundle descriptor layout",
)

# These are the actual Native operation bounds, not retired header aliases.
for name, value in (
    ("RETAIL_FEE_BRIDGE_MAX_INPUT_BYTES", "262_144"),
    ("RETAIL_FEE_ASSESSMENT_MAX_BYTES", "4_096"),
    ("RETAIL_FEE_MARKER_MAX_BYTES", "4_096"),
):
    require(rf"const\s+{name}\s*:\s*usize\s*=\s*{value}\s*;", rust, name)
require(r"pub\s+const\s+RETAIL_FEE_ASSESSMENT_METADATA_KEY\s*:\s*&str\s*=\s*\"validation_fee_assessment\"", retail_model, "typed retail assessment metadata owner")
for name, maximum, operation in (
    ("connect_norito_retail_fee_intent_hash_v1", "RETAIL_FEE_BRIDGE_MAX_INPUT_BYTES", "retail_fee_intent_hash_v1"),
    ("connect_norito_retail_fee_assessment_marker_v1", "RETAIL_FEE_ASSESSMENT_MAX_BYTES", "retail_fee_assessment_marker_v1"),
    ("connect_norito_retail_fee_assessment_decode_v1", "RETAIL_FEE_MARKER_MAX_BYTES", "retail_fee_assessment_decode_v1"),
):
    require(rf"fn\s+{name}\b[^{{]+\{{.*?retail_fee_bridge_call\s*\([^;]*?\b{maximum}\s*,\s*{operation}\b", rust, f"{name} exact operation input bound")
require(r"value\.qualifying_payments\s*>\s*1_000", rust, "Native retail qualifying count cap")
require(r"bytes\.len\(\)\s*>\s*\(RETAIL_FEE_MARKER_MAX_BYTES\s*-\s*RETAIL_FEE_MARKER_PREFIX\.len\(\)\)\s*/\s*2", rust, "Native complete marker output cap")
require(r"json\.len\(\)\s*>\s*RETAIL_FEE_ASSESSMENT_MAX_BYTES", rust, "Native assessment JSON output cap")

require(r"pub\s+const\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1\s*:\s*usize\s*=\s*32\s*;", rust, "Parliament seed width")
require(r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1\s+32\b", header, "C Parliament seed width")
require(r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1\s+8388608\b", header, "C Parliament proof cap")
require(r"pub\s+const\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1\s*:\s*usize\s*=\s*41\s*;", rust, "Parliament page summary width")
require(r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1\s+41\b", header, "C Parliament page summary width")
require(r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1\s+32\b", header, "C Parliament trust anchor width")

def umbrella_contract(contents: str) -> list[str]:
    # Documentation and formatting do not alter the sole canonical include owner.
    without_comments = re.sub(r"/\*.*?\*/|//[^\n]*", "", contents, flags=re.S)
    return without_comments.split()


if umbrella_contract(umbrella) != [
    "#ifndef", "NORITOBRIDGE_H", "#define", "NORITOBRIDGE_H",
    "#include", '"connect_norito_bridge.h"', "#endif",
]:
    raise SystemExit("[connect-norito-header] umbrella header drift")

print(
    "[connect-norito-header] ABI 25 synchronized: "
    f"{len(KAGEMUSHA_EXPORTS)} KAGEMUSHA, {len(PRIVACY_EXPORTS)} privacy, "
    f"{len(SORAFS_REFERENCE_EXPORTS)} SoraFS, {len(DETACHED_EXPORTS)} detached, "
    f"{len(PARLIAMENT_EXPORTS)} Parliament, {len(RETAIL_EXPORTS)} retail-fee, "
    f"{len(PRIVATE_SETTLEMENT_EXPORTS)} private-settlement, and "
    f"{len(TRANSACTION_SIGNER_EXPORTS)} transaction-signer exports"
)
PY
}

compile_header() {
  if ! command -v "${CC:-cc}" >/dev/null 2>&1; then
    echo "[connect-norito-header] required C compiler not found: ${CC:-cc}" >&2
    exit 1
  fi
  if ! command -v "${CXX:-c++}" >/dev/null 2>&1; then
    echo "[connect-norito-header] required C++ compiler not found: ${CXX:-c++}" >&2
    exit 1
  fi

  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-bridge-header-compile.XXXXXX")"
  trap 'rm -rf "${tmp}"' RETURN
  printf '#include "%s"\nint main(void) { return 0; }\n' "${HEADER}" >"${tmp}/header.c"
  printf '#include "%s"\nint main() { return 0; }\n' "${HEADER}" >"${tmp}/header.cc"
  "${CC:-cc}" -std=c11 -fsyntax-only "${tmp}/header.c"
  "${CXX:-c++}" -std=c++17 -fsyntax-only "${tmp}/header.cc"
}

replace_once() {
  local path="$1"
  local source="$2"
  local replacement="$3"
  python3 - "${path}" "${source}" "${replacement}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
source = sys.argv[2]
replacement = sys.argv[3]
text = path.read_text(encoding="utf-8")
if text.count(source) != 1:
    raise SystemExit(f"negative-control mutation count is not one for {source!r}")
path.write_text(text.replace(source, replacement), encoding="utf-8")
PY
}

replace_regex_once() {
  local path="$1"
  local pattern="$2"
  local replacement="$3"
  python3 - "${path}" "${pattern}" "${replacement}" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
pattern = sys.argv[2]
replacement = sys.argv[3]
text = path.read_text(encoding="utf-8")
updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
if count != 1:
    raise SystemExit(
        f"negative-control regex mutation count must be one (found {count}): {pattern}"
    )
path.write_text(updated, encoding="utf-8")
PY
}

make_negative_workspace() {
  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-bridge-header.XXXXXX")"
  cp "${RUST_LIB}" "${tmp}/lib.rs"
  cp "${PARLIAMENT_RUST}" "${tmp}/parliament_timed_ovn_ffi.rs"
  cp "${PRIVATE_SETTLEMENT_RUST}" "${tmp}/private_settlement_ffi.rs"
  cp "${RESERVE_FINALITY_RUST}" "${tmp}/kagemusha_reserve_finality_v1.rs"
  cp "${TESTNET_OBSERVATION_RUST}" "${tmp}/kagemusha_testnet_observation_v1.rs"
  cp "${TESTNET_VALUE_LEDGER_RUST}" "${tmp}/kagemusha_testnet_native_value_ledger_v1.rs"
  cp "${TESTNET_STARTUP_RUST}" "${tmp}/kagemusha_testnet_native_startup_v1.rs"
  cp "${ORDINARY_STARTUP_RUST}" "${tmp}/ordinary_native_startup.rs"
  cp "${ORDINARY_CURRENT_RUST}" "${tmp}/ordinary_current_control.rs"
  cp "${ORDINARY_OUTGOING_RUST}" "${tmp}/ordinary_outgoing_driver.rs"
  cp "${PRIVACY_MODEL}" "${tmp}/privacy.rs"
  cp "${RETAIL_MODEL}" "${tmp}/retail_fee_model.rs"
  cp "${HEADER}" "${tmp}/connect_norito_bridge.h"
  cp "${UMBRELLA}" "${tmp}/NoritoBridge.h"
  printf '%s' "${tmp}"
}

expect_contract_rejection() {
  local tmp="$1"
  local expected_diagnostic="${2:-}"
  local output
  if output="$(run_contract_check \
      "${tmp}/lib.rs" \
      "${tmp}/connect_norito_bridge.h" \
      "${tmp}/NoritoBridge.h" \
      "${tmp}/privacy.rs" \
      "${tmp}/parliament_timed_ovn_ffi.rs" \
      "${tmp}/retail_fee_model.rs" \
      "${tmp}/private_settlement_ffi.rs" \
      "${tmp}/kagemusha_reserve_finality_v1.rs" \
      "${tmp}/kagemusha_testnet_observation_v1.rs" \
      "${tmp}/kagemusha_testnet_native_value_ledger_v1.rs" \
      "${tmp}/kagemusha_testnet_native_startup_v1.rs" \
      "${tmp}/ordinary_native_startup.rs" \
      "${tmp}/ordinary_current_control.rs" \
      "${tmp}/ordinary_outgoing_driver.rs" 2>&1)"; then
    echo "[connect-norito-header] negative control unexpectedly passed: ${MODE}" >&2
    exit 1
  fi
  if [[ -n "${expected_diagnostic}" && "${output}" != *"${expected_diagnostic}"* ]]; then
    echo "[connect-norito-header] negative control rejected for the wrong reason: ${MODE}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "[connect-norito-header] negative control rejected expected drift: ${MODE}"
}

if [[ "${MODE}" == "--self-test" ]]; then
  "${BASH_SOURCE[0]}"
  for control in "${SELF_TESTS[@]}"; do
    "${BASH_SOURCE[0]}" "${control}"
  done
  exit 0
fi

if [[ "${MODE}" == --self-test-* ]]; then
  # Prove the authoritative inputs pass before mutating a private copy. This
  # prevents an unrelated source error from masquerading as a negative test.
  run_contract_check \
    "${RUST_LIB}" \
    "${HEADER}" \
    "${UMBRELLA}" \
    "${PRIVACY_MODEL}" \
    "${PARLIAMENT_RUST}" \
    "${RETAIL_MODEL}" \
    "${PRIVATE_SETTLEMENT_RUST}" \
    "${RESERVE_FINALITY_RUST}" \
    "${TESTNET_OBSERVATION_RUST}" \
    "${TESTNET_VALUE_LEDGER_RUST}" \
    "${TESTNET_STARTUP_RUST}" \
    "${ORDINARY_STARTUP_RUST}" \
    "${ORDINARY_CURRENT_RUST}" \
    "${ORDINARY_OUTGOING_RUST}" >/dev/null
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT
  tmp_rust="${tmp}/lib.rs"
  tmp_header="${tmp}/connect_norito_bridge.h"
  tmp_umbrella="${tmp}/NoritoBridge.h"
  expected_diagnostic=""

  case "${MODE}" in
    --self-test-missing-domain-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_domain_id_validate_v1" "removed_domain_id_validate_v1"
      ;;
    --self-test-missing-domain-rust-symbol)
      replace_once "${tmp_rust}" \
        'pub unsafe extern "C" fn connect_norito_domain_id_validate_v1' \
        'pub unsafe extern "C" fn removed_domain_id_validate_v1'
      ;;
    --self-test-bad-domain-length-width)
      replace_once "${tmp_header}" \
        'connect_norito_domain_id_validate_v1(const char* input, unsigned long input_len)' \
        'connect_norito_domain_id_validate_v1(const char* input, uint32_t input_len)'
      ;;
    --self-test-bad-parliament-page-rust-width)
      replace_once "${tmp}/parliament_timed_ovn_ffi.rs" \
        'pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1: usize = 41;' \
        'pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1: usize = 40;'
      expected_diagnostic="missing or invalid Parliament page summary width"
      ;;
    --self-test-bad-parliament-page-header-width)
      replace_once "${tmp_header}" \
        '#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1 41' \
        '#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1 40'
      expected_diagnostic="missing or invalid C Parliament page summary width"
      ;;
    --self-test-retired-parliament-page-rust-name)
      replace_once "${tmp}/parliament_timed_ovn_ffi.rs" \
        'pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1:' \
        'pub const CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1:'
      expected_diagnostic="missing or invalid Parliament page summary width"
      ;;
    --self-test-retired-parliament-page-header-name)
      replace_once "${tmp_header}" \
        '#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_SUMMARY_BYTES_V1 ' \
        '#define CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1 '
      expected_diagnostic="missing or invalid C Parliament page summary width"
      ;;
    --self-test-missing-reserve-finality-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_reserve_finality_hint_v1' \
        'removed_reserve_finality_hint_v1'
      ;;
    --self-test-missing-reserve-finality-rust-symbol)
      replace_once "${tmp}/kagemusha_reserve_finality_v1.rs" \
        'connect_norito_kagemusha_reserve_finality_verify_v1' \
        'removed_reserve_finality_verify_v1'
      ;;
    --self-test-bad-reserve-finality-checkpoint-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_reserve_finality_verify_v1\s*\([^;]*?)unsigned long trusted_checkpoint_len' \
        '\g<1>uint32_t trusted_checkpoint_len'
      ;;
    --self-test-missing-reserve-finality-request-binding)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_reserve_finality_verify_v1\s*\([^;]*?)const uint8_t\* expected_request,\s*unsigned long expected_request_len,\s*' \
        '\g<1>'
      ;;
    --self-test-missing-top-up-binding-header)
      replace_once "${tmp_header}" 'connect_norito_kagemusha_top_up_signed_request_validate_v1' 'removed_top_up_binding_v1'
      ;;
    --self-test-missing-top-up-binding-rust)
      replace_once "${tmp}/kagemusha_reserve_finality_v1.rs" 'connect_norito_kagemusha_top_up_signed_request_validate_v1' 'removed_top_up_binding_v1'
      ;;
    --self-test-bad-top-up-binding-width)
      replace_regex_once "${tmp_header}" '(connect_norito_kagemusha_top_up_signed_request_validate_v1\s*\([^;]*?)unsigned long signed_transaction_len' '\g<1>uint32_t signed_transaction_len'
      ;;
    --self-test-missing-top-up-binding-request)
      replace_regex_once "${tmp_header}" '(connect_norito_kagemusha_top_up_signed_request_validate_v1\s*\([^;]*?),\s*const uint8_t \*expected_request_ptr, unsigned long expected_request_len' '\g<1>'
      ;;
    --self-test-bad-abi)
      replace_once "${tmp_header}" \
        "#define CONNECT_NORITO_BRIDGE_ABI_VERSION 25" \
        "#define CONNECT_NORITO_BRIDGE_ABI_VERSION 22"
      ;;
    --self-test-missing-ordinary-startup-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_ordinary_runtime_startup_v1' \
        'removed_ordinary_startup_v1'
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_runtime_startup_v1']"
      ;;
    --self-test-missing-ordinary-startup-rust-symbol)
      replace_once "${tmp}/ordinary_native_startup.rs" \
        'pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_runtime_startup_v1' \
        'pub unsafe extern "C" fn removed_ordinary_startup_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_runtime_startup_v1']"
      ;;
    --self-test-bad-ordinary-startup-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_ordinary_runtime_startup_v1\s*\([^;]*?)uint64_t id' \
        '\g<1>uint32_t id'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_ordinary_runtime_startup_v1"
      ;;
    --self-test-missing-ordinary-current-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_ordinary_current_control_v1' \
        'removed_ordinary_current_v1'
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_current_control_v1']"
      ;;
    --self-test-missing-ordinary-current-rust-symbol)
      replace_once "${tmp}/ordinary_current_control.rs" \
        'pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_current_control_v1' \
        'pub unsafe extern "C" fn removed_ordinary_current_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_current_control_v1']"
      ;;
    --self-test-bad-ordinary-current-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_ordinary_current_control_v1\s*\([^;]*?)uint64_t core_handle' \
        '\g<1>uint32_t core_handle'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_ordinary_current_control_v1"
      ;;
    --self-test-missing-ordinary-outgoing-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_ordinary_outgoing_v1' \
        'removed_ordinary_outgoing_v1'
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_outgoing_v1']"
      ;;
    --self-test-missing-ordinary-outgoing-rust-symbol)
      replace_once "${tmp}/ordinary_outgoing_driver.rs" \
        'pub unsafe extern "C" fn connect_norito_kagemusha_ordinary_outgoing_v1' \
        'pub unsafe extern "C" fn removed_ordinary_outgoing_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_ordinary_outgoing_v1']"
      ;;
    --self-test-bad-ordinary-outgoing-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_ordinary_outgoing_v1\s*\([^;]*?)size_t input_len' \
        '\g<1>uint32_t input_len'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_ordinary_outgoing_v1"
      ;;
    --self-test-missing-kagemusha-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_v1_payment_validate" \
        "removed_kagemusha_v1_payment_validate"
      ;;
    --self-test-missing-kagemusha-install-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_core_coordinator_install_v1" \
        "removed_kagemusha_core_coordinator_install_v1"
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_core_coordinator_install_v1']"
      ;;
    --self-test-missing-kagemusha-install-rust-symbol)
      replace_once "${tmp_rust}" \
        'pub unsafe extern "C" fn connect_norito_kagemusha_core_coordinator_install_v1' \
        'pub unsafe extern "C" fn removed_kagemusha_core_coordinator_install_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_core_coordinator_install_v1']"
      ;;
    --self-test-bad-kagemusha-install-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_core_coordinator_install_v1\s*\([^;]*?)size_t storage_path_length' \
        '\g<1>uint32_t storage_path_length'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_core_coordinator_install_v1"
      ;;
    --self-test-missing-kagemusha-close-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_core_coordinator_close_v1" \
        "removed_kagemusha_core_coordinator_close_v1"
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_core_coordinator_close_v1']"
      ;;
    --self-test-missing-kagemusha-testnet-observation-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_testnet_state_proof_observe_v1" \
        "removed_kagemusha_testnet_state_proof_observe_v1"
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_testnet_state_proof_observe_v1']"
      ;;
    --self-test-missing-kagemusha-rust-symbol)
      replace_once "${tmp_rust}" \
        "connect_norito_kagemusha_v1_payment_validate" \
        "removed_kagemusha_v1_payment_validate"
      ;;
    --self-test-bad-kagemusha-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_v1_payment_validate\s*\([^;]*?)unsigned long payment_len' \
        '\g<1>uint32_t payment_len'
      ;;
    --self-test-missing-kagemusha-command-binding)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_device_command_response_v1_verify\s*\([^;]*?)const uint8_t\* canonical_command,\s*size_t canonical_command_len,\s*' \
        '\g<1>'
      ;;
    --self-test-bad-kagemusha-error-code)
      replace_once "${tmp_header}" \
        "#define CONNECT_NORITO_ERR_KAGEMUSHA_V1 -311" \
        "#define CONNECT_NORITO_ERR_KAGEMUSHA_V1 -310"
      ;;
    --self-test-missing-kagemusha-mint-stage-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_device_mint_stage_command_v1_validate" \
        "removed_kagemusha_device_mint_stage_command_v1_validate"
      ;;
    --self-test-bad-kagemusha-mint-stage-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_device_mint_stage_result_v1_validate\s*\([^;]*?)unsigned long result_len' \
        '\g<1>uint32_t result_len'
      ;;
    --self-test-missing-finalized-mint-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_testnet_finalized_mint_observe_v1' \
        'removed_testnet_finalized_mint_observe_v1'
      ;;
    --self-test-missing-finalized-mint-rust-symbol)
      replace_regex_once "${tmp}/kagemusha_testnet_observation_v1.rs" \
        '(pub unsafe extern "C" fn )connect_norito_kagemusha_testnet_finalized_mint_observe_v1' \
        '\g<1>removed_testnet_finalized_mint_observe_v1'
      ;;
    --self-test-missing-testnet-value-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_testnet_value_admit_v1' \
        'removed_testnet_value_admit_v1'
      ;;
    --self-test-missing-testnet-value-rust-symbol)
      replace_regex_once "${tmp}/kagemusha_testnet_observation_v1.rs" \
        '(pub unsafe extern "C" fn )connect_norito_kagemusha_testnet_value_admit_v1' \
        '\g<1>removed_testnet_value_admit_v1'
      ;;
    --self-test-missing-testnet-credit-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_testnet_value_credit_v1' \
        'removed_testnet_value_credit_v1'
      ;;
    --self-test-missing-testnet-credit-rust-symbol)
      replace_regex_once "${tmp}/kagemusha_testnet_native_value_ledger_v1.rs" \
        '(pub unsafe extern "C" fn )connect_norito_kagemusha_testnet_value_credit_v1' \
        '\g<1>removed_testnet_value_credit_v1'
      ;;
    --self-test-missing-testnet-startup-contract-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_testnet_native_startup_contract_v1' \
        'removed_testnet_native_startup_contract_v1'
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_testnet_native_startup_contract_v1']"
      ;;
    --self-test-missing-testnet-startup-contract-rust-symbol)
      replace_regex_once "${tmp}/kagemusha_testnet_native_startup_v1.rs" \
        '(pub unsafe extern "C" fn )connect_norito_kagemusha_testnet_native_startup_contract_v1' \
        '\g<1>removed_testnet_native_startup_contract_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_testnet_native_startup_contract_v1']"
      ;;
    --self-test-missing-testnet-startup-activate-header-symbol)
      replace_once "${tmp_header}" \
        'connect_norito_kagemusha_testnet_native_startup_activate_v1' \
        'removed_testnet_native_startup_activate_v1'
      expected_diagnostic="C KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_testnet_native_startup_activate_v1']"
      ;;
    --self-test-missing-testnet-startup-activate-rust-symbol)
      replace_regex_once "${tmp}/kagemusha_testnet_native_startup_v1.rs" \
        '(pub unsafe extern "C" fn )connect_norito_kagemusha_testnet_native_startup_activate_v1' \
        '\g<1>removed_testnet_native_startup_activate_v1'
      expected_diagnostic="Rust KAGEMUSHA inventory mismatch: missing=['connect_norito_kagemusha_testnet_native_startup_activate_v1']"
      ;;
    --self-test-bad-testnet-startup-contract-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_testnet_native_startup_contract_v1\s*\(\s*)uint32_t\* output' \
        '\g<1>uint64_t* output'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_testnet_native_startup_contract_v1"
      ;;
    --self-test-bad-testnet-startup-activate-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_testnet_native_startup_activate_v1\s*\([^;]*?)size_t signed_bootstrap_length' \
        '\g<1>uint32_t signed_bootstrap_length'
      expected_diagnostic="Rust/C FFI signature mismatch for connect_norito_kagemusha_testnet_native_startup_activate_v1"
      ;;
    --self-test-missing-privacy-header-symbol)
      replace_once "${tmp_header}" \
        "iroha_privacy_compiled_profile_catalog_v1" \
        "removed_iroha_privacy_compiled_profile_catalog_v1"
      ;;
    --self-test-bad-privacy-signature)
      replace_regex_once "${tmp_header}" \
        '(iroha_privacy_compiled_profile_catalog_v1\s*\([^;]*?)unsigned long\* out_len' \
        '\g<1>unsigned long out_len'
      ;;
    --self-test-missing-privacy-rust-symbol)
      replace_once "${tmp_rust}" \
        'pub unsafe extern "C" fn iroha_privacy_compiled_profile_catalog_v1' \
        'pub unsafe extern "C" fn removed_iroha_privacy_compiled_profile_catalog_v1'
      ;;
    --self-test-extra-privacy-symbol)
      replace_once "${tmp_header}" \
        $'#ifdef __cplusplus\nextern "C" {' \
        $'int32_t iroha_privacy_retired_v1(void);\n\n#ifdef __cplusplus\nextern "C" {'
      ;;
    --self-test-missing-parliament-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_parliament_timed_ovn_ballot_from_proof_v1" \
        "removed_connect_norito_parliament_timed_ovn_ballot_from_proof_v1"
      ;;
    --self-test-missing-retail-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_retail_fee_assessment_decode_v1" \
        "removed_connect_norito_retail_fee_assessment_decode_v1"
      ;;
    --self-test-bad-retail-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_retail_fee_assessment_decode_v1\s*\(\s*const uint8_t\* input,\s*)unsigned long input_len' \
        '\g<1>uint32_t input_len'
      ;;
    --self-test-bad-retail-marker-bound)
      replace_once "${tmp_rust}" \
        "const RETAIL_FEE_MARKER_MAX_BYTES: usize = 4_096;" \
        "const RETAIL_FEE_MARKER_MAX_BYTES: usize = 4_095;"
      ;;
    --self-test-bad-retail-intent-bound)
      replace_once "${tmp_rust}" \
        "const RETAIL_FEE_BRIDGE_MAX_INPUT_BYTES: usize = 262_144;" \
        "const RETAIL_FEE_BRIDGE_MAX_INPUT_BYTES: usize = 262_145;"
      ;;
    --self-test-bad-retail-assessment-bound)
      replace_once "${tmp_rust}" \
        "const RETAIL_FEE_ASSESSMENT_MAX_BYTES: usize = 4_096;" \
        "const RETAIL_FEE_ASSESSMENT_MAX_BYTES: usize = 4_097;"
      ;;
    --self-test-retired-fee-header-symbol)
      printf '\nint32_t connect_norito_validation_fee_hijiri_quote_request_v1(void);\n' >> "${tmp_header}"
      ;;
    --self-test-missing-sorafs-reference-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json" \
        "removed_connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json"
      ;;
    --self-test-missing-sorafs-reference-rust-symbol)
      replace_once "${tmp_rust}" \
        'pub unsafe extern "C" fn connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json' \
        'pub unsafe extern "C" fn removed_connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json'
      ;;
    --self-test-bad-sorafs-reference-bundle-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_sorafs_reference_validate_bundle_json\s*\(\s*)const ConnectNoritoSorafsReferenceBundlePayload\*' \
        '\g<1>ConnectNoritoSorafsReferenceBundlePayload*'
      ;;
    --self-test-bad-sorafs-reference-bundle-layout)
      replace_regex_once "${tmp_header}" \
        '(typedef struct ConnectNoritoSorafsReferenceBundlePayload\s*\{\s*)uint32_t kind' \
        '\g<1>uint16_t kind'
      ;;
    --self-test-bad-sorafs-reference-bundle-limit)
      replace_once "${tmp_header}" \
        "#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1 64" \
        "#define CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1 63"
      ;;
    --self-test-missing-generated-transaction-signer)
      replace_once "${tmp_rust}" \
        "    connect_norito_encode_burn_signed_transaction =>" \
        "    removed_connect_norito_encode_burn_signed_transaction =>"
      ;;
    --self-test-missing-conviction-update-signer-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_encode_governance_update_plain_conviction_signed_transaction_alg" \
        "removed_governance_update_plain_conviction_signed_transaction_alg"
      expected_diagnostic="C transaction signer inventory mismatch: missing=['connect_norito_encode_governance_update_plain_conviction_signed_transaction_alg']"
      ;;
    --self-test-bad-generated-transaction-signer-signature)
      replace_once "${tmp_rust}" \
        '$algorithm_code: u8,' \
        '$algorithm_code: u16,'
      ;;
    --self-test-forbidden-retired-transaction-signer)
      replace_once "${tmp_rust}" \
        "    connect_norito_encode_burn_signed_transaction =>" \
        "    connect_norito_encode_shield_signed_transaction =>"
      ;;
    --self-test-bad-deallocator-signature)
      replace_once "${tmp_header}" \
        "void connect_norito_free(uint8_t *ptr);" \
        "void connect_norito_free(const uint8_t *ptr);"
      ;;
    --self-test-umbrella-comment-growth)
      printf '\n/* Documentary growth preserves the canonical include owner. */\n// Additional source guidance.\n' >> "${tmp_umbrella}"
      run_contract_check \
        "${tmp_rust}" "${tmp_header}" "${tmp_umbrella}" \
        "${tmp}/privacy.rs" "${tmp}/parliament_timed_ovn_ffi.rs" \
        "${tmp}/retail_fee_model.rs" "${tmp}/private_settlement_ffi.rs" \
        "${tmp}/kagemusha_reserve_finality_v1.rs" \
        "${tmp}/kagemusha_testnet_observation_v1.rs" \
        "${tmp}/kagemusha_testnet_native_value_ledger_v1.rs" \
        "${tmp}/kagemusha_testnet_native_startup_v1.rs" \
        "${tmp}/ordinary_native_startup.rs" \
        "${tmp}/ordinary_current_control.rs" \
        "${tmp}/ordinary_outgoing_driver.rs"
      echo "[connect-norito-header] positive control preserved canonical umbrella: ${MODE}"
      exit 0
      ;;
    --self-test-umbrella-drift)
      replace_once "${tmp_umbrella}" \
        '#include "connect_norito_bridge.h"' \
        '#include "wrong_bridge.h"'
      ;;
    *)
      usage
      exit 2
      ;;
  esac

  expect_contract_rejection "${tmp}" "${expected_diagnostic}"
  exit 0
fi

if [[ -n "${MODE}" ]]; then
  usage
  exit 2
fi

run_contract_check \
  "${RUST_LIB}" \
  "${HEADER}" \
  "${UMBRELLA}" \
  "${PRIVACY_MODEL}" \
  "${PARLIAMENT_RUST}" \
  "${RETAIL_MODEL}" \
  "${PRIVATE_SETTLEMENT_RUST}" \
  "${RESERVE_FINALITY_RUST}" \
  "${TESTNET_OBSERVATION_RUST}" \
  "${TESTNET_VALUE_LEDGER_RUST}" \
  "${TESTNET_STARTUP_RUST}" \
  "${ORDINARY_STARTUP_RUST}" \
  "${ORDINARY_CURRENT_RUST}" \
  "${ORDINARY_OUTGOING_RUST}"
compile_header
