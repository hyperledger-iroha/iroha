#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SDK_PARITY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

if [[ $# -ne 0 ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_sdk_parity.sh" >&2
  exit 2
fi

python3 - "$ROOT_DIR" <<'PY'
from __future__ import annotations

import re
import sys
from pathlib import Path


root = Path(sys.argv[1]).resolve()

SWIFT_SOURCE_ROOT = Path("IrohaSwift/Sources/IrohaSwift")
SWIFT_TEST_ROOT = Path("IrohaSwift/Tests/IrohaSwiftTests")
SWIFT_PROTOCOL = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2.swift"
SWIFT_ARTIFACT_COORDINATOR = SWIFT_SOURCE_ROOT / "KagemushaArtifactCoordinator.swift"
SWIFT_CODECS = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2Codecs.swift"
SWIFT_NATIVE = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV2Native.swift"
SWIFT_AMOUNT = SWIFT_SOURCE_ROOT / "KagemushaScaledAmount.swift"
SWIFT_PEER_TRANSPORT = SWIFT_SOURCE_ROOT / "KagemushaPeerTransport.swift"
SWIFT_QR_STREAM = SWIFT_SOURCE_ROOT / "KagemushaQRStream.swift"
SWIFT_NFC = SWIFT_SOURCE_ROOT / "KagemushaNFC.swift"
SWIFT_NEARBY = SWIFT_SOURCE_ROOT / "KagemushaNearby.swift"
SWIFT_TORII_MODELS = SWIFT_SOURCE_ROOT / "ToriiKagemushaAPIModels.swift"
SWIFT_TORII_CLIENT = SWIFT_SOURCE_ROOT / "ToriiClient.swift"
SWIFT_TX_BUILDER = SWIFT_SOURCE_ROOT / "TxBuilder.swift"
SWIFT_ATTESTATION = SWIFT_SOURCE_ROOT / "OfflineDeviceAttestation.swift"
SWIFT_PROTOCOL_TESTS = SWIFT_TEST_ROOT / "KagemushaRecursiveSpendV2Tests.swift"
SWIFT_ARTIFACT_COORDINATOR_TESTS = SWIFT_TEST_ROOT / "KagemushaArtifactCoordinatorTests.swift"
SWIFT_PACKAGE = Path("IrohaSwift/Package.swift")
NATIVE_RUST = Path("crates/connect_norito_bridge/src/lib.rs")
RUST_DATA_MODEL = Path("crates/iroha_data_model/src/offline/mod.rs")
NATIVE_HEADER = Path("crates/connect_norito_bridge/include/connect_norito_bridge.h")
NATIVE_UMBRELLA_HEADER = Path("crates/connect_norito_bridge/include/NoritoBridge.h")

REQUIRED_FILES = (
    SWIFT_PROTOCOL,
    SWIFT_ARTIFACT_COORDINATOR,
    SWIFT_CODECS,
    SWIFT_NATIVE,
    SWIFT_AMOUNT,
    SWIFT_PEER_TRANSPORT,
    SWIFT_QR_STREAM,
    SWIFT_NFC,
    SWIFT_NEARBY,
    SWIFT_TORII_MODELS,
    SWIFT_TORII_CLIENT,
    SWIFT_TX_BUILDER,
    SWIFT_ATTESTATION,
    SWIFT_PROTOCOL_TESTS,
    SWIFT_ARTIFACT_COORDINATOR_TESTS,
    SWIFT_PACKAGE,
    NATIVE_RUST,
    RUST_DATA_MODEL,
    NATIVE_HEADER,
    NATIVE_UMBRELLA_HEADER,
)

ALLOWED_SWIFT_OFFLINE_SOURCE_FILES = frozenset(
    (
        "KagemushaArtifactCoordinator.swift",
        "KagemushaRecursiveSpendV2.swift",
        "KagemushaRecursiveSpendV2Codecs.swift",
        "KagemushaRecursiveSpendV2Native.swift",
        "KagemushaScaledAmount.swift",
        "KagemushaPeerTransport.swift",
        "KagemushaQRStream.swift",
        "KagemushaNFC.swift",
        "KagemushaNearby.swift",
        "OfflineDeviceAttestation.swift",
        "ToriiKagemushaAPIModels.swift",
    )
)

ALLOWED_SWIFT_OFFLINE_TEST_FILES = frozenset(
    (
        "KagemushaArtifactCoordinatorTests.swift",
        "KagemushaRecursiveSpendV2Tests.swift",
        "KagemushaScaledAmountTests.swift",
        "KagemushaPeerTransportTestFixtures.swift",
        "KagemushaPeerTransportTests.swift",
        "KagemushaQRStreamTests.swift",
        "KagemushaNFCTests.swift",
        "KagemushaNearbyTests.swift",
        "OfflineDeviceAttestationABI19ParityTests.swift",
        "ToriiKagemushaAPIModelsTests.swift",
    )
)

ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES = frozenset(
    (
        "KagemushaConfidentialVerifierBinding",
        "KagemushaDeviceAttestation",
        "KagemushaDeviceAttestationError",
        "KagemushaDeviceAttestationRegistration",
        "KagemushaDeviceAttestationUnsignedTransaction",
        "KagemushaAxtErrorDetails",
        "KagemushaNFCAvailability",
        "KagemushaNFCAvailabilityReason",
        "KagemushaNFCCardHandleResult",
        "KagemushaNFCCardRejectionReason",
        "KagemushaNFCCardSession",
        "KagemushaNFCCardStateMachine",
        "KagemushaNFCCommand",
        "KagemushaNFCConfiguration",
        "KagemushaNFCError",
        "KagemushaNFCEvent",
        "KagemushaNFCMessages",
        "KagemushaNFCPayloadInfo",
        "KagemushaNFCProtocol",
        "KagemushaNFCReader",
        "KagemushaNearbyError",
        "KagemushaNearbyAuthenticationPolicy",
        "KagemushaNearbyEvent",
        "KagemushaNearbyExchange",
        "KagemushaNearbyPairingChallenge",
        "KagemushaNearbyPairingDecision",
        "KagemushaNearbyPairingSymbol",
        "KagemushaNearbyTransportPolicy",
        "KagemushaNoteMembershipWitness",
        "KagemushaNoteOpening",
        "KagemushaOperationCodec",
        "KagemushaOperationError",
        "KagemushaOperationErrorDetails",
        "KagemushaOperationErrorEnvelope",
        "KagemushaOperationKind",
        "KagemushaOperationReference",
        "KagemushaOperationResult",
        "KagemushaOperationState",
        "KagemushaOperationStatus",
        "KagemushaPeerPayload",
        "KagemushaPeerPayloadKind",
        "KagemushaPeerSendResult",
        "KagemushaPeerTextCodec",
        "KagemushaPeerTransportContract",
        "KagemushaPeerTransportError",
        "KagemushaPublicKey",
        "KagemushaQRDecodeResult",
        "KagemushaQRStreamCodec",
        "KagemushaQRStreamDecoder",
        "KagemushaQRStreamError",
        "KagemushaQRStreamOptions",
        "KagemushaQueueErrorSnapshot",
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
        "KagemushaRecursiveSpendArtifactCoordinator",
        "KagemushaRecursiveSpendArtifactIngest",
        "KagemushaRecursiveSpendArtifactInstallSessionV3",
        "KagemushaRecursiveSpendArtifactManifestArchive",
        "KagemushaRecursiveSpendArtifactBinding",
        "KagemushaRecursiveSpendArtifactStream",
        "KagemushaRecursiveSpendBranch",
        "KagemushaRecursiveSpendBranchClaim",
        "KagemushaRecursiveSpendBranchPath",
        "KagemushaRecursiveSpendBundle",
        "KagemushaRecursiveSpendBundleSummary",
        "KagemushaRecursiveSpendCodecs",
        "KagemushaRecursiveSpendError",
        "KagemushaRecursiveSpendInitRequest",
        "KagemushaRecursiveSpendInitResult",
        "KagemushaRecursiveSpendInputBranch",
        "KagemushaRecursiveSpendInstalledArtifactLease",
        "KagemushaRecursiveSpendInstalledArtifactSet",
        "KagemushaRecursiveSpendLineageProjection",
        "KagemushaRecursiveSpendNativeCapabilities",
        "KagemushaRecursiveSpendPeerPayment",
        "KagemushaRecursiveSpendRedeemChangeBranch",
        "KagemushaRecursiveSpendRedeemBuildRequest",
        "KagemushaRecursiveSpendRedeemBuildResult",
        "KagemushaRecursiveSpendRedeemRecoveryEvidence",
        "KagemushaRecursiveSpendRedeemRequest",
        "KagemushaRecursiveSpendRedeemResult",
        "KagemushaRecursiveSpendRedeemUnsigned",
        "KagemushaRecursiveSpendRedemptionIntent",
        "KagemushaRecursiveSpendSplitIntent",
        "KagemushaRecursiveSpendSplitIntentBuildRequest",
        "KagemushaRecursiveSpendSplitResult",
        "KagemushaRecursiveSpendTopUpAnchor",
        "KagemushaRecursiveSpendTopUpAnchorRef",
        "KagemushaRecursiveSpendTopUpRequest",
        "KagemushaRecursiveSpendTopUpUnsigned",
        "KagemushaRecursiveSpendVerifyRequest",
        "KagemushaRecursiveSpendVerifyResult",
        "KagemushaRedeemRequest",
        "KagemushaRedeemResult",
        "KagemushaRequestAuthorization",
        "KagemushaRequestAuthorizationFields",
        "KagemushaScaledAmount",
        "KagemushaScaledAmountError",
        "KagemushaSpendableNoteDescriptor",
        "KagemushaTopUpAnchor",
        "KagemushaTopUpFinalityProof",
        "KagemushaTopUpFinalityProofArchive",
        "KagemushaTopUpFinalityRosterArtifactArchive",
        "KagemushaTopUpShieldBuildRequest",
        "KagemushaTopUpShieldEvidence",
        "KagemushaTopUpShieldPreparation",
        "KagemushaTopUpShieldReadinessExpectation",
        "KagemushaTopUpShieldSnapshotBinding",
        "KagemushaTopUpShieldVerifierBinding",
        "KagemushaTopUpRequest",
        "KagemushaTopUpResult",
        "KagemushaToriiAPI",
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
    "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v2",
    "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v2",
    "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
    "connect_norito_kagemusha_recursive_spend_init_v2",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
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
    swift_peer_transport = texts[SWIFT_PEER_TRANSPORT]
    swift_qr_stream = texts[SWIFT_QR_STREAM]
    swift_nfc = texts[SWIFT_NFC]
    swift_nearby = texts[SWIFT_NEARBY]
    swift_models = texts[SWIFT_TORII_MODELS]
    swift_client = texts[SWIFT_TORII_CLIENT]
    swift_builder = texts[SWIFT_TX_BUILDER]
    swift_attestation = texts[SWIFT_ATTESTATION]
    swift_protocol_tests = texts[SWIFT_PROTOCOL_TESTS]
    swift_package = texts[SWIFT_PACKAGE]
    native_rust = texts[NATIVE_RUST]
    rust_data_model = texts[RUST_DATA_MODEL]
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

    library_products = re.findall(
        r"\.library\(\s*name:\s*\"([^\"]+)\"",
        swift_package,
        re.S,
    )
    if library_products != ["IrohaSwift"]:
        raise CheckFailure(
            "Swift package must publish only the IrohaSwift library product; "
            f"found={library_products}"
        )

    expected_native_exports = set(REQUIRED_NATIVE_EXPORTS)
    native_export_patterns = (
        (
            NATIVE_RUST,
            native_rust,
            r'pub\s+unsafe\s+extern\s+"C"\s+fn\s+'
            r'(connect_norito_kagemusha_[a-z0-9_]+)',
        ),
        (
            NATIVE_HEADER,
            native_header,
            r'\bint32_t\s+(connect_norito_kagemusha_[a-z0-9_]+)\s*\(',
        ),
        (
            SWIFT_NATIVE,
            swift_native,
            r'"(connect_norito_kagemusha_[a-z0-9_]+)"',
        ),
    )
    for path, source, export_pattern in native_export_patterns:
        actual_native_exports = set(
            re.findall(export_pattern, source)
        )
        if actual_native_exports != expected_native_exports:
            raise CheckFailure(
                f"Kagemusha native export inventory mismatch in {path}: "
                f"missing={sorted(expected_native_exports - actual_native_exports)}, "
                f"extra={sorted(actual_native_exports - expected_native_exports)}"
            )

    swift_api = swift_protocol + "\n" + swift_amount
    require(
        swift_protocol,
        'wire("KagemushaRecursiveSpendNativeCapabilitiesV1")',
        "Swift ABI-19 capability wire",
    )
    if "KagemushaRecursiveSpendNativeCapabilitiesV3" in swift_protocol:
        raise CheckFailure("Swift must not expose the artifact release number as a capability ABI")
    rust_gate_match = re.search(
        r"fn\s+kagemusha_v3_missing_gates\(\)\s*->\s*Vec<String>\s*\{"
        r"\s*\[(?P<body>[\s\S]*?)\]\s*\.map",
        rust_data_model,
    )
    swift_gate_match = re.search(
        r"public\s+static\s+let\s+unavailableProofBackendGates\s*=\s*"
        r"\[(?P<body>[\s\S]*?)\]",
        swift_protocol,
    )
    test_gate_match = re.search(
        r"XCTAssertEqual\(\s*"
        r"KagemushaRecursiveSpend\.unavailableProofBackendGates\s*,\s*"
        r"\[(?P<body>[\s\S]*?)\]\s*\)",
        swift_protocol_tests,
    )
    if rust_gate_match is None or swift_gate_match is None or test_gate_match is None:
        raise CheckFailure("Kagemusha V3 missing-gate inventories must be explicit")
    rust_gates = re.findall(r'"([^"]+)"', rust_gate_match.group("body"))
    swift_gates = re.findall(r'"([^"]+)"', swift_gate_match.group("body"))
    tested_gates = re.findall(r'"([^"]+)"', test_gate_match.group("body"))
    if swift_gates != rust_gates or tested_gates != rust_gates:
        raise CheckFailure(
            "Kagemusha V3 missing-gate order drift: "
            f"rust={rust_gates}, swift={swift_gates}, tests={tested_gates}"
        )
    require(
        swift_protocol,
        "public static let maximumInputsPerTransition = 2",
        "Swift two-input transition bound",
    )
    require(
        swift_protocol,
        "public static let maximumPeerHops: UInt32 = 8",
        "Swift 8-peer-hop bound",
    )
    require(
        swift_protocol_tests,
        "func testPeerHopLimitIsEightAtMaximumBranchDepth()",
        "Swift peer-hop boundary regression",
    )
    require(
        swift_protocol_tests,
        "XCTAssertEqual(KagemushaRecursiveSpend.maximumPeerHops, 8)",
        "Swift exact peer-hop assertion",
    )
    for needle in (
        "public struct KagemushaScaledAmount",
        "public struct KagemushaNoteOpening",
        "public struct KagemushaRecipientOutputDerivationRequest",
        "public struct KagemushaRecursiveSpendArtifactBinding",
        "public struct KagemushaRecursiveSpendInitRequest",
        "public struct KagemushaRecursiveSpendAppendRequest",
        "public struct KagemushaRecursiveSpendVerifyRequest",
        "public struct KagemushaRecursiveSpendRedeemUnsigned",
        "public struct KagemushaRecursiveSpendRedeemRequest",
        "public struct KagemushaRecursiveSpendRedeemBuildRequest",
        "public struct KagemushaRecursiveSpendRedeemBuildResult",
        "public static func initSpend(",
        "public static func appendSpend(",
        "public static func verifySpend(",
        "public static func buildRedeem(",
    ):
        require(swift_api, needle, "Swift typed Kagemusha lifecycle")

    require(swift_attestation, "public enum KagemushaDeviceAttestation", "Swift Kagemusha attestation")
    require(swift_attestation, "public enum KagemushaDeviceAttestationError", "Swift Kagemusha attestation errors")
    require(swift_attestation, "public struct KagemushaDeviceAttestationRegistration", "Swift Kagemusha attestation registration")
    require(swift_attestation, "public struct RegisterKagemushaDeviceAttestationRequest", "Swift Kagemusha attestation request")
    require(swift_attestation, "buildUnsignedRegisterKagemushaDeviceAttestation(", "Swift Kagemusha attestation builder")
    require(swift_builder, "public func prepareKagemushaTopUpShield(", "Swift authoritative top-up preparation")
    require(swift_builder, "expectedReadiness: KagemushaTopUpShieldReadinessExpectation", "Swift verifier snapshot binding")

    for needle in (
        'receiveRequestTextPrefix = "PKK2R."',
        'paymentTextPrefix = "PKK2P."',
        'acknowledgementTextPrefix = "PKK2A."',
        'qrStreamTextPrefix = "PKKQ1."',
        'nfcApplicationIdentifierHex = "F0504B45504B524E464301"',
        'nearbyServiceName = "pk-kagemusha"',
        '"text/vnd.pk.kagemusha-v2.receive-request"',
        '"text/vnd.pk.kagemusha-v2.payment"',
        '"text/vnd.pk.kagemusha-v2.ack"',
        "public enum KagemushaPeerPayload",
        "public enum KagemushaPeerTextCodec",
    ):
        require(swift_peer_transport, needle, "Swift peer transport wire contract")
    for source, needles, label in (
        (
            swift_qr_stream,
            ("public enum KagemushaQRStreamCodec", "public final class KagemushaQRStreamDecoder"),
            "Swift Kagemusha QR stream",
        ),
        (
            swift_nfc,
            ("public final class KagemushaNFCReader", "public final class KagemushaNFCCardSession"),
            "Swift Kagemusha NFC transport",
        ),
        (
            swift_nearby,
            (
                "public final class KagemushaNearbyExchange",
                "public struct KagemushaNearbyPairingChallenge",
                "public static let hasAuditedAuthenticatedTranscriptBackend = false",
                "#if KAGEMUSHA_NEARBY_AUDITED_AUTHENTICATED_TRANSCRIPT && canImport(MultipeerConnectivity)",
            ),
            "Swift Kagemusha Nearby transport",
        ),
    ):
        for needle in needles:
            require(source, needle, label)

    for route in (
        'case readiness = "/v1/offline/readiness"',
        'case topUp = "/v1/offline/top-up"',
        'case redeem = "/v1/offline/redeem"',
        'case operations = "/v1/offline/operations"',
    ):
        require(swift_models, route, "Swift direct Torii route")
    for needle in (
        "public enum KagemushaToriiAPI",
        "public struct KagemushaTopUpRequest",
        "public struct KagemushaRedeemRequest",
        "public enum KagemushaOperationStatus",
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

try:
    check(texts)
except CheckFailure as error:
    raise SystemExit(f"Kagemusha SDK parity failed: {error}")

print("Kagemusha Swift/native single-protocol parity passed")
PY
