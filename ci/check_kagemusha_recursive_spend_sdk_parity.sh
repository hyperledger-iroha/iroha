#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SDK_PARITY_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
MODE="${1:-}"

if [[ -n "${MODE}" && "${MODE}" != "--self-test" ]] || [[ $# -gt 1 ]]; then
  echo "usage: ci/check_kagemusha_recursive_spend_sdk_parity.sh [--self-test]" >&2
  exit 2
fi

python3 - "$ROOT_DIR" "$MODE" <<'PY'
from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path


root = Path(sys.argv[1]).resolve()
self_test = sys.argv[2] == "--self-test"

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
SWIFT_FINALITY = SWIFT_SOURCE_ROOT / "KagemushaOperationFinalityCoordinator.swift"
SWIFT_V4 = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV4.swift"
SWIFT_V4_CODECS = SWIFT_SOURCE_ROOT / "KagemushaRecursiveSpendV4Codecs.swift"
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
    SWIFT_FINALITY,
    SWIFT_V4,
    SWIFT_V4_CODECS,
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
        "KagemushaOperationFinalityCoordinator.swift",
        "KagemushaRecursiveSpendV2.swift",
        "KagemushaRecursiveSpendV2Codecs.swift",
        "KagemushaRecursiveSpendV2Native.swift",
        "KagemushaRecursiveSpendV4.swift",
        "KagemushaRecursiveSpendV4Codecs.swift",
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
        "KagemushaDeviceAttestationSignedTransactionTests.swift",
        "KagemushaDeviceAuthorityV2Tests.swift",
        "KagemushaOperationFinalityCoordinatorTests.swift",
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
        "KagemushaDefinitiveSubmissionFailure",
        "KagemushaDeviceAttestation",
        "KagemushaDeviceAttestationError",
        "KagemushaDeviceAttestationRegistration",
        "KagemushaDeviceAttestationSignedTransaction",
        "KagemushaDeviceAttestationSignedTransactionError",
        "KagemushaDeviceAttestationUnsignedTransaction",
        "KagemushaDevicePublicKeyV2",
        "KagemushaDeviceSignatureV2",
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
        "KagemushaOperationContinuity",
        "KagemushaOperationError",
        "KagemushaOperationErrorDetails",
        "KagemushaOperationErrorEnvelope",
        "KagemushaOperationFinalRejection",
        "KagemushaOperationFinalityConfiguration",
        "KagemushaOperationFinalityCoordinator",
        "KagemushaOperationFinalityError",
        "KagemushaOperationFinalityOutcome",
        "KagemushaOperationFinalityResolution",
        "KagemushaOperationFinalityTransport",
        "KagemushaOperationKind",
        "KagemushaOperationReference",
        "KagemushaOperationResult",
        "KagemushaOperationState",
        "KagemushaOperationStatus",
        "KagemushaOperationSubmission",
        "KagemushaOperationTerminalFailure",
        "KagemushaOutputMembershipLeafPaths",
        "KagemushaOutputMembershipLeafPathsV4",
        "KagemushaOutputMembershipPaths",
        "KagemushaOutputMembershipPathsV4",
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
        "KagemushaRecursiveSpendAppendInputV4",
        "KagemushaRecursiveSpendAppendLocalRequestV4",
        "KagemushaRecursiveSpendAppendRequest",
        "KagemushaRecursiveSpendArtifactBinding",
        "KagemushaRecursiveSpendArtifactBindingV4",
        "KagemushaRecursiveSpendArtifactCoordinator",
        "KagemushaRecursiveSpendArtifactIngest",
        "KagemushaRecursiveSpendArtifactIngestV3",
        "KagemushaRecursiveSpendArtifactInstallSessionV3",
        "KagemushaRecursiveSpendArtifactInstallSessionV4",
        "KagemushaRecursiveSpendArtifactManifestArchive",
        "KagemushaRecursiveSpendArtifactManifestArchiveV3",
        "KagemushaRecursiveSpendArtifactRoleV4",
        "KagemushaRecursiveSpendArtifactStream",
        "KagemushaRecursiveSpendBranch",
        "KagemushaRecursiveSpendBranchClaim",
        "KagemushaRecursiveSpendBranchPath",
        "KagemushaRecursiveSpendBundle",
        "KagemushaRecursiveSpendBundleSummary",
        "KagemushaRecursiveSpendBundleV4",
        "KagemushaRecursiveSpendCodecs",
        "KagemushaRecursiveSpendError",
        "KagemushaRecursiveSpendInitLocalRequest",
        "KagemushaRecursiveSpendInitLocalRequestV4",
        "KagemushaRecursiveSpendInitRequest",
        "KagemushaRecursiveSpendInitRequestV4",
        "KagemushaRecursiveSpendInitResult",
        "KagemushaRecursiveSpendInitResultV4",
        "KagemushaRecursiveSpendInputBranch",
        "KagemushaRecursiveSpendInstalledArtifactLease",
        "KagemushaRecursiveSpendInstalledArtifactSetV3",
        "KagemushaRecursiveSpendInstalledArtifactSetV4",
        "KagemushaRecursiveSpendLineageProjection",
        "KagemushaRecursiveSpendNativeCapabilities",
        "KagemushaRecursiveSpendNativeCapabilitiesV4",
        "KagemushaRecursiveSpendPeerPayment",
        "KagemushaRecursiveSpendRedeemChangeBranch",
        "KagemushaRecursiveSpendRedeemBuildRequest",
        "KagemushaRecursiveSpendRedeemBuildResult",
        "KagemushaRecursiveSpendRedeemBuildResultV4",
        "KagemushaRecursiveSpendRedeemRecoveryEvidence",
        "KagemushaRecursiveSpendRedeemLocalRequestV4",
        "KagemushaRecursiveSpendRedeemRequest",
        "KagemushaRecursiveSpendRedeemResult",
        "KagemushaRecursiveSpendRedeemUnsigned",
        "KagemushaRecursiveSpendRedemptionIntent",
        "KagemushaRecursiveSpendReleaseAuthenticationV3",
        "KagemushaRecursiveSpendReleaseAuthenticationV4",
        "KagemushaRecursiveSpendSpendableBranchV4",
        "KagemushaRecursiveSpendSplitIntent",
        "KagemushaRecursiveSpendSplitIntentBuildRequest",
        "KagemushaRecursiveSpendSplitResult",
        "KagemushaRecursiveSpendSplitResultV4",
        "KagemushaRecursiveSpendTopUpAnchor",
        "KagemushaRecursiveSpendTopUpAnchorRef",
        "KagemushaRecursiveSpendTopUpAnchorV4",
        "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
        "KagemushaRecursiveSpendTopUpRequest",
        "KagemushaRecursiveSpendTopUpUnsigned",
        "KagemushaRecursiveSpendVerifyLocalRequestV4",
        "KagemushaRecursiveSpendVerifyRequest",
        "KagemushaRecursiveSpendVerifyRequestV4",
        "KagemushaRecursiveSpendVerifyResult",
        "KagemushaRecursiveSpendVerifyResultV4",
        "KagemushaRedeemRequest",
        "KagemushaRedeemResult",
        "KagemushaRequestAuthorization",
        "KagemushaRequestAuthorizationFields",
        "KagemushaScaledAmount",
        "KagemushaScaledAmountError",
        "KagemushaSpendableNoteDescriptor",
        "KagemushaSubmissionFailureClassifier",
        "KagemushaSubmissionFailureDisposition",
        "KagemushaSubmissionTarget",
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
    "connect_norito_kagemusha_recursive_spend_capabilities_v4",
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
    "connect_norito_kagemusha_recursive_spend_init_v3",
    "connect_norito_kagemusha_recursive_spend_init_v4",
    "connect_norito_kagemusha_topup_shield_build_unsigned_v2",
    "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_topup_v2",
    "connect_norito_kagemusha_recursive_spend_append_v2",
    "connect_norito_kagemusha_recursive_spend_append_v3",
    "connect_norito_kagemusha_recursive_spend_append_v4",
    "connect_norito_kagemusha_recursive_spend_verify_v2",
    "connect_norito_kagemusha_recursive_spend_verify_v3",
    "connect_norito_kagemusha_recursive_spend_verify_v4",
    "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v2",
    "connect_norito_kagemusha_recursive_spend_redeem_v3",
    "connect_norito_kagemusha_recursive_spend_redeem_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v3",
    "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
    "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
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
    swift_kagemusha_sources = "\n".join(
        texts[SWIFT_SOURCE_ROOT / file_name]
        for file_name in sorted(ALLOWED_SWIFT_OFFLINE_SOURCE_FILES)
    )

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

    swift_public_type_occurrences = []
    for path in (root / SWIFT_SOURCE_ROOT).glob("*.swift"):
        relative = path.relative_to(root)
        source = texts.get(relative)
        if source is None:
            source = path.read_text(encoding="utf-8")
        swift_public_type_occurrences.extend(
            re.findall(
                r"^(?:public|open)\s+(?:(?:final|indirect)\s+)*"
                r"(?:struct|enum|class|actor|protocol|typealias)\s+"
                r"(Kagemusha\w+)",
                source,
                re.M,
            )
        )
    actual_public_type_counts = Counter(swift_public_type_occurrences)
    expected_public_type_counts = Counter({
        name: 2 if name in {
            "KagemushaNFCCardSession",
            "KagemushaNFCReader",
            "KagemushaNearbyExchange",
        } else 1
        for name in ALLOWED_SWIFT_KAGEMUSHA_PUBLIC_TYPES
    })
    if actual_public_type_counts != expected_public_type_counts:
        raise CheckFailure(
            "Swift Kagemusha public symbol multiplicity drifted: "
            f"expected={dict(sorted(expected_public_type_counts.items()))}, "
            f"actual={dict(sorted(actual_public_type_counts.items()))}"
        )
    swift_public_types = set(swift_public_type_occurrences)
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
    header_without_comments = re.sub(r"/\*[\s\S]*?\*/|//[^\n]*", "", native_header)
    native_export_patterns = (
        (
            NATIVE_RUST,
            native_rust,
            r'pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+'
            r'(connect_norito_kagemusha_[a-z0-9_]+)',
        ),
        (
            NATIVE_HEADER,
            header_without_comments,
            r'\bint32_t\s+(connect_norito_kagemusha_[a-z0-9_]+)\s*\(',
        ),
        (
            SWIFT_SOURCE_ROOT,
            swift_kagemusha_sources,
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
        "Swift frozen ABI-19 capability wire",
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
        r"let\s+expectedGates\s*=\s*\[(?P<body>[\s\S]*?)\]",
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
        swift_protocol_tests,
        "KagemushaRecursiveSpend.unavailableProofBackendGates,\n            expectedGates",
        "Swift frozen V3 missing-gate assertion",
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

if self_test:
    mutations: list[tuple[str, dict[Path, str]]] = []

    wrong_return = dict(texts)
    symbol = "connect_norito_kagemusha_recursive_spend_capabilities_v1"
    declaration = f"int32_t {symbol}("
    if wrong_return[NATIVE_HEADER].count(declaration) != 1:
        raise SystemExit("Kagemusha SDK parity self-test cannot locate exact header declaration")
    wrong_return[NATIVE_HEADER] = wrong_return[NATIVE_HEADER].replace(
        declaration,
        f"void {symbol}(",
        1,
    ) + f"\n/* {declaration} */\n"
    mutations.append(("header-wrong-return-type-comment-spoof", wrong_return))

    open_type = dict(texts)
    open_type[SWIFT_PROTOCOL] += "\nopen class KagemushaUnreviewedSurface {}\n"
    mutations.append(("unreviewed-open-swift-type", open_type))

    duplicate_type = dict(texts)
    duplicate_type[SWIFT_AMOUNT] += "\npublic struct KagemushaScaledAmount {}\n"
    mutations.append(("duplicate-swift-public-type", duplicate_type))

    for name, mutated in mutations:
        try:
            check(mutated)
        except CheckFailure:
            print(f"self-test passed: {name}")
        else:
            raise SystemExit(f"Kagemusha SDK parity self-test unexpectedly passed: {name}")

print("Kagemusha Swift/native frozen-V3 and current-V4 parity passed")
PY
