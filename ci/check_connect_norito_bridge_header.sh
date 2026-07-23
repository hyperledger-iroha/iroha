#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_LIB="${ROOT_DIR}/crates/connect_norito_bridge/src/lib.rs"
HEADER="${ROOT_DIR}/crates/connect_norito_bridge/include/connect_norito_bridge.h"
UMBRELLA="${ROOT_DIR}/crates/connect_norito_bridge/include/NoritoBridge.h"
SWIFT_CONTRACT="${ROOT_DIR}/IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
MODE="${1:-}"

SELF_TESTS=(
  --self-test-bad-abi
  --self-test-missing-header-symbol
  --self-test-forbidden-v3-alias
  --self-test-bad-capability-signature
  --self-test-bad-proof-signature
  --self-test-bad-artifact-signature
  --self-test-missing-export-pair
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

  python3 - "${rust_lib}" "${header}" "${umbrella}" "${swift_contract}" <<'PY'
from pathlib import Path
import re
import sys

rust = Path(sys.argv[1]).read_text(encoding="utf-8")
header = Path(sys.argv[2]).read_text(encoding="utf-8")
umbrella = Path(sys.argv[3]).read_text(encoding="utf-8")
swift = Path(sys.argv[4]).read_text(encoding="utf-8")

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
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
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
    "connect_norito_kagemusha_request_authorization_create_v2",
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
}

required_privacy_ffi = (
    "iroha_privacy_capabilities_v1",
    "iroha_privacy_proof_request_v1",
    "iroha_privacy_build_proof_v1",
    "iroha_privacy_verify_proof_v1",
    "iroha_privacy_free_buffer",
)
PRIVACY_EXPORTS = set(required_privacy_ffi)

SORAFS_REFERENCE_EXPORTS = {
    "connect_norito_sorafs_reference_build_signed_orderbook_order_cancel",
    "connect_norito_sorafs_reference_build_signed_orderbook_order_request",
    "connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt",
    "connect_norito_sorafs_reference_derive_orderbook_order_id",
    "connect_norito_sorafs_reference_sign_orderbook_payload",
    "connect_norito_sorafs_reference_validate_hedging_json",
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

def rust_exports(prefix: str) -> set[str]:
    return set(re.findall(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+({re.escape(prefix)}[a-z0-9_]+)\s*\(',
        rust,
    ))

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
    "iroha_privacy_capabilities_v1": 2,
    "iroha_privacy_proof_request_v1": 14,
    "iroha_privacy_build_proof_v1": 4,
    "iroha_privacy_verify_proof_v1": 4,
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

rust_detached = rust_exports("connect_norito_detached_transaction_") | rust_exports("connect_norito_canonical_json_")
header_detached = header_exports("connect_norito_detached_transaction_") | header_exports("connect_norito_canonical_json_")
exact("Rust detached transaction", DETACHED_EXPORTS, rust_detached)
exact("C header detached transaction", DETACHED_EXPORTS, header_detached)

signer_name = re.compile(
    r"^connect_norito_encode_[a-z0-9_]+_signed_transaction(?:_alg)?$"
)
rust_transaction_signers = {
    name for name in rust_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
header_transaction_signers = {
    name for name in header_exports("connect_norito_encode_") if signer_name.fullmatch(name)
}
if len(rust_transaction_signers) != 34:
    raise SystemExit(
        "Rust transaction signer inventory must contain exactly 17 base/algorithm pairs: "
        f"found {len(rust_transaction_signers)}"
    )
exact("C header transaction signer", rust_transaction_signers, header_transaction_signers)
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
    | rust_transaction_signers
    | {"connect_norito_bridge_abi_version", "connect_norito_free"}
)

abi = re.search(r"CONNECT_NORITO_BRIDGE_ABI_VERSION\s*:\s*u32\s*=\s*([0-9]+)\s*;", rust)
if abi is None or abi.group(1) != "21":
    raise SystemExit("connect_norito bridge ABI must be exactly 21")
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
        "Swift ABI-21 inventory must contain 4 proof and "
        f"{expected_protocol_count} protocol symbols"
    )
swift_exports = swift_proof_exports | swift_protocol_exports
exact("Swift Kagemusha", KAGEMUSHA_EXPORTS, swift_exports)
if re.search(r"requiredNativeSymbols\s*=\s*requiredProofSymbols\s*\+\s*requiredProtocolSymbols", swift) is None:
    raise SystemExit("Swift requiredNativeSymbols must combine the exact proof and protocol inventories")

print(
    "bridge header contract passed: ABI 21, "
    f"{len(KAGEMUSHA_EXPORTS)} Kagemusha exports, "
    f"{len(PRIVACY_EXPORTS)} privacy exports, "
    f"{len(SORAFS_REFERENCE_EXPORTS)} SoraFS exports, and "
    f"{len(DETACHED_EXPORTS)} detached-transaction exports"
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
      "${tmp}/KagemushaRecursiveSpendV2.swift" 2>&1)"; then
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
  run_contract_check "${RUST_LIB}" "${HEADER}" "${UMBRELLA}" "${SWIFT_CONTRACT}" >/dev/null
  tmp="$(make_negative_workspace)"
  trap 'rm -rf "${tmp}"' EXIT
  tmp_rust="${tmp}/lib.rs"
  tmp_header="${tmp}/connect_norito_bridge.h"
  tmp_umbrella="${tmp}/NoritoBridge.h"
  tmp_swift="${tmp}/KagemushaRecursiveSpendV2.swift"

  case "${MODE}" in
    --self-test-bad-abi)
      replace_once "${tmp_rust}" \
        "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 21;" \
        "const CONNECT_NORITO_BRIDGE_ABI_VERSION: u32 = 19;"
      ;;
    --self-test-missing-header-symbol)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recursive_spend_redeem_v4" \
        "removed_connect_norito_kagemusha_recursive_spend_redeem_v4"
      ;;
    --self-test-forbidden-v3-alias)
      replace_once "${tmp_header}" \
        "connect_norito_kagemusha_recursive_spend_init_v4" \
        "connect_norito_kagemusha_recursive_spend_init_v3"
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
        "iroha_privacy_build_proof_v1" \
        "removed_iroha_privacy_build_proof_v1"
      ;;
    --self-test-bad-privacy-signature)
      replace_regex_once "${tmp_header}" \
        '(iroha_privacy_build_proof_v1\s*\([^;]*?)unsigned long request_len' \
        '\g<1>unsigned long* request_len'
      ;;
    --self-test-missing-privacy-rust-symbol)
      replace_once "${tmp_rust}" \
        "iroha_privacy_build_proof_v1" \
        "removed_iroha_privacy_build_proof_v1"
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

run_contract_check "${RUST_LIB}" "${HEADER}" "${UMBRELLA}" "${SWIFT_CONTRACT}"

if command -v "${CC:-cc}" >/dev/null 2>&1; then
  "${CC:-cc}" -fsyntax-only -x c \
    -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${HEADER}"
fi

if command -v "${CXX:-c++}" >/dev/null 2>&1; then
  "${CXX:-c++}" -fsyntax-only -x c++ \
    -I"${ROOT_DIR}/crates/connect_norito_bridge/include" "${UMBRELLA}"
fi
