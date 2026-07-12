#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SDK_PARITY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

python3 - "$ROOT_DIR" "$MODE" <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path


root = Path(sys.argv[1]).resolve()
mode = sys.argv[2]

SWIFT_SOURCE_ROOT = Path("IrohaSwift/Sources/IrohaSwift")
SWIFT_TEST_ROOT = Path("IrohaSwift/Tests/IrohaSwiftTests")
SWIFT_PROTOCOL = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2.swift"
SWIFT_CODECS = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2Codecs.swift"
SWIFT_NATIVE = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2Native.swift"
SWIFT_AMOUNT = SWIFT_SOURCE_ROOT / "KagemushaScaledAmount.swift"
SWIFT_TORII_MODELS = SWIFT_SOURCE_ROOT / "ToriiKagemushaAPIModels.swift"
SWIFT_TORII_CLIENT = SWIFT_SOURCE_ROOT / "ToriiClient.swift"
SWIFT_TX_BUILDER = SWIFT_SOURCE_ROOT / "TxBuilder.swift"
SWIFT_ATTESTATION = SWIFT_SOURCE_ROOT / "OfflineDeviceAttestation.swift"
NATIVE_RUST = Path("crates/connect_norito_bridge/src/lib.rs")
NATIVE_HEADER = Path("crates/connect_norito_bridge/include/connect_norito_bridge.h")
NATIVE_UMBRELLA_HEADER = Path("crates/connect_norito_bridge/include/NoritoBridge.h")

REQUIRED_FILES = (
    SWIFT_PROTOCOL,
    SWIFT_CODECS,
    SWIFT_NATIVE,
    SWIFT_AMOUNT,
    SWIFT_TORII_MODELS,
    SWIFT_TORII_CLIENT,
    SWIFT_TX_BUILDER,
    SWIFT_ATTESTATION,
    NATIVE_RUST,
    NATIVE_HEADER,
    NATIVE_UMBRELLA_HEADER,
)

ALLOWED_SWIFT_OFFLINE_SOURCE_FILES = frozenset(
    (
        "KagemushaRecursiveSpendV2.swift",
        "KagemushaRecursiveSpendV2Codecs.swift",
        "KagemushaRecursiveSpendV2Native.swift",
        "KagemushaScaledAmount.swift",
        "OfflineDeviceAttestation.swift",
        "ToriiKagemushaAPIModels.swift",
    )
)

ALLOWED_SWIFT_OFFLINE_TEST_FILES = frozenset(
    (
        "KagemushaRecursiveSpendV2Tests.swift",
        "KagemushaScaledAmountTests.swift",
        "ToriiKagemushaAPIModelsTests.swift",
    )
)

ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES = frozenset(
    (
        "KagemushaDeviceAttestation",
        "KagemushaDeviceAttestationError",
        "KagemushaNoteOpening",
        "KagemushaOfflineSpendMode",
        "KagemushaPublicKey",
        "KagemushaReceiverAcknowledgement",
        "KagemushaReceiverAcknowledgementPayload",
        "KagemushaReceiverAcknowledgementVerifyResult",
        "KagemushaRecipientOutputDerivationRequest",
        "KagemushaRecipientOutputDerivationResult",
        "KagemushaRecipientPaymentRequest",
        "KagemushaRecipientPaymentRequestSigningPayload",
        "KagemushaRecursiveSpend",
        "KagemushaRecursiveSpendAppendInput",
        "KagemushaRecursiveSpendAppendRequest",
        "KagemushaRecursiveSpendArtifactIngest",
        "KagemushaRecursiveSpendArtifactInstallSessionV3",
        "KagemushaRecursiveSpendArtifactManifestArchive",
        "KagemushaRecursiveSpendArtifactReference",
        "KagemushaRecursiveSpendArtifactRole",
        "KagemushaRecursiveSpendBranch",
        "KagemushaRecursiveSpendBranchClaim",
        "KagemushaRecursiveSpendBranchPath",
        "KagemushaRecursiveSpendBundle",
        "KagemushaRecursiveSpendBundleSummary",
        "KagemushaRecursiveSpendCodecs",
        "KagemushaRecursiveSpendError",
        "KagemushaRecursiveSpendInitRequest",
        "KagemushaRecursiveSpendInputBranch",
        "KagemushaRecursiveSpendLineageMode",
        "KagemushaRecursiveSpendLineageNode",
        "KagemushaRecursiveSpendLineageWitness",
        "KagemushaRecursiveSpendNativeCapabilities",
        "KagemushaRecursiveSpendPeerPayment",
        "KagemushaRecursiveSpendRedeemChangeBranch",
        "KagemushaRecursiveSpendRedeemChangeBuildRequest",
        "KagemushaRecursiveSpendRedeemChangeBuildResult",
        "KagemushaRecursiveSpendRedeemRequest",
        "KagemushaRecursiveSpendRedeemResult",
        "KagemushaRecursiveSpendRedeemUnsigned",
        "KagemushaRecursiveSpendRedemptionIntent",
        "KagemushaRecursiveSpendRedemptionIntentBuildRequest",
        "KagemushaRecursiveSpendSplitIntent",
        "KagemushaRecursiveSpendSplitIntentBuildRequest",
        "KagemushaRecursiveSpendSplitResult",
        "KagemushaRecursiveSpendTopUpAnchor",
        "KagemushaRecursiveSpendTopUpAnchorRef",
        "KagemushaRecursiveSpendTopUpRequest",
        "KagemushaRecursiveSpendTopUpUnsigned",
        "KagemushaRecursiveSpendVerifierRecordRef",
        "KagemushaRecursiveSpendVerifyRequest",
        "KagemushaRecursiveSpendVerifyResult",
        "KagemushaRequestAuthorization",
        "KagemushaRequestAuthorizationFields",
        "KagemushaScaledAmount",
        "KagemushaScaledAmountError",
        "KagemushaSpendableNoteDescriptor",
        "KagemushaTopUpFinalityProofArchive",
        "KagemushaTopUpFinalityRosterArtifactArchive",
        "KagemushaTopUpShieldBuildRequest",
        "KagemushaTopUpShieldEvidence",
        "KagemushaTopUpShieldPreparation",
        "KagemushaTopUpShieldReadinessExpectation",
        "KagemushaTopUpShieldSnapshotBinding",
        "KagemushaTopUpShieldVerifierBinding",
        "KagemushaUnshieldPublicInputsBinding",
        "KagemushaVerifiedRecipientPaymentRequest",
    )
)

REQUIRED_NATIVE_EXPORTS = (
    "connect_norito_kagemusha_recursive_spend_capabilities_v1",
    "connect_norito_kagemusha_topup_finality_verify_v2",
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
    "connect_norito_kagemusha_recursive_spend_build_split_intent_v2",
    "connect_norito_kagemusha_recursive_spend_build_redemption_intent_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
    "connect_norito_kagemusha_recursive_spend_init_v2",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
    "connect_norito_kagemusha_recursive_spend_verify_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v2",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
)


class CheckFailure(RuntimeError):
    pass


def read_required(path: Path) -> str:
    absolute = root / path
    if not absolute.is_file():
        raise CheckFailure(f"required file missing: {path}")
    return absolute.read_text(encoding="utf-8")


def require(text: str, needle: str, label: str) -> None:
    if needle not in text:
        raise CheckFailure(f"{label}: missing {needle!r}")


def check(texts: dict[Path, str]) -> None:
    swift_protocol = texts[SWIFT_PROTOCOL]
    swift_amount = texts[SWIFT_AMOUNT]
    swift_native = texts[SWIFT_NATIVE]
    swift_models = texts[SWIFT_TORII_MODELS]
    swift_client = texts[SWIFT_TORII_CLIENT]
    swift_builder = texts[SWIFT_TX_BUILDER]
    swift_attestation = texts[SWIFT_ATTESTATION]
    native_rust = texts[NATIVE_RUST]
    native_header = texts[NATIVE_HEADER]
    native_umbrella = texts[NATIVE_UMBRELLA_HEADER]

    actual_source_files = {
        path.name
        for path in (root / SWIFT_SOURCE_ROOT).glob("*.swift")
        if "Kagemusha" in path.name or "Offline" in path.name
    }
    if actual_source_files != ALLOWED_SWIFT_OFFLINE_SOURCE_FILES:
        raise CheckFailure(
            "Swift offline source inventory mismatch: "
            f"missing={sorted(ALLOWED_SWIFT_OFFLINE_SOURCE_FILES - actual_source_files)}, "
            f"extra={sorted(actual_source_files - ALLOWED_SWIFT_OFFLINE_SOURCE_FILES)}"
        )
    actual_test_files = {
        path.name
        for path in (root / SWIFT_TEST_ROOT).glob("*.swift")
        if "Kagemusha" in path.name or "Offline" in path.name
    }
    if actual_test_files != ALLOWED_SWIFT_OFFLINE_TEST_FILES:
        raise CheckFailure(
            "Swift offline test inventory mismatch: "
            f"missing={sorted(ALLOWED_SWIFT_OFFLINE_TEST_FILES - actual_test_files)}, "
            f"extra={sorted(actual_test_files - ALLOWED_SWIFT_OFFLINE_TEST_FILES)}"
        )

    swift_public_types = set()
    for path in (root / SWIFT_SOURCE_ROOT).glob("*.swift"):
        relative = path.relative_to(root)
        source = texts.get(relative)
        if source is None:
            source = path.read_text(encoding="utf-8")
        swift_public_types.update(
            re.findall(
                r"^public\s+(?:final\s+)?(?:struct|enum|class|protocol|typealias)\s+"
                r"(Kagemusha\w+)",
                source,
                re.M,
            )
        )
    if swift_public_types != ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES:
        raise CheckFailure(
            "Swift Kagemusha public symbol inventory mismatch: "
            f"missing={sorted(ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES - swift_public_types)}, "
            f"extra={sorted(swift_public_types - ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES)}"
        )

    expected_native_exports = set(REQUIRED_NATIVE_EXPORTS)
    for path, source in (
        (NATIVE_RUST, native_rust),
        (NATIVE_HEADER, native_header),
        (SWIFT_NATIVE, swift_native),
    ):
        actual_native_exports = set(
            re.findall(r"\bconnect_norito_kagemusha_[a-z0-9_]+\b", source)
        )
        if actual_native_exports != expected_native_exports:
            raise CheckFailure(
                f"Kagemusha native export inventory mismatch in {path}: "
                f"missing={sorted(expected_native_exports - actual_native_exports)}, "
                f"extra={sorted(actual_native_exports - expected_native_exports)}"
            )

    mode_match = re.search(
        r"public enum KagemushaOfflineSpendMode[^\{]*\{(?P<body>.*?)\n\}",
        swift_protocol,
        re.S,
    )
    if mode_match is None:
        raise CheckFailure("Swift sole Kagemusha product selector is missing")
    cases = re.findall(r"^\s*case\s+", mode_match.group("body"), re.M)
    if len(cases) != 1:
        raise CheckFailure(f"Swift must expose exactly one offline spend mode, found {len(cases)}")
    require(mode_match.group("body"), 'case recursiveSpend = "recursive_spend_v1"', "Swift product selector")
    require(swift_protocol, 'public static let productMode = "recursive_spend_v1"', "Swift product mode")
    require(swift_protocol, 'public static let artifactManifestMode = "recursive_spend_v2"', "Swift artifact mode")
    require(swift_protocol, "public static let mode = productMode", "Swift product mode alias")
    require(swift_protocol, "value == productMode", "Swift spend-again selector")

    swift_api = swift_protocol + "\n" + swift_amount
    for needle in (
        "public struct KagemushaScaledAmount",
        "public struct KagemushaNoteOpening",
        "public struct KagemushaRecipientOutputDerivationRequest",
        "public struct KagemushaRecursiveSpendInitRequest",
        "public struct KagemushaRecursiveSpendAppendRequest",
        "public struct KagemushaRecursiveSpendVerifyRequest",
        "public struct KagemushaRecursiveSpendVerifierRecordRef",
        "public struct KagemushaRecursiveSpendRedeemUnsigned",
        "public struct KagemushaRecursiveSpendRedeemRequest",
        "public struct KagemushaRecursiveSpendRedeemChangeBuildRequest",
        "public struct KagemushaRecursiveSpendLineageWitness",
        "public static func initSpend(",
        "public static func appendSpend(",
        "public static func verifySpend(",
        "public static func redeemSpend(",
    ):
        require(swift_api, needle, "Swift typed Kagemusha lifecycle")

    require(swift_attestation, "public enum KagemushaDeviceAttestation", "Swift Kagemusha attestation")
    require(swift_attestation, "public enum KagemushaDeviceAttestationError", "Swift Kagemusha attestation errors")
    require(swift_builder, "public func prepareKagemushaTopUpShield(", "Swift authoritative top-up preparation")
    require(swift_builder, "expectedReadiness: KagemushaTopUpShieldReadinessExpectation", "Swift verifier snapshot binding")

    for route in (
        'case readiness = "/v1/offline/readiness"',
        'case topUp = "/v1/offline/top-up"',
        'case redeem = "/v1/offline/redeem"',
        'case operations = "/v1/offline/operations"',
    ):
        require(swift_models, route, "Swift direct Torii route")
    for needle in (
        "public struct OfflineTopUpRequest",
        "public struct OfflineRedeemRequest",
        "public enum OfflineOperationStatus",
        '"Content-Type": "application/x-norito"',
        '"Accept": "application/x-norito"',
        '"Idempotency-Key": operationId',
        "try ensureStatus(response, equals: 202",
    ):
        require(swift_models + swift_client, needle, "Swift direct Torii lifecycle")

    actual_routes = set(
        re.findall(r'"(/v1/offline/[^"\\]+)"', swift_models + swift_client)
    )
    expected_routes = {
        "/v1/offline/readiness",
        "/v1/offline/top-up",
        "/v1/offline/redeem",
        "/v1/offline/operations",
    }
    if actual_routes != expected_routes:
        raise CheckFailure(
            "Swift direct Torii route inventory mismatch: "
            f"missing={sorted(expected_routes - actual_routes)}, "
            f"extra={sorted(actual_routes - expected_routes)}"
        )
    if re.search(r"base64", swift_models, re.I):
        raise CheckFailure("Swift direct Torii request models must carry canonical Norito bytes only")

    require(native_umbrella, '#include "connect_norito_bridge.h"', "native umbrella header")


texts = {path: read_required(path) for path in REQUIRED_FILES}

negative_controls = {
    "--negative-control-extra-swift-protocol": (
        SWIFT_PROTOCOL,
        "\npublic struct KagemushaAlternativeSpendProtocol {}\n",
        "Swift Kagemusha public symbol inventory",
    ),
    "--negative-control-extra-native-export": (
        NATIVE_HEADER,
        "\nint32_t connect_norito_kagemusha_alternative_spend(void);\n",
        "Kagemusha native export inventory",
    ),
    "--negative-control-product-mode": (
        SWIFT_PROTOCOL,
        None,
        "Swift product mode",
    ),
    "--negative-control-direct-route": (
        SWIFT_TORII_MODELS,
        None,
        "Swift direct Torii route",
    ),
    "--negative-control-required-native-export": (
        NATIVE_HEADER,
        None,
        "Kagemusha native export inventory",
    ),
}

if mode:
    if mode not in negative_controls:
        raise SystemExit(f"unsupported parity mode: {mode}")
    target, suffix, expected = negative_controls[mode]
    mutated = dict(texts)
    if suffix is not None:
        mutated[target] += suffix
    elif mode == "--negative-control-product-mode":
        mutated[target] = mutated[target].replace(
            'public static let productMode = "recursive_spend_v1"',
            'public static let productMode = "recursive_spend_v9"',
            1,
        )
    elif mode == "--negative-control-direct-route":
        mutated[target] = mutated[target].replace(
            'case topUp = "/v1/offline/top-up"',
            'case topUp = "/v1/offline/v2/kagemusha/topup"',
            1,
        )
    else:
        mutated[target] = mutated[target].replace(
            "connect_norito_kagemusha_recursive_spend_append_v2",
            "connect_norito_kagemusha_recursive_spend_append_removed_v2",
        )
    try:
        check(mutated)
    except CheckFailure as error:
        if expected not in str(error):
            raise SystemExit(f"negative control failed for the wrong reason: {error}")
        print(f"negative control rejected drift: {error}")
        raise SystemExit(0)
    raise SystemExit(f"negative control was not rejected: {mode}")

try:
    check(texts)
except CheckFailure as error:
    raise SystemExit(f"Kagemusha SDK parity failed: {error}")

print("Kagemusha Swift/native single-protocol parity passed")
PY
