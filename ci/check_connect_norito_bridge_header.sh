#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIB="${ROOT_DIR}/crates/connect_norito_bridge/src/lib.rs"
PARLIAMENT_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/parliament_timed_ovn_ffi.rs"
PRIVATE_SETTLEMENT_RUST="${ROOT_DIR}/crates/connect_norito_bridge/src/private_settlement_ffi.rs"
DATA_MODEL_PRIVACY="${ROOT_DIR}/crates/iroha_data_model/src/privacy/protocol.rs"
HIJIRI_API="${ROOT_DIR}/crates/iroha_torii_shared/src/validation_fee_api.rs"
HEADER="${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h"
UMBRELLA="${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h"
SWIFT_CONTRACT="${ROOT_DIR}/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
MODE="${1:-}"

SELF_TESTS=(
  --self-test-bad-abi
  --self-test-missing-header-symbol
  --self-test-forbidden-v3-alias
  --self-test-forbidden-lineage-v1
  --self-test-forbidden-auth-create-v2
  --self-test-bad-capability-signature
  --self-test-bad-proof-signature
  --self-test-bad-artifact-signature
  --self-test-missing-export-pair
  --self-test-missing-generated-kagemusha-rust-symbol
  --self-test-missing-generated-transaction-signer
  --self-test-bad-generated-transaction-signer-signature
  --self-test-forbidden-retired-transaction-signer
  --self-test-missing-protocol-export
  --self-test-bad-receiver-key-signature
  --self-test-bad-verification-time-signature
  --self-test-bad-acknowledgement-signature
  --self-test-bad-capability-rust-signature
  --self-test-missing-swift-symbol
  --self-test-bad-deallocator-signature
  --self-test-bad-secret-deallocator-signature
  --self-test-missing-rust-symbol
  --self-test-missing-privacy-header-symbol
  --self-test-bad-privacy-signature
  --self-test-missing-privacy-rust-symbol
  --self-test-missing-parliament-header-symbol
  --self-test-missing-hijiri-header-symbol
  --self-test-bad-hijiri-signature
  --self-test-bad-hijiri-constant
  --self-test-missing-sorafs-reference-header-symbol
  --self-test-missing-sorafs-reference-rust-symbol
  --self-test-bad-sorafs-reference-bundle-signature
  --self-test-bad-sorafs-reference-bundle-layout
  --self-test-bad-sorafs-reference-bundle-limit
  --self-test-umbrella-drift
)

usage() {
  echo "usage: ci/check_connect_norito_bridge_header.sh [--self-test]" >&2
}

run_contract_check() {
  local rust_lib="$1"
  local header="$2"
  local umbrella="$3"
  local swift_contract="$4"
  local data_model_privacy="$5"
  local parliament_rust="$6"
  local hijiri_api="$7"
  local private_settlement_rust="$8"

  python3 - "${rust_lib}" "${header}" "${umbrella}" "${swift_contract}" "${data_model_privacy}" "${parliament_rust}" "${hijiri_api}" "${private_settlement_rust}" <<'PY'
from pathlib import Path
import re
import sys

rust = Path(sys.argv[1]).read_text(encoding="utf-8")
header = Path(sys.argv[2]).read_text(encoding="utf-8")
umbrella = Path(sys.argv[3]).read_text(encoding="utf-8")
swift = Path(sys.argv[4]).read_text(encoding="utf-8")
privacy_model = Path(sys.argv[5]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[6]).read_text(encoding="utf-8")
hijiri_api = Path(sys.argv[7]).read_text(encoding="utf-8")
rust += "\n" + Path(sys.argv[8]).read_text(encoding="utf-8")

KAGEMUSHA_EXPORTS = {
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_recipient_lineage_query_create_v2",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
    "connect_norito_kagemusha_recipient_receive_offer_create_v2",
    "connect_norito_kagemusha_recipient_receive_offer_project_v2",
    "connect_norito_kagemusha_recipient_receive_offer_verify_v2",
    "connect_norito_kagemusha_output_membership_frontier_build_v4",
    "connect_norito_kagemusha_output_membership_paths_derive_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v4",
    "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_topup_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
    "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_topup_finality_verify_v4",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
    "connect_norito_kagemusha_secret_free_buffer",
}
KAGEMUSHA_CANDIDATE_LAB_EXPORTS = {
    "connect_norito_kagemusha_recursive_spend_candidate_lab_accepted_identity_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_append_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_init_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_verify_v4",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_proof_phase_v1",
    "connect_norito_kagemusha_recursive_spend_candidate_lab_apple_restart_phase_v1",
}
FORBIDDEN_FIRST_RELEASE_EXPORTS = {
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_validate_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
}

required_privacy_ffi = (
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
)
PRIVACY_EXPORTS = set(required_privacy_ffi)

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

PARLIAMENT_TIMED_OVN_EXPORTS = {
    "connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1",
    "connect_norito_parliament_timed_ovn_verify_casting_proof_v1",
    "connect_norito_parliament_timed_ovn_ballot_from_proof_v1",
    "connect_norito_parliament_timed_ovn_registration_from_proof_v1",
}

VALIDATION_FEE_HIJIRI_QUOTE_EXPORTS = {
    "connect_norito_validation_fee_hijiri_quote_request_v1",
    "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
}

PRIVATE_SETTLEMENT_RESPONSE_EXPORTS = {
    "connect_norito_private_settlement_committee_proof_response_verify_v1",
    "connect_norito_private_settlement_auditor_capsule_response_verify_v1",
    "connect_norito_private_settlement_audit_approval_response_verify_v1",
}

TRANSACTION_SIGNER_BASE_EXPORTS = {
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
TRANSACTION_SIGNER_EXPORTS = TRANSACTION_SIGNER_BASE_EXPORTS | {
    f"{name}_alg" for name in TRANSACTION_SIGNER_BASE_EXPORTS
}

def header_exports(prefix: str) -> set[str]:
    return set(re.findall(
        rf'(?:int32_t|uint32_t|void)\s+({re.escape(prefix)}[a-z0-9_]+)\s*\(',
        header,
    ))

def exact(label: str, expected: set[str], actual: set[str]) -> None:
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise SystemExit(f"{label} inventory mismatch: missing={missing}, extra={extra}")

def split_parameters(value: str) -> list[str]:
    value = value.strip()
    if not value or value == "void":
        return []
    return [part.strip() for part in value.split(",") if part.strip()]

def generated_template_signature(template_name: str) -> tuple[str, list[str]]:
    match = re.search(
        rf'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
        rf'{re.escape(f"${template_name}")}\s*'
        rf'\((.*?)\)\s*->\s*([^\s{{]+)\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse generated Rust FFI template: ${template_name}")
    return match.group(2), split_parameters(match.group(1))

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
        raise SystemExit(f"cannot parse signer Rust FFI template: ${template_name}")
    return match.group(2), split_parameters(match.group(1))

GENERATED_RUST_SIGNATURES: dict[str, tuple[str, list[str]]] = {}

def register_generated_signature(
    name: str,
    return_type: str,
    parameters: list[str],
) -> None:
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
        raise SystemExit(
            f"generated signer algorithm export must pair with {default_name}: {algorithm_name}"
        )
    arguments = split_parameters(match.group("arguments"))
    register_generated_signature(
        default_name,
        signer_default_return,
        arguments + signer_default_suffix,
    )
    algorithm_suffix = [
        parameter.replace("$algorithm_code", match.group("algorithm_code"))
        for parameter in signer_algorithm_suffix
    ]
    register_generated_signature(
        algorithm_name,
        signer_algorithm_return,
        arguments + algorithm_suffix,
    )

kagemusha_lifecycle_invocation_pattern = re.compile(
    r'^\s*kagemusha_recursive_spend_lifecycle_exports!\s*\{(?P<body>.*?)^\s*\}',
    re.M | re.S,
)
kagemusha_lifecycle_name_pattern = re.compile(
    r'=>\s*'
    r'(?P<name>connect_norito_kagemusha_recursive_spend_'
    r'(?:candidate_lab_)?(?P<role>init|append|verify|redeem)_v4)\s*,',
)
for invocation in kagemusha_lifecycle_invocation_pattern.finditer(rust):
    for match in kagemusha_lifecycle_name_pattern.finditer(invocation.group("body")):
        return_type, parameters = generated_template_signature(f'{match.group("role")}_name')
        register_generated_signature(match.group("name"), return_type, parameters)

DIRECT_RUST_EXPORTS = set(re.findall(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(',
    rust,
))
generated_direct_overlap = set(GENERATED_RUST_SIGNATURES) & DIRECT_RUST_EXPORTS
if generated_direct_overlap:
    raise SystemExit(
        f"Rust FFI exports are both direct and macro-generated: {sorted(generated_direct_overlap)}"
    )

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
        raise SystemExit(f"unsupported Rust FFI type in bridge checker: {value}") from error

def canonical_c_type(value: str) -> str:
    match = re.fullmatch(r"(.+?)([A-Za-z_][A-Za-z0-9_]*)", value.strip(), re.S)
    if match is None:
        raise SystemExit(f"cannot parse C FFI parameter: {value}")
    return "".join(match.group(1).split())

def rust_signature(name: str) -> tuple[str, list[str]]:
    generated = GENERATED_RUST_SIGNATURES.get(name)
    if generated is not None:
        return_type, raw_parameters = generated
        parameters = []
        for parameter in raw_parameters:
            if ":" not in parameter:
                raise SystemExit(f"cannot parse generated Rust FFI parameter for {name}: {parameter}")
            parameters.append(canonical_rust_type(parameter.split(":", 1)[1]))
        return canonical_rust_type(return_type), parameters
    match = re.search(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(name)}\s*'
        rf'\((.*?)\)\s*(?:->\s*([^\s{{]+))?\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse Rust FFI signature: {name}")
    parameters = []
    for parameter in split_parameters(match.group(1)):
        if ":" not in parameter:
            raise SystemExit(f"cannot parse Rust FFI parameter for {name}: {parameter}")
        parameters.append(canonical_rust_type(parameter.split(":", 1)[1]))
    return canonical_rust_type(match.group(2) or "()"), parameters

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
                f"Rust/C FFI signature mismatch for {name}: "
                f"rust={rust_value}, c={c_value}"
            )

def rust_parameter_names(name: str) -> list[str]:
    generated = GENERATED_RUST_SIGNATURES.get(name)
    if generated is not None:
        return [
            parameter.split(":", 1)[0].strip()
            for parameter in generated[1]
        ]
    match = re.search(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(name)}\s*'
        rf'\((.*?)\)\s*(?:->\s*[^\s{{]+)?\s*{{',
        rust,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse Rust FFI parameters: {name}")
    return [parameter.split(":", 1)[0].strip() for parameter in split_parameters(match.group(1))]

def c_parameter_names(name: str) -> list[str]:
    match = re.search(
        rf'(?:int32_t|uint32_t|void)\s+{re.escape(name)}\s*\((.*?)\)\s*;',
        header,
        re.S,
    )
    if match is None:
        raise SystemExit(f"cannot parse C FFI parameters: {name}")
    names = []
    for parameter in split_parameters(match.group(1)):
        parsed = re.search(r'([A-Za-z_][A-Za-z0-9_]*)\s*$', parameter)
        if parsed is None:
            raise SystemExit(f"cannot parse C FFI parameter name for {name}: {parameter}")
        names.append(parsed.group(1))
    return names

rust_kagemusha = rust_exports("connect_norito_kagemusha_")
header_kagemusha = header_exports("connect_norito_kagemusha_")
swift_kagemusha = set(re.findall(r'"(connect_norito_kagemusha_[a-z0-9_]+)"', swift))
for label, actual in (
    ("Rust Kagemusha", rust_kagemusha),
    ("C header Kagemusha", header_kagemusha),
):
    forbidden = sorted(actual & FORBIDDEN_FIRST_RELEASE_EXPORTS)
    if forbidden:
        raise SystemExit(f"{label} contains forbidden first-release compatibility exports: {forbidden}")
exact(
    "Rust Kagemusha",
    KAGEMUSHA_EXPORTS | KAGEMUSHA_CANDIDATE_LAB_EXPORTS,
    rust_kagemusha,
)
exact(
    "C header Kagemusha",
    KAGEMUSHA_EXPORTS | KAGEMUSHA_CANDIDATE_LAB_EXPORTS,
    header_kagemusha,
)
exact("Rust privacy", PRIVACY_EXPORTS, rust_exports("iroha_privacy_"))
exact("C header privacy", PRIVACY_EXPORTS, header_exports("iroha_privacy_"))

# Keep an explicit privacy-only declaration audit in addition to the generic
# inventory checks above. This makes accidental header omissions and signature
# drift independently visible in the release guard.
privacy_declaration_pattern = re.compile(
    r'(?:int32_t|void)\s+(iroha_privacy_[a-z0-9_]+)\s*\((.*?)\)\s*;',
    re.S,
)
header_privacy_declarations = {
    name: parameters for name, parameters in privacy_declaration_pattern.findall(header)
}
undeclared_privacy_exports = set(required_privacy_ffi) - set(header_privacy_declarations)
if undeclared_privacy_exports:
    raise SystemExit(
        f"C header is missing privacy declarations: {sorted(undeclared_privacy_exports)}"
    )

expected_privacy_signatures = {
    "iroha_privacy_compiled_profile_catalog_v1": 2,
    "iroha_privacy_validate_compiled_profile_catalog_v1": 2,
    "iroha_privacy_exact12_fixture_bundle_v1": 2,
    "iroha_privacy_validate_exact12_fixture_bundle_v1": 2,
    "iroha_privacy_free_buffer": 1,
}
for name, expected_parameter_count in expected_privacy_signatures.items():
    actual_parameter_count = len(split_parameters(header_privacy_declarations[name]))
    if actual_parameter_count != expected_parameter_count:
        raise SystemExit(
            f"C header privacy declaration has wrong signature for {name}: "
            f"expected {expected_parameter_count} parameters, found {actual_parameter_count}"
        )
exact("Rust SoraFS reference", SORAFS_REFERENCE_EXPORTS, rust_exports("connect_norito_sorafs_reference_"))
exact("C header SoraFS reference", SORAFS_REFERENCE_EXPORTS, header_exports("connect_norito_sorafs_reference_"))
for name, expected in {
    "CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_PAYLOADS_V1": "64",
    "CONNECT_NORITO_SORAFS_REFERENCE_BUNDLE_MAX_TOTAL_BYTES_V1": "67108864",
    "CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_MAX_BLOCKS_V1": "64",
    "CONNECT_NORITO_SORAFS_REFERENCE_GOVERNANCE_DAG_CID_BYTES_V1": "32",
    "CONNECT_NORITO_SORAFS_REFERENCE_MAX_INPUT_BYTES_V1": "67108864",
    "CONNECT_NORITO_SORAFS_REFERENCE_MAX_LABEL_BYTES_V1": "1024",
}.items():
    rust_constant = re.search(
        rf"pub\s+const\s+{name}\s*:\s*u32\s*=\s*([0-9]+)\s*;",
        rust,
    )
    header_constant = re.search(rf"#define\s+{name}\s+([0-9]+)\b", header)
    if rust_constant is None or rust_constant.group(1) != expected:
        raise SystemExit(f"Rust SoraFS bridge constant drift: {name}")
    if header_constant is None or header_constant.group(1) != expected:
        raise SystemExit(f"C header SoraFS bridge constant drift: {name}")
if re.search(
    r"typedef\s+struct\s+ConnectNoritoSorafsReferenceInput\s*\{\s*"
    r"const\s+uint8_t\s*\*\s*bytes_ptr\s*;\s*"
    r"size_t\s+bytes_len\s*;\s*"
    r"const\s+uint8_t\s*\*\s*label_ptr\s*;\s*"
    r"size_t\s+label_len\s*;\s*"
    r"\}\s*ConnectNoritoSorafsReferenceInput\s*;",
    header,
) is None:
    raise SystemExit("C header SoraFS governance input descriptor layout drift")
if re.search(
    r"typedef\s+struct\s+ConnectNoritoSorafsReferenceBundlePayload\s*\{\s*"
    r"uint32_t\s+kind\s*;\s*"
    r"const\s+uint8_t\s*\*\s*bytes_ptr\s*;\s*"
    r"size_t\s+bytes_len\s*;\s*"
    r"const\s+uint8_t\s*\*\s*label_ptr\s*;\s*"
    r"size_t\s+label_len\s*;\s*"
    r"\}\s*ConnectNoritoSorafsReferenceBundlePayload\s*;",
    header,
) is None:
    raise SystemExit("C header SoraFS bundle payload descriptor layout drift")

rust_detached = rust_exports("connect_norito_detached_transaction_") | rust_exports("connect_norito_canonical_json_")
header_detached = header_exports("connect_norito_detached_transaction_") | header_exports("connect_norito_canonical_json_")
exact("Rust detached transaction", DETACHED_EXPORTS, rust_detached)
exact("C header detached transaction", DETACHED_EXPORTS, header_detached)

rust_parliament_timed_ovn = rust_exports("connect_norito_parliament_timed_ovn_")
header_parliament_timed_ovn = header_exports("connect_norito_parliament_timed_ovn_")
exact(
    "Rust Parliament timed-OVN",
    PARLIAMENT_TIMED_OVN_EXPORTS,
    rust_parliament_timed_ovn,
)
exact(
    "C header Parliament timed-OVN",
    PARLIAMENT_TIMED_OVN_EXPORTS,
    header_parliament_timed_ovn,
)

rust_hijiri_quote = rust_exports("connect_norito_validation_fee_hijiri_quote_")
header_hijiri_quote = header_exports("connect_norito_validation_fee_hijiri_quote_")
exact(
    "Rust validation-fee Hijiri quote",
    VALIDATION_FEE_HIJIRI_QUOTE_EXPORTS,
    rust_hijiri_quote,
)
exact(
    "C header validation-fee Hijiri quote",
    VALIDATION_FEE_HIJIRI_QUOTE_EXPORTS,
    header_hijiri_quote,
)

rust_private_settlement_response = rust_exports("connect_norito_private_settlement_")
header_private_settlement_response = header_exports("connect_norito_private_settlement_")
exact(
    "Rust private-settlement response verifier",
    PRIVATE_SETTLEMENT_RESPONSE_EXPORTS,
    rust_private_settlement_response,
)
exact(
    "C header private-settlement response verifier",
    PRIVATE_SETTLEMENT_RESPONSE_EXPORTS,
    header_private_settlement_response,
)

signer_name = re.compile(
    r"^connect_norito_encode_[a-z0-9_]+_signed_transaction(?:_alg)?$"
)
rust_transaction_signers = {
    name for name in rust_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
header_transaction_signers = {
    name for name in header_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
exact("Rust transaction signer", TRANSACTION_SIGNER_EXPORTS, rust_transaction_signers)
exact("C header transaction signer", TRANSACTION_SIGNER_EXPORTS, header_transaction_signers)
base_transaction_signers = {
    name for name in rust_transaction_signers if not name.endswith("_alg")
}
expected_transaction_signers = base_transaction_signers | {
    f"{name}_alg" for name in base_transaction_signers
}
exact("Rust transaction signer algorithm pairing", expected_transaction_signers, rust_transaction_signers)
for name in sorted(rust_transaction_signers):
    rust_names = rust_parameter_names(name)
    header_names = c_parameter_names(name)
    if rust_names[:2] != ["network_id_ptr", "network_id_len"]:
        raise SystemExit(
            f"Rust signer {name} must begin with exact NetworkId pointer/length arguments"
        )
    if header_names[:2] != ["network_id", "network_id_len"]:
        raise SystemExit(
            f"C signer {name} must begin with exact NetworkId pointer/length arguments"
        )
    rust_fee_index = rust_names.index("fee_payment_json_ptr")
    header_fee_index = header_names.index("fee_payment_json")
    if rust_names[rust_fee_index:rust_fee_index + 4] != [
        "fee_payment_json_ptr",
        "fee_payment_json_len",
        "private_key_ptr",
        "private_key_len",
    ]:
        raise SystemExit(
            f"Rust signer {name} must require fee payment immediately before its private key"
        )
    if header_names[header_fee_index:header_fee_index + 4] != [
        "fee_payment_json",
        "fee_payment_json_len",
        "private_key",
        "private_key_len",
    ]:
        raise SystemExit(
            f"C signer {name} must require fee payment immediately before its private key"
        )
require_signature_parity(
    KAGEMUSHA_EXPORTS
    | KAGEMUSHA_CANDIDATE_LAB_EXPORTS
    | PRIVACY_EXPORTS
    | SORAFS_REFERENCE_EXPORTS
    | DETACHED_EXPORTS
    | PARLIAMENT_TIMED_OVN_EXPORTS
    | VALIDATION_FEE_HIJIRI_QUOTE_EXPORTS
    | PRIVATE_SETTLEMENT_RESPONSE_EXPORTS
    | rust_transaction_signers
    | {"connect_norito_bridge_abi_version", "connect_norito_free"}
)

if re.search(
    r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*"
    r"PRIVACY_BRIDGE_ABI_VERSION_V1\s*;",
    rust,
) is None:
    raise SystemExit("connect_norito bridge ABI must use the shared privacy ABI constant")
if re.search(
    r"pub\s+const\s+PRIVACY_BRIDGE_ABI_VERSION_V1\s*:\s*u32\s*=\s*23\s*;",
    privacy_model,
) is None:
    raise SystemExit("shared privacy bridge ABI must be exactly 23")
if re.search(
    r"#define\s+CONNECT_NORITO_BRIDGE_ABI_VERSION\s+23(?:\s|$)",
    header,
) is None:
    raise SystemExit("C bridge ABI macro must be exactly 23")
if re.search(r"const\s+ERR_PARLIAMENT_TIMED_OVN\s*:\s*c_int\s*=\s*-505\s*;", rust) is None:
    raise SystemExit("Rust Parliament timed-OVN error code must be exactly -505")
if re.search(r"#define\s+CONNECT_NORITO_ERR_PARLIAMENT_TIMED_OVN\s+-505(?:\s|$)", header) is None:
    raise SystemExit("C Parliament timed-OVN error code must be exactly -505")
if re.search(r"const\s+ERR_VALIDATION_FEE_HIJIRI_QUOTE\s*:\s*c_int\s*=\s*-506\s*;", rust) is None:
    raise SystemExit("Rust validation-fee Hijiri quote error code must be exactly -506")
if re.search(
    r"#define\s+CONNECT_NORITO_ERR_VALIDATION_FEE_HIJIRI_QUOTE\s+-506(?:\s|$)",
    header,
) is None:
    raise SystemExit("C validation-fee Hijiri quote error code must be exactly -506")

for rust_name, rust_type, rust_value, header_name, header_value in (
    (
        "VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1",
        "u16",
        r"1",
        "CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_VERSION_V1",
        "1",
    ),
    (
        "VALIDATION_FEE_HIJIRI_QUOTE_MAX_QUALIFYING_TRANSFERS_V1",
        "u32",
        r"100_000",
        "CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_TRANSFERS_V1",
        "100000",
    ),
    (
        "VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1",
        "usize",
        r"4\s*\*\s*1024",
        "CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_REQUEST_BYTES_V1",
        "4096",
    ),
    (
        "VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1",
        "usize",
        r"64\s*\*\s*1024",
        "CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1",
        "65536",
    ),
):
    if re.search(
        rf"pub\s+const\s+{rust_name}\s*:\s*{rust_type}\s*=\s*{rust_value}\s*;",
        hijiri_api,
    ) is None:
        raise SystemExit(f"shared validation-fee Hijiri quote constant drift: {rust_name}")
    if re.search(rf"#define\s+{header_name}\s+{header_value}(?:\s|$)", header) is None:
        raise SystemExit(f"C validation-fee Hijiri quote constant drift: {header_name}")
if re.search(
    r"pub\s+const\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1\s*:\s*usize\s*=\s*32\s*;",
    rust,
) is None:
    raise SystemExit("Rust Parliament timed-OVN seed width must be exactly 32")
if re.search(
    r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_SEED_BYTES_V1\s+32(?:\s|$)",
    header,
) is None:
    raise SystemExit("C Parliament timed-OVN seed width must be exactly 32")
if re.search(
    r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_BYTES_V1\s+8388608(?:\s|$)",
    header,
) is None:
    raise SystemExit("C Parliament timed-OVN casting-proof bound must be exactly 8 MiB")
if re.search(
    r"pub\s+const\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1\s*:\s*usize\s*=\s*41\s*;",
    rust,
) is None:
    raise SystemExit("Rust Parliament timed-OVN casting-proof page result width must be 41")
if re.search(
    r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_CASTING_PROOF_PAGE_RESULT_BYTES_V1\s+41(?:\s|$)",
    header,
) is None:
    raise SystemExit("C Parliament timed-OVN casting-proof page result width must be 41")
if re.search(
    r"#define\s+CONNECT_NORITO_PARLIAMENT_TIMED_OVN_TRUST_ANCHOR_BYTES_V1\s+32(?:\s|$)",
    header,
) is None:
    raise SystemExit("C Parliament timed-OVN trust-anchor width must be exactly 32")
if re.search(r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_bridge_abi_version\s*\(', rust) is None:
    raise SystemExit("Rust bridge ABI export is missing")
if re.search(r"uint32_t\s+connect_norito_bridge_abi_version\s*\(\s*void\s*\)\s*;", header) is None:
    raise SystemExit("C bridge ABI declaration is missing")
if re.search(r"void\s+connect_norito_free\s*\(\s*uint8_t\s*\*\s*ptr\s*\)\s*;", header) is None:
    raise SystemExit("C bridge deallocator declaration is missing")
if re.search(
    r"void\s+connect_norito_kagemusha_secret_free_buffer\s*"
    r"\(\s*uint8_t\s*\*\s*ptr\s*\)\s*;",
    header,
) is None:
    raise SystemExit("C Kagemusha secret deallocator declaration is missing")
if '#include "connect_norito_bridge.h"' not in umbrella:
    raise SystemExit("NoritoBridge.h must include connect_norito_bridge.h")

def swift_array(name: str) -> set[str]:
    match = re.search(rf"{name}\s*=\s*\[(.*?)\n\s*\]", swift, re.S)
    if match is None:
        raise SystemExit(f"Swift {name} inventory is missing")
    values = re.findall(r'"(connect_norito_kagemusha_[a-z0-9_]+)"', match.group(1))
    if len(values) != len(set(values)):
        raise SystemExit(f"Swift {name} inventory contains duplicates")
    return set(values)

swift_proof_exports = swift_array("requiredProofSymbols")
swift_protocol_exports = swift_array("requiredProtocolSymbols")
if swift_proof_exports & swift_protocol_exports:
    raise SystemExit("Swift proof and protocol symbol inventories must be disjoint")
expected_protocol_count = len(KAGEMUSHA_EXPORTS) - 4
if len(swift_proof_exports) != 4 or len(swift_protocol_exports) != expected_protocol_count:
    raise SystemExit(
        "Swift ABI-23 inventory must contain 4 proof and "
        f"{expected_protocol_count} protocol symbols"
    )
swift_exports = swift_proof_exports | swift_protocol_exports
forbidden_swift_exports = sorted(swift_exports & FORBIDDEN_FIRST_RELEASE_EXPORTS)
if forbidden_swift_exports:
    raise SystemExit(
        "Swift Kagemusha contains forbidden first-release compatibility exports: "
        f"{forbidden_swift_exports}"
    )
exact("Swift Kagemusha", KAGEMUSHA_EXPORTS, swift_exports)
if re.search(r"requiredNativeSymbols\s*=\s*requiredProofSymbols\s*\+\s*requiredProtocolSymbols", swift) is None:
    raise SystemExit("Swift requiredNativeSymbols must combine the exact proof and protocol inventories")

print(
    "bridge header contract passed: bridge ABI 23, "
    f"{len(KAGEMUSHA_EXPORTS)} Kagemusha exports, "
    f"{len(PRIVACY_EXPORTS)} privacy exports, "
    f"{len(SORAFS_REFERENCE_EXPORTS)} SoraFS exports, "
    f"{len(DETACHED_EXPORTS)} detached-transaction exports, and "
    f"{len(PARLIAMENT_TIMED_OVN_EXPORTS)} Parliament timed-OVN exports, and "
    f"{len(VALIDATION_FEE_HIJIRI_QUOTE_EXPORTS)} validation-fee Hijiri quote exports, and "
    f"{len(PRIVATE_SETTLEMENT_RESPONSE_EXPORTS)} private-settlement response exports"
)
PY
}

replace_once() {
  local path="$1"
  local before="$2"
  local after="$3"
  python3 - "${path}" "${before}" "${after}" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
before = sys.argv[2]
after = sys.argv[3]
text = path.read_text(encoding="utf-8")
if before not in text:
    raise SystemExit(f"negative-control mutation target is missing: {before}")
path.write_text(text.replace(before, after, 1), encoding="utf-8")
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
    raise SystemExit(f"negative-control regex mutation count must be one (found {count}): {pattern}")
path.write_text(updated, encoding="utf-8")
PY
}

make_negative_workspace() {
  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-bridge-header.XXXXXX")"
  cp "${RUST_LIB}" "${tmp}/lib.rs"
  cp "${PARLIAMENT_RUST}" "${tmp}/parliament_timed_ovn_ffi.rs"
  cp "${PRIVATE_SETTLEMENT_RUST}" "${tmp}/private_settlement_ffi.rs"
  cp "${DATA_MODEL_PRIVACY}" "${tmp}/privacy.rs"
  cp "${HIJIRI_API}" "${tmp}/validation_fee_api.rs"
  cp "${HEADER}" "${tmp}/connect_norito_bridge.h"
  cp "${UMBRELLA}" "${tmp}/NoritoBridge.h"
  cp "${SWIFT_CONTRACT}" "${tmp}/KagemushaRecursiveSpendV2.swift"
  printf '%s' "${tmp}"
}

expect_contract_rejection() {
  local tmp="$1"
  local output
  if output="$(run_contract_check \
      "${tmp}/lib.rs" \
      "${tmp}/connect_norito_bridge.h" \
      "${tmp}/NoritoBridge.h" \
      "${tmp}/KagemushaRecursiveSpendV2.swift" \
      "${tmp}/privacy.rs" \
      "${tmp}/parliament_timed_ovn_ffi.rs" \
      "${tmp}/validation_fee_api.rs" \
      "${tmp}/private_settlement_ffi.rs" 2>&1)"; then
    echo "[bridge-header] negative control unexpectedly passed: ${MODE}" >&2
    exit 1
  fi
  echo "[bridge-header] negative control rejected expected ABI drift: ${MODE}" >&2
  echo "${output}" >&2
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
    "${SWIFT_CONTRACT}" \
    "${DATA_MODEL_PRIVACY}" \
    "${PARLIAMENT_RUST}" \
    "${HIJIRI_API}" \
    "${PRIVATE_SETTLEMENT_RUST}" >/dev/null
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT
  tmp_rust="${tmp}/lib.rs"
  tmp_header="${tmp}/connect_norito_bridge.h"
  tmp_umbrella="${tmp}/NoritoBridge.h"
  tmp_swift="${tmp}/KagemushaRecursiveSpendV2.swift"
  tmp_parliament_rust="${tmp}/parliament_timed_ovn_ffi.rs"

  case "${MODE}" in
    --self-test-bad-abi)
      replace_once "${tmp_rust}" \
        "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = PRIVACY_BRIDGE_ABI_VERSION_V1;" \
        "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 19;"
      ;;
    --self-test-missing-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recursive_spend_redeem_v4" \
        "removed_connect_norito_kagemusha_recursive_spend_redeem_v4"
      ;;
    --self-test-missing-parliament-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_parliament_timed_ovn_ballot_from_proof_v1" \
        "removed_connect_norito_parliament_timed_ovn_ballot_from_proof_v1"
      ;;
    --self-test-missing-hijiri-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_validation_fee_hijiri_quote_response_verify_v1" \
        "removed_connect_norito_validation_fee_hijiri_quote_response_verify_v1"
      ;;
    --self-test-bad-hijiri-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_validation_fee_hijiri_quote_response_verify_v1\s*\(\s*const uint8_t\* response_norito,\s*)unsigned long response_norito_len' \
        '\g<1>uint32_t response_norito_len'
      ;;
    --self-test-bad-hijiri-constant)
      replace_once "${tmp_header}" \
        "#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1 65536" \
        "#define CONNECT_NORITO_VALIDATION_FEE_HIJIRI_QUOTE_MAX_RESPONSE_BYTES_V1 65535"
      ;;
    --self-test-forbidden-v3-alias)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recursive_spend_init_v4" \
        "connect_norito_kagemusha_recursive_spend_init_v3"
      ;;
    --self-test-forbidden-lineage-v1)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recipient_registration_lineage_verify_v2" \
        "connect_norito_kagemusha_recipient_registration_lineage_verify_v1"
      ;;
    --self-test-forbidden-auth-create-v2)
      replace_once "${tmp_swift}" \
        '"connect_norito_kagemusha_request_authorization_finalize_hardware_v2"' \
        '"connect_norito_kagemusha_request_authorization_create_v2"'
      ;;
    --self-test-bad-capability-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_recursive_spend_capabilities_v4\s*\(\s*)uint8_t\*\*' \
        '\g<1>uint8_t*'
      ;;
    --self-test-bad-proof-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_recursive_spend_init_v4\s*\(\s*)const uint8_t\*' \
        '\g<1>uint8_t*'
      ;;
    --self-test-bad-artifact-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_recursive_spend_artifact_begin_v4\s*\([^;]*?)uint64_t\* out_handle' \
        '\g<1>uint32_t* out_handle'
      ;;
    --self-test-missing-export-pair)
      replace_once "${tmp_rust}" \
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v4" \
        "removed_connect_norito_kagemusha_recursive_spend_bundle_summary_v4"
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v4" \
        "removed_connect_norito_kagemusha_recursive_spend_bundle_summary_v4"
      ;;
    --self-test-missing-generated-kagemusha-rust-symbol)
      replace_once "${tmp_rust}" \
        '        => connect_norito_kagemusha_recursive_spend_redeem_v4, "krv4-redeem";' \
        '        => removed_connect_norito_kagemusha_recursive_spend_redeem_v4, "krv4-redeem";'
      ;;
    --self-test-missing-generated-transaction-signer)
      replace_once "${tmp_rust}" \
        "    connect_norito_encode_burn_signed_transaction =>" \
        "    removed_connect_norito_encode_burn_signed_transaction =>"
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
    --self-test-missing-protocol-export)
      replace_once "${tmp_rust}" \
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2" \
        "removed_connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2"
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2" \
        "removed_connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2"
      ;;
    --self-test-bad-receiver-key-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_receiver_key_reference_v2\s*\([^;]*?)unsigned long public_key_len' \
        '\g<1>uint32_t public_key_len'
      ;;
    --self-test-bad-verification-time-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_recipient_payment_request_verify_v2\s*\([^;]*?)uint64_t verified_at_ms' \
        '\g<1>uint32_t verified_at_ms'
      ;;
    --self-test-bad-acknowledgement-signature)
      replace_regex_once "${tmp_header}" \
        '(connect_norito_kagemusha_receiver_acknowledgement_create_v2\s*\([^;]*?)unsigned long peer_payment_norito_len' \
        '\g<1>uint32_t peer_payment_norito_len'
      ;;
    --self-test-bad-capability-rust-signature)
      replace_regex_once "${tmp_rust}" \
        '(fn connect_norito_kagemusha_recursive_spend_capabilities_v4\s*\(\s*out_capabilities_ptr:\s*)\*mut \*mut c_uchar' \
        '\g<1>*mut c_uchar'
      ;;
    --self-test-missing-swift-symbol)
      replace_once "${tmp_swift}" \
        '"connect_norito_kagemusha_topup_shield_build_unsigned_v4"' \
        '"removed_connect_norito_kagemusha_topup_shield_build_unsigned_v4"'
      ;;
    --self-test-bad-deallocator-signature)
      replace_once "${tmp_header}" \
        "void connect_norito_free(uint8_t* ptr);" \
        "void connect_norito_free(const uint8_t* ptr);"
      ;;
    --self-test-bad-secret-deallocator-signature)
      replace_once "${tmp_header}" \
        "void connect_norito_kagemusha_secret_free_buffer(uint8_t* ptr);" \
        "void connect_norito_kagemusha_secret_free_buffer(const uint8_t* ptr);"
      ;;
    --self-test-missing-rust-symbol)
      replace_once "${tmp_rust}" \
        "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4" \
        "removed_connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4"
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
        "iroha_privacy_compiled_profile_catalog_v1" \
        "removed_iroha_privacy_compiled_profile_catalog_v1"
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
    --self-test-umbrella-drift)
      replace_once "${tmp_umbrella}" \
        '#include "connect_norito_bridge.h"' \
        '#include "removed_connect_norito_bridge.h"'
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  expect_contract_rejection "${tmp}"
  exit 0
elif [[ -n "${MODE}" ]]; then
  usage
  exit 2
fi

run_contract_check \
  "${RUST_LIB}" \
  "${HEADER}" \
  "${UMBRELLA}" \
  "${SWIFT_CONTRACT}" \
  "${DATA_MODEL_PRIVACY}" \
  "${PARLIAMENT_RUST}" \
  "${HIJIRI_API}" \
  "${PRIVATE_SETTLEMENT_RUST}"

if ! command -v "${CC:-cc}" >/dev/null 2>&1; then
  echo "[connect-norito-header] required C compiler not found: ${CC:-cc}" >&2
  exit 1
fi
"${CC:-cc}" -fsyntax-only -x c \
  -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${HEADER}"

if ! command -v "${CXX:-c++}" >/dev/null 2>&1; then
  echo "[connect-norito-header] required C++ compiler not found: ${CXX:-c++}" >&2
  exit 1
fi
"${CXX:-c++}" -fsyntax-only -x c++ \
  -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${UMBRELLA}"
