#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_V4_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

MODE="${1:-}"
if [[ $# -gt 1 || ( -n "$MODE" && "$MODE" != "--self-test" ) ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_v4_sdk_contract.sh [--self-test]" >&2
  exit 2
fi

python3 - "$ROOT_DIR" "$MODE" "${BASH_SOURCE[0]}" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


root = Path(sys.argv[1]).resolve()
mode = sys.argv[2]
script = Path(sys.argv[3]).resolve()
paths = {
    "data_model": Path("crates/iroha_data_model/src/offline/mod.rs"),
    "data_model_component": Path(
        "crates/iroha_data_model/src/offline/kagemusha_model.rs"
    ),
    "data_model_verifier": Path(
        "crates/iroha_data_model/src/offline/kagemusha_release_verifier.rs"
    ),
    "rust": Path("crates/connect_norito_bridge/src/lib.rs"),
    "rust_platform_jni": Path("crates/connect_norito_bridge/src/platform_jni.rs"),
    "rust_platform_jni_part_1": Path(
        "crates/connect_norito_bridge/src/platform_jni/part_1.rs"
    ),
    "rust_platform_jni_part_2": Path(
        "crates/connect_norito_bridge/src/platform_jni/part_2.rs"
    ),
    "rust_platform_jni_part_3": Path(
        "crates/connect_norito_bridge/src/platform_jni/part_3.rs"
    ),
    "rust_platform_jni_private_settlement": Path(
        "crates/connect_norito_bridge/src/platform_jni/private_settlement.rs"
    ),
    "header": Path("crates/connect_norito_bridge/include/connect_norito_bridge.h"),
    "swift": Path("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift"),
    "swift_v4": Path("IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4.swift"),
    "swift_v4_codecs": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV4Codecs.swift"
    ),
    "swift_codecs": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Codecs.swift"
    ),
    "swift_native": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
    ),
    "swift_bridge": Path("IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"),
    "swift_coordinator": Path(
        "IrohaSwift/Sources/IrohaSwift/KagemushaArtifactCoordinator.swift"
    ),
    "swift_hardware_test": Path(
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaHardwareAuthorizationV2Tests.swift"
    ),
    "hardware_authorization_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "kagemusha_request_authorization_v3_hardware.hex"
    ),
    "recipient_request_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_payment_request_v2.hex"
    ),
    "recipient_receive_offer_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_receive_offer_v2.hex"
    ),
    "recipient_lineage_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_registration_lineage_v2.hex"
    ),
    "recipient_checkpoint_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_checkpoint_envelope.hex"
    ),
    "peer_payment_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/offline_peer_payment_v4.hex"
    ),
    "peer_payment_generator": Path(
        "tools/kotlin-fixture-gen/src/bin/swift_kagemusha_peer_payment_v4.rs"
    ),
    "swift_peer_fixtures": Path(
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaPeerTransportTestFixtures.swift"
    ),
    "swift_peer_tests": Path(
        "IrohaSwift/Tests/IrohaSwiftTests/KagemushaPeerTransportTests.swift"
    ),
    "kagami": Path("crates/iroha_kagami/src/kagemusha.rs"),
    "bundle": Path("crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle.rs"),
    "bundle_source_seal_inputs": Path(
        "crates/iroha_core/src/bin/kagemusha_recursive_spend_v4_bundle/"
        "source_seal_build_inputs.rs"
    ),
    "core_artifact": Path("crates/iroha_core/src/zk/kagemusha_artifact_v4.rs"),
    "kotlin": Path(
        "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/"
        "KagemushaRecursiveSpendProver.kt"
    ),
    "java": Path(
        "java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/"
        "KagemushaRecursiveSpendProver.java"
    ),
    "android_keymint": Path(
        "java/iroha_android/android/src/main/java/org/hyperledger/iroha/android/offline/"
        "KagemushaAndroidKeyMint.java"
    ),
    "kotlin_android_keymint": Path(
        "kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/"
        "KagemushaAndroidKeyMint.kt"
    ),
    "kotlin_android_keymint_test": Path(
        "kotlin/client-android/src/test/java/org/hyperledger/iroha/sdk/offline/"
        "KagemushaAndroidKeyMintTest.kt"
    ),
    "kotlin_android_keymint_device_test": Path(
        "kotlin/client-android/src/androidTest/java/org/hyperledger/iroha/sdk/offline/"
        "KagemushaAndroidKeyMintInstrumentedTest.kt"
    ),
    "xcframework_build": Path("scripts/build_norito_xcframework.sh"),
    "mobile_check": Path("scripts/check_mobile_sdk_artifacts.sh"),
    "mobile_check_test": Path("scripts/check_mobile_sdk_artifacts_test.sh"),
    "mobile_check_test_fixture_helpers": Path(
        "scripts/tests/mobile_sdk_artifact_fixture_helpers.sh"
    ),
}

texts: dict[str, str] = {}
for label, relative in paths.items():
    absolute = root / relative
    if not absolute.is_file():
        raise SystemExit(f"required ABI21 contract file is missing: {relative}")
    texts[label] = absolute.read_text(encoding="utf-8")

mobile_check_test_shellcheck = (
    "# shellcheck source=tests/mobile_sdk_artifact_fixture_helpers.sh"
)
mobile_check_test_source = (
    'source "$SCRIPT_DIR/tests/mobile_sdk_artifact_fixture_helpers.sh"'
)
if texts["mobile_check_test"].count(mobile_check_test_shellcheck) != 1:
    raise SystemExit(
        f"{paths['mobile_check_test']}: expected exactly one canonical "
        "fixture-helper shellcheck directive"
    )
if texts["mobile_check_test"].count(mobile_check_test_source) != 1:
    raise SystemExit(
        f"{paths['mobile_check_test']}: expected exactly one reviewed "
        "fixture-helper source"
    )
texts["mobile_check_test"] = texts["mobile_check_test"].replace(
    mobile_check_test_source,
    texts["mobile_check_test_fixture_helpers"],
    1,
)

data_model_include = 'include!("kagemusha_model.rs");'
if texts["data_model"].count(data_model_include) != 1:
    raise SystemExit(
        f"{paths['data_model']}: expected exactly one reviewed "
        f"{paths['data_model_component'].name} include"
    )
texts["data_model"] = texts["data_model"].replace(
    data_model_include,
    texts["data_model_component"],
    1,
)
data_model_verifier_module = "mod kagemusha_release_verifier;"
if texts["data_model"].count(data_model_verifier_module) != 1:
    raise SystemExit(
        f"{paths['data_model']}: expected exactly one reviewed "
        f"{paths['data_model_verifier'].name} module"
    )
for marker in (
    "const VERIFIER_IDENTITY_SCHEMA_V4",
    "pub fn kagemusha_recursive_spend_verifier_key_id_v4",
):
    if texts["data_model_verifier"].count(marker) != 1:
        raise SystemExit(
            f"{paths['data_model_verifier']}: expected exactly one {marker!r}"
        )
texts["data_model"] = texts["data_model"].replace(
    data_model_verifier_module,
    "mod kagemusha_release_verifier {\n"
    + texts["data_model_verifier"]
    + "\n}",
    1,
)

if len(re.findall(r"^mod platform_jni;$", texts["rust"], re.MULTILINE)) != 1:
    raise SystemExit(
        f"{paths['rust']}: expected exactly one reviewed platform_jni module"
    )
platform_jni_includes = tuple(
    re.findall(
        r'^include!\("([^"]+)"\);$',
        texts["rust_platform_jni"],
        re.MULTILINE,
    )
)
expected_platform_jni_includes = (
    *(f"platform_jni/part_{part}.rs" for part in range(1, 4)),
    "platform_jni/private_settlement.rs",
)
if platform_jni_includes != expected_platform_jni_includes:
    raise SystemExit(
        f"{paths['rust_platform_jni']}: expected the reviewed JNI include "
        f"closure, found {platform_jni_includes!r}"
    )
texts["rust"] = "\n".join(
    (
        texts["rust"],
        texts["rust_platform_jni"],
        *(texts[f"rust_platform_jni_part_{part}"] for part in range(1, 4)),
        texts["rust_platform_jni_private_settlement"],
    )
)
bundle_source_seal_include = (
    'include!("kagemusha_recursive_spend_v4_bundle/source_seal_build_inputs.rs");'
)
if texts["bundle"].count(bundle_source_seal_include) != 1:
    raise SystemExit(
        f"{paths['bundle']}: expected exactly one reviewed source-seal input include"
    )
texts["bundle"] = texts["bundle"].replace(
    bundle_source_seal_include, texts["bundle_source_seal_inputs"], 1
)

jni_forwarder_methods = re.findall(
    r"^\s*(native[A-Za-z0-9]+)\s*\{",
    texts["rust_platform_jni_part_3"],
    re.MULTILINE,
)
duplicate_jni_forwarders = sorted(
    method for method in set(jni_forwarder_methods) if jni_forwarder_methods.count(method) != 1
)
lifecycle_macro_exports = set(
    re.findall(
        r"=>\s*(connect_norito_kagemusha_recursive_spend_[a-z0-9_]+_v4)\s*,",
        texts["rust"],
    )
)

errors: list[str] = []

if duplicate_jni_forwarders:
    errors.append(
        f"{paths['rust_platform_jni_part_3']}: duplicate generated JNI methods "
        f"{duplicate_jni_forwarders!r}"
    )


def require(label: str, needle: str) -> None:
    if needle not in texts[label]:
        errors.append(f"{paths[label]}: missing {needle!r}")


def require_regex(label: str, pattern: str, description: str) -> None:
    if re.search(pattern, texts[label], re.MULTILINE | re.DOTALL) is None:
        errors.append(f"{paths[label]}: missing {description}")


def canonical_hex_vector(label: str) -> bytes:
    lines = texts[label].splitlines()
    if (
        not lines
        or any(not re.fullmatch(r"[0-9a-f]{2,64}", line) for line in lines)
        or any(len(line) != 64 for line in lines[:-1])
        or len(lines[-1]) % 2 != 0
    ):
        errors.append(f"{paths[label]}: fixture is not canonical lowercase 32-byte-line hex")
        return b""
    return bytes.fromhex("".join(lines))


require_regex(
    "rust",
    r"macro_rules!\s+kagemusha_sdk_android_forwarders\s*\{[\s\S]*?"
    r"Java_org_hyperledger_iroha_sdk_offline_KagemushaRecursiveSpendProver_[\s\S]*?"
    r"stringify!\(\$method\)[\s\S]*?pub unsafe extern \"system\" fn sdk[\s\S]*?"
    r"Java_org_hyperledger_iroha_android_offline_KagemushaRecursiveSpendProver_[\s\S]*?"
    r"stringify!\(\$method\)[\s\S]*?pub unsafe extern \"system\" fn android",
    "reviewed dual-namespace JNI forwarder generator",
)
require_regex(
    "rust",
    r"macro_rules!\s+kagemusha_recursive_spend_lifecycle_exports\s*\{[\s\S]*?"
    r"pub unsafe extern \"C\" fn \$init_name[\s\S]*?"
    r"pub unsafe extern \"C\" fn \$append_name[\s\S]*?"
    r"pub unsafe extern \"C\" fn \$verify_name[\s\S]*?"
    r"pub unsafe extern \"C\" fn \$redeem_name",
    "reviewed four-stage C lifecycle export generator",
)


for label, expected_size, expected_sha256 in (
    ("recipient_request_vector", 753,
     "d325566b1117fa368703a971367056173f2d8349d2e86101dc06187aaf8fd2b4"),
    ("recipient_receive_offer_vector", 12_435,
     "393f8a8827b66069e8fd47d2aa301a497cf800f1ce011a2b468ef22a5f2237c6"),
    ("recipient_lineage_vector", 11_297,
     "b61dd641527bfb9e09479906c008b6c061b54009229e6e9ec5f0717572cfb561"),
    ("recipient_checkpoint_vector", 405,
     "e6f7bbdd91955dc0b1a6f94a3d8ad284ae44c48e34a710484d8753e4e800973c"),
    ("peer_payment_vector", 12_896,
     "37ee56ad5663ab67b8b5b9a72927f1e0811142122bf04fa28a55634f96b7d3af"),
):
    vector = canonical_hex_vector(label)
    if len(vector) != expected_size:
        errors.append(
            f"{paths[label]}: expected {expected_size} bytes, found {len(vector)}"
        )
    if hashlib.sha256(vector).hexdigest() != expected_sha256:
        errors.append(f"{paths[label]}: canonical SHA-256 mismatch")
for needle in (
    "--recipient-request-hex",
    "request.digest()",
    "0_u8..4",
    "bls_normal_pop_prove",
    "DualQuorum::from_roster",
    "complete_context.id()",
    "signers: vec![0, 1, 2]",
    "norito::to_bytes(&payment)",
):
    require("peer_payment_generator", needle)
require_regex(
    "peer_payment_generator",
    r"payment\s*\.validate_public_binding\(\)",
    "public-binding validation of the emitted peer payment",
)
require_regex(
    "peer_payment_generator",
    r"let mut validator_keys = \(0_u8\.\.4\).*?"
    r"validator_keys\.sort_unstable_by_key.*?"
    r"let validator_set = validator_keys.*?power: 1,.*?"
    r"let validator_set_pops = validator_keys.*?bls_normal_pop_prove",
    "ordered four-validator unit-power roster with matching BLS PoPs",
)
require_regex(
    "peer_payment_generator",
    r"let complete_context = HeightContext \{.*?"
    r"mode: ConsensusMode::Permissioned,.*?"
    r"roster: validator_set\.to_vec\(\),.*?"
    r"quorum: DualQuorum::from_roster\(validator_set\).*?"
    r"complete_context\s*\.validate\(\).*?"
    r"let context_id = complete_context\.id\(\);.*?"
    r"phase: GlobalPhase::Commit,.*?signers: vec!\[0, 1, 2\]",
    "one validated four-validator context reused by the three-signer Commit QC",
)
for needle in (
    "fixture_uses_a_canonical_four_validator_commit_quorum",
    "assert_eq!(window.validator_set.len(), 4)",
    "assert_eq!(window.validator_set_pops.len(), 4)",
    "assert_eq!(certificate.signers, [0, 1, 2])",
):
    require("peer_payment_generator", needle)
for needle in (
    'rustFixtureData("offline_recipient_payment_request_v2.hex")',
    'rustFixtureData("offline_peer_payment_v4.hex")',
    "KagemushaReceiverAcknowledgement.prepare(",
):
    require("swift_peer_fixtures", needle)
for needle in (
    '"37ee56ad5663ab67b8b5b9a72927f1e0811142122bf04fa28a55634f96b7d3af"',
    '"cb6508a5aa6b56ada90978d4db638b2176f20f154e9de4ed8a450d95a940c71b"',
    ".archiveTooLarge(",
    "maximumTextArchiveBytes",
):
    require("swift_peer_tests", needle)


v4_files = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
v4_roles = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
v4_case_names = (
    "stepEqParamsIpa",
    "stepEqProvingKey",
    "stepEqVerifyingKey",
    "stepEqBootstrapWitness",
    "stepEpParamsIpa",
    "stepEpProvingKey",
    "stepEpVerifyingKey",
    "stepEpBootstrapWitness",
)
sdk_schema_types = {
    "InitRequestV4": "KagemushaRecursiveSpendInitLocalRequestV4",
    "AppendRequestV4": "KagemushaRecursiveSpendAppendLocalRequestV4",
    "VerifyRequestV4": "KagemushaRecursiveSpendVerifyLocalRequestV4",
    "RedeemRequestV5": "KagemushaRecursiveSpendRedeemLocalRequestV5",
    "InitResultV4": "KagemushaRecursiveSpendInitResultV4",
    "SplitResultV4": "KagemushaRecursiveSpendSplitResultV4",
    "VerifyResultV4": "KagemushaRecursiveSpendVerifyResultV4",
    "RedeemBuildResultV4": "KagemushaRecursiveSpendRedeemBuildResultV4",
}

native_methods = (
    "nativeKagemushaContractRevision",
    "nativePastaCycleV4BackendAvailable",
    "nativeArtifactBeginV4",
    "nativeArtifactWriteV4",
    "nativeArtifactFinalizeV4",
    "nativeArtifactCancelV4",
    "nativeArtifactSetInstallV4",
    "nativeArtifactSetIsInstalledV4",
    "nativeArtifactSetUninstallV4",
    "nativeBuildOutputMembershipFrontierV4",
    "nativeBuildOutputMembershipPathsV4",
    "nativeDeriveOutputMembershipPathsV4",
    "nativeValidateSpendableBranchV4",
    "nativeBuildInitRequestV4",
    "nativeBuildAppendRequestV4",
    "nativeBuildVerifyRequestV4",
    "nativeBuildRedeemRequestV5",
    "nativePrepareAuthorizationV3",
    "nativeFinalizeHardwareAuthorizationV3",
    "nativeFinalizeIosAppAttestAuthorizationV3",
    "nativeFinalizeTopUpV5",
    "nativeFinalizeRedeemV5",
    "nativePrepareTopUpV5",
    "nativeProjectOperationReferenceV2",
    "nativeProjectOperationStatusV2",
    "nativeInitSpendV4",
    "nativeAppendSpendV4",
    "nativeVerifySpendV4",
    "nativeBuildRedeemV4",
)

for label in ("kotlin", "java"):
    require(label, "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION")
    require(label, "V4_ARTIFACT_MANIFEST_SCHEMA")
    require(label, "V4_ARTIFACT_COUNT")
    require(label, "kagemusha.offline.recursive_spend.artifact_manifest.v4")
    require(label, "requireV4ArtifactBridge")
    require(label, "requireV4ProofBackend")
    require(label, "MAX_PROMOTION_RECORD_BYTES")
    require(label, "promotionRecordNorito")
    for file_name in v4_files:
        require(label, file_name)
    for type_name, schema in sdk_schema_types.items():
        require_regex(
            label,
            rf"class\s+{re.escape(type_name)}\b[\s\S]{{0,700}}?\"{re.escape(schema)}\"",
            f"distinct {type_name} wrapper bound to {schema}",
        )
    for method in ("initSpendV4", "appendSpendV4", "verifySpendV4", "buildRedeemV4"):
        require_regex(label, rf"\b{method}\s*\(", f"distinct public V4 lifecycle method {method}")
    for method in (
        "buildOutputMembershipFrontierV4",
        "decodeOutputMembershipFrontierV4",
        "deriveOutputMembershipPathsV4",
        "restoreSpendableBranchV4",
    ):
        require_regex(label, rf"\b{method}\s*\(", f"proof-bound V4 frontier method {method}")
    require(label, "connect_norito_bridge::KagemushaOutputMembershipFrontierV4")

require_regex(
    "kotlin",
    r"class\s+ReleaseAuthentication\s*\("
    r"[\s\S]{0,900}?cryptographicReview[^\n]*\n"
    r"[\s\S]{0,120}?promotionRecordNorito[\s\S]{0,40}?\)\s*\{",
    "mandatory promotion record constructor input",
)
require_regex(
    "java",
    r"public\s+ReleaseAuthentication\s*\("
    r"[\s\S]{0,900}?cryptographicReview[^\n]*\n"
    r"[\s\S]{0,120}?promotionRecordNorito[\s\S]{0,40}?\)\s*\{",
    "mandatory promotion record constructor input",
)
for label in ("kotlin", "java"):
    require_regex(
        label,
        r"nativeArtifactSetInstallV4\s*\("
        r"[\s\S]{0,900}?cryptographicReview[^\n]*\n"
        r"[\s\S]{0,120}?promotionRecordNorito[^\n]*\n"
        r"[\s\S]{0,120}?artifactHandles",
        "promotion-record-bound native install signature",
    )

require("swift", "maximumPromotionRecordBytesV4")
require("swift", "promotionRecordNorito")
require_regex(
    "swift",
    r"struct\s+KagemushaRecursiveSpendReleaseAuthenticationV4\b"
    r"[\s\S]{0,1300}?cryptographicReview:\s*Data,"
    r"[\s\S]{0,160}?promotionRecordNorito:\s*Data",
    "mandatory Swift promotion record constructor input",
)
require_regex(
    "swift_native",
    r"kagemushaRecursiveSpendArtifactSetInstallV4\s*\("
    r"[\s\S]{0,600}?cryptographicReview:\s*Data,"
    r"[\s\S]{0,160}?promotionRecordArchive:\s*Data,"
    r"[\s\S]{0,100}?handles:\s*\[UInt64\]",
    "promotion-record-bound Swift native install signature",
)
require_regex(
    "swift",
    r"authorizeWithIosAppAttest\s*\("
    r"[\s\S]{0,900}?service:\s*DCAppAttestService\s*=\s*\.shared"
    r"[\s\S]{0,900}?service\.isSupported"
    r"[\s\S]{0,1300}?service\.generateAssertion\s*\("
    r"[\s\S]{0,300}?clientDataHash:\s*signingBytes"
    r"[\s\S]{0,1300}?finalizeIosAppAttest\(assertionObject:\s*assertionObject\)",
    "physical App Attest assertion over the exact authorization preparation",
)
require("swift_hardware_test", "final class KagemushaHardwareAuthorizationV2Tests")
require(
    "swift_hardware_test",
    '.appendingPathComponent("crates/connect_norito_bridge/tests/fixtures")',
)
require(
    "swift_hardware_test",
    '.appendingPathComponent("kagemusha_request_authorization_v3_hardware.hex")',
)
for vector_key in (
    "authority_public_key",
    "registration_hash",
    "operation_id",
    "android_preparation",
    "android_signing_preimage",
    "ios_preparation",
    "ios_client_data_hash",
):
    require("swift_hardware_test", f'"{vector_key}"')
canonical_hardware_vector_identifiers = {
    "authority_public_key":
        "a09aa5f47a6759802ff955f8dc2d2a14a5c99d23be97f864127ff9383455a4f0",
    "registration_hash":
        "289ab8f0dcaad32e86ab947b6bd48a3a63385b4d52b85f09f54260ad106d00c3",
}
for vector_key, canonical_value in canonical_hardware_vector_identifiers.items():
    require(
        "hardware_authorization_vector",
        f"{vector_key}={canonical_value}",
    )
    require("swift_hardware_test", f'try hex("{canonical_value}")')
require_regex(
    "swift_hardware_test",
    r'XCTAssertEqual\s*\(\s*try\s+hex\s*\(try\s+XCTUnwrap\s*\('
    r'values\["authority_public_key"\]\s*\)\s*\)\s*,\s*try\s+authorityPublicKey\s*\(\s*\)',
    "canonical authority_public_key fixture binding",
)
require_regex(
    "swift_hardware_test",
    r'XCTAssertEqual\s*\(\s*try\s+hex\s*\(try\s+XCTUnwrap\s*\('
    r'values\["registration_hash"\]\s*\)\s*\)\s*,\s*try\s+registrationHash\s*\(\s*\)',
    "canonical registration_hash fixture binding",
)
require(
    "hardware_authorization_vector",
    "operation_id=f3e599fc0f07c3e748bbc75e7736961a5b289625a269f3494219bcc0ff200cbd",
)
require_regex(
    "swift_hardware_test",
    r'values\["operation_id"\][\s\S]{0,900}?KagemushaOperationIdentityDerivation\.operationID\s*\('
    r'[\s\S]{0,700}?compactNoritoAccountControllerPayload\s*\(\s*\)'
    r'[\s\S]{0,300}?nonce:\s*fixed32\(0x32\)',
    "canonical derived operation_id fixture binding",
)
for needle in (
    "PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY",
    "KeyProperties.KEY_ALGORITHM_EC",
    'CURVE_NAME = "secp256r1"',
    "KeyProperties.PURPOSE_SIGN",
    "KeyProperties.DIGEST_SHA256",
    ".setAttestationChallenge(request.challenge())",
    ".setMaxUsageCount(1)",
    'SIGNATURE_ALGORITHM = "SHA256withECDSA"',
    "StrongBoxPolicy.REQUIRED",
    "builder.setIsStrongBoxBacked(true)",
    "keyInfo.isInsideSecureHardware()",
    "keyInfo.getRemainingUsageCount() != 1",
    "getCertificateChain(request.alias())",
    "DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash",
    "KagemushaP256Codec.rawLowSFromStrictDer(signatureDer)",
):
    require("android_keymint", needle)
require_regex(
    "android_keymint",
    r"generateRegistration\s*\("
    r"[\s\S]{0,900}?RegistrationParameters\s+parameters"
    r"[\s\S]{0,1300}?requiredParameters\.attestationChallenge\(\)"
    r"[\s\S]{0,900}?requiredParameters\.registration\(material\)",
    "registration-derived Android KeyMint pre-key challenge flow",
)
require_regex(
    "android_keymint",
    r"authorize\s*\("
    r"[\s\S]{0,900}?RequestAuthorizationPreparation\s+preparation"
    r"[\s\S]{0,1300}?requiredPreparation\.signingBytes\(\)"
    r"[\s\S]{0,900}?finalizeRequestAuthorization\s*\("
    r"[\s\S]{0,180}?requiredPreparation,\s*signatureDer",
    "physical Android KeyMint signature over the exact authorization preparation",
)
require_regex(
    "android_keymint",
    r"new\s+KeyGenParameterSpec\.Builder\(request\.alias\(\),\s*"
    r"KeyProperties\.PURPOSE_SIGN\)"
    r"[\s\S]{0,500}?new\s+ECGenParameterSpec\(\"secp256r1\"\)"
    r"[\s\S]{0,300}?setDigests\(KeyProperties\.DIGEST_SHA256\)"
    r"[\s\S]{0,300}?setAttestationChallenge\(request\.challenge\(\)\)"
    r"[\s\S]{0,300}?setMaxUsageCount\(1\)",
    "exact sign-only P-256/SHA-256/challenge/single-use KeyMint generation profile",
)
if "KeyProperties.DIGEST_NONE" in texts["android_keymint"]:
    errors.append(
        f"{paths['android_keymint']}: physical KeyMint path must not use DIGEST_NONE"
    )
if "PREFERRED" in texts["android_keymint"]:
    errors.append(
        f"{paths['android_keymint']}: physical KeyMint path must not silently prefer/downgrade StrongBox"
    )

for needle in (
    "PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY",
    "KeyProperties.KEY_ALGORITHM_EC",
    'const val CURVE_NAME: String = "secp256r1"',
    "KeyProperties.PURPOSE_SIGN",
    "KeyProperties.DIGEST_SHA256",
    ".setAttestationChallenge(request.challenge())",
    ".setMaxUsageCount(MAX_USAGE_COUNT)",
    'const val MAX_USAGE_COUNT: Int = 1',
    'const val SIGNATURE_ALGORITHM: String = "SHA256withECDSA"',
    "StrongBoxPolicy.REQUIRED",
    "builder.setIsStrongBoxBacked(true)",
    "keyInfo.isInsideSecureHardware",
    "keyInfo.remainingUsageCount != MAX_USAGE_COUNT",
    "getCertificateChain(request.alias())",
    "DeviceAttestationRegistration.androidPreKeyGenerationChallengeHash",
    "MessageDigest.isEqual(generatedPublicKey, attestedPublicKey)",
    "KagemushaP256Codec.rawLowSFromStrictDer(signatureDer)",
):
    require("kotlin_android_keymint", needle)
for needle in (
    "class KagemushaAndroidKeyMintTest",
    "high-level registration derives and uses the exact pre-key challenge",
    "generates exact single-use P256 profile and signs preparation bytes",
    "StrongBox is explicit required and never downgrades",
):
    require("kotlin_android_keymint_test", needle)
for needle in (
    "class KagemushaAndroidKeyMintInstrumentedTest",
    "PackageManager.FEATURE_KEYSTORE_SINGLE_USE_KEY",
    "generateRegistrationMaterial",
    "keyMint.delete(material)",
):
    require("kotlin_android_keymint_device_test", needle)
if "KeyProperties.DIGEST_NONE" in texts["kotlin_android_keymint"]:
    errors.append(
        f"{paths['kotlin_android_keymint']}: physical KeyMint path must not use DIGEST_NONE"
    )
if "PREFERRED" in texts["kotlin_android_keymint"]:
    errors.append(
        f"{paths['kotlin_android_keymint']}: physical KeyMint path must not silently prefer/downgrade StrongBox"
    )

swift_v4_types = (
    "KagemushaRecursiveSpendArtifactBindingV4",
    "KagemushaRecursiveSpendBundleV4",
    "KagemushaRecursiveSpendTopUpAnchorV4",
    "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
    "KagemushaOutputMembershipLeafPathsV4",
    "KagemushaOutputMembershipPathsV4",
    "KagemushaRecursiveSpendInitRequestV4",
    "KagemushaRecursiveSpendInitLocalRequestV4",
    "KagemushaRecursiveSpendAppendInputV4",
    "KagemushaRecursiveSpendSpendableBranchV4",
    "KagemushaRecursiveSpendAppendLocalRequestV4",
    "KagemushaRecursiveSpendVerifyRequestV4",
    "KagemushaRecursiveSpendVerifyLocalRequestV4",
    "KagemushaRecursiveSpendRedeemLocalRequestV5",
    "KagemushaRecursiveSpendInitResultV4",
    "KagemushaRecursiveSpendSplitResultV4",
    "KagemushaRecursiveSpendVerifyResultV4",
    "KagemushaRecursiveSpendRedeemBuildResultV4",
    "KagemushaRecursiveSpendRedemptionChangePreparationV4",
)
for type_name in swift_v4_types:
    require_regex(
        "swift_v4",
        rf"public\s+struct\s+{re.escape(type_name)}\b",
        f"distinct Swift carrier {type_name}",
    )

for needle in (
    "redemptionChangePrepareRequestWireNameV5",
    "KagemushaRecursiveSpendRedemptionChangePrepareRequestV5",
    "redemptionChangePrepareResultWireNameV4",
    "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
):
    require("swift", needle)
require_regex(
    "swift_v4",
    r"static\s+func\s+prepareRedemptionChangeV5\s*\("
    r"[\s\S]{0,700}?input:\s*KagemushaRecursiveSpendSpendableBranchV4,"
    r"[\s\S]{0,350}?changeAmount:\s*KagemushaScaledAmount,"
    r"[\s\S]{0,350}?recipient:\s*String,"
    r"[\s\S]{0,350}?nonce:\s*Data,"
    r"[\s\S]{0,350}?entropy:\s*Data"
    r"[\s\S]{0,3000}?kagemushaRecursiveSpendRedemptionChangePrepareV5",
    "native-derived Swift redemption-change workflow",
)
require_regex(
    "swift_v4_codecs",
    r"encodeRedemptionChangePrepareRequest\s*\("
    r"[\s\S]{0,1800}?writeField\(uint16\(request\.version\)\)"
    r"[\s\S]{0,700}?request\.bundle\.noritoArchive"
    r"[\s\S]{0,700}?request\.inputOpening"
    r"[\s\S]{0,700}?request\.changeAmount"
    r"[\s\S]{0,700}?request\.recipient"
    r"[\s\S]{0,500}?request\.nonce"
    r"[\s\S]{0,500}?request\.entropy"
    r"[\s\S]{0,500}?redemptionChangePrepareRequestWireNameV5",
    "field-for-field Swift redemption-change request encoder",
)
require_regex(
    "swift_codecs",
    r"decodeRedemptionChangePrepareResultV4\s*\("
    r"[\s\S]{0,3000}?canonical\s*=\s*try\s+"
    r"encodeRedemptionChangePrepareResultV4\(preparation\)"
    r"[\s\S]{0,500}?canonical\s*==\s*archive",
    "strict Swift redemption-change result canonical re-encode",
)
require_regex(
    "swift_native",
    r"kagemushaRecursiveSpendRedemptionChangePrepareV5\s*\("
    r"[\s\S]{0,1800}?connect_norito_kagemusha_secret_free_buffer"
    r"[\s\S]{0,1000}?NativeBridgeError\.fromStatus\(status\)"
    r"[\s\S]{0,500}?secureFree\(output\)"
    r"[\s\S]{0,500}?copyKagemushaNativeSecretArchiveOutput",
    "Swift secret output secure-free on native error and success",
)
if re.search(
    r"kagemushaRecursiveSpendRedemptionChangePrepareV5\s*\("
    r"[\s\S]{0,2800}?connect_norito_free",
    texts["swift_native"],
):
    errors.append(
        f"{paths['swift_native']}: secret redemption-change output must never use connect_norito_free"
    )
if "prepareRedemptionChangeV4" in texts["swift_v4"] or re.search(
    r"KagemushaRecursiveSpendRedemptionChangePrepareRequestV5[\s\S]{0,900}?operationID",
    texts["swift_v4"],
):
    errors.append(
        f"{paths['swift_v4']}: redemption change must accept canonical recipient + nonce, never a caller operation id"
    )
if "redemptionChange(spendKey:" in texts["swift"] or re.search(
    r"redemptionChange[\s\S]{0,300}?defaultDiversifier\(\)",
    texts["swift"],
):
    errors.append(
        f"{paths['swift']}: callers must not fabricate redemption-change rho or diversifier"
    )

for method, native_call, result_type in (
    ("initSpendV4", "kagemushaRecursiveSpendInitV4", "KagemushaRecursiveSpendInitResultV4"),
    ("appendSpendV4", "kagemushaRecursiveSpendAppendV4", "KagemushaRecursiveSpendSplitResultV4"),
    ("verifySpendV4", "kagemushaRecursiveSpendVerifyV4", "KagemushaRecursiveSpendVerifyResultV4"),
    ("buildRedeemV4", "kagemushaRecursiveSpendRedeemV4", "KagemushaRecursiveSpendRedeemBuildResultV4"),
):
    require_regex(
        "swift_v4",
        rf"static\s+func\s+{method}\s*\([\s\S]{{0,1800}}?\.{native_call}\s*\([\s\S]{{0,900}}?{result_type}\s*\(",
        f"direct Swift ABI21 lifecycle {method}",
    )

if texts["swift_v4"].count(
    "catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable"
) != 4:
    errors.append(
        f"{paths['swift_v4']}: all four V4 lifecycle calls must catch the explicit V4-unavailable error"
    )
if "kagemushaRecursiveSpendV2Unavailable" in texts["swift_v4"]:
    errors.append(
        f"{paths['swift_v4']}: V4 lifecycle must not catch the ABI20 unavailable error"
    )
for needle in (
    "case kagemushaRecursiveSpendV4Unavailable",
    "case kagemushaRecursiveSpendV4Artifact",
    "case -316: return .kagemushaRecursiveSpendV4Unavailable",
    "case -317: return .kagemushaRecursiveSpendV4Artifact",
):
    require("swift_bridge", needle)
require_regex(
    "swift",
    r"copyKagemushaV4Output\([\s\S]{0,650}?NativeBridgeError\.fromStatus\(status\)",
    "Swift V4 status mapping through the explicit bridge error table",
)

for encoder, wire_name in (
    ("encodeInitLocalRequest", "initLocalRequestWireNameV4"),
    ("encodeAppendLocalRequest", "appendLocalRequestWireNameV4"),
    ("encodeVerifyLocalRequest", "verifyLocalRequestWireNameV4"),
):
    require_regex(
        "swift_v4_codecs",
        rf"static\s+func\s+{encoder}\s*\([\s\S]{{0,2600}}?localWitnessVersionV4[\s\S]{{0,2600}}?{wire_name}",
        f"field-for-field Swift V4 encoder {encoder}",
    )
require_regex(
    "swift_v4_codecs",
    r"static\s+func\s+encodeRedeemLocalRequest\s*\([\s\S]{0,2600}?"
    r"redeemLocalWitnessVersionV5[\s\S]{0,2600}?redeemLocalRequestWireNameV5",
    "field-for-field Swift V5 redemption carrier encoder",
)

require_regex(
    "swift",
    r"public\s+struct\s+KagemushaRecursiveSpendInstalledArtifactSetV4\b[\s\S]{0,500}?KagemushaRecursiveSpendArtifactBindingV4",
    "Swift V4-only installed artifact set",
)
require_regex(
    "swift_coordinator",
    r"KagemushaRecursiveSpendArtifactBindingV4[\s\S]{0,800}?KagemushaRecursiveSpendInstalledArtifactSetV4",
    "Swift V4-only artifact coordinator",
)

for label in ("rust", "kotlin", "java"):
    for forbidden in ("nativeWrapInitRequestV4", "nativeWrapAppendRequestV4",
                      "nativeWrapVerifyRequestV4", "nativeWrapRedeemRequestV4"):
        if forbidden in texts[label]:
            errors.append(f"{paths[label]}: forbidden ABI20-to-ABI21 wrapper {forbidden!r}")

for label in ("swift", "swift_v4", "swift_v4_codecs", "swift_native"):
    for forbidden in (
        "legacyRequest",
        "encodeInitLocalRequestV4",
        "encodeAppendLocalRequestV4",
        "encodeVerifyLocalRequestV4",
        "encodeRedeemLocalRequestV4",
    ):
        if forbidden in texts[label]:
            errors.append(f"{paths[label]}: forbidden shared V3/V4 alias {forbidden!r}")

if re.search(
    r"static\s+func\s+(?:initSpendV4|appendSpendV4|verifySpendV4|buildRedeemV4)\b"
    r"[\s\S]{0,2600}?(?:decode(?:Init|Split|Verify|Redeem)[A-Za-z]*\(|"
    r"kagemushaRecursiveSpend(?:Init|Append|Verify|Redeem)V[23]\s*\()",
    texts["swift_v4"],
):
    errors.append(
        f"{paths['swift_v4']}: Swift V4 lifecycle must not invoke a V2/V3 native call or decoder"
    )

if re.search(
    r"static\s+func\s+(?:initSpend|appendSpend|verifySpend|buildRedeem)\b"
    r"[\s\S]{0,2600}?kagemushaRecursiveSpend(?:Init|Append|Verify|Redeem)V4\s*\(",
    texts["swift"],
):
    errors.append(
        f"{paths['swift']}: frozen Swift lifecycle must not invoke an ABI21 symbol"
    )

for label in ("kotlin", "java"):
    if "legacyRequest" in texts[label]:
        errors.append(f"{paths[label]}: V4 SDK must not route through a legacy request archive")
    require_regex(
        label,
        r"nativeBuildInitRequestV4\s*\(\s*(?:final\s+)?(?:byte\[\]|anchor\s*:\s*ByteArray)[\s\S]{0,450}?(?:proof|topUpFinalityProof)[\s\S]{0,450}?roster",
        "genuine V4 init builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildAppendRequestV4\s*\([\s\S]{0,350}?bundles[\s\S]{0,350}?openings[\s\S]{0,350}?(?:witnesses|membershipWitnesses)",
        "genuine V4 append builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildVerifyRequestV4\s*\([\s\S]{0,350}?bundle[\s\S]{0,350}?(?:recipientRequest|recipient_request)[\s\S]{0,350}?(?:topUpProvenance|top_up_provenance)",
        "genuine V4 verify builder inputs",
    )
    require_regex(
        label,
        r"nativeBuildRedeemRequestV5\s*\([\s\S]{0,350}?bundle[\s\S]{0,350}?opening[\s\S]{0,350}?(?:membershipWitness|membership_witness)",
        "genuine V4 redeem builder inputs",
    )
    require_regex(
        label,
        r"class\s+BundleV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendBundleV4\"",
        "distinct BundleV4 wrapper",
    )
    require_regex(
        label,
        r"class\s+TopUpAnchorV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendTopUpAnchorV4\"",
        "distinct TopUpAnchorV4 wrapper",
    )
    require_regex(
        label,
        r"class\s+TopUpFinalityEvidenceV4\b[\s\S]{0,500}?\"KagemushaRecursiveSpendTopUpFinalityEvidenceV4\"",
        "distinct TopUpFinalityEvidenceV4 wrapper",
    )
    if re.search(
        r"project(?:Split|Verify|Redeem)[A-Za-z]*V4\s*\([\s\S]{0,1800}?nativeProject[A-Za-z]*V2",
        texts[label],
    ):
        errors.append(f"{paths[label]}: V4 result projection must not invoke a V2 decoder")

require_regex(
    "kotlin",
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*:\s*Int\s*=\s*23\b",
    "exact ABI23 Kotlin constant",
)
require_regex(
    "data_model",
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4:\s*u32\s*=\s*191_862\s*;",
    "exact data-model release proof-pair maximum",
)
require_regex(
    "data_model",
    r"KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4:\s*u32\s*=\s*384\s*\*\s*1024\s*;",
    "exact data-model defensive proof-pair ceiling",
)
require_regex(
    "swift",
    r"releaseMaximumProofPairBytesV4:\s*UInt32\s*=\s*191_862\b",
    "exact Swift release proof-pair maximum",
)
require_regex(
    "swift",
    r"absoluteMaximumProofPairBytesV4:\s*UInt32\s*=\s*384\s*\*\s*1024\b",
    "exact Swift defensive proof-pair ceiling",
)
require_regex(
    "kotlin",
    r"RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4:\s*Int\s*=\s*191_862\b",
    "exact Kotlin release proof-pair maximum",
)
require_regex(
    "kotlin",
    r"ABSOLUTE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4:\s*Int\s*=\s*384\s*\*\s*1024\b",
    "exact Kotlin defensive proof-pair ceiling",
)
if re.search(
    r"const\s+val\s+MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4\b",
    texts["kotlin"],
):
    errors.append(
        f"{paths['kotlin']}: ambiguous recursive proof-pair maximum is forbidden"
    )
require_regex(
    "kotlin",
    r"MAX_TORII_TOP_UP_REQUEST_BYTES_V4\s*:\s*Int\s*=\s*512\s*\*\s*1024\b",
    "exact Kotlin V4 top-up request ceiling",
)
require_regex(
    "kotlin",
    r"MAX_TORII_REDEEM_REQUEST_BYTES_V4\s*:\s*Int\s*=\s*48\s*\*\s*1024\s*\*\s*1024\b",
    "exact Kotlin V4 redeem request ceiling",
)
require_regex(
    "kotlin",
    r"class\s+TopUpRequest\b[\s\S]{0,400}?MAX_TORII_TOP_UP_REQUEST_BYTES_V4",
    "Kotlin top-up request ceiling binding",
)
require_regex(
    "kotlin",
    r"class\s+RedeemSubmissionRequest\b[\s\S]{0,400}?MAX_TORII_REDEEM_REQUEST_BYTES_V4",
    "Kotlin redeem request ceiling binding",
)


def quoted_inventory(label: str, pattern: str, declaration: str) -> tuple[str, ...]:
    match = re.search(pattern, texts[label], re.MULTILINE | re.DOTALL)
    if match is None:
        errors.append(f"{paths[label]}: missing ordered {declaration} inventory")
        return ()
    return tuple(re.findall(r'"([^"]+)"', match.group(1)))


data_roles_match = re.search(
    r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4\s*:\s*\[&str;\s*8\]\s*=\s*\[(.*?)\];",
    texts["data_model"],
    re.MULTILINE | re.DOTALL,
)
if data_roles_match is None:
    errors.append(f"{paths['data_model']}: missing exact [&str; 8] V4 role inventory")
else:
    data_roles = tuple(re.findall(r'"([^"]+)"', data_roles_match.group(1)))
    if data_roles != v4_roles:
        errors.append(f"{paths['data_model']}: V4 roles are not canonical: {data_roles!r}")

artifact_kind_match = re.search(
    r"pub\s+enum\s+KagemushaPastaCycleArtifactKindV4\s*\{(.*?)\n\s*\}",
    texts["data_model"],
    re.MULTILINE | re.DOTALL,
)
if artifact_kind_match is None:
    errors.append(f"{paths['data_model']}: missing V4 artifact-kind enum")
else:
    kind_cases = tuple(re.findall(r"^\s*([A-Z][A-Za-z0-9]*)\s*,", artifact_kind_match.group(1), re.MULTILINE))
    if kind_cases != ("ParamsIpa", "ProvingKey", "VerifyingKey", "BootstrapWitness"):
        errors.append(f"{paths['data_model']}: V4 artifact kinds are not canonical: {kind_cases!r}")

require_regex(
    "rust",
    r"KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_COUNT_V4\s*:\s*usize\s*=\s*8\s*;",
    "exact eight-artifact bridge inventory",
)
require_regex(
    "bundle",
    r"const\s+INPUTS\s*:\s*\[InputSpec;\s*8\]\s*=\s*\[(.*?)\n\s*\];",
    "exact [InputSpec; 8] bundle inventory",
)
bundle_inputs_match = re.search(
    r"const\s+INPUTS\s*:\s*\[InputSpec;\s*8\]\s*=\s*\[(.*?)\n\s*\];",
    texts["bundle"],
    re.MULTILINE | re.DOTALL,
)
if bundle_inputs_match is not None:
    bundle_kinds = tuple(re.findall(r"kind:\s*KagemushaPastaCycleArtifactKindV4::([A-Za-z0-9]+)", bundle_inputs_match.group(1)))
    expected_kinds = (
        "ParamsIpa", "ProvingKey", "VerifyingKey", "BootstrapWitness",
        "ParamsIpa", "ProvingKey", "VerifyingKey", "BootstrapWitness",
    )
    if bundle_kinds != expected_kinds:
        errors.append(f"{paths['bundle']}: V4 bundle kind order is not canonical: {bundle_kinds!r}")

require_regex(
    "kagami",
    r"REPORT_ARTIFACT_PURPOSES_V4\s*:\s*\[&str;\s*8\]",
    "exact Kagami [&str; 8] report inventory",
)
require_regex(
    "kagami",
    r"fn\s+validate_artifacts_sequentially[\s\S]{0,500}?for\s+artifact\s+in\s+artifacts"
    r"[\s\S]{0,200}?drop\(validate\(artifact\)\?\)",
    "sequential Kagami artifact validation with immediate payload drop",
)

swift_roles = quoted_inventory(
    "swift",
    r"public\s+static\s+let\s+artifactRolesV4\s*=\s*\[(.*?)\n\s*\]",
    "artifactRolesV4",
)
if swift_roles != v4_roles:
    errors.append(f"{paths['swift']}: V4 roles are not canonical: {swift_roles!r}")
swift_files = quoted_inventory(
    "swift",
    r"public\s+static\s+let\s+artifactFileNamesV4\s*=\s*\[(.*?)\n\s*\]",
    "artifactFileNamesV4",
)
if swift_files != v4_files:
    errors.append(f"{paths['swift']}: V4 files are not canonical: {swift_files!r}")
kotlin_files = quoted_inventory(
    "kotlin",
    r"V4_ARTIFACT_FILES\s*:\s*List<String>\s*=\s*listOf\((.*?)\n\s*\)",
    "V4_ARTIFACT_FILES",
)
if kotlin_files != v4_files:
    errors.append(f"{paths['kotlin']}: V4 files are not canonical: {kotlin_files!r}")
java_files = quoted_inventory(
    "java",
    r"V4_ARTIFACT_FILES\s*=\s*Collections\.unmodifiableList\(Arrays\.asList\((.*?)\)\s*\);",
    "V4_ARTIFACT_FILES",
)
if java_files != v4_files:
    errors.append(f"{paths['java']}: V4 files are not canonical: {java_files!r}")

swift_role_enum = re.search(
    r"public\s+enum\s+KagemushaRecursiveSpendArtifactRoleV4\b(.*?)public\s+var\s+fileName",
    texts["swift"],
    re.MULTILINE | re.DOTALL,
)
if swift_role_enum is None:
    errors.append(f"{paths['swift']}: missing V4 artifact-role enum")
else:
    swift_cases = tuple(re.findall(r"\bcase\s+([A-Za-z][A-Za-z0-9]*)", swift_role_enum.group(1)))
    if swift_cases != v4_case_names:
        errors.append(f"{paths['swift']}: V4 enum order is not canonical: {swift_cases!r}")

for label, pattern in (
    ("kotlin", r"enum\s+class\s+ArtifactRoleV4\b.*?\{(.*?)\n\s*\}"),
    ("java", r"enum\s+ArtifactRoleV4\b.*?\{(.*?)\n\s*\}"),
):
    enum_match = re.search(pattern, texts[label], re.MULTILINE | re.DOTALL)
    if enum_match is None:
        errors.append(f"{paths[label]}: missing V4 artifact-role enum")
        continue
    enum_files = tuple(re.findall(r'\b[A-Z][A-Z0-9_]*\s*\(\s*"([^"]+)"\s*\)', enum_match.group(1)))
    if enum_files != v4_files:
        errors.append(f"{paths[label]}: V4 enum files are not canonical: {enum_files!r}")

release_pairs = tuple(re.findall(
    r'"role":\s*"(step_(?:eq|ep)_[^"]+)"[\s\S]{0,220}?"file_name":\s*"([^"]+)"',
    texts["xcframework_build"],
))
if release_pairs != tuple(zip(v4_roles, v4_files)):
    errors.append(
        f"{paths['xcframework_build']}: ABI21 release artifact metadata is not canonical: "
        f"{release_pairs!r}"
    )

for label in (
    "data_model", "rust", "header", "swift", "swift_v4", "swift_v4_codecs",
    "swift_coordinator", "kotlin", "java", "kagami", "bundle", "core_artifact",
    "xcframework_build",
):
    # Inline profile parameters and their authenticated header digests are
    # mandatory. Only a ninth/tenth standalone role or file is forbidden.
    if (
        "circuit-params.krv4" in texts[label]
        or re.search(r"\bCIRCUIT_PARAMS_FILE_NAME_V4\b", texts[label])
        or "KagemushaPastaCycleArtifactKindV4::CircuitParams" in texts[label]
        or re.search(r'\bCircuitParams\s*,', texts[label])
        or re.search(r'["\']step_(?:eq|ep)_circuit_params["\']', texts[label])
    ):
        errors.append(f"{paths[label]}: separate V4 CircuitParams artifact path is forbidden")
    if re.search(
        r"(?:exact(?:ly)?\s+(?:ten|10)|ten[- ](?:artifact|file)|all\s+ten|"
        r"\[&str;\s*10\]|\[InputSpec;\s*10\]|ARTIFACT_COUNT[^\n=]*=\s*10\b)",
        texts[label],
        re.IGNORECASE,
    ):
        errors.append(f"{paths[label]}: exact-ten V4 inventory language is forbidden")

# The exact-eight external inventory does not weaken circuit-profile binding:
# parameters remain inline in the authenticated manifest and every artifact
# header carries their domain-separated digest.
for required in (
    "KagemushaStepCircuitParamsV4",
    "circuit_params: KagemushaStepCircuitParamsV4",
    "circuit_params_sha256",
    "step_eq_circuit_params_sha256",
    "step_ep_circuit_params_sha256",
):
    require("data_model", required)
for label in ("core_artifact", "bundle"):
    require(label, "circuit_params_sha256")
require("bundle", "KagemushaStepCircuitParamsV4")
require("core_artifact", "profile.circuit_params")
require("core_artifact", ".sha256()")
require("header", "signed manifest profile")
require("header", "domain-separated digest")

require_regex(
    "header",
    r"exact\s+eight-artifact\s+KRV4\s+inventory",
    "exact-eight ABI21 inventory documentation",
)
require_regex(
    "java",
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*=\s*23\s*;",
    "exact ABI23 Java constant",
)
require_regex(
    "java",
    r"RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4\s*=\s*191_862\s*;",
    "exact Java release proof-pair maximum",
)
require_regex(
    "java",
    r"ABSOLUTE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4\s*=\s*384\s*\*\s*1024\s*;",
    "exact Java defensive proof-pair ceiling",
)
if re.search(
    r"static\s+final\s+int\s+MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4\b",
    texts["java"],
):
    errors.append(
        f"{paths['java']}: ambiguous recursive proof-pair maximum is forbidden"
    )
require_regex(
    "java",
    r"MAX_TORII_TOP_UP_REQUEST_BYTES_V4\s*=\s*512\s*\*\s*1024\s*;",
    "exact Java V4 top-up request ceiling",
)
require_regex(
    "java",
    r"MAX_TORII_REDEEM_REQUEST_BYTES_V4\s*=\s*48\s*\*\s*1024\s*\*\s*1024\s*;",
    "exact Java V4 redeem request ceiling",
)
require_regex(
    "java",
    r"class\s+TopUpRequest\b[\s\S]{0,500}?MAX_TORII_TOP_UP_REQUEST_BYTES_V4",
    "Java top-up request ceiling binding",
)
require_regex(
    "java",
    r"class\s+RedeemSubmissionRequest\b[\s\S]{0,500}?MAX_TORII_REDEEM_REQUEST_BYTES_V4",
    "Java redeem request ceiling binding",
)
require_regex(
    "kotlin",
    r"V4_ARTIFACT_COUNT\s*:\s*Int\s*=\s*8\b",
    "exact eight-artifact Kotlin inventory",
)
require_regex(
    "java",
    r"V4_ARTIFACT_COUNT\s*=\s*8\s*;",
    "exact eight-artifact Java inventory",
)

for method in native_methods:
    require_regex(
        "kotlin",
        rf"external\s+fun\s+{re.escape(method)}\s*\(",
        f"Kotlin native declaration {method}",
    )
    require_regex(
        "java",
        rf"native\s+[^;\n]+\s+{re.escape(method)}\s*\(",
        f"Java native declaration {method}",
    )
    for package in (
        "org_hyperledger_iroha_sdk_offline",
        "org_hyperledger_iroha_android_offline",
    ):
        if method not in jni_forwarder_methods:
            require_regex(
                "rust",
                rf"fn\s+Java_{package}_KagemushaRecursiveSpendProver_{re.escape(method)}\s*\(",
                f"Rust JNI export {package}.{method}",
            )

privacy_compiled_profile_symbols = (
    "iroha_privacy_compiled_profile_catalog_v1",
    "iroha_privacy_validate_compiled_profile_catalog_v1",
    "iroha_privacy_exact12_fixture_bundle_v1",
    "iroha_privacy_validate_exact12_fixture_bundle_v1",
    "iroha_privacy_free_buffer",
)
parliament_timed_ovn_symbols = (
    "connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1",
    "connect_norito_parliament_timed_ovn_verify_casting_proof_v1",
    "connect_norito_parliament_timed_ovn_registration_from_proof_v1",
    "connect_norito_parliament_timed_ovn_ballot_from_proof_v1",
)
base_bridge_symbols = (
    "connect_norito_bridge_abi_version",
    "connect_norito_free",
    "connect_norito_chain_discriminant_scope_enter",
    "connect_norito_chain_discriminant_scope_exit",
    "connect_norito_encode_transfer_signed_transaction",
    "connect_norito_encode_transfer_instruction_box",
    "connect_norito_detached_transaction_scaffold_inspect_v1",
    "connect_norito_detached_transaction_scaffold_finalize_ed25519_v1",
    "connect_norito_canonical_json_blake3_v1",
    "connect_norito_encode_account_onboarding_plan_body_v1",
    "connect_norito_alias_instruction_round_trip_v1",
    *parliament_timed_ovn_symbols,
    *privacy_compiled_profile_symbols,
    "connect_norito_sorafs_reference_validate_bundle_json",
    "connect_norito_sorafs_reference_validate_governance_json",
    "connect_norito_sorafs_reference_validate_governance_dag_block_json",
    "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
    "connect_norito_validation_fee_current_policy_proof_request_v1",
    "connect_norito_validation_fee_current_policy_proof_verify_v1",
    "connect_norito_validation_fee_hijiri_quote_request_v1",
    "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
    "connect_norito_private_settlement_committee_proof_response_verify_v1",
    "connect_norito_private_settlement_auditor_capsule_response_verify_v1",
    "connect_norito_private_settlement_audit_approval_response_verify_v1",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
)
c_symbols = (
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
    "connect_norito_kagemusha_native_contract_revision",
    "connect_norito_kagemusha_offline_operation_status_validate_v2",
    "connect_norito_kagemusha_offline_operation_status_json_validate_v2",
    "connect_norito_kagemusha_topup_finality_verify_v4",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    "connect_norito_kagemusha_output_membership_frontier_build_v4",
    "connect_norito_kagemusha_output_membership_paths_derive_v4",
    "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
    "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_topup_v4",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v5",
    "connect_norito_kagemusha_secret_free_buffer",
    "connect_norito_kagemusha_receiver_key_reference_v2",
    "connect_norito_kagemusha_recipient_output_derive_v2",
    "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
    "connect_norito_kagemusha_recipient_payment_request_create_v2",
    "connect_norito_kagemusha_recipient_payment_request_verify_v2",
    "connect_norito_kagemusha_recipient_lineage_query_create_v2",
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
    "connect_norito_kagemusha_recipient_receive_offer_create_v2",
    "connect_norito_kagemusha_recipient_receive_offer_project_v2",
    "connect_norito_kagemusha_recipient_receive_offer_verify_v2",
    "connect_norito_kagemusha_request_authorization_signing_bytes_v3",
    "connect_norito_kagemusha_request_authorization_finalize_hardware_v3",
    "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v3",
    "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
    "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
    "connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v4",
)
required_bridge_symbols = base_bridge_symbols + c_symbols


def parse_shell_symbol_array(label: str, name: str) -> tuple[str, ...]:
    assignment_matches = tuple(
        re.finditer(
            rf"(?<![A-Za-z0-9_]){re.escape(name)}"
            r"(?:\[[^\]\n]+\])?[ \t]*(?:\+?=)",
            texts[label],
        )
    )
    matches = tuple(
        re.finditer(
            rf"^{re.escape(name)}=\(\s*\n(?P<body>.*?)^\)",
            texts[label],
            re.MULTILINE | re.DOTALL,
        )
    )
    if len(assignment_matches) != 1 or len(matches) != 1:
        errors.append(
            f"{paths[label]}: shell array {name} must have exactly one "
            f"canonical assignment (found {len(assignment_matches)} assignments "
            f"and {len(matches)} canonical blocks)"
        )
        return ()
    match = matches[0]
    values: list[str] = []
    for line_number, raw_line in enumerate(match.group("body").splitlines(), start=1):
        try:
            tokens = shlex.split(raw_line, comments=True, posix=True)
        except ValueError as error:
            errors.append(
                f"{paths[label]}: shell array {name} line {line_number} "
                f"is not canonical: {error}"
            )
            return ()
        if not tokens:
            continue
        if len(tokens) != 1:
            errors.append(
                f"{paths[label]}: shell array {name} line {line_number} "
                "must contain exactly one symbol or array expansion"
            )
            return ()
        values.append(tokens[0])
    return tuple(values)


def parse_manifest_symbol_inventory(label: str) -> tuple[str, ...]:
    matches = tuple(
        re.finditer(
            r'"required_symbols"\s*:\s*\[(?P<body>.*?)\]',
            texts[label],
            re.MULTILINE | re.DOTALL,
        )
    )
    if len(matches) != 1:
        errors.append(
            f"{paths[label]}: required_symbols manifest inventory must occur exactly once "
            f"(found {len(matches)})"
        )
        return ()
    try:
        values = json.loads(f"[{matches[0].group('body')}]")
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        errors.append(
            f"{paths[label]}: required_symbols manifest inventory is not strict JSON: "
            f"{error}"
        )
        return ()
    if not isinstance(values, list) or any(
        not isinstance(value, str) or not value for value in values
    ):
        errors.append(
            f"{paths[label]}: required_symbols manifest inventory must contain only "
            "non-empty JSON strings"
        )
        return ()
    return tuple(values)


actual_kagemusha_symbols = parse_shell_symbol_array("mobile_check", "KAGEMUSHA_C_SYMBOLS")
if actual_kagemusha_symbols != c_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered {len(c_symbols)}-symbol Kagemusha C inventory mismatch "
        f"(found {len(actual_kagemusha_symbols)})"
    )

actual_appeal_finance_symbols = parse_shell_symbol_array(
    "mobile_check", "SORAFS_APPEAL_FINANCE_C_SYMBOLS"
)
expected_appeal_finance_symbols = (
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
)
if actual_appeal_finance_symbols != expected_appeal_finance_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered appeal-finance C inventory mismatch "
        f"(found {len(actual_appeal_finance_symbols)})"
    )

actual_privacy_compiled_profile_symbols = parse_shell_symbol_array(
    "mobile_check", "PRIVACY_COMPILED_PROFILE_C_SYMBOLS"
)
if actual_privacy_compiled_profile_symbols != privacy_compiled_profile_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered "
        f"{len(privacy_compiled_profile_symbols)}-symbol privacy compiled-profile "
        "C inventory mismatch "
        f"(found {len(actual_privacy_compiled_profile_symbols)})"
    )

actual_parliament_timed_ovn_symbols = parse_shell_symbol_array(
    "mobile_check", "PARLIAMENT_TIMED_OVN_C_SYMBOLS"
)
if actual_parliament_timed_ovn_symbols != parliament_timed_ovn_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered "
        f"{len(parliament_timed_ovn_symbols)}-symbol Parliament timed-OVN "
        "C inventory mismatch "
        f"(found {len(actual_parliament_timed_ovn_symbols)})"
    )

actual_required_bridge_symbols: list[str] = []
for value in parse_shell_symbol_array("mobile_check", "REQUIRED_BRIDGE_SYMBOLS"):
    if value == "${KAGEMUSHA_C_SYMBOLS[@]}":
        actual_required_bridge_symbols.extend(actual_kagemusha_symbols)
    elif value == "${PARLIAMENT_TIMED_OVN_C_SYMBOLS[@]}":
        actual_required_bridge_symbols.extend(actual_parliament_timed_ovn_symbols)
    elif value == "${SORAFS_APPEAL_FINANCE_C_SYMBOLS[@]}":
        actual_required_bridge_symbols.extend(actual_appeal_finance_symbols)
    elif value == "${PRIVACY_COMPILED_PROFILE_C_SYMBOLS[@]}":
        actual_required_bridge_symbols.extend(actual_privacy_compiled_profile_symbols)
    else:
        actual_required_bridge_symbols.append(value)
if tuple(actual_required_bridge_symbols) != required_bridge_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered {len(required_bridge_symbols)}-symbol "
        "required bridge inventory mismatch "
        f"(found {len(actual_required_bridge_symbols)})"
    )

for label in ("xcframework_build", "mobile_check_test"):
    actual_manifest_symbols = parse_manifest_symbol_inventory(label)
    if actual_manifest_symbols != required_bridge_symbols:
        errors.append(
            f"{paths[label]}: exact ordered {len(required_bridge_symbols)}-symbol "
            "required bridge inventory mismatch "
            f"(found {len(actual_manifest_symbols)})"
        )

require_regex(
    "rust",
    r"impl\s+iroha_core::zk::kagemusha_artifact_source_v4::"
    r"KagemushaAuthenticatedArtifactSourceV4[\s\S]{0,800}?"
    r"fn\s+with_framed_artifact[\s\S]{0,800}?self\.with_selected_file",
    "authenticated-release V4 pinned artifact source",
)
require_regex(
    "rust",
    r"qualify_kagemusha_authenticated_artifact_source_v4\s*\(\s*qualification_source\s*\)",
    "complete authenticated-release V4 source qualification",
)
require_regex(
    "rust",
    r"KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source\s*\("
    r"[\s\S]{0,120}?Arc::clone\(&self\.qualified_source\)",
    "qualified source-backed V4 verifier construction",
)
require_regex(
    "rust",
    r"KagemushaPastaCycleOpaqueProverV4::from_qualified_artifact_source\s*\("
    r"[\s\S]{0,120}?Arc::clone\(&self\.qualified_source\)",
    "qualified source-backed V4 prover construction",
)
if "KagemushaPastaCycleProverArtifactsV4::new" in texts["rust"]:
    errors.append(f"{paths['rust']}: in-memory all-role V4 prover construction is forbidden")

for symbol in c_symbols:
    if symbol not in lifecycle_macro_exports:
        require_regex("rust", rf"fn\s+{re.escape(symbol)}\s*\(", f"Rust C export {symbol}")
    require_regex("header", rf"\b{re.escape(symbol)}\s*\(", f"C header declaration {symbol}")
    for label in ("xcframework_build", "mobile_check", "mobile_check_test"):
        require(label, symbol)

# First-release ABI-21 does not publish compatibility aliases for recursive
# lifecycle or artifact installation. Shared V2 leaf primitives above remain
# intentionally legal because V4 reuses their unchanged wire types.
forbidden_recursive_alias = re.compile(
    r"\bconnect_norito_kagemusha_recursive_spend_"
    r"(?:init|append|verify|redeem|topup|peer_payment_from_split|"
    r"peer_payment_validate|bundle_summary|build_split_intent|artifact_[a-z_]+)_v[23]\b"
)
if forbidden_recursive_alias.search(texts["header"]):
    errors.append(f"{paths['header']}: published V2/V3 recursive lifecycle alias is forbidden")
if re.search(
    r"pub\s+(?:unsafe\s+)?extern\s+\"C\"\s+fn\s+" + forbidden_recursive_alias.pattern,
    texts["rust"],
):
    errors.append(f"{paths['rust']}: exported V2/V3 recursive lifecycle alias is forbidden")
swift_symbol_inventory = "\n".join(
    match.group(1)
    for match in re.finditer(
        r"required(?:Proof|Protocol)Symbols\s*=\s*\[(.*?)\n\s*\]",
        texts["swift"],
        re.MULTILINE | re.DOTALL,
    )
)
if forbidden_recursive_alias.search(swift_symbol_inventory):
    errors.append(f"{paths['swift']}: required-symbol inventory contains a V2/V3 lifecycle alias")

# The first release has only the query-backed lineage verifier and the two
# physical authorization finalizers. The fail-only lineage verifier, generic
# authorization creator, and their JNI creator shims must not reappear in any
# published source inventory.
forbidden_first_release_c_exports = (
    "connect_norito_kagemusha_recipient_registration_lineage_verify_v1",
    "connect_norito_kagemusha_request_authorization_create_v2",
)
for symbol in forbidden_first_release_c_exports:
    if re.search(rf"\b{re.escape(symbol)}\s*\(", texts["header"]):
        errors.append(f"{paths['header']}: first-release compatibility C export {symbol} is forbidden")
    if re.search(
        rf'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+{re.escape(symbol)}\s*\(',
        texts["rust"],
    ):
        errors.append(f"{paths['rust']}: first-release compatibility C export {symbol} is forbidden")
    if symbol in swift_symbol_inventory or symbol in texts["swift_native"]:
        errors.append(f"{paths['swift']}: first-release compatibility C export {symbol} is forbidden")
    if symbol in actual_kagemusha_symbols:
        errors.append(f"{paths['mobile_check']}: first-release compatibility C export {symbol} is forbidden")

for label in ("kotlin", "java"):
    if re.search(r"\bnativeCreateAuthorizationV2\s*\(", texts[label]):
        errors.append(
            f"{paths[label]}: first-release JNI compatibility method "
            "nativeCreateAuthorizationV2 is forbidden"
        )
for package in (
    "org_hyperledger_iroha_sdk_offline",
    "org_hyperledger_iroha_android_offline",
):
    forbidden_jni_symbol = (
        f"Java_{package}_KagemushaRecursiveSpendProver_nativeCreateAuthorizationV2"
    )
    if re.search(rf"\bfn\s+{re.escape(forbidden_jni_symbol)}\s*\(", texts["rust"]):
        errors.append(
            f"{paths['rust']}: first-release JNI compatibility export "
            f"{forbidden_jni_symbol} is forbidden"
        )

for macro in (
    "CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_UNAVAILABLE",
    "CONNECT_NORITO_ERR_KAGEMUSHA_RECURSIVE_SPEND_V4_ARTIFACT",
):
    require("header", macro)

if errors:
    print("Kagemusha ABI21 SDK contract failed:", file=sys.stderr)
    for error in errors:
        print(f" - {error}", file=sys.stderr)
    raise SystemExit(1)

if mode == "--self-test":
    def replace_once(path: Path, old: str, new: str) -> None:
        value = path.read_text(encoding="utf-8")
        if value.count(old) != 1:
            raise SystemExit(
                f"self-test fixture expected exactly one {old!r} in {path}, "
                f"found {value.count(old)}"
            )
        path.write_text(value.replace(old, new, 1), encoding="utf-8")

    def run_negative(name: str, mutate, expected: str) -> None:
        with tempfile.TemporaryDirectory(prefix="kagemusha-v4-sdk-guard-") as temporary:
            fixture = Path(temporary)
            for relative in paths.values():
                destination = fixture / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(root / relative, destination)
            mutate(fixture)
            environment = os.environ.copy()
            environment["KAGEMUSHA_RECURSIVE_SPEND_V4_SDK_ROOT"] = str(fixture)
            result = subprocess.run(
                ["bash", str(script)],
                cwd=fixture,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )
            if result.returncode == 0 or expected not in result.stdout:
                raise SystemExit(
                    f"self-test {name!r} did not fail for {expected!r}:\n{result.stdout}"
                )

    run_negative(
        "reviewed data-model component cannot detach",
        lambda fixture: replace_once(
            fixture / paths["data_model"],
            'include!("kagemusha_model.rs");',
            "// reviewed Kagemusha model component detached",
        ),
        "expected exactly one reviewed kagemusha_model.rs include",
    )

    run_negative(
        "reviewed release-verifier component cannot detach",
        lambda fixture: replace_once(
            fixture / paths["data_model_verifier"],
            "const VERIFIER_IDENTITY_SCHEMA_V4",
            "const DETACHED_VERIFIER_IDENTITY_SCHEMA_V4",
        ),
        "expected exactly one 'const VERIFIER_IDENTITY_SCHEMA_V4'",
    )

    run_negative(
        "reviewed JNI fragment closure cannot detach",
        lambda fixture: replace_once(
            fixture / paths["rust_platform_jni"],
            'include!("platform_jni/private_settlement.rs");',
            "// reviewed JNI fragment detached",
        ),
        "expected the reviewed JNI include closure",
    )

    run_negative(
        "standalone circuit-parameter artifact cannot enter the inventory",
        lambda fixture: replace_once(
            fixture / paths["kagami"],
            "const RELEASE_STEP_EQ_CIRCUIT_PARAMS_FILE_NAME_V4: &str = "
            '"step-eq-circuit-params.norito";',
            "const CIRCUIT_PARAMS_FILE_NAME_V4: &str = "
            '"step-eq-circuit-params.norito";',
        ),
        "separate V4 CircuitParams artifact path is forbidden",
    )

    run_negative(
        "ABI23 cannot regress to ABI21",
        lambda fixture: replace_once(
            fixture / paths["kotlin"],
            "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 23",
            "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 21",
        ),
        "exact ABI23 Kotlin constant",
    )
    run_negative(
        "data-model release proof-pair maximum cannot drift",
        lambda fixture: replace_once(
            fixture / paths["data_model"],
            "KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4: u32 = 191_862;",
            "KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4: u32 = 191_863;",
        ),
        "exact data-model release proof-pair maximum",
    )
    run_negative(
        "Swift release proof-pair maximum cannot drift",
        lambda fixture: replace_once(
            fixture / paths["swift"],
            "releaseMaximumProofPairBytesV4: UInt32 = 191_862",
            "releaseMaximumProofPairBytesV4: UInt32 = 191_863",
        ),
        "exact Swift release proof-pair maximum",
    )
    run_negative(
        "Kotlin release proof-pair maximum cannot drift",
        lambda fixture: replace_once(
            fixture / paths["kotlin"],
            "RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4: Int = 191_862",
            "RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4: Int = 191_863",
        ),
        "exact Kotlin release proof-pair maximum",
    )
    run_negative(
        "Java release proof-pair maximum cannot drift",
        lambda fixture: replace_once(
            fixture / paths["java"],
            "RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4 = 191_862;",
            "RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4 = 191_863;",
        ),
        "exact Java release proof-pair maximum",
    )
    run_negative(
        "promotion record cannot be removed from SDK authentication",
        lambda fixture: replace_once(
            fixture / paths["kotlin"],
            "        cryptographicReview: ByteArray,\n"
            "        promotionRecordNorito: ByteArray,\n"
            "    ) {",
            "        cryptographicReview: ByteArray,\n"
            "    ) {",
        ),
        "mandatory promotion record constructor input",
    )
    run_negative(
        "exact-eight inventory rejects substitution",
        lambda fixture: replace_once(
            fixture / paths["swift"],
            '"step-eq.params-ipa.krv4",',
            '"step-eq.proving-key.krv4",',
        ),
        "V4 files are not canonical",
    )
    run_negative(
        "App Attest cannot sign a substituted client-data hash",
        lambda fixture: replace_once(
            fixture / paths["swift"],
            "                clientDataHash: signingBytes\n",
            "                clientDataHash: Data(repeating: 0, count: 32)\n",
        ),
        "physical App Attest assertion over the exact authorization preparation",
    )
    run_negative(
        "Swift hardware parity cannot leave the shared fixture",
        lambda fixture: replace_once(
            fixture / paths["swift_hardware_test"],
            '.appendingPathComponent("kagemusha_request_authorization_v3_hardware.hex")',
            '.appendingPathComponent("dummy_hardware_authorization.hex")',
        ),
        "kagemusha_request_authorization_v3_hardware.hex",
    )
    run_negative(
        "Swift peer-payment parity cannot leave the shared fixture",
        lambda fixture: replace_once(
            fixture / paths["swift_peer_fixtures"],
            'rustFixtureData("offline_peer_payment_v4.hex")',
            'rustFixtureData("dummy_peer_payment_v4.hex")',
        ),
        "offline_peer_payment_v4.hex",
    )

    def corrupt_peer_payment_vector(fixture: Path) -> None:
        vector = fixture / paths["peer_payment_vector"]
        encoded = vector.read_text(encoding="utf-8")
        vector.write_text("00" + encoded[2:], encoding="utf-8")

    run_negative(
        "shared peer-payment bytes are digest-bound",
        corrupt_peer_payment_vector,
        "canonical SHA-256 mismatch",
    )
    run_negative(
        "peer-payment fixture requires a four-validator committee and three-signer commit quorum",
        lambda fixture: replace_once(
            fixture / paths["peer_payment_generator"],
            "signers: vec![0, 1, 2]",
            "signers: vec![0, 1]",
        ),
        "missing 'signers: vec![0, 1, 2]'",
    )
    run_negative(
        "peer-payment fixture cannot shrink the validator committee",
        lambda fixture: replace_once(
            fixture / paths["peer_payment_generator"],
            "let mut validator_keys = (0_u8..4)",
            "let mut validator_keys = (0_u8..3)",
        ),
        "ordered four-validator unit-power roster with matching BLS PoPs",
    )
    run_negative(
        "peer-payment fixture cannot detach roster ordering",
        lambda fixture: replace_once(
            fixture / paths["peer_payment_generator"],
            "validator_keys.sort_unstable_by_key",
            "validator_keys.reverse",
        ),
        "ordered four-validator unit-power roster with matching BLS PoPs",
    )
    run_negative(
        "peer-payment fixture context must reuse the qualified roster",
        lambda fixture: replace_once(
            fixture / paths["peer_payment_generator"],
            "roster: validator_set.to_vec(),",
            "roster: Vec::new(),",
        ),
        "one validated four-validator context reused by the three-signer Commit QC",
    )
    run_negative(
        "Swift hardware parity cannot substitute the canonical authority key",
        lambda fixture: replace_once(
            fixture / paths["swift_hardware_test"],
            'values["authority_public_key"]',
            'values["dummy_authority_public_key"]',
        ),
        "canonical authority_public_key fixture binding",
    )
    run_negative(
        "Android KeyMint cannot relax the hardware usage count",
        lambda fixture: replace_once(
            fixture / paths["android_keymint"],
            ".setMaxUsageCount(1);",
            ".setMaxUsageCount(2);",
        ),
        "exact sign-only P-256/SHA-256/challenge/single-use KeyMint generation profile",
    )
    run_negative(
        "Kotlin Android KeyMint cannot relax the hardware usage count",
        lambda fixture: replace_once(
            fixture / paths["kotlin_android_keymint"],
            ".setMaxUsageCount(MAX_USAGE_COUNT)",
            ".setMaxUsageCount(2)",
        ),
        "missing '.setMaxUsageCount(MAX_USAGE_COUNT)'",
    )
    run_negative(
        "required bridge symbol order drift is rejected",
        lambda fixture: replace_once(
            fixture / paths["xcframework_build"],
            '    "connect_norito_chain_discriminant_scope_enter",\n'
            '    "connect_norito_chain_discriminant_scope_exit",',
            '    "connect_norito_chain_discriminant_scope_exit",\n'
            '    "connect_norito_chain_discriminant_scope_enter",',
        ),
        f"exact ordered {len(required_bridge_symbols)}-symbol required bridge inventory mismatch",
    )
    run_negative(
        "privacy bridge symbol order drift is rejected",
        lambda fixture: replace_once(
            fixture / paths["mobile_check"],
            "  iroha_privacy_compiled_profile_catalog_v1\n"
            "  iroha_privacy_validate_compiled_profile_catalog_v1",
            "  iroha_privacy_validate_compiled_profile_catalog_v1\n"
            "  iroha_privacy_compiled_profile_catalog_v1",
        ),
        "exact ordered 5-symbol privacy compiled-profile C inventory mismatch",
    )

    def append_duplicate_kagemusha_shell_inventory(fixture: Path) -> None:
        mobile_check = fixture / paths["mobile_check"]
        mobile_check.write_text(
            mobile_check.read_text(encoding="utf-8")
            + "\nKAGEMUSHA_C_SYMBOLS=(\n  connect_norito_free\n)\n",
            encoding="utf-8",
        )

    run_negative(
        "later shell inventory override cannot hide behind a canonical decoy",
        append_duplicate_kagemusha_shell_inventory,
        "shell array KAGEMUSHA_C_SYMBOLS must have exactly one canonical assignment",
    )

    run_negative(
        "unrecognized manifest symbol is not filtered before comparison",
        lambda fixture: replace_once(
            fixture / paths["xcframework_build"],
            '    "connect_norito_bridge_abi_version",\n',
            '    "connect_norito_bridge_abi_version",\n'
            '    "unexpected_mobile_bridge_symbol",\n',
        ),
        f"exact ordered {len(required_bridge_symbols)}-symbol required bridge inventory mismatch",
    )

    def append_duplicate_manifest_inventory(fixture: Path) -> None:
        build = fixture / paths["xcframework_build"]
        build.write_text(
            build.read_text(encoding="utf-8")
            + "\n: <<'DUPLICATE_REQUIRED_SYMBOLS'\n"
            + '{"required_symbols": []}\n'
            + "DUPLICATE_REQUIRED_SYMBOLS\n",
            encoding="utf-8",
        )

    run_negative(
        "manifest inventory decoy cannot hide a second block",
        append_duplicate_manifest_inventory,
        "required_symbols manifest inventory must occur exactly once",
    )

    run_negative(
        "privacy bridge manifest omission is rejected",
        lambda fixture: replace_once(
            fixture / paths["xcframework_build"],
            '    "iroha_privacy_exact12_fixture_bundle_v1",\n',
            "",
        ),
        f"exact ordered {len(required_bridge_symbols)}-symbol required bridge inventory mismatch",
    )

    def inject_lineage_v1_compatibility_export(fixture: Path) -> None:
        header = fixture / paths["header"]
        header.write_text(
            header.read_text(encoding="utf-8")
            + "\nint32_t "
            "connect_norito_kagemusha_recipient_registration_lineage_verify_v1(void);\n",
            encoding="utf-8",
        )

    run_negative(
        "fail-only lineage V1 compatibility export is rejected",
        inject_lineage_v1_compatibility_export,
        "first-release compatibility C export "
        "connect_norito_kagemusha_recipient_registration_lineage_verify_v1 is forbidden",
    )

    def inject_jni_authorization_creator(fixture: Path) -> None:
        kotlin = fixture / paths["kotlin"]
        kotlin.write_text(
            kotlin.read_text(encoding="utf-8")
            + "\nexternal fun nativeCreateAuthorizationV2()\n",
            encoding="utf-8",
        )

    run_negative(
        "generic JNI authorization creator is rejected",
        inject_jni_authorization_creator,
        "first-release JNI compatibility method nativeCreateAuthorizationV2 is forbidden",
    )

    def inject_v3_alias(fixture: Path) -> None:
        header = fixture / paths["header"]
        header.write_text(
            header.read_text(encoding="utf-8")
            + "\nint32_t connect_norito_kagemusha_recursive_spend_init_v3(void);\n",
            encoding="utf-8",
        )

    run_negative(
        "V3 lifecycle alias is rejected",
        inject_v3_alias,
        "published V2/V3 recursive lifecycle alias is forbidden",
    )

print(
    "Kagemusha ABI21 SDK contract passed: exact8 DM/Kagami/bundle inventory, "
    "digest-bound offline peer-payment fixture, and promotion-record-bound direct "
    "C/JNI/Swift/Kotlin/Java lifecycle parity are complete"
    + ("; negative self-tests passed." if mode == "--self-test" else ".")
)
PY
