#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_V3_RELEASE_CONTRACT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]] || [[ $# -gt 1 ]]; then
  echo "usage: ci/check_kagemusha_v3_release_contract.sh [--self-test]" >&2
  exit 2
fi

# This path is retained because release automation still invokes the frozen
# ABI-19/V3 compatibility gate by name. ABI-20/V4 is the active release path,
# so both contracts must pass: V3 must remain byte-for-byte compatible while
# V4 must remain the exact-eight, independently typed production surface.
bash "${ROOT_DIR}/ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh"

python3 - "${ROOT_DIR}" "${MODE}" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
self_test = sys.argv[2] == "--self-test"
errors: list[str] = []

def read(relative: str) -> str:
    path = root / relative
    if not path.is_file():
        errors.append(f"missing required file: {relative}")
        return ""
    return path.read_text(encoding="utf-8")

def require(relative: str, text: str, *needles: str) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{relative}: missing {needle!r}")

def require_regex(relative: str, text: str, pattern: str, description: str) -> None:
    if re.search(pattern, text, re.MULTILINE) is None:
        errors.append(f"{relative}: missing {description}")

def forbid(relative: str, text: str, *needles: str) -> None:
    for needle in needles:
        if needle in text:
            errors.append(f"{relative}: retired or unauthenticated surface remains: {needle!r}")

model_path = "crates/iroha_data_model/src/offline/mod.rs"
packager_path = "crates/iroha_core/src/bin/kagemusha_recursive_spend_v3_bundle.rs"
bridge_path = "crates/connect_norito_bridge/src/lib.rs"
header_path = "crates/connect_norito_bridge/include/connect_norito_bridge.h"
build_path = "scripts/build_norito_xcframework.sh"
model = read(model_path)
packager = read(packager_path)
bridge = read(bridge_path)
header = read(header_path)
build = read(build_path)

require(
    model_path,
    model,
    'KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V3: &str =\n    "kagemusha.offline.recursive_spend.artifact_manifest.v3"',
    'KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2: &str = "topup-finality-roster.norito"',
    "pub source_tree_sha256: [u8; 32]",
    "pub source_repo_dirty: bool",
)
require_regex(
    model_path,
    model,
    r"^pub const KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V3:\s*u32\s*=\s*19\s*;$",
    "exact frozen ABI-19/V3 bridge constant",
)

artifact_constants = {
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMETERS_FILE_NAME_V3": "step-eq.parameters.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V3": "step-eq.proving-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V3": "step-eq.verifying-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMETERS_FILE_NAME_V3": "step-ep.parameters.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V3": "step-ep.proving-key.krv3",
    "KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V3": "step-ep.verifying-key.krv3",
}
declared_artifact_constants = {
    name: value
    for name, value in re.findall(
        r'pub const (KAGEMUSHA_RECURSIVE_SPEND_[A-Z_]+FILE_NAME_V3):\s*&str\s*=\s*\n?\s*"([^"]+)";',
        model,
    )
}
if declared_artifact_constants != artifact_constants:
    errors.append(
        f"{model_path}: V3 proof-key inventory must be exactly six files; "
        f"found {declared_artifact_constants}"
    )

inputs = re.search(
    r"const INPUTS:\s*&\[InputSpec\]\s*=\s*&\[(?P<body>[\s\S]*?)\n\];",
    packager,
)
if inputs is None:
    errors.append(f"{packager_path}: missing exact INPUTS inventory")
else:
    body = inputs.group("body")
    if body.count("InputSpec {") != 6:
        errors.append(f"{packager_path}: INPUTS must contain exactly six proof-key artifacts")
    for name in artifact_constants:
        if body.count(name) != 1:
            errors.append(f"{packager_path}: INPUTS must contain {name} exactly once")
require(
    packager_path,
    packager,
    'required(options, "topup-finality-roster")',
    'parse_digest(options, "source-tree-sha256")',
    'parse_bool(options, "source-repo-dirty")',
    "KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V2",
    "KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2",
)

require_regex(
    bridge_path,
    bridge,
    r"^const CONNECT_NORITO_BRIDGE_ABI_VERSION:\s*u32\s*=\s*20\s*;$",
    "exact active ABI-20 bridge constant",
)
release_symbols = (
    "connect_norito_bridge_abi_version",
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
)
for symbol in release_symbols:
    require(bridge_path, bridge, symbol)
    require(header_path, header, symbol)

frozen_header_declarations = {
    "connect_norito_bridge_abi_version":
        "uint32_t connect_norito_bridge_abi_version(void);",
    "connect_norito_kagemusha_recursive_spend_capabilities_v1":
        "int32_t connect_norito_kagemusha_recursive_spend_capabilities_v1("
        "uint8_t** out_capabilities_ptr,unsigned long* out_capabilities_len);",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_begin_v3("
        "const uint8_t* manifest_norito_ptr,unsigned long manifest_norito_len,"
        "const uint8_t* expected_manifest_sha256_ptr,"
        "unsigned long expected_manifest_sha256_len,"
        "const uint8_t* expected_artifact_sha256_ptr,"
        "unsigned long expected_artifact_sha256_len,uint64_t* out_handle);",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_write_v3("
        "uint64_t handle,const uint8_t* chunk_ptr,unsigned long chunk_len);",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_finalize_v3("
        "uint64_t handle);",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_cancel_v3("
        "uint64_t handle);",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_set_install_v3("
        "const uint8_t* manifest_norito_ptr,unsigned long manifest_norito_len,"
        "const uint8_t* expected_manifest_sha256_ptr,"
        "unsigned long expected_manifest_sha256_len,"
        "const uint8_t* trusted_policy_norito_ptr,unsigned long trusted_policy_norito_len,"
        "const uint8_t* release_attestation_norito_ptr,"
        "unsigned long release_attestation_norito_len,"
        "const uint8_t* benchmark_evidence_ptr,unsigned long benchmark_evidence_len,"
        "const uint8_t* cryptographic_review_ptr,unsigned long cryptographic_review_len,"
        "const uint64_t* handles_ptr,unsigned long handles_len);",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3("
        "const uint8_t* manifest_norito_ptr,unsigned long manifest_norito_len,"
        "const uint8_t* expected_manifest_sha256_ptr,"
        "unsigned long expected_manifest_sha256_len,uint8_t* out_installed);",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3":
        "int32_t connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3("
        "const uint8_t* expected_manifest_sha256_ptr,"
        "unsigned long expected_manifest_sha256_len);",
}

def normalize_c_declaration(declaration: str) -> str:
    declaration = re.sub(r"\s+", " ", declaration).strip()
    declaration = re.sub(r"\s*([(),;])\s*", r"\1", declaration)
    return re.sub(r"\s*\*\s*", "*", declaration)

def frozen_header_signature_errors(source: str) -> list[str]:
    source = re.sub(r"/\*[\s\S]*?\*/|//[^\n]*", "", source)
    signature_errors: list[str] = []
    for symbol, expected in frozen_header_declarations.items():
        matches = re.findall(
            rf"\b(?:uint32_t|int32_t)\s+{re.escape(symbol)}\s*\([^;]*\);",
            source,
        )
        actual = [normalize_c_declaration(match) for match in matches]
        expected = normalize_c_declaration(expected)
        if actual != [expected]:
            signature_errors.append(
                f"{header_path}: frozen declaration drift for {symbol}: {actual!r}"
            )
    return signature_errors

errors.extend(frozen_header_signature_errors(header))
if self_test:
    old_parameter = "unsigned long expected_manifest_sha256_len"
    begin_match = re.search(
        r"int32_t\s+connect_norito_kagemusha_recursive_spend_artifact_begin_v3\s*"
        r"\([^;]*\);",
        header,
    )
    if begin_match is None:
        errors.append("V3 self-test could not locate the frozen artifact-begin declaration")
        mutated = header
    else:
        changed_declaration = begin_match.group(0).replace(
            old_parameter,
            "uint32_t expected_manifest_sha256_len",
            1,
        )
        mutated = header[:begin_match.start()] + changed_declaration + header[begin_match.end():]
    mutated += f"\n/* {frozen_header_declarations['connect_norito_kagemusha_recursive_spend_artifact_begin_v3']} */\n"
    if not frozen_header_signature_errors(mutated):
        errors.append("V3 self-test accepted ABI parameter drift hidden by a comment")
    else:
        print("self-test passed: frozen-ABI19-parameter-drift-comment-spoof")

require(
    bridge_path,
    bridge,
    "trusted_policy_norito_ptr: *const c_uchar",
    "release_attestation_norito_ptr: *const c_uchar",
    "benchmark_evidence_ptr: *const c_uchar",
    "cryptographic_review_ptr: *const c_uchar",
    "authenticated_artifact_install_rejects_release_material_substitution_atomically",
)
require(
    header_path,
    header,
    "const uint8_t* trusted_policy_norito_ptr",
    "const uint8_t* release_attestation_norito_ptr",
    "const uint8_t* benchmark_evidence_ptr",
    "const uint8_t* cryptographic_review_ptr",
)
forbid(
    bridge_path,
    bridge,
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_authenticated_v3",
)
forbid(
    header_path,
    header,
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_authenticated_v3",
)

require(
    build_path,
    build,
    "--privacy-production-enabled",
    '"ios-arm64"',
    '"ios-arm64_x86_64-simulator"',
    '"macos-arm64"',
    '"connect_norito_bridge_abi_version"',
    '"connect_norito_kagemusha_recursive_spend_artifact_set_install_v4"',
    '"source_commit"',
    '"bridge_header_sha256"',
)

current_mobile_artifacts = (
    "step-eq.parameters.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.parameters.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
for relative in (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
):
    text = read(relative)
    require(
        relative,
        text,
        "REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
        "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION",
        "20",
        '"kagemusha.offline.recursive_spend.artifact_manifest.v4"',
    )
    for file_name in current_mobile_artifacts:
        require(relative, text, file_name)

swift_path = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"
swift_native_path = "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
swift = read(swift_path)
swift_native = read(swift_native_path)
require(
    swift_path,
    swift,
    "KagemushaRecursiveSpendReleaseAuthenticationV3",
    "authentication: KagemushaRecursiveSpendReleaseAuthenticationV3",
    "trustedPolicyArchive: authentication.trustedPolicyNorito",
    "cryptographicReview: authentication.cryptographicReview",
)
require(
    swift_native_path,
    swift_native,
    "trustedPolicyArchive: Data",
    "releaseAttestationArchive: Data",
    "benchmarkEvidence: Data",
    "cryptographicReview: Data",
)

for relative in (
    "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/KagemushaRecursiveSpendProver.java",
    "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaRecursiveSpendProver.kt",
):
    text = read(relative)
    require(
        relative,
        text,
        "ReleaseAuthentication",
        "trustedPolicyNorito",
        "releaseAttestationNorito",
        "benchmarkEvidence",
        "cryptographicReview",
    )

if errors:
    print("Kagemusha release compatibility contract failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)
print(
    "Kagemusha release compatibility contract passed: frozen ABI 19/V3 "
    "six-key surface plus active ABI 20/V4 exact-eight surface."
)
PY
