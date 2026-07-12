#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIB="${ROOT_DIR}/crates/connect_norito_bridge/src/lib.rs"
HEADER="${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h"
UMBRELLA="${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h"
SWIFT_V2_CONTRACT="${ROOT_DIR}/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
MODE="${1:-}"

usage() {
  cat >&2 <<'EOF'
usage: ci/check_connect_norito_bridge_header.sh [negative-control]

negative-control:
  --negative-control-bad-bridge-abi
  --negative-control-missing-recursive-header
  --negative-control-bad-recursive-signature
  --negative-control-bad-recursive-v2-signature
  --negative-control-bad-recursive-v2-artifact-signature
  --negative-control-missing-recursive-v2-export-pair
  --negative-control-missing-kagemusha-v2-protocol-export-pair
  --negative-control-bad-kagemusha-v2-receiver-key-signature
  --negative-control-bad-kagemusha-v2-verify-at-time-signature
  --negative-control-bad-kagemusha-v2-ack-create-signature
  --negative-control-bad-connect-norito-free-signature
  --negative-control-missing-rust-export
  --negative-control-missing-privacy-header
  --negative-control-bad-privacy-signature
  --negative-control-missing-privacy-rust-export
  --negative-control-umbrella-drift
EOF
}

run_contract_check() {
  local rust_lib="${1}"
  local header="${2}"
  local umbrella="${3}"
  local swift_v2_contract="${4}"

  python3 - "$rust_lib" "$header" "$umbrella" "$swift_v2_contract" <<'PY'
import re
import sys
from pathlib import Path

rust_lib = Path(sys.argv[1])
header = Path(sys.argv[2])
umbrella = Path(sys.argv[3])
swift_v2_contract = Path(sys.argv[4])

rust_text = rust_lib.read_text(encoding="utf-8")
header_text = header.read_text(encoding="utf-8")
umbrella_text = umbrella.read_text(encoding="utf-8")
swift_v2_contract_text = swift_v2_contract.read_text(encoding="utf-8")

recursive_export_pattern = re.compile(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
    r'(connect_norito_kagemusha_(?:recursive_spend_|topup_finality_)[a-z0-9_]+)\s*\('
)
recursive_declaration_pattern = re.compile(
    r'int32_t\s+'
    r'(connect_norito_kagemusha_(?:recursive_spend_|topup_finality_)[a-z0-9_]+)\s*\('
)
privacy_export_pattern = re.compile(
    r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
    r'(iroha_privacy_[a-z0-9_]+)\s*\('
)
privacy_declaration_pattern = re.compile(
    r'(?:int32_t|void)\s+'
    r'(iroha_privacy_[a-z0-9_]+)\s*\('
)
sorafs_reference_export_pattern = re.compile(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
    r'(connect_norito_sorafs_reference_[a-z0-9_]+)\s*\('
)
sorafs_reference_declaration_pattern = re.compile(
    r'int32_t\s+'
    r'(connect_norito_sorafs_reference_[a-z0-9_]+)\s*\('
)

rust_exports = set(recursive_export_pattern.findall(rust_text))
header_declarations = set(recursive_declaration_pattern.findall(header_text))
rust_privacy_exports = set(privacy_export_pattern.findall(rust_text))
header_privacy_declarations = set(privacy_declaration_pattern.findall(header_text))
rust_sorafs_reference_exports = set(sorafs_reference_export_pattern.findall(rust_text))
header_sorafs_reference_declarations = set(
    sorafs_reference_declaration_pattern.findall(header_text)
)
bridge_abi_export = re.search(
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_bridge_abi_version\s*\(',
    rust_text,
) is not None
bridge_abi_declaration = re.search(
    r"uint32_t\s+connect_norito_bridge_abi_version\s*\(\s*void\s*\)\s*;",
    header_text,
) is not None
bridge_abi_constant = re.search(
    r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*([0-9]+)\s*;",
    rust_text,
)

required_kagemusha_v2_proof_exports = {
    "connect_norito_kagemusha_recursive_spend_init_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
    "connect_norito_kagemusha_recursive_spend_verify_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v2",
}
required_kagemusha_v2_protocol_exports = {
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_topup_finality_verify_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
    "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
    "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
}
required_kagemusha_native_exports = (
    required_kagemusha_v2_proof_exports | required_kagemusha_v2_protocol_exports
)

kagemusha_native_export_pattern = re.compile(
    r'#\[unsafe\(no_mangle\)\]\s*'
    r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
    r'(connect_norito_kagemusha_(?:recursive_spend|topup_finality|receiver|recipient|request)_[a-z0-9_]+)\s*\('
)
kagemusha_native_declaration_pattern = re.compile(
    r'int32_t\s+'
    r'(connect_norito_kagemusha_(?:recursive_spend|topup_finality|receiver|recipient|request)_[a-z0-9_]+)\s*\('
)
rust_kagemusha_native_exports = set(kagemusha_native_export_pattern.findall(rust_text))
header_kagemusha_native_declarations = set(
    kagemusha_native_declaration_pattern.findall(header_text)
)

def swift_symbol_inventory(name):
    match = re.search(
        rf"public\s+static\s+let\s+{name}\s*=\s*\[(.*?)\n\s*\]",
        swift_v2_contract_text,
        re.DOTALL,
    )
    if match is None:
        return None
    return re.findall(r'"([^"]+)"', match.group(1))

swift_v2_proof_inventory = swift_symbol_inventory("requiredProofSymbols")
swift_v2_protocol_inventory = swift_symbol_inventory("requiredProtocolSymbols")

def const_u8_ptr(name):
    return rf"const\s+uint8_t\s*\*\s*{name}"

def const_char_ptr(name):
    return rf"const\s+char\s*\*\s*{name}"

def u8_out_ptr(name):
    return rf"uint8_t\s*\*\*\s*{name}"

def u8_ptr(name):
    return rf"uint8_t\s*\*\s*{name}"

def const_u64_ptr(name):
    return rf"const\s+uint64_t\s*\*\s*{name}"

def ulong(name):
    return rf"unsigned\s+long\s+{name}"

def ulong_ptr(name):
    return rf"unsigned\s+long\s*\*\s*{name}"

def uint8(name):
    return rf"uint8_t\s+{name}"

def uint32(name):
    return rf"uint32_t\s+{name}"

def uint64(name):
    return rf"uint64_t\s+{name}"

def uint64_ptr(name):
    return rf"uint64_t\s*\*\s*{name}"

def c_signature(return_type, name, params):
    return (
        rf"{return_type}\s+{name}\s*\(\s*"
        + r"\s*,\s*".join(params)
        + r"\s*\)\s*;"
    )

def recursive_request_signature(name, out_ptr_name, out_len_name):
    return c_signature(
        "int32_t",
        name,
        [
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            u8_out_ptr(out_ptr_name),
            ulong_ptr(out_len_name),
        ],
    )

def rust_param(name, type_pattern):
    return rf"_?{name}\s*:\s*{type_pattern}"

def rust_signature(name, params):
    return (
        rf'#\[unsafe\(no_mangle\)\]\s*pub\s+unsafe\s+extern\s+"C"\s+fn\s+{name}\s*\(\s*'
        + r"\s*,\s*".join(params)
        + r"\s*,?\s*\)\s*->\s*c_int"
    )

def rust_const_u8_ptr(name):
    return rust_param(name, r"\*const\s+c_uchar")

def rust_u8_out_ptr(name):
    return rust_param(name, r"\*mut\s+\*mut\s+c_uchar")

def rust_ulong(name):
    return rust_param(name, r"c_ulong")

def rust_ulong_ptr(name):
    return rust_param(name, r"\*mut\s+c_ulong")

def rust_u8(name):
    return rust_param(name, r"u8")

def rust_u64(name):
    return rust_param(name, r"u64")

def rust_u64_ptr(name):
    return rust_param(name, r"\*mut\s+u64")

def rust_u8_ptr(name):
    return rust_param(name, r"\*mut\s+u8")

def rust_const_u64_ptr(name):
    return rust_param(name, r"\*const\s+u64")

def rust_archive_out_signature(name, input_name, output_name):
    return rust_signature(
        name,
        [
            rust_const_u8_ptr(f"{input_name}_ptr"),
            rust_ulong(f"{input_name}_len"),
            rust_u8_out_ptr(f"out_{output_name}_ptr"),
            rust_ulong_ptr(f"out_{output_name}_len"),
        ],
    )

expected_recursive_signatures = {
    "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
        [
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_compact_token_ptr"),
            ulong_ptr("out_compact_token_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_init": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_init",
        "out_bundle_ptr",
        "out_bundle_len",
    ),
    "connect_norito_kagemusha_recursive_spend_append": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_append",
        "out_bundle_ptr",
        "out_bundle_len",
    ),
    "connect_norito_kagemusha_recursive_spend_init_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_init_v2",
        "out_bundle_ptr",
        "out_bundle_len",
    ),
    "connect_norito_kagemusha_recursive_spend_capabilities_v1": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_capabilities_v1",
        [
            u8_out_ptr("out_capabilities_ptr"),
            ulong_ptr("out_capabilities_len"),
        ],
    ),
    "connect_norito_kagemusha_topup_finality_verify_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_topup_finality_verify_v2",
        [
            const_u8_ptr("proof_norito_ptr"),
            ulong("proof_norito_len"),
            const_u8_ptr("roster_norito_ptr"),
            ulong("roster_norito_len"),
            const_u8_ptr("anchor_norito_ptr"),
            ulong("anchor_norito_len"),
            const_u8_ptr("manifest_norito_ptr"),
            ulong("manifest_norito_len"),
            const_u8_ptr("expected_manifest_sha256_ptr"),
            ulong("expected_manifest_sha256_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
        [
            const_u8_ptr("manifest_norito_ptr"),
            ulong("manifest_norito_len"),
            const_u8_ptr("expected_manifest_sha256_ptr"),
            ulong("expected_manifest_sha256_len"),
            const_u8_ptr("expected_artifact_sha256_ptr"),
            ulong("expected_artifact_sha256_len"),
            r"uint64_t\s*\*\s*out_handle",
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
        [
            r"uint64_t\s+handle",
            const_u8_ptr("chunk_ptr"),
            ulong("chunk_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
        [r"uint64_t\s+handle"],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
        [r"uint64_t\s+handle"],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
        [
            const_u8_ptr("manifest_norito_ptr"),
            ulong("manifest_norito_len"),
            const_u8_ptr("expected_manifest_sha256_ptr"),
            ulong("expected_manifest_sha256_len"),
            const_u64_ptr("handles_ptr"),
            ulong("handles_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
        [
            const_u8_ptr("manifest_norito_ptr"),
            ulong("manifest_norito_len"),
            const_u8_ptr("expected_manifest_sha256_ptr"),
            ulong("expected_manifest_sha256_len"),
            u8_ptr("out_installed"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
        [
            const_u8_ptr("expected_manifest_sha256_ptr"),
            ulong("expected_manifest_sha256_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
        "out_digest_ptr",
        "out_digest_len",
    ).replace("request_norito", "unsigned_norito"),
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
        [
            const_u8_ptr("unsigned_norito_ptr"),
            ulong("unsigned_norito_len"),
            const_u8_ptr("authorization_norito_ptr"),
            ulong("authorization_norito_len"),
            u8_out_ptr("out_request_ptr"),
            ulong_ptr("out_request_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_topup_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_topup_v2",
        "out_instruction_ptr",
        "out_instruction_len",
    ),
    "connect_norito_kagemusha_recursive_spend_append_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_append_v2",
        [
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            const_u8_ptr("recipient_request_norito_ptr"),
            ulong("recipient_request_norito_len"),
            r"uint64_t\s+verified_at_ms",
            u8_out_ptr("out_split_result_ptr"),
            ulong_ptr("out_split_result_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_redeem_change_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
        "out_result_ptr",
        "out_result_len",
    ),
    "connect_norito_kagemusha_recursive_spend_verify_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_verify_v2",
        "out_result_ptr",
        "out_result_len",
    ),
    "connect_norito_kagemusha_recursive_spend_redeem_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_redeem_v2",
        "out_instruction_ptr",
        "out_instruction_len",
    ),
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
        "out_digest_ptr",
        "out_digest_len",
    ).replace("request_norito", "unsigned_norito"),
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
        [
            const_u8_ptr("unsigned_norito_ptr"),
            ulong("unsigned_norito_len"),
            const_u8_ptr("authorization_norito_ptr"),
            ulong("authorization_norito_len"),
            u8_out_ptr("out_request_ptr"),
            ulong_ptr("out_request_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_build_split_intent_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
        "out_intent_ptr",
        "out_intent_len",
    ),
    "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
        "out_intent_ptr",
        "out_intent_len",
    ),
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
        [
            const_u8_ptr("split_result_norito_ptr"),
            ulong("split_result_norito_len"),
            u8_out_ptr("out_payment_ptr"),
            ulong_ptr("out_payment_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
        [
            const_u8_ptr("payment_norito_ptr"),
            ulong("payment_norito_len"),
            u8_out_ptr("out_payment_ptr"),
            ulong_ptr("out_payment_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
        [
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_summary_ptr"),
            ulong_ptr("out_summary_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_topup": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_topup",
        "out_instruction_ptr",
        "out_instruction_len",
    ),
    "connect_norito_kagemusha_recursive_spend_transition_profile_init": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_transition_profile_init",
        "out_profile_ptr",
        "out_profile_len",
    ),
    "connect_norito_kagemusha_recursive_spend_transition_profile_append": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_transition_profile_append",
        "out_profile_ptr",
        "out_profile_len",
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_append_boundary": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
        [
            const_u8_ptr("profile_norito_ptr"),
            ulong("profile_norito_len"),
            u8_out_ptr("out_boundary_ptr"),
            ulong_ptr("out_boundary_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
        [
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_witness_ptr"),
            ulong_ptr("out_witness_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result": c_signature(
        "int32_t",
        "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
        [
            const_u8_ptr("previous_witness_norito_ptr"),
            ulong("previous_witness_norito_len"),
            const_u8_ptr("request_norito_ptr"),
            ulong("request_norito_len"),
            const_u8_ptr("bundle_norito_ptr"),
            ulong("bundle_norito_len"),
            u8_out_ptr("out_witness_ptr"),
            ulong_ptr("out_witness_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_verify": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_verify",
        "out_result_ptr",
        "out_result_len",
    ),
    "connect_norito_kagemusha_recursive_spend_redeem": recursive_request_signature(
        "connect_norito_kagemusha_recursive_spend_redeem",
        "out_instruction_ptr",
        "out_instruction_len",
    ),
}
# The dictionary also documents retired unsuffixed signatures for historical
# negative-control fixtures. Only the exact ABI-18/V3 export inventory is a
# required first-release surface.
expected_recursive_signatures = {
    name: signature
    for name, signature in expected_recursive_signatures.items()
    if name in required_kagemusha_native_exports
}
required_recursive_ffi = set(expected_recursive_signatures)

expected_kagemusha_v2_signatures = {
    name: expected_recursive_signatures[name]
    for name in (
        required_kagemusha_v2_proof_exports
        | {"connect_norito_kagemusha_recursive_spend_bundle_summary_v2"}
    )
}
expected_kagemusha_v2_signatures.update(
    {
        "connect_norito_kagemusha_receiver_key_reference_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_receiver_key_reference_v2",
            [
                uint8("algorithm"),
                const_u8_ptr("public_key_ptr"),
                ulong("public_key_len"),
                u8_out_ptr("out_reference_ptr"),
                ulong_ptr("out_reference_len"),
            ],
        ),
        "connect_norito_kagemusha_recipient_output_derive_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_recipient_output_derive_v2",
            [
                const_u8_ptr("request_norito_ptr"),
                ulong("request_norito_len"),
                const_u8_ptr("receiver_spend_secret_ptr"),
                ulong("receiver_spend_secret_len"),
                u8_out_ptr("out_result_ptr"),
                ulong_ptr("out_result_len"),
            ],
        ),
        "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
            [
                const_u8_ptr("payload_norito_ptr"),
                ulong("payload_norito_len"),
                u8_out_ptr("out_signing_bytes_ptr"),
                ulong_ptr("out_signing_bytes_len"),
            ],
        ),
        "connect_norito_kagemusha_recipient_payment_request_create_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_recipient_payment_request_create_v2",
            [
                const_u8_ptr("payload_norito_ptr"),
                ulong("payload_norito_len"),
                const_u8_ptr("signature_ptr"),
                ulong("signature_len"),
                u8_out_ptr("out_request_ptr"),
                ulong_ptr("out_request_len"),
            ],
        ),
        "connect_norito_kagemusha_recipient_payment_request_verify_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_recipient_payment_request_verify_v2",
            [
                const_u8_ptr("request_norito_ptr"),
                ulong("request_norito_len"),
                uint64("verified_at_ms"),
                u8_out_ptr("out_digest_ptr"),
                ulong_ptr("out_digest_len"),
            ],
        ),
        "connect_norito_kagemusha_request_authorization_signing_bytes_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
            [
                const_u8_ptr("template_norito_ptr"),
                ulong("template_norito_len"),
                u8_out_ptr("out_signing_bytes_ptr"),
                ulong_ptr("out_signing_bytes_len"),
            ],
        ),
        "connect_norito_kagemusha_request_authorization_create_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_request_authorization_create_v2",
            [
                const_u8_ptr("template_norito_ptr"),
                ulong("template_norito_len"),
                const_u8_ptr("signature_ptr"),
                ulong("signature_len"),
                u8_out_ptr("out_authorization_ptr"),
                ulong_ptr("out_authorization_len"),
            ],
        ),
        "connect_norito_kagemusha_receiver_acknowledgement_payload_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
            [
                const_u8_ptr("request_norito_ptr"),
                ulong("request_norito_len"),
                const_u8_ptr("peer_payment_norito_ptr"),
                ulong("peer_payment_norito_len"),
                uint64("accepted_at_ms"),
                u8_out_ptr("out_payload_ptr"),
                ulong_ptr("out_payload_len"),
            ],
        ),
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
            [
                const_u8_ptr("payload_norito_ptr"),
                ulong("payload_norito_len"),
                u8_out_ptr("out_signing_bytes_ptr"),
                ulong_ptr("out_signing_bytes_len"),
            ],
        ),
        "connect_norito_kagemusha_receiver_acknowledgement_create_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
            [
                const_u8_ptr("payload_norito_ptr"),
                ulong("payload_norito_len"),
                const_u8_ptr("signature_ptr"),
                ulong("signature_len"),
                const_u8_ptr("request_norito_ptr"),
                ulong("request_norito_len"),
                const_u8_ptr("peer_payment_norito_ptr"),
                ulong("peer_payment_norito_len"),
                u8_out_ptr("out_acknowledgement_ptr"),
                ulong_ptr("out_acknowledgement_len"),
            ],
        ),
        "connect_norito_kagemusha_receiver_acknowledgement_verify_v2": c_signature(
            "int32_t",
            "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
            [
                const_u8_ptr("acknowledgement_norito_ptr"),
                ulong("acknowledgement_norito_len"),
                const_u8_ptr("request_norito_ptr"),
                ulong("request_norito_len"),
                const_u8_ptr("peer_payment_norito_ptr"),
                ulong("peer_payment_norito_len"),
                u8_out_ptr("out_result_ptr"),
                ulong_ptr("out_result_len"),
            ],
        ),
    }
)

expected_kagemusha_v2_rust_signatures = {
    "connect_norito_kagemusha_recursive_spend_init_v2": rust_signature(
        "connect_norito_kagemusha_recursive_spend_init_v2",
        [
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_u8_out_ptr("out_bundle_ptr"), rust_ulong_ptr("out_bundle_len"),
        ],
    ),
    "connect_norito_kagemusha_topup_finality_verify_v2": rust_signature(
        "connect_norito_kagemusha_topup_finality_verify_v2",
        [
            rust_const_u8_ptr("proof_norito_ptr"), rust_ulong("proof_norito_len"),
            rust_const_u8_ptr("roster_norito_ptr"), rust_ulong("roster_norito_len"),
            rust_const_u8_ptr("anchor_norito_ptr"), rust_ulong("anchor_norito_len"),
            rust_const_u8_ptr("manifest_norito_ptr"), rust_ulong("manifest_norito_len"),
            rust_const_u8_ptr("expected_manifest_sha256_ptr"),
            rust_ulong("expected_manifest_sha256_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
        [
            rust_const_u8_ptr("manifest_norito_ptr"), rust_ulong("manifest_norito_len"),
            rust_const_u8_ptr("expected_manifest_sha256_ptr"),
            rust_ulong("expected_manifest_sha256_len"),
            rust_const_u8_ptr("expected_artifact_sha256_ptr"),
            rust_ulong("expected_artifact_sha256_len"), rust_u64_ptr("out_handle"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
        [rust_u64("handle"), rust_const_u8_ptr("chunk_ptr"), rust_ulong("chunk_len")],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3", [rust_u64("handle")]
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3", [rust_u64("handle")]
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
        [
            rust_const_u8_ptr("manifest_norito_ptr"), rust_ulong("manifest_norito_len"),
            rust_const_u8_ptr("expected_manifest_sha256_ptr"),
            rust_ulong("expected_manifest_sha256_len"),
            rust_const_u64_ptr("handles_ptr"), rust_ulong("handles_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
        [
            rust_const_u8_ptr("manifest_norito_ptr"), rust_ulong("manifest_norito_len"),
            rust_const_u8_ptr("expected_manifest_sha256_ptr"),
            rust_ulong("expected_manifest_sha256_len"), rust_u8_ptr("out_installed"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3": rust_signature(
        "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
        [
            rust_const_u8_ptr("expected_manifest_sha256_ptr"),
            rust_ulong("expected_manifest_sha256_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_topup_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recursive_spend_topup_v2", "request_norito", "instruction"
    ),
    "connect_norito_kagemusha_recursive_spend_append_v2": rust_signature(
        "connect_norito_kagemusha_recursive_spend_append_v2",
        [
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_const_u8_ptr("recipient_request_norito_ptr"), rust_ulong("recipient_request_norito_len"),
            rust_u64("verified_at_ms"),
            rust_u8_out_ptr("out_split_result_ptr"), rust_ulong_ptr("out_split_result_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_redeem_change_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recursive_spend_redeem_change_v2", "request_norito", "result"
    ),
    "connect_norito_kagemusha_recursive_spend_verify_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recursive_spend_verify_v2", "request_norito", "result"
    ),
    "connect_norito_kagemusha_recursive_spend_redeem_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recursive_spend_redeem_v2", "request_norito", "instruction"
    ),
    "connect_norito_kagemusha_receiver_key_reference_v2": rust_signature(
        "connect_norito_kagemusha_receiver_key_reference_v2",
        [
            rust_u8("algorithm"), rust_const_u8_ptr("public_key_ptr"), rust_ulong("public_key_len"),
            rust_u8_out_ptr("out_reference_ptr"), rust_ulong_ptr("out_reference_len"),
        ],
    ),
    "connect_norito_kagemusha_recipient_output_derive_v2": rust_signature(
        "connect_norito_kagemusha_recipient_output_derive_v2",
        [
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_const_u8_ptr("receiver_spend_secret_ptr"),
            rust_ulong("receiver_spend_secret_len"),
            rust_u8_out_ptr("out_result_ptr"), rust_ulong_ptr("out_result_len"),
        ],
    ),
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
        "payload_norito", "signing_bytes",
    ),
    "connect_norito_kagemusha_recipient_payment_request_create_v2": rust_signature(
        "connect_norito_kagemusha_recipient_payment_request_create_v2",
        [
            rust_const_u8_ptr("payload_norito_ptr"), rust_ulong("payload_norito_len"),
            rust_const_u8_ptr("signature_ptr"), rust_ulong("signature_len"),
            rust_u8_out_ptr("out_request_ptr"), rust_ulong_ptr("out_request_len"),
        ],
    ),
    "connect_norito_kagemusha_recipient_payment_request_verify_v2": rust_signature(
        "connect_norito_kagemusha_recipient_payment_request_verify_v2",
        [
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_u64("verified_at_ms"), rust_u8_out_ptr("out_digest_ptr"),
            rust_ulong_ptr("out_digest_len"),
        ],
    ),
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
        "template_norito", "signing_bytes",
    ),
    "connect_norito_kagemusha_request_authorization_create_v2": rust_signature(
        "connect_norito_kagemusha_request_authorization_create_v2",
        [
            rust_const_u8_ptr("template_norito_ptr"), rust_ulong("template_norito_len"),
            rust_const_u8_ptr("signature_ptr"), rust_ulong("signature_len"),
            rust_u8_out_ptr("out_authorization_ptr"), rust_ulong_ptr("out_authorization_len"),
        ],
    ),
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2": rust_signature(
        "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
        [
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_const_u8_ptr("peer_payment_norito_ptr"), rust_ulong("peer_payment_norito_len"),
            rust_u64("accepted_at_ms"), rust_u8_out_ptr("out_payload_ptr"),
            rust_ulong_ptr("out_payload_len"),
        ],
    ),
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
        "payload_norito", "signing_bytes",
    ),
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2": rust_signature(
        "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
        [
            rust_const_u8_ptr("payload_norito_ptr"), rust_ulong("payload_norito_len"),
            rust_const_u8_ptr("signature_ptr"), rust_ulong("signature_len"),
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_const_u8_ptr("peer_payment_norito_ptr"), rust_ulong("peer_payment_norito_len"),
            rust_u8_out_ptr("out_acknowledgement_ptr"), rust_ulong_ptr("out_acknowledgement_len"),
        ],
    ),
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2": rust_signature(
        "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
        [
            rust_const_u8_ptr("acknowledgement_norito_ptr"), rust_ulong("acknowledgement_norito_len"),
            rust_const_u8_ptr("request_norito_ptr"), rust_ulong("request_norito_len"),
            rust_const_u8_ptr("peer_payment_norito_ptr"), rust_ulong("peer_payment_norito_len"),
            rust_u8_out_ptr("out_result_ptr"), rust_ulong_ptr("out_result_len"),
        ],
    ),
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2": rust_archive_out_signature(
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v2", "bundle_norito", "summary"
    ),
}

retired_kagemusha_v2_artifact_ingest = {
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v2",
}

expected_connect_norito_free_header_signature = c_signature(
    "void", "connect_norito_free", [r"uint8_t\s*\*\s*ptr"]
)
expected_connect_norito_free_rust_signature = (
    r'#\[unsafe\(no_mangle\)\]\s*pub\s+extern\s+"C"\s+fn\s+connect_norito_free\s*\(\s*'
    r'ptr_\s*:\s*\*mut\s+c_uchar\s*,?\s*\)\s*\{'
)
required_privacy_ffi = {
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "iroha_privacy_free_buffer",
}
required_sorafs_reference_ffi = {
    "connect_norito_sorafs_reference_validate_orderbook_json",
    "connect_norito_sorafs_reference_validate_pop_json",
    "connect_norito_sorafs_reference_validate_hedging_json",
    "connect_norito_sorafs_reference_sign_orderbook_payload",
    "connect_norito_sorafs_reference_build_signed_orderbook_order_request",
    "connect_norito_sorafs_reference_build_signed_orderbook_order_cancel",
    "connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt",
    "connect_norito_sorafs_reference_validate_pdp_payload_json",
    "connect_norito_sorafs_reference_validate_pdp_commitment_challenge_json",
    "connect_norito_sorafs_reference_validate_pdp_challenge_proof_json",
    "connect_norito_sorafs_reference_validate_pdp_bundle_json",
}
expected_privacy_signatures = {
    "iroha_privacy_capabilities_v1": (
        r"int32_t\s+iroha_privacy_capabilities_v1\s*\(\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_proof_request_v1": (
        r"int32_t\s+iroha_privacy_proof_request_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*algorithm_id_ptr\s*,\s*"
        r"unsigned\s+long\s+algorithm_id_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*entrypoint_ptr\s*,\s*"
        r"unsigned\s+long\s+entrypoint_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*vk_ref_ptr\s*,\s*"
        r"unsigned\s+long\s+vk_ref_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*public_inputs_ptr\s*,\s*"
        r"unsigned\s+long\s+public_inputs_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*witness_ptr\s*,\s*"
        r"unsigned\s+long\s+witness_len\s*,\s*"
        r"const\s+uint8_t\s*\*\s*proof_ptr\s*,\s*"
        r"unsigned\s+long\s+proof_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_build_proof_v1": (
        r"int32_t\s+iroha_privacy_build_proof_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*request_ptr\s*,\s*"
        r"unsigned\s+long\s+request_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_verify_proof_v1": (
        r"int32_t\s+iroha_privacy_verify_proof_v1\s*\(\s*"
        r"const\s+uint8_t\s*\*\s*request_ptr\s*,\s*"
        r"unsigned\s+long\s+request_len\s*,\s*"
        r"uint8_t\s*\*\*\s*out_ptr\s*,\s*"
        r"unsigned\s+long\s*\*\s*out_len\s*"
        r"\)\s*;"
    ),
    "iroha_privacy_free_buffer": (
        r"void\s+iroha_privacy_free_buffer\s*\(\s*"
        r"uint8_t\s*\*\s*ptr\s*"
        r"\)\s*;"
    ),
}

missing_exports = sorted(required_recursive_ffi - rust_exports)
missing_header_declarations = sorted(required_recursive_ffi - header_declarations)
unexpected_exports = sorted(rust_exports - required_recursive_ffi)
unexpected_header_declarations = sorted(header_declarations - required_recursive_ffi)
missing_kagemusha_native_rust_exports = sorted(
    required_kagemusha_native_exports - rust_kagemusha_native_exports
)
missing_kagemusha_native_header_declarations = sorted(
    required_kagemusha_native_exports - header_kagemusha_native_declarations
)
undeclared_exports = sorted(rust_exports - header_declarations)
stale_header_declarations = sorted(header_declarations - rust_exports)
missing_privacy_exports = sorted(required_privacy_ffi - rust_privacy_exports)
missing_privacy_header_declarations = sorted(
    required_privacy_ffi - header_privacy_declarations
)
undeclared_privacy_exports = sorted(rust_privacy_exports - header_privacy_declarations)
stale_privacy_header_declarations = sorted(
    header_privacy_declarations - rust_privacy_exports
)
missing_sorafs_reference_exports = sorted(
    required_sorafs_reference_ffi - rust_sorafs_reference_exports
)
missing_sorafs_reference_header_declarations = sorted(
    required_sorafs_reference_ffi - header_sorafs_reference_declarations
)
undeclared_sorafs_reference_exports = sorted(
    rust_sorafs_reference_exports - header_sorafs_reference_declarations
)
stale_sorafs_reference_header_declarations = sorted(
    header_sorafs_reference_declarations - rust_sorafs_reference_exports
)

errors = []
retired_rust_exports = sorted(retired_kagemusha_v2_artifact_ingest & rust_exports)
retired_header_declarations = sorted(
    retired_kagemusha_v2_artifact_ingest & header_declarations
)
if retired_rust_exports:
    errors.append(
        "retired Rust Kagemusha V2 artifact-ingest exports reintroduced: "
        + ", ".join(retired_rust_exports)
    )
if retired_header_declarations:
    errors.append(
        "retired C header Kagemusha V2 artifact-ingest declarations reintroduced: "
        + ", ".join(retired_header_declarations)
    )
retired_role_macros = sorted(
    name
    for name in (
        "CONNECT_NORITO_KAGEMUSHA_ARTIFACT_ROLE_LINEAGE_INIT_V2",
        "CONNECT_NORITO_KAGEMUSHA_ARTIFACT_ROLE_LINEAGE_APPEND_V2",
        "CONNECT_NORITO_KAGEMUSHA_ARTIFACT_ROLE_REDEEM_CHANGE_V2",
    )
    if name in header_text
)
if retired_role_macros:
    errors.append(
        "retired C header Kagemusha V2 artifact-ingest role macros reintroduced: "
        + ", ".join(retired_role_macros)
    )
if "KagemushaRecursiveSpendArtifactIngestV2" in swift_v2_contract_text:
    errors.append("retired Swift Kagemusha V2 artifact-ingest wrapper reintroduced")
if len(required_kagemusha_v2_proof_exports) != 5:
    errors.append("internal ABI-18 Kagemusha V2 proof inventory must contain exactly 5 symbols")
if len(required_kagemusha_v2_protocol_exports) != 30:
    errors.append("internal ABI-18 Kagemusha V2 protocol inventory must contain exactly 30 symbols")
if swift_v2_proof_inventory is None:
    errors.append("Swift ABI-18 Kagemusha V2 requiredProofSymbols inventory is missing")
elif len(swift_v2_proof_inventory) != len(set(swift_v2_proof_inventory)):
    errors.append("Swift ABI-18 Kagemusha V2 requiredProofSymbols inventory contains duplicates")
elif set(swift_v2_proof_inventory) != required_kagemusha_v2_proof_exports:
    errors.append("Swift ABI-18 Kagemusha V2 requiredProofSymbols inventory drifted")
if swift_v2_protocol_inventory is None:
    errors.append("Swift ABI-18 Kagemusha V2 requiredProtocolSymbols inventory is missing")
elif len(swift_v2_protocol_inventory) != len(set(swift_v2_protocol_inventory)):
    errors.append("Swift ABI-18 Kagemusha V2 requiredProtocolSymbols inventory contains duplicates")
elif set(swift_v2_protocol_inventory) != required_kagemusha_v2_protocol_exports:
    errors.append("Swift ABI-18 Kagemusha V2 requiredProtocolSymbols inventory drifted")
if re.search(
    r"requiredNativeSymbols\s*=\s*requiredProofSymbols\s*\+\s*requiredProtocolSymbols",
    swift_v2_contract_text,
) is None:
    errors.append("Swift ABI-18 Kagemusha V2 requiredNativeSymbols must combine proof and protocol inventories")
if missing_exports:
    errors.append(
        "missing required Rust recursive-spend exports: " + ", ".join(missing_exports)
    )
if missing_header_declarations:
    errors.append(
        "missing required C header declarations: "
        + ", ".join(missing_header_declarations)
    )
if unexpected_exports:
    errors.append(
        "unexpected Rust recursive-spend exports: " + ", ".join(unexpected_exports)
    )
if unexpected_header_declarations:
    errors.append(
        "unexpected C header recursive-spend declarations: "
        + ", ".join(unexpected_header_declarations)
    )
if missing_kagemusha_native_rust_exports:
    errors.append(
        "missing required Rust ABI-18 Kagemusha V2 exports: "
        + ", ".join(missing_kagemusha_native_rust_exports)
    )
if missing_kagemusha_native_header_declarations:
    errors.append(
        "missing required C header ABI-18 Kagemusha V2 declarations: "
        + ", ".join(missing_kagemusha_native_header_declarations)
    )
if undeclared_exports:
    errors.append(
        "Rust recursive-spend exports missing from C header: "
        + ", ".join(undeclared_exports)
    )
if stale_header_declarations:
    errors.append(
        "C header recursive-spend declarations missing Rust exports: "
        + ", ".join(stale_header_declarations)
    )
for name, pattern in expected_recursive_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header recursive-spend declaration has wrong signature: {name}")
for name, pattern in expected_kagemusha_v2_rust_signatures.items():
    if re.search(pattern, rust_text) is None:
        errors.append(f"Rust ABI-18 Kagemusha V2 export has wrong signature: {name}")
for name, pattern in expected_kagemusha_v2_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header ABI-18 Kagemusha V2 declaration has wrong signature: {name}")
if re.search(expected_connect_norito_free_rust_signature, rust_text) is None:
    errors.append("Rust connect_norito_free export has wrong signature")
if re.search(expected_connect_norito_free_header_signature, header_text) is None:
    errors.append("C header connect_norito_free declaration has wrong signature")
if missing_privacy_exports:
    errors.append(
        "missing required Rust privacy FFI exports: "
        + ", ".join(missing_privacy_exports)
    )
if missing_privacy_header_declarations:
    errors.append(
        "missing required C header privacy declarations: "
        + ", ".join(missing_privacy_header_declarations)
    )
if undeclared_privacy_exports:
    errors.append(
        "Rust privacy FFI exports missing from C header: "
        + ", ".join(undeclared_privacy_exports)
    )
if stale_privacy_header_declarations:
    errors.append(
        "C header privacy declarations missing Rust exports: "
        + ", ".join(stale_privacy_header_declarations)
    )
for name, pattern in expected_privacy_signatures.items():
    if re.search(pattern, header_text) is None:
        errors.append(f"C header privacy declaration has wrong signature: {name}")
if missing_sorafs_reference_exports:
    errors.append(
        "missing required Rust SoraFS reference exports: "
        + ", ".join(missing_sorafs_reference_exports)
    )
if missing_sorafs_reference_header_declarations:
    errors.append(
        "missing required C header SoraFS reference declarations: "
        + ", ".join(missing_sorafs_reference_header_declarations)
    )
if undeclared_sorafs_reference_exports:
    errors.append(
        "Rust SoraFS reference exports missing from C header: "
        + ", ".join(undeclared_sorafs_reference_exports)
    )
if stale_sorafs_reference_header_declarations:
    errors.append(
        "C header SoraFS reference declarations missing Rust exports: "
        + ", ".join(stale_sorafs_reference_header_declarations)
    )
if not bridge_abi_export:
    errors.append("missing Rust C export: connect_norito_bridge_abi_version")
if not bridge_abi_declaration:
    errors.append("missing C header declaration: connect_norito_bridge_abi_version")
if bridge_abi_constant is None or bridge_abi_constant.group(1) != "18":
    errors.append("first-release connect_norito bridge ABI must be exactly 18")
if '#include "connect_norito_bridge.h"' not in umbrella_text:
    errors.append("NoritoBridge.h must include connect_norito_bridge.h")

if errors:
    raise SystemExit("\n".join(errors))

print(
    "connect_norito_bridge.h declares all "
    f"{len(required_recursive_ffi)} recursive spend symbols and "
    f"{len(required_kagemusha_v2_proof_exports)} ABI-18 V2 proof symbols and "
    f"{len(required_kagemusha_v2_protocol_exports)} ABI-18 V2 protocol symbols and "
    "the exact connect_norito_free deallocator and "
    f"{len(required_privacy_ffi)} privacy FFI symbols and "
    f"{len(required_sorafs_reference_ffi)} SoraFS reference symbols"
)
PY
}

make_negative_workspace() {
  local tmp
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/iroha-bridge-header.XXXXXX")"
  cp "${RUST_LIB}" "${tmp}/lib.rs"
  cp "${HEADER}" "${tmp}/connect_norito_bridge.h"
  cp "${UMBRELLA}" "${tmp}/NoritoBridge.h"
  cp "${SWIFT_V2_CONTRACT}" "${tmp}/KagemushaRecursiveSpendV2.swift"
  echo "${tmp}"
}

expect_contract_rejection() {
  local expected_fragment="${1}"
  local rust_lib="${2}"
  local header="${3}"
  local umbrella="${4}"
  local swift_v2_contract
  local output
  local status

  set +e
  swift_v2_contract="$(dirname "${rust_lib}")/KagemushaRecursiveSpendV2.swift"
  output="$(run_contract_check "${rust_lib}" "${header}" "${umbrella}" "${swift_v2_contract}" 2>&1)"
  status=$?
  set -e

  if [[ "${status}" -eq 0 ]]; then
    echo "[bridge-header] negative control unexpectedly passed" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected_fragment}"* ]]; then
    echo "[bridge-header] negative control failed for the wrong reason" >&2
    echo "[bridge-header] expected output fragment: ${expected_fragment}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "[bridge-header] negative control rejected expected drift: ${expected_fragment}"
}

if [[ "${MODE}" == --negative-control-* ]]; then
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT

  case "${MODE}" in
    --negative-control-bad-bridge-abi)
      perl -0pi -e 's/(CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*)18/${1}17/ or die "missing exact bridge ABI target\n"' "${tmp}/lib.rs"
      expect_contract_rejection \
        "first-release connect_norito bridge ABI must be exactly 18" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-recursive-header)
      perl -0pi -e 's/int32_t\s+connect_norito_kagemusha_recursive_spend_redeem\s*\([^;]*\);\n//s or die "missing recursive declaration target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required C header declarations" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-recursive-signature)
      perl -0pi -e 's/(int32_t\s+connect_norito_kagemusha_recursive_spend_capabilities_v1\s*\(\s*)uint8_t\*\*\s+out_capabilities_ptr/${1}uint8_t* out_capabilities_ptr/s or die "missing recursive capability signature target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "C header recursive-spend declaration has wrong signature: connect_norito_kagemusha_recursive_spend_capabilities_v1" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-recursive-v2-signature)
      perl -0pi -e 's/(connect_norito_kagemusha_recursive_spend_init_v2\s*\([^;]*?)const\s+uint8_t\*\s+request_norito_ptr/$1uint8_t* request_norito_ptr/s or die "missing recursive V2 signature target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "C header recursive-spend declaration has wrong signature: connect_norito_kagemusha_recursive_spend_init_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-recursive-v2-artifact-signature)
      perl -0pi -e 's|(// ---------------- Legacy V2 protocol scaffolding ----------------)|int32_t connect_norito_kagemusha_recursive_spend_artifact_begin_v2(const uint8_t* reference_norito_ptr, unsigned long reference_norito_len, uint32_t expected_role, uint64_t* out_handle);\n\n$1|s or die "missing legacy V2 protocol section target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "retired C header Kagemusha V2 artifact-ingest declarations reintroduced: connect_norito_kagemusha_recursive_spend_artifact_begin_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-recursive-v2-export-pair)
      perl -0pi -e 's/fn\s+connect_norito_kagemusha_recursive_spend_bundle_summary_v2\s*\(/fn removed_connect_norito_kagemusha_recursive_spend_bundle_summary_v2(/s or die "missing recursive V2 Rust export target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/int32_t\s+connect_norito_kagemusha_recursive_spend_bundle_summary_v2\s*\(/int32_t removed_connect_norito_kagemusha_recursive_spend_bundle_summary_v2(/s or die "missing recursive V2 header export target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required Rust recursive-spend exports: connect_norito_kagemusha_recursive_spend_bundle_summary_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-kagemusha-v2-protocol-export-pair)
      perl -0pi -e 's/fn\s+connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2\s*\(/fn removed_connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2(/s or die "missing Kagemusha V2 Rust protocol export target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/int32_t\s+connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2\s*\(/int32_t removed_connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2(/s or die "missing Kagemusha V2 header protocol export target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required Rust ABI-18 Kagemusha V2 exports: connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-kagemusha-v2-receiver-key-signature)
      perl -0pi -e 's/(fn\s+connect_norito_kagemusha_receiver_key_reference_v2\s*\(\s*)algorithm:\s*u8/${1}algorithm: u32/s or die "missing Kagemusha V2 Rust receiver-key algorithm target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/(connect_norito_kagemusha_receiver_key_reference_v2\s*\(\s*)uint8_t\s+algorithm/${1}uint32_t algorithm/s or die "missing Kagemusha V2 header receiver-key algorithm target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "Rust ABI-18 Kagemusha V2 export has wrong signature: connect_norito_kagemusha_receiver_key_reference_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-kagemusha-v2-verify-at-time-signature)
      perl -0pi -e 's/(fn\s+connect_norito_kagemusha_recipient_payment_request_verify_v2\s*\([^)]*?)verified_at_ms:\s*u64/${1}verified_at_ms: u32/s or die "missing Kagemusha V2 Rust verified-at target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/(connect_norito_kagemusha_recipient_payment_request_verify_v2\s*\([^;]*?)uint64_t\s+verified_at_ms/${1}uint32_t verified_at_ms/s or die "missing Kagemusha V2 header verified-at target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "Rust ABI-18 Kagemusha V2 export has wrong signature: connect_norito_kagemusha_recipient_payment_request_verify_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-kagemusha-v2-ack-create-signature)
      perl -0pi -e 's/(fn\s+connect_norito_kagemusha_receiver_acknowledgement_create_v2\s*\([^)]*?)\s*peer_payment_norito_ptr:\s*\*const\s+c_uchar,\s*peer_payment_norito_len:\s*c_ulong,/${1}/s or die "missing Kagemusha V2 Rust four-archive ACK target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/(connect_norito_kagemusha_receiver_acknowledgement_create_v2\s*\([^;]*?)\s*const\s+uint8_t\*\s+peer_payment_norito_ptr,\s*unsigned\s+long\s+peer_payment_norito_len,/${1}/s or die "missing Kagemusha V2 header four-archive ACK target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "Rust ABI-18 Kagemusha V2 export has wrong signature: connect_norito_kagemusha_receiver_acknowledgement_create_v2" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-connect-norito-free-signature)
      perl -0pi -e 's/(pub\s+extern\s+"C"\s+fn\s+connect_norito_free\s*\(\s*ptr_:\s*)\*mut\s+c_uchar/${1}*const c_uchar/s or die "missing Rust connect_norito_free target\n"' "${tmp}/lib.rs"
      perl -0pi -e 's/void\s+connect_norito_free\s*\(\s*uint8_t\s*\*\s*ptr\s*\)/void connect_norito_free(const uint8_t* ptr)/s or die "missing header connect_norito_free target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "Rust connect_norito_free export has wrong signature" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-rust-export)
      perl -0pi -e 's/pub\s+unsafe\s+extern\s+"C"\s+fn\s+connect_norito_kagemusha_recursive_spend_lineage_append_boundary\s*\(/pub unsafe extern "C" fn connect_norito_kagemusha_recursive_spend_lineage_append_boundary_removed(/s or die "missing Rust export target\n"' "${tmp}/lib.rs"
      expect_contract_rejection \
        "missing required Rust recursive-spend exports" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-privacy-header)
      perl -0pi -e 's/int32_t\s+iroha_privacy_build_proof_v1\s*\([^;]*\);\n//s or die "missing privacy declaration target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "missing required C header privacy declarations" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-bad-privacy-signature)
      perl -0pi -e 's/unsigned\s+long\s+request_len/unsigned long* request_len/s or die "missing privacy signature target\n"' "${tmp}/connect_norito_bridge.h"
      expect_contract_rejection \
        "C header privacy declaration has wrong signature: iroha_privacy_build_proof_v1" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-missing-privacy-rust-export)
      perl -0pi -e 's/pub\s+unsafe\s+extern\s+"C"\s+fn\s+iroha_privacy_build_proof_v1\s*\(/pub unsafe extern "C" fn iroha_privacy_build_proof_v1_removed(/s or die "missing privacy Rust export target\n"' "${tmp}/lib.rs"
      expect_contract_rejection \
        "missing required Rust privacy FFI exports" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    --negative-control-umbrella-drift)
      perl -0pi -e 's/#include "connect_norito_bridge\.h"\n//s or die "missing umbrella include target\n"' "${tmp}/NoritoBridge.h"
      expect_contract_rejection \
        "NoritoBridge.h must include connect_norito_bridge.h" \
        "${tmp}/lib.rs" \
        "${tmp}/connect_norito_bridge.h" \
        "${tmp}/NoritoBridge.h"
      ;;
    *)
      usage
      exit 2
      ;;
  esac
  exit 0
elif [[ -n "${MODE}" ]]; then
  usage
  exit 2
fi

run_contract_check "${RUST_LIB}" "${HEADER}" "${UMBRELLA}" "${SWIFT_V2_CONTRACT}"

if command -v "${CC:-cc}" >/dev/null 2>&1; then
  "${CC:-cc}" -fsyntax-only -x c -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${HEADER}"
else
  echo "[bridge-header] skipping C syntax check: ${CC:-cc} not found" >&2
fi

if command -v "${CXX:-c++}" >/dev/null 2>&1; then
  "${CXX:-c++}" -fsyntax-only -x c++ -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${UMBRELLA}"
else
  echo "[bridge-header] skipping C++ syntax check: ${CXX:-c++} not found" >&2
fi
