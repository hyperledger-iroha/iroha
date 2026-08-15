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
import os
import re
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
    "data_model_fragment": Path(
        "crates/iroha_data_model/src/offline/kagemusha_model.rs"
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
        "kagemusha_request_authorization_v2_hardware.hex"
    ),
    "recipient_request_vector": Path(
        "crates/connect_norito_bridge/tests/fixtures/"
        "offline_recipient_payment_request_v2.hex"
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
    "xcframework_build": Path("scripts/build_norito_xcframework.sh"),
    "mobile_check": Path("scripts/check_mobile_sdk_artifacts.sh"),
    "mobile_check_test": Path("scripts/check_mobile_sdk_artifacts_test.sh"),
}

texts: dict[str, str] = {}
for label, relative in paths.items():
    absolute = root / relative
    if not absolute.is_file():
        raise SystemExit(f"required ABI21 contract file is missing: {relative}")
    texts[label] = absolute.read_text(encoding="utf-8")

data_model_include = 'include!("kagemusha_model.rs");'
if texts["data_model"].count(data_model_include) != 1:
    raise SystemExit(
        f"{paths['data_model']}: expected exactly one reviewed "
        f"{paths['data_model_fragment'].name} include"
    )
texts["data_model"] = texts["data_model"].replace(
    data_model_include,
    texts["data_model_fragment"],
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
expected_platform_jni_includes = tuple(
    f"platform_jni/part_{part}.rs" for part in range(1, 4)
)
if platform_jni_includes != expected_platform_jni_includes:
    raise SystemExit(
        f"{paths['rust_platform_jni']}: expected the reviewed three-part JNI include "
        f"closure, found {platform_jni_includes!r}"
    )
texts["rust"] = "\n".join(
    (
        texts["rust"],
        texts["rust_platform_jni"],
        *(texts[f"rust_platform_jni_part_{part}"] for part in range(1, 4)),
    )
)

# The check is dormant on branches that have no ABI21 SDK work. As soon as a
# V4 lifecycle method or carrier is introduced, the entire boundary must land
# atomically instead of relying on symbol presence or an ABI20 fallback.
v4_markers = (
    "nativeInitSpendV4",
    "KagemushaRecursiveSpendInitLocalRequestV4",
    "connect_norito_kagemusha_recursive_spend_init_v4",
)
if not any(
    marker in texts[label]
    for marker in v4_markers
    for label in ("rust", "header", "swift_v4", "kotlin", "java")
):
    print("Kagemusha ABI21 SDK contract is not exposed; fail-closed pre-V4 state accepted.")
    raise SystemExit(0)

errors: list[str] = []


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


recipient_request_vector = canonical_hex_vector("recipient_request_vector")
peer_payment_vector = canonical_hex_vector("peer_payment_vector")
if len(recipient_request_vector) != 765:
    errors.append(
        f"{paths['recipient_request_vector']}: expected 765 bytes, "
        f"found {len(recipient_request_vector)}"
    )
if hashlib.sha256(recipient_request_vector).hexdigest() != (
    "899c9b4d44630e6c0c010d04ab4b0c570c2062fba5a54e678d6a0baf0e8b02b0"
):
    errors.append(f"{paths['recipient_request_vector']}: canonical SHA-256 mismatch")
if len(peer_payment_vector) != 11_887:
    errors.append(
        f"{paths['peer_payment_vector']}: expected 11887 bytes, "
        f"found {len(peer_payment_vector)}"
    )
if hashlib.sha256(peer_payment_vector).hexdigest() != (
    "ae20bc0718f3a3ff31e18b6452422549d017b66301cde799d43609614661d019"
):
    errors.append(f"{paths['peer_payment_vector']}: canonical SHA-256 mismatch")
for needle in (
    "--recipient-request-hex",
    "request.digest()",
    "norito::to_bytes(&payment)",
):
    require("peer_payment_generator", needle)
require_regex(
    "peer_payment_generator",
    r"payment\s*\.validate_public_binding\(\)",
    "public-binding validation of the emitted peer payment",
)
for needle in (
    'rustFixtureData("offline_recipient_payment_request_v2.hex")',
    'rustFixtureData("offline_peer_payment_v4.hex")',
    "KagemushaReceiverAcknowledgement.prepare(",
):
    require("swift_peer_fixtures", needle)
for needle in (
    '"ae20bc0718f3a3ff31e18b6452422549d017b66301cde799d43609614661d019"',
    '"e9aa5e352f5e14687adac62b40dbcfba2050463624ce3d21377e83fc3f34de08"',
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
    "RedeemRequestV4": "KagemushaRecursiveSpendRedeemLocalRequestV4",
    "InitResultV4": "KagemushaRecursiveSpendInitResultV4",
    "SplitResultV4": "KagemushaRecursiveSpendSplitResultV4",
    "VerifyResultV4": "KagemushaRecursiveSpendVerifyResultV4",
    "RedeemBuildResultV4": "KagemushaRecursiveSpendRedeemBuildResultV4",
}

native_methods = (
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
    "nativeBuildRedeemRequestV4",
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
    '.appendingPathComponent("kagemusha_request_authorization_v2_hardware.hex")',
)
for vector_key in (
    "authority_public_key",
    "registration_hash",
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
    "KagemushaRecursiveSpendRedeemLocalRequestV4",
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
    "redemptionChangePrepareRequestWireNameV4",
    "KagemushaRecursiveSpendRedemptionChangePrepareRequestV4",
    "redemptionChangePrepareResultWireNameV4",
    "KagemushaRecursiveSpendRedemptionChangePrepareResultV4",
):
    require("swift", needle)
require_regex(
    "swift_v4",
    r"static\s+func\s+prepareRedemptionChangeV4\s*\("
    r"[\s\S]{0,700}?input:\s*KagemushaRecursiveSpendSpendableBranchV4,"
    r"[\s\S]{0,350}?changeAmount:\s*KagemushaScaledAmount,"
    r"[\s\S]{0,350}?operationID:\s*Data,"
    r"[\s\S]{0,350}?entropy:\s*Data"
    r"[\s\S]{0,2600}?kagemushaRecursiveSpendRedemptionChangePrepareV4",
    "native-derived Swift redemption-change workflow",
)
require_regex(
    "swift_v4_codecs",
    r"encodeRedemptionChangePrepareRequest\s*\("
    r"[\s\S]{0,1800}?writeField\(uint16\(request\.version\)\)"
    r"[\s\S]{0,700}?request\.bundle\.noritoArchive"
    r"[\s\S]{0,700}?request\.inputOpening"
    r"[\s\S]{0,700}?request\.changeAmount"
    r"[\s\S]{0,500}?request\.operationID"
    r"[\s\S]{0,500}?request\.entropy"
    r"[\s\S]{0,500}?redemptionChangePrepareRequestWireNameV4",
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
    r"kagemushaRecursiveSpendRedemptionChangePrepareV4\s*\("
    r"[\s\S]{0,1800}?connect_norito_kagemusha_secret_free_buffer"
    r"[\s\S]{0,1000}?NativeBridgeError\.fromStatus\(status\)"
    r"[\s\S]{0,500}?secureFree\(output\)"
    r"[\s\S]{0,500}?copyKagemushaNativeSecretArchiveOutput",
    "Swift secret output secure-free on native error and success",
)
if re.search(
    r"kagemushaRecursiveSpendRedemptionChangePrepareV4\s*\("
    r"[\s\S]{0,2800}?connect_norito_free",
    texts["swift_native"],
):
    errors.append(
        f"{paths['swift_native']}: secret redemption-change output must never use connect_norito_free"
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
    ("encodeRedeemLocalRequest", "redeemLocalRequestWireNameV4"),
):
    require_regex(
        "swift_v4_codecs",
        rf"static\s+func\s+{encoder}\s*\([\s\S]{{0,2600}}?localWitnessVersionV4[\s\S]{{0,2600}}?{wire_name}",
        f"field-for-field Swift V4 encoder {encoder}",
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
        r"nativeBuildRedeemRequestV4\s*\([\s\S]{0,350}?bundle[\s\S]{0,350}?opening[\s\S]{0,350}?(?:membershipWitness|membership_witness)",
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
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*:\s*Int\s*=\s*22\b",
    "exact ABI22 Kotlin constant",
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
    r"V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION\s*=\s*22\s*;",
    "exact ABI22 Java constant",
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
    *privacy_compiled_profile_symbols,
    "connect_norito_sorafs_reference_validate_bundle_json",
    "connect_norito_sorafs_reference_validate_governance_json",
    "connect_norito_sorafs_reference_validate_governance_dag_block_json",
    "connect_norito_sorafs_reference_validate_governance_dag_head_chain_json",
    "connect_norito_validation_fee_current_policy_proof_request_v1",
    "connect_norito_validation_fee_current_policy_proof_verify_v1",
    "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
)
c_symbols = (
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
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
    "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
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
    "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
    "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
    "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
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
    match = re.search(
        rf"^{re.escape(name)}=\(\s*\n(?P<body>.*?)^\)",
        texts[label],
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        errors.append(f"{paths[label]}: missing shell array {name}")
        return ()
    values: list[str] = []
    for raw_line in match.group("body").splitlines():
        value = raw_line.strip()
        if not value or value.startswith("#"):
            continue
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1]
        values.append(value)
    return tuple(values)


def parse_manifest_symbol_inventory(label: str) -> tuple[str, ...]:
    match = re.search(
        r'"required_symbols"\s*:\s*\[(?P<body>.*?)\]',
        texts[label],
        re.MULTILINE | re.DOTALL,
    )
    if match is None:
        errors.append(f"{paths[label]}: missing required_symbols manifest inventory")
        return ()
    return tuple(
        re.findall(
            r'"((?:connect_norito|iroha_privacy)_[A-Za-z0-9_]+)"',
            match.group("body"),
        )
    )


actual_kagemusha_symbols = parse_shell_symbol_array("mobile_check", "KAGEMUSHA_C_SYMBOLS")
if actual_kagemusha_symbols != c_symbols:
    errors.append(
        f"{paths['mobile_check']}: exact ordered 48-symbol Kagemusha C inventory mismatch "
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

actual_required_bridge_symbols: list[str] = []
for value in parse_shell_symbol_array("mobile_check", "REQUIRED_BRIDGE_SYMBOLS"):
    if value == "${KAGEMUSHA_C_SYMBOLS[@]}":
        actual_required_bridge_symbols.extend(actual_kagemusha_symbols)
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
        "reviewed data-model fragment cannot detach",
        lambda fixture: replace_once(
            fixture / paths["data_model"],
            'include!("kagemusha_model.rs");',
            "// reviewed Kagemusha model fragment detached",
        ),
        "expected exactly one reviewed kagemusha_model.rs include",
    )

    run_negative(
        "reviewed JNI fragment closure cannot detach",
        lambda fixture: replace_once(
            fixture / paths["rust_platform_jni"],
            'include!("platform_jni/part_3.rs");',
            "// reviewed JNI fragment detached",
        ),
        "expected the reviewed three-part JNI include closure",
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
        "ABI22 cannot regress to ABI21",
        lambda fixture: replace_once(
            fixture / paths["kotlin"],
            "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 22",
            "V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 21",
        ),
        "exact ABI22 Kotlin constant",
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
            '.appendingPathComponent("kagemusha_request_authorization_v2_hardware.hex")',
            '.appendingPathComponent("dummy_hardware_authorization.hex")',
        ),
        "kagemusha_request_authorization_v2_hardware.hex",
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
