import CryptoKit
import Foundation

public enum KagemushaRecursiveSpendError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)
    case nativeBridgeUnavailable
    case proofBackendUnavailable
    case finalityTrustUnavailable

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha recursive spend V2 field: \(field)."
        case let .invalidArchive(field):
            return "Invalid Kagemusha recursive spend V2 Norito archive: \(field)."
        case .nativeBridgeUnavailable:
            return "The ABI-18 Kagemusha recursive spend V2 bridge is unavailable."
        case .proofBackendUnavailable:
            return "Kagemusha recursive spend V2 is unavailable until the branch-safe proof backend is linked."
        case .finalityTrustUnavailable:
            return "Kagemusha top-up finality is unavailable until the authenticated release trust root is wired and recursive init consumes its result."
        }
    }
}

/// Exact capability record returned by the loaded ABI-18 native bridge.
/// Wallet readiness uses this authenticated, canonical Norito record rather
/// than inferring proof availability from symbol presence.
public struct KagemushaRecursiveSpendNativeCapabilities: Equatable, Sendable {
    public let bridgeABIVersion: UInt32
    public let artifactManifestSchema: String
    public let mode: String
    public let proofBackend: String
    public let transcriptProfile: String
    public let proofEnvelopeVersion: UInt16
    public let stateBoundaryVersion: UInt16
    public let transitionCircuitID: String
    public let stateCircuitID: String
    public let maxProofBytes: UInt32
    public let proofBackendAvailable: Bool
    public let missingGates: [String]

    public init(
        bridgeABIVersion: UInt32,
        artifactManifestSchema: String,
        mode: String,
        proofBackend: String,
        transcriptProfile: String,
        proofEnvelopeVersion: UInt16,
        stateBoundaryVersion: UInt16,
        transitionCircuitID: String,
        stateCircuitID: String,
        maxProofBytes: UInt32,
        proofBackendAvailable: Bool,
        missingGates: [String]
    ) throws {
        guard bridgeABIVersion == KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion,
              artifactManifestSchema == KagemushaRecursiveSpend.artifactManifestSchema,
              mode == KagemushaRecursiveSpend.mode,
              proofBackend == KagemushaRecursiveSpend.pastaCycleBackend,
              transcriptProfile == KagemushaRecursiveSpend.pastaCycleTranscript,
              proofEnvelopeVersion == KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersion,
              stateBoundaryVersion == KagemushaRecursiveSpend.stateBoundaryVersion,
              transitionCircuitID == KagemushaRecursiveSpend.transitionEqCircuitID,
              stateCircuitID == KagemushaRecursiveSpend.stateEpCircuitID,
              maxProofBytes == UInt32(KagemushaRecursiveSpend.releaseMaximumProofBytes),
              missingGates == (proofBackendAvailable
                ? []
                : KagemushaRecursiveSpend.unavailableProofBackendGates) else {
            throw KagemushaRecursiveSpendError.invalidField("nativeCapabilities")
        }
        self.bridgeABIVersion = bridgeABIVersion
        self.artifactManifestSchema = artifactManifestSchema
        self.mode = mode
        self.proofBackend = proofBackend
        self.transcriptProfile = transcriptProfile
        self.proofEnvelopeVersion = proofEnvelopeVersion
        self.stateBoundaryVersion = stateBoundaryVersion
        self.transitionCircuitID = transitionCircuitID
        self.stateCircuitID = stateCircuitID
        self.maxProofBytes = maxProofBytes
        self.proofBackendAvailable = proofBackendAvailable
        self.missingGates = missingGates
    }
}

public enum KagemushaRecursiveSpend {
    public static let requiredNativeBridgeAbiVersion: UInt32 = 18
    public static let artifactManifestSchema =
        "kagemusha.offline.recursive_spend.artifact_manifest.v3"
    public static let mode = "recursive_spend_v2"
    public static let pastaCycleBackend = "halo2/ipa-pasta-cycle-v1"
    public static let pastaCycleTranscript = "kagemusha-pasta-cycle-poseidon-v1"
    public static let pastaCycleProofEnvelopeVersion: UInt16 = 1
    public static let stateBoundaryVersion: UInt16 = 1
    public static let transitionEqCircuitID =
        "kagemusha-recursive-spend-transition-eq-v1"
    public static let stateEpCircuitID = "kagemusha-recursive-spend-state-ep-v1"
    public static let releaseMaximumProofBytes = 4_096
    public static let artifactMaximumFileBytes = 256 * 1024 * 1024
    public static let topUpFinalityProofMaximumArchiveBytes = 2 * 1_024 * 1_024
    public static let topUpFinalityRosterMaximumArchiveBytes = 2 * 1_024 * 1_024
    public static let topUpFinalityAnchorMaximumArchiveBytes = 64 * 1_024
    public static let unavailableProofBackendGates = [
        "opposite_field_pasta_loader",
        "cross_field_poseidon_transcript",
        "two_layer_recursive_accumulator",
        "authenticated_release_envelope",
        "topup_finality_bound_init",
        "independent_cryptographic_review",
        "physical_device_performance_evidence",
    ]

    /// Canonical supporting archives consumed by the V2 request records.
    /// These are not alternate spend modes; they are authenticated inputs to
    /// the sole first-release `recursive_spend_v2` product mode.
    public static let verifiedFoldRecordBundleWireName =
        "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle"

    /// Return whether a raw capability value is the spend-again product mode.
    public static func isSpendAgainMode(_ value: String?) -> Bool {
        value == mode
    }
    public static let proofAttachmentWireName =
        "iroha_data_model::proof::ProofAttachment"
    public static let verifyingKeyRecordWireName =
        "iroha_data_model::proof::VerifyingKeyRecord"

    public static let scaledAmountWireName = wire("KagemushaScaledAmountV2")
    public static let noteWireName = wire("KagemushaSpendableNoteDescriptorV2")
    public static let recipientOutputDerivationRequestWireName =
        wire("KagemushaRecipientOutputDerivationRequestV2")
    public static let recipientOutputDerivationResultWireName =
        wire("KagemushaRecipientOutputDerivationResultV2")
    public static let branchPathWireName = wire("KagemushaRecursiveSpendBranchPathV2")
    public static let branchClaimWireName = wire("KagemushaRecursiveSpendBranchClaimV2")
    public static let recipientRequestPayloadWireName =
        wire("KagemushaRecipientPaymentRequestSigningPayloadV2")
    public static let recipientRequestWireName = wire("KagemushaRecipientPaymentRequestV2")
    public static let authorizationWireName = wire("KagemushaRequestAuthorizationV2")
    public static let artifactReferenceWireName =
        wire("KagemushaRecursiveSpendArtifactReferenceV2")
    public static let artifactManifestWireName =
        wire("KagemushaRecursiveSpendArtifactManifestV3")
    public static let nativeCapabilitiesWireName =
        wire("KagemushaRecursiveSpendNativeCapabilitiesV1")
    public static let initRequestWireName = wire("KagemushaRecursiveSpendInitRequestV2")
    public static let topUpShieldEvidenceWireName = wire("KagemushaTopUpShieldEvidenceV2")
    public static let topUpUnsignedWireName = wire("KagemushaRecursiveSpendTopUpUnsignedV2")
    public static let topUpRequestWireName = "iroha.torii.v1.offline.top_up.request"
    public static let topUpAnchorWireName = wire("KagemushaRecursiveSpendTopUpAnchorV2")
    public static let topUpAnchorRefWireName = wire("KagemushaRecursiveSpendTopUpAnchorRefV2")
    public static let topUpFinalityProofWireName = wire("KagemushaTopUpFinalityProofV2")
    public static let topUpFinalityRosterArtifactWireName =
        wire("KagemushaTopUpFinalityRosterArtifactV2")
    public static let inputBranchWireName = wire("KagemushaRecursiveSpendInputBranchV2")
    public static let appendInputWireName = wire("KagemushaRecursiveSpendAppendInputV2")
    public static let splitIntentBuildRequestWireName =
        wire("KagemushaRecursiveSpendSplitIntentBuildRequestV2")
    public static let splitIntentWireName = wire("KagemushaRecursiveSpendSplitIntentV2")
    public static let appendRequestWireName = wire("KagemushaRecursiveSpendAppendRequestV2")
    public static let branchWireName = wire("KagemushaRecursiveSpendBranchV2")
    public static let lineageModeWireName = wire("KagemushaRecursiveSpendLineageModeV2")
    public static let bundleWireName = wire("KagemushaRecursiveSpendBundleV2")
    public static let bundleSummaryWireName = wire("KagemushaRecursiveSpendBundleSummaryV2")
    public static let splitResultWireName = wire("KagemushaRecursiveSpendSplitResultV2")
    public static let peerPaymentWireName = wire("KagemushaRecursiveSpendPeerPaymentV2")
    public static let verifyRequestWireName = wire("KagemushaRecursiveSpendVerifyRequestV2")
    public static let verifyResultWireName = wire("KagemushaRecursiveSpendVerifyResultV2")
    public static let lineageNodeWireName =
        wire("KagemushaRecursiveSpendLineageNodeV2")
    public static let lineageWitnessWireName =
        wire("KagemushaRecursiveSpendLineageWitnessV2")
    public static let acknowledgementPayloadWireName =
        wire("KagemushaReceiverAcknowledgementPayloadV2")
    public static let acknowledgementWireName = wire("KagemushaReceiverAcknowledgementV2")
    public static let acknowledgementVerifyResultWireName =
        wire("KagemushaReceiverAcknowledgementVerifyResultV2")
    public static let redeemRequestWireName = "iroha.torii.v1.offline.redeem.request"
    public static let redeemUnsignedWireName = wire("KagemushaRecursiveSpendRedeemUnsignedV2")
    public static let redeemResultWireName = wire("KagemushaRecursiveSpendRedeemResultV2")
    public static let redemptionIntentWireName =
        wire("KagemushaRecursiveSpendRedemptionIntentV2")
    public static let redemptionIntentBuildRequestWireName =
        wire("KagemushaRecursiveSpendRedemptionIntentBuildRequestV2")
    public static let unshieldBindingWireName = wire("KagemushaUnshieldPublicInputsBindingV2")
    public static let redeemChangeBranchWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBranchV2")
    public static let redeemChangeBuildRequestWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBuildRequestV2")
    public static let redeemChangeBuildResultWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBuildResultV2")

    public static let reservedInitCircuitID = "kagemusha-recursive-spend-reserved-init-v2"
    public static let reservedAppendCircuitID = "kagemusha-recursive-spend-reserved-append-v2"
    public static let semanticCircuitID = "kagemusha-recursive-spend-semantic-v2"
    public static let reservedRedeemChangeCircuitID =
        "kagemusha-recursive-spend-reserved-redeem-change-v2"
    public static let lineageArtifactType = "KagemushaRecursiveSpendPastaCycleArtifactsV3"
    public static let maximumPeerTextEnvelopeBytes = 12 * 1024
    /// Largest raw archive whose unpadded base64url representation plus the
    /// six-byte `PKK2?.` prefix still fits the 12 KiB transport envelope.
    public static let maximumPeerArchiveBytes = 9_211
    public static let maximumInputNullifiers = 2
    public static let maximumBranchClaims = 2
    public static let transitionTagBytes = 24
    public static let transitionTagDomain =
        "iroha:kagemusha:v2:transition-tag:sha256-192"
    public static let semanticMaximumHops: UInt32 = 8
    public static let semanticLineageMaximumNodes = 64
    public static let semanticLineageMaximumNodeArchiveBytes = 64 * 1024
    public static let semanticLineageMaximumTotalArchiveBytes = 2 * 1024 * 1024
    public static let maximumAuthorizationTTLMilliseconds: UInt64 = 5 * 60 * 1_000

    public static let requiredProofSymbols = [
        "connect_norito_kagemusha_recursive_spend_init_v2",
        "connect_norito_kagemusha_recursive_spend_append_v2",
        "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
        "connect_norito_kagemusha_recursive_spend_verify_v2",
        "connect_norito_kagemusha_recursive_spend_redeem_v2",
    ]

    public static let requiredProtocolSymbols = [
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
    ]

    /// Complete native-symbol inventory required by V2 readiness checks.
    public static let requiredNativeSymbols = requiredProofSymbols + requiredProtocolSymbols

    public static func ensureProofBackendAvailable() throws {
        guard isProofBackendAvailable else {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }

    public static var isNativeStubAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendV2StubAvailable
            && NoritoNativeBridge.shared.hasKagemushaRecursiveSpendV2Symbols(
                requiredNativeSymbols
            )
    }

    public static func nativeCapabilities() throws
        -> KagemushaRecursiveSpendNativeCapabilities
    {
        // Capability admission is intentionally evaluated on every call. A
        // backend-enabled bridge rejects this query until one complete V3
        // artifact generation is installed; caching that pre-install failure
        // would otherwise make the backend unavailable for the process's
        // entire lifetime even after a successful installation.
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendCapabilitiesV1() else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeNativeCapabilities(archive)
    }

    /// Authoritative first-release availability. Native capability archives
    /// remain inspectable, but cannot activate a backend that this SDK release
    /// has not audited and compiled in.
    public static let isProofBackendAvailable = false

    /// Exact local production capability; Torii readiness remains an additional requirement.
    public static var isProductionAvailable: Bool {
        guard isProofBackendAvailable, isNativeStubAvailable else {
            return false
        }
        let cachedNativeCapabilities = try? nativeCapabilities()
        return cachedNativeCapabilities?.proofBackendAvailable == true
    }

    /// Select V2 only after the explicit ABI-18 proof capability is green.
    public static var preferredProductionMode: KagemushaOfflineSpendMode? {
        preferredProductionMode(
            proofBackendAvailable: isProofBackendAvailable,
            nativeStubAvailable: isNativeStubAvailable
        )
    }

    public static func preferredProductionMode(
        proofBackendAvailable: Bool,
        nativeStubAvailable: Bool
    ) -> KagemushaOfflineSpendMode? {
        proofBackendAvailable && nativeStubAvailable ? .recursiveSpend : nil
    }

    public static func initSpend(
        request: KagemushaRecursiveSpendInitRequest,
        rosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> Data {
        try verifyTopUpFinality(
            proof: request.topUpFinalityProof,
            rosterArtifact: rosterArtifact,
            anchor: request.topUpAnchor,
            manifest: manifest
        )
        let requestArchive = try request.noritoEncoded()
        try ensureProofBackendAvailable()
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendInitV2(
                requestArchive: requestArchive
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }

    public static func topUpSpend(requestArchive: Data) throws -> Data {
        try callSingleArchive(requestArchive, schema: topUpRequestWireName) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendTopUpV2(requestArchive: requestArchive)
        }
    }

    public static func topUpSpend(
        request: KagemushaRecursiveSpendTopUpRequest
    ) throws -> Data {
        try topUpSpend(requestArchive: request.noritoEncoded())
    }

    /// Verify chain finality before admitting an initialized top-up branch to
    /// the local spendable set.
    public static func verifyTopUpFinality(
        proof: KagemushaTopUpFinalityProofArchive,
        rosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive,
        anchor: KagemushaRecursiveSpendTopUpAnchor,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws {
        do {
            guard try NoritoNativeBridge.shared.kagemushaTopUpFinalityVerifyV2(
                proofArchive: proof.noritoArchive,
                rosterArtifactArchive: rosterArtifact.noritoArchive,
                anchorArchive: anchor.archive,
                manifestArchive: manifest.noritoArchive,
                expectedManifestSHA256: manifest.sha256
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendError.finalityTrustUnavailable
        }
    }

    public static func appendSpend(
        requestArchive: Data,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data {
        try requireArchive(requestArchive, schema: appendRequestWireName, field: "requestArchive")
        guard verifiedAtMilliseconds == signedRecipientRequest.verifiedAtMilliseconds else {
            throw KagemushaRecursiveSpendError.invalidField("verifiedAtMilliseconds")
        }
        try ensureProofBackendAvailable()
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppendV2(
                requestArchive: requestArchive,
                recipientRequestArchive: signedRecipientRequest.request.archive,
                verifiedAtMilliseconds: verifiedAtMilliseconds
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }

    public static func verifySpend(requestArchive: Data) throws -> Data {
        try callSingleArchive(requestArchive, schema: verifyRequestWireName) {
            try ensureProofBackendAvailable()
            return try NoritoNativeBridge.shared.kagemushaRecursiveSpendVerifyV2(
                requestArchive: requestArchive
            )
        }
    }

    public static func proveRedeemChange(
        request: KagemushaRecursiveSpendRedeemChangeBuildRequest
    ) throws -> KagemushaRecursiveSpendRedeemChangeBuildResult {
        let archive = try request.noritoEncoded()
        try ensureProofBackendAvailable()
        do {
            guard let result = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendRedeemChangeV2(requestArchive: archive) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return try KagemushaRecursiveSpendCodecs.decodeRedeemChangeBuildResult(result)
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }

    public static func redeemSpend(requestArchive: Data) throws -> Data {
        try callSingleArchive(requestArchive, schema: redeemRequestWireName) {
            try ensureProofBackendAvailable()
            return try NoritoNativeBridge.shared.kagemushaRecursiveSpendRedeemV2(
                requestArchive: requestArchive
            )
        }
    }

    public static func redeemSpend(
        request: KagemushaRecursiveSpendRedeemRequest
    ) throws -> Data {
        try redeemSpend(requestArchive: request.noritoEncoded())
    }

    static func requireArchive(_ archive: Data, schema: String, field: String) throws {
        guard !archive.isEmpty,
              archive.count <= artifactMaximumFileBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0,
              !frame.payload.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
    }

    static func requireNonzeroFixed32(_ value: Data, field: String) throws {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    static func transitionTag(for transitionBinding: Data) throws -> Data {
        try requireNonzeroFixed32(transitionBinding, field: "transitionBinding")
        var preimage = Data(transitionTagDomain.utf8)
        preimage.append(0)
        preimage.append(transitionBinding)
        let tag = Data(SHA256.hash(data: preimage).prefix(transitionTagBytes))
        guard tag.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("transitionTag")
        }
        return tag
    }

    static func requirePortableText(_ value: String, field: String, maximum: Int = 128) throws {
        guard !value.isEmpty,
              value.count <= maximum,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    static func validateBranchClaims(
        _ claims: [KagemushaRecursiveSpendBranchClaim]
    ) throws {
        guard (1...maximumBranchClaims).contains(claims.count) else {
            throw KagemushaRecursiveSpendError.invalidField("branchClaims")
        }
        for index in claims.indices where index > claims.startIndex {
            let claim = claims[index]
            guard claims[index - 1].path.canonicallyPrecedes(claim.path) else {
                throw KagemushaRecursiveSpendError.invalidField("branchClaims.order")
            }
            guard !claims[..<index].contains(where: { $0.path.conflicts(with: claim.path) }) else {
                throw KagemushaRecursiveSpendError.invalidField("branchClaims.conflict")
            }
            for previous in claims[..<index] {
                guard previous.path.lineageRoot == claim.path.lineageRoot else { continue }
                let sharedDepth = min(previous.path.depth, claim.path.depth)
                for parentDepth in 0..<sharedDepth
                    where previous.path.hasSamePrefix(as: claim.path, depth: parentDepth)
                {
                    guard previous.transitionTags[Int(parentDepth)]
                        == claim.transitionTags[Int(parentDepth)] else {
                        throw KagemushaRecursiveSpendError.invalidField(
                            "branchClaims.transitionChoice"
                        )
                    }
                }
            }
        }
    }

    private static func wire(_ type: String) -> String {
        "iroha_data_model::offline::model::\(type)"
    }

    private static func callSingleArchive(
        _ requestArchive: Data,
        schema: String,
        body: () throws -> Data?
    ) throws -> Data {
        try requireArchive(requestArchive, schema: schema, field: "requestArchive")
        do {
            guard let output = try body() else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }
}

public struct KagemushaPublicKey: Equatable, Hashable, Sendable {
    public let algorithm: UInt8
    public let payload: Data

    public init(algorithm: UInt8 = 0, payload: Data) throws {
        guard !payload.isEmpty, payload.count <= 8_192 else {
            throw KagemushaRecursiveSpendError.invalidField("receiverPublicKey")
        }
        if algorithm == 0, payload.count != 32 {
            throw KagemushaRecursiveSpendError.invalidField("receiverPublicKey.ed25519")
        }
        self.algorithm = algorithm
        self.payload = Data(payload)
    }

    public func receiverKeyReference() throws -> Data {
        guard let reference = try NoritoNativeBridge.shared.kagemushaReceiverKeyReferenceV2(
            algorithm: algorithm,
            publicKey: payload
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            reference,
            field: "recipientKeyReference"
        )
        return reference
    }
}

public struct KagemushaSpendableNoteDescriptor: Equatable, Hashable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let amount: KagemushaScaledAmount

    public init(
        chainID: String,
        assetDefinitionID: String,
        noteCommitment: Data,
        spendNullifier: Data,
        amount: KagemushaScaledAmount
    ) throws {
        try KagemushaRecursiveSpend.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            noteCommitment,
            field: "noteCommitment"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            spendNullifier,
            field: "spendNullifier"
        )
        guard noteCommitment != spendNullifier else {
            throw KagemushaRecursiveSpendError.invalidField("spendNullifier")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.noteCommitment = Data(noteCommitment)
        self.spendNullifier = Data(spendNullifier)
        self.amount = amount
    }
}

public struct KagemushaRecipientOutputDerivationRequest: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let requestID: Data

    public init(
        chainID: String,
        assetDefinitionID: String,
        amount: KagemushaScaledAmount,
        requestID: Data
    ) throws {
        try KagemushaRecursiveSpend.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(requestID, field: "requestID")
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.amount = amount
        self.requestID = Data(requestID)
    }

    public func derive(
        receiverSpendSecret: Data
    ) throws -> KagemushaRecipientOutputDerivationResult {
        guard receiverSpendSecret.count == 32,
              receiverSpendSecret.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("receiverSpendSecret")
        }
        let requestArchive = try KagemushaRecursiveSpendCodecs
            .encodeRecipientOutputDerivationRequest(self)
        guard let resultArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientOutputDeriveV2(
                requestArchive: requestArchive,
                receiverSpendSecret: receiverSpendSecret
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeRecipientOutputDerivationResult(
            resultArchive,
            request: self
        )
    }
}

public struct KagemushaRecipientOutputDerivationResult: Equatable, Sendable {
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    public let recipientOutputProverMaterial: Data

    init(
        recipientOutput: KagemushaSpendableNoteDescriptor,
        recipientOutputProverMaterial: Data,
        request: KagemushaRecipientOutputDerivationRequest
    ) throws {
        guard recipientOutput.chainID == request.chainID,
              recipientOutput.assetDefinitionID == request.assetDefinitionID,
              recipientOutput.amount == request.amount,
              !recipientOutputProverMaterial.isEmpty,
              recipientOutputProverMaterial.count <= 4 * 1_024 else {
            throw KagemushaRecursiveSpendError.invalidField(
                "recipientOutputProverMaterial"
            )
        }
        self.recipientOutput = recipientOutput
        self.recipientOutputProverMaterial = Data(recipientOutputProverMaterial)
    }
}

public enum KagemushaRecursiveSpendBranch: UInt32, Equatable, Sendable {
    case recipient = 0
    case change = 1
}

public enum KagemushaRecursiveSpendLineageMode: UInt32, Equatable, Sendable {
    case reserved = 0
    case semantic = 1
}

public struct KagemushaRecursiveSpendBranchPath: Equatable, Hashable, Sendable {
    public static let maximumDepth: UInt8 = 64
    public let lineageRoot: Data
    public let depth: UInt8
    public let pathBits: Data

    public init(lineageRoot: Data, depth: UInt8, pathBits: Data) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(lineageRoot, field: "lineageRoot")
        guard depth <= Self.maximumDepth, pathBits.count == 8 else {
            throw KagemushaRecursiveSpendError.invalidField("branchPath")
        }
        let unused = 64 - Int(depth)
        if unused > 0 {
            let value = pathBits.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
            let mask = unused == 64 ? UInt64.max : (UInt64(1) << UInt64(unused)) - 1
            guard value & mask == 0 else {
                throw KagemushaRecursiveSpendError.invalidField("branchPath.pathBits")
            }
        }
        self.lineageRoot = Data(lineageRoot)
        self.depth = depth
        self.pathBits = Data(pathBits)
    }

    public static func root(_ lineageRoot: Data) throws -> Self {
        try Self(lineageRoot: lineageRoot, depth: 0, pathBits: Data(repeating: 0, count: 8))
    }

    var parent: Self? {
        guard depth > 0 else { return nil }
        var bytes = Array(pathBits)
        let bitIndex = Int(depth - 1)
        bytes[bitIndex / 8] &= ~(1 << UInt8(7 - bitIndex % 8))
        return try? Self(
            lineageRoot: lineageRoot,
            depth: depth - 1,
            pathBits: Data(bytes)
        )
    }

    func isExactSibling(of other: Self) -> Bool {
        guard depth > 0,
              depth == other.depth,
              lineageRoot == other.lineageRoot,
              parent == other.parent else {
            return false
        }
        let bitIndex = Int(depth - 1)
        let mask = UInt8(1 << UInt8(7 - bitIndex % 8))
        return (pathBits[bitIndex / 8] & mask) != (other.pathBits[bitIndex / 8] & mask)
    }

    func canonicallyPrecedes(_ other: Self) -> Bool {
        if lineageRoot != other.lineageRoot {
            return lineageRoot.lexicographicallyPrecedes(other.lineageRoot)
        }
        if depth != other.depth { return depth < other.depth }
        return pathBits.lexicographicallyPrecedes(other.pathBits)
    }

    func isPrefix(of other: Self) -> Bool {
        guard lineageRoot == other.lineageRoot, depth <= other.depth else { return false }
        let fullBytes = Int(depth / 8)
        guard pathBits.prefix(fullBytes) == other.pathBits.prefix(fullBytes) else { return false }
        let partialBits = Int(depth % 8)
        guard partialBits > 0 else { return true }
        let mask = UInt8.max << UInt8(8 - partialBits)
        return pathBits[fullBytes] & mask == other.pathBits[fullBytes] & mask
    }

    func conflicts(with other: Self) -> Bool {
        isPrefix(of: other) || other.isPrefix(of: self)
    }

    func hasSamePrefix(as other: Self, depth prefixDepth: UInt8) -> Bool {
        guard lineageRoot == other.lineageRoot,
              prefixDepth <= depth,
              prefixDepth <= other.depth else {
            return false
        }
        let fullBytes = Int(prefixDepth / 8)
        guard pathBits.prefix(fullBytes) == other.pathBits.prefix(fullBytes) else {
            return false
        }
        let partialBits = Int(prefixDepth % 8)
        guard partialBits > 0 else { return true }
        let mask = UInt8.max << UInt8(8 - partialBits)
        return pathBits[fullBytes] & mask == other.pathBits[fullBytes] & mask
    }
}

/// Replay-safe claim for one independently spendable lineage leaf.
///
/// The exact `path.depth` tags select the transition used at every ancestor
/// edge. Each tag is the domain-separated 192-bit prefix of the producing
/// proof-bound 256-bit transition digest. Swift exposes one `Data` per tag;
/// Norito concatenates them into a single exact-depth byte vector.
public struct KagemushaRecursiveSpendBranchClaim: Equatable, Hashable, Sendable {
    public let path: KagemushaRecursiveSpendBranchPath
    public let transitionTags: [Data]

    public init(
        path: KagemushaRecursiveSpendBranchPath,
        transitionTags: [Data]
    ) throws {
        guard transitionTags.count == Int(path.depth),
              transitionTags.allSatisfy({
                  $0.count == KagemushaRecursiveSpend.transitionTagBytes
                    && $0.contains(where: { $0 != 0 })
              }) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "branchClaim.transitionTags"
            )
        }
        self.path = path
        self.transitionTags = transitionTags.map { Data($0) }
    }

    public static func root(lineageRoot: Data) throws -> Self {
        try Self(
            path: KagemushaRecursiveSpendBranchPath.root(lineageRoot),
            transitionTags: []
        )
    }
}

public enum KagemushaRecursiveSpendArtifactRole: UInt32, Equatable, Sendable {
    case transferProver = 0
    case unshieldProver = 1
    case lineageInitProver = 2
    case lineageAppendProver = 3
    case redeemChangeProver = 4
}

public struct KagemushaRecursiveSpendArtifactReference: Equatable, Sendable {
    public let role: KagemushaRecursiveSpendArtifactRole
    public let generation: String
    public let circuitID: String
    public let artifactType: String
    public let sizeBytes: UInt64
    public let sha256: Data

    public init(
        role: KagemushaRecursiveSpendArtifactRole,
        generation: String,
        circuitID: String,
        artifactType: String = KagemushaRecursiveSpend.lineageArtifactType,
        sizeBytes: UInt64,
        sha256: Data
    ) throws {
        try KagemushaRecursiveSpend.requirePortableText(generation, field: "generation")
        try KagemushaRecursiveSpend.requirePortableText(circuitID, field: "circuitID")
        guard sizeBytes > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("sizeBytes")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(sha256, field: "sha256")
        switch role {
        case .lineageInitProver:
            guard circuitID == KagemushaRecursiveSpend.reservedInitCircuitID,
                  artifactType == KagemushaRecursiveSpend.lineageArtifactType else {
                throw KagemushaRecursiveSpendError.invalidField("lineageArtifact")
            }
        case .lineageAppendProver:
            guard circuitID == KagemushaRecursiveSpend.reservedAppendCircuitID,
                  artifactType == KagemushaRecursiveSpend.lineageArtifactType else {
                throw KagemushaRecursiveSpendError.invalidField("lineageArtifact")
            }
        case .redeemChangeProver:
            guard circuitID == KagemushaRecursiveSpend.reservedRedeemChangeCircuitID,
                  artifactType == KagemushaRecursiveSpend.lineageArtifactType else {
                throw KagemushaRecursiveSpendError.invalidField("lineageArtifact")
            }
        default:
            throw KagemushaRecursiveSpendError.invalidField("lineageArtifact.role")
        }
        self.role = role
        self.generation = generation
        self.circuitID = circuitID
        self.artifactType = artifactType
        self.sizeBytes = sizeBytes
        self.sha256 = Data(sha256)
    }
}

public struct KagemushaRecipientPaymentRequestSigningPayload: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let recipient: String
    public let recipientKeyReference: Data
    public let receiverDeviceID: String
    public let receiverPublicKey: KagemushaPublicKey
    public let requestID: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    public let recipientOutputProverMaterial: Data

    public init(
        chainID: String,
        assetDefinitionID: String,
        amount: KagemushaScaledAmount,
        recipient: String,
        recipientKeyReference: Data,
        receiverDeviceID: String,
        receiverPublicKey: KagemushaPublicKey,
        requestID: Data,
        issuedAtMilliseconds: UInt64,
        expiresAtMilliseconds: UInt64,
        recipientOutput: KagemushaSpendableNoteDescriptor,
        recipientOutputProverMaterial: Data
    ) throws {
        try KagemushaRecursiveSpend.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            recipientKeyReference,
            field: "recipientKeyReference"
        )
        try KagemushaRecursiveSpend.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(requestID, field: "requestID")
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds,
              recipientOutput.chainID == chainID,
              recipientOutput.assetDefinitionID == assetDefinitionID,
              recipientOutput.amount == amount,
              !recipientOutputProverMaterial.isEmpty,
              recipientOutputProverMaterial.count <= 4 * 1024 else {
            throw KagemushaRecursiveSpendError.invalidField("recipientRequest")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.amount = amount
        self.recipient = recipient
        self.recipientKeyReference = Data(recipientKeyReference)
        self.receiverDeviceID = receiverDeviceID
        self.receiverPublicKey = receiverPublicKey
        self.requestID = Data(requestID)
        self.issuedAtMilliseconds = issuedAtMilliseconds
        self.expiresAtMilliseconds = expiresAtMilliseconds
        self.recipientOutput = recipientOutput
        self.recipientOutputProverMaterial = Data(recipientOutputProverMaterial)
    }

    public func signingBytes() throws -> Data {
        let archive = try KagemushaRecursiveSpendCodecs.encodeRecipientRequestPayload(self)
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestSigningBytesV2(payloadArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }

    public func signed(signature: Data) throws -> KagemushaRecipientPaymentRequest {
        let payloadArchive = try KagemushaRecursiveSpendCodecs.encodeRecipientRequestPayload(self)
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestCreateV2(
                payloadArchive: payloadArchive,
                signature: signature
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecipientPaymentRequest(
            payload: self,
            signature: signature,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecipientPaymentRequest: Equatable, Sendable {
    public let payload: KagemushaRecipientPaymentRequestSigningPayload
    public let signature: Data
    public let archive: Data

    init(
        payload: KagemushaRecipientPaymentRequestSigningPayload,
        signature: Data,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "recipientRequest"
        )
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("signature")
        }
        self.payload = payload
        self.signature = Data(signature)
        self.archive = Data(archive)
    }

    public func verified(atMilliseconds: UInt64) throws -> KagemushaVerifiedRecipientPaymentRequest {
        guard let digest = try NoritoNativeBridge.shared.kagemushaRecipientPaymentRequestVerifyV2(
            requestArchive: archive,
            verifiedAtMilliseconds: atMilliseconds
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(digest, field: "requestDigest")
        return KagemushaVerifiedRecipientPaymentRequest(
            request: self,
            digest: digest,
            verifiedAtMilliseconds: atMilliseconds
        )
    }
}

public struct KagemushaVerifiedRecipientPaymentRequest: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let digest: Data
    public let verifiedAtMilliseconds: UInt64

    init(
        request: KagemushaRecipientPaymentRequest,
        digest: Data,
        verifiedAtMilliseconds: UInt64
    ) {
        self.request = request
        self.digest = Data(digest)
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
    }
}

/// Unsigned fields of the self-contained account/device authorization used by
/// top-up and redemption. Private key material stays with the caller-provided
/// signing closure and never enters this model.
public struct KagemushaRequestAuthorizationFields: Equatable, Sendable {
    public let authority: String
    public let deviceID: String
    public let operationID: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let nonce: Data
    public let payloadDigest: Data
    public let appAttestEvidenceSHA256: Data?
    public let appAttestEvidence: Data?

    public init(
        authority: String,
        deviceID: String,
        operationID: Data,
        issuedAtMilliseconds: UInt64,
        expiresAtMilliseconds: UInt64,
        nonce: Data,
        payloadDigest: Data,
        appAttestEvidenceSHA256: Data? = nil,
        appAttestEvidence: Data? = nil
    ) throws {
        _ = try AccountAddress.parseEncoded(authority, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpend.requirePortableText(deviceID, field: "deviceID")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(nonce, field: "nonce")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(payloadDigest, field: "payloadDigest")
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.expiry")
        }
        switch (appAttestEvidenceSHA256, appAttestEvidence) {
        case (nil, nil):
            break
        case let (.some(digest), .some(evidence)):
            try KagemushaRecursiveSpend.requireNonzeroFixed32(
                digest,
                field: "appAttestEvidenceSHA256"
            )
            guard !evidence.isEmpty, evidence.count <= 16 * 1024 else {
                throw KagemushaRecursiveSpendError.invalidField("appAttestEvidence")
            }
        default:
            throw KagemushaRecursiveSpendError.invalidField("appAttestEvidence")
        }
        self.authority = authority
        self.deviceID = deviceID
        self.operationID = Data(operationID)
        self.issuedAtMilliseconds = issuedAtMilliseconds
        self.expiresAtMilliseconds = expiresAtMilliseconds
        self.nonce = Data(nonce)
        self.payloadDigest = Data(payloadDigest)
        self.appAttestEvidenceSHA256 = appAttestEvidenceSHA256.map { Data($0) }
        self.appAttestEvidence = appAttestEvidence.map { Data($0) }
    }

    public func signingBytes() throws -> Data {
        let template = try KagemushaRecursiveSpendCodecs.encodeAuthorizationTemplate(self)
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaRequestAuthorizationSigningBytesV2(templateArchive: template) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }

    public func signed(signature: Data) throws -> KagemushaRequestAuthorization {
        let template = try KagemushaRecursiveSpendCodecs.encodeAuthorizationTemplate(self)
        guard let archive = try NoritoNativeBridge.shared.kagemushaRequestAuthorizationCreateV2(
            templateArchive: template,
            signature: signature
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRequestAuthorization(
            fields: self,
            signature: signature,
            archive: archive
        )
    }
}

public struct KagemushaRequestAuthorization: Equatable, Sendable {
    public let fields: KagemushaRequestAuthorizationFields
    public let signature: Data
    public let archive: Data

    init(fields: KagemushaRequestAuthorizationFields, signature: Data, archive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.authorizationWireName,
            field: "authorization"
        )
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.signature")
        }
        self.fields = fields
        self.signature = Data(signature)
        self.archive = Data(archive)
    }
}

public struct KagemushaTopUpShieldEvidence: Equatable, Sendable {
    public let initialRoot: Data
    public let finalizedRoot: Data
    public let leafIndex: UInt32
    public let proofAttachment: Data

    public init(
        initialRoot: Data,
        finalizedRoot: Data,
        leafIndex: UInt32,
        proofAttachment: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(initialRoot, field: "initialRoot")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(finalizedRoot, field: "finalizedRoot")
        guard initialRoot != finalizedRoot, leafIndex < (1 << 16) else {
            throw KagemushaRecursiveSpendError.invalidField("shieldEvidence")
        }
        try KagemushaRecursiveSpend.requireArchive(
            proofAttachment,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "shieldEvidence.proofAttachment"
        )
        self.initialRoot = Data(initialRoot)
        self.finalizedRoot = Data(finalizedRoot)
        self.leafIndex = leafIndex
        self.proofAttachment = Data(proofAttachment)
    }
}

public struct KagemushaRecursiveSpendInitRequest: Equatable, Sendable {
    public let topUpAnchor: KagemushaRecursiveSpendTopUpAnchor
    public let topUpFinalityProof: KagemushaTopUpFinalityProofArchive
    public let lineageMode: KagemushaRecursiveSpendLineageMode
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReference?

    public init(
        topUpAnchor: KagemushaRecursiveSpendTopUpAnchor,
        topUpFinalityProof: KagemushaTopUpFinalityProofArchive,
        lineageMode: KagemushaRecursiveSpendLineageMode,
        lineageArtifact: KagemushaRecursiveSpendArtifactReference? = nil
    ) throws {
        guard lineageArtifact.map({ topUpAnchor.artifactGeneration == $0.generation }) ?? true else {
            throw KagemushaRecursiveSpendError.invalidField("topUpAnchor.finality")
        }
        switch (lineageMode, lineageArtifact) {
        case let (.reserved, .some(artifact)) where artifact.role == .lineageInitProver:
            break
        case (.semantic, nil):
            break
        default:
            throw KagemushaRecursiveSpendError.invalidField("lineageArtifact")
        }
        self.topUpAnchor = topUpAnchor
        self.topUpFinalityProof = topUpFinalityProof
        self.lineageMode = lineageMode
        self.lineageArtifact = lineageArtifact
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeInitRequest(self)
    }

    public static func decode(_ archive: Data) throws -> Self {
        try KagemushaRecursiveSpendCodecs.decodeInitRequest(archive)
    }
}

public struct KagemushaRecursiveSpendTopUpUnsigned: Equatable, Sendable {
    public let assetID: String
    public let amount: KagemushaScaledAmount
    public let currentNote: KagemushaSpendableNoteDescriptor
    public let shieldEvidence: KagemushaTopUpShieldEvidence
    public let artifactGeneration: String
    public let operationID: Data

    public init(
        assetID: String,
        amount: KagemushaScaledAmount,
        currentNote: KagemushaSpendableNoteDescriptor,
        shieldEvidence: KagemushaTopUpShieldEvidence,
        artifactGeneration: String,
        operationID: Data
    ) throws {
        let canonicalAssetID = try KagemushaRecursiveSpendCodecs.canonicalAssetID(assetID)
        let assetParts = canonicalAssetID.split(separator: "#", omittingEmptySubsequences: false)
        try KagemushaRecursiveSpend.requirePortableText(
            artifactGeneration,
            field: "artifactGeneration"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        guard currentNote.amount == amount,
              !assetParts.isEmpty,
              String(assetParts[0]) == currentNote.assetDefinitionID else {
            throw KagemushaRecursiveSpendError.invalidField("topUpUnsigned")
        }
        self.assetID = canonicalAssetID
        self.amount = amount
        self.currentNote = currentNote
        self.shieldEvidence = shieldEvidence
        self.artifactGeneration = artifactGeneration
        self.operationID = Data(operationID)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeTopUpUnsigned(self)
    }

    public func authorizationPayloadDigest() throws -> Data {
        let archive = try noritoEncoded()
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpUnsignedPayloadDigestV2(
                unsignedArchive: archive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            digest,
            field: "topUpUnsigned.payloadDigest"
        )
        return digest
    }

    public func finalize(
        authorization: KagemushaRequestAuthorization
    ) throws -> KagemushaRecursiveSpendTopUpRequest {
        let unsignedArchive = try noritoEncoded()
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == (try authorizationPayloadDigest()) else {
            throw KagemushaRecursiveSpendError.invalidField("authorization")
        }
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpFinalizeRequestV2(
                unsignedArchive: unsignedArchive,
                authorizationArchive: authorization.archive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendTopUpRequest(
            unsigned: self,
            authorization: authorization,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecursiveSpendTopUpRequest: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendTopUpUnsigned
    public let authorization: KagemushaRequestAuthorization
    public let archive: Data

    public var assetID: String { unsigned.assetID }
    public var amount: KagemushaScaledAmount { unsigned.amount }
    public var currentNote: KagemushaSpendableNoteDescriptor { unsigned.currentNote }
    public var shieldEvidence: KagemushaTopUpShieldEvidence { unsigned.shieldEvidence }
    public var artifactGeneration: String { unsigned.artifactGeneration }
    public var operationID: Data { unsigned.operationID }

    init(
        unsigned: KagemushaRecursiveSpendTopUpUnsigned,
        authorization: KagemushaRequestAuthorization,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            field: "topUpRequest"
        )
        self.unsigned = unsigned
        self.authorization = authorization
        self.archive = Data(archive)
        guard try KagemushaRecursiveSpendCodecs.encodeTopUpRequest(self) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpRequest.canonical")
        }
    }

    public func noritoEncoded() -> Data { archive }
}

/// Immutable chain-finality receipt consumed by the local init prover. A
/// wallet must never construct hop-0 cash from the pre-finality top-up request.
public struct KagemushaRecursiveSpendTopUpAnchor: Equatable, Sendable {
    public let version: UInt16
    public let chainID: String
    public let payer: String
    public let assetID: String
    public let assetScale: UInt32
    public let amount: KagemushaScaledAmount
    public let initialRoot: Data
    public let finalizedRoot: Data
    public let shieldLeafIndex: UInt32
    public let currentNote: KagemushaSpendableNoteDescriptor
    public let topUpOperationID: Data
    public let shieldVerifierID: String
    public let shieldVerifierCommitment: Data
    public let artifactGeneration: String
    public let finalizedHeight: UInt64
    public let finalizedTransactionHash: Data
    public let anchorDigest: Data
    public let archive: Data

    init(
        version: UInt16,
        chainID: String,
        payer: String,
        assetID: String,
        assetScale: UInt32,
        amount: KagemushaScaledAmount,
        initialRoot: Data,
        finalizedRoot: Data,
        shieldLeafIndex: UInt32,
        currentNote: KagemushaSpendableNoteDescriptor,
        topUpOperationID: Data,
        shieldVerifierID: String,
        shieldVerifierCommitment: Data,
        artifactGeneration: String,
        finalizedHeight: UInt64,
        finalizedTransactionHash: Data,
        anchorDigest: Data,
        archive: Data
    ) throws {
        let canonicalAssetID: String
        do {
            canonicalAssetID = try KagemushaRecursiveSpendCodecs.canonicalAssetID(assetID)
        } catch {
            throw KagemushaRecursiveSpendError.invalidField("topUpAnchor")
        }
        let assetParts = canonicalAssetID.split(
            separator: "#",
            omittingEmptySubsequences: false
        )
        guard version == 2,
              canonicalAssetID == assetID,
              (assetParts.count == 2 || assetParts.count == 3),
              String(assetParts[0]) == currentNote.assetDefinitionID,
              String(assetParts[1]) == payer,
              assetScale == amount.scale,
              currentNote.amount == amount,
              currentNote.chainID == chainID,
              shieldLeafIndex < (1 << 16),
              initialRoot != finalizedRoot,
              finalizedHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("topUpAnchor")
        }
        try KagemushaRecursiveSpend.requirePortableText(chainID, field: "chainID")
        try KagemushaRecursiveSpend.requirePortableText(payer, field: "payer")
        try KagemushaRecursiveSpend.requirePortableText(assetID, field: "assetID")
        try KagemushaRecursiveSpend.requirePortableText(
            shieldVerifierID,
            field: "shieldVerifierID"
        )
        try KagemushaRecursiveSpend.requirePortableText(
            artifactGeneration,
            field: "artifactGeneration"
        )
        for (field, value) in [
            ("initialRoot", initialRoot),
            ("finalizedRoot", finalizedRoot),
            ("topUpOperationID", topUpOperationID),
            ("shieldVerifierCommitment", shieldVerifierCommitment),
            ("finalizedTransactionHash", finalizedTransactionHash),
            ("anchorDigest", anchorDigest),
        ] {
            try KagemushaRecursiveSpend.requireNonzeroFixed32(value, field: field)
        }
        self.version = version
        self.chainID = chainID
        self.payer = payer
        self.assetID = assetID
        self.assetScale = assetScale
        self.amount = amount
        self.initialRoot = Data(initialRoot)
        self.finalizedRoot = Data(finalizedRoot)
        self.shieldLeafIndex = shieldLeafIndex
        self.currentNote = currentNote
        self.topUpOperationID = Data(topUpOperationID)
        self.shieldVerifierID = shieldVerifierID
        self.shieldVerifierCommitment = Data(shieldVerifierCommitment)
        self.artifactGeneration = artifactGeneration
        self.finalizedHeight = finalizedHeight
        self.finalizedTransactionHash = Data(finalizedTransactionHash)
        self.anchorDigest = Data(anchorDigest)
        self.archive = Data(archive)
    }

    public static func decode(_ archive: Data) throws -> Self {
        try KagemushaRecursiveSpendCodecs.decodeTopUpAnchor(archive)
    }

    public func compactReference() throws -> KagemushaRecursiveSpendTopUpAnchorRef {
        try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: topUpOperationID,
            anchorDigest: anchorDigest
        )
    }
}

/// Compact chain-resolvable top-up identity carried by peer bundles.
public struct KagemushaRecursiveSpendTopUpAnchorRef: Equatable, Hashable, Sendable {
    public let topUpOperationID: Data
    public let anchorDigest: Data

    public init(topUpOperationID: Data, anchorDigest: Data) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            topUpOperationID,
            field: "topUpAnchorRef.topUpOperationID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            anchorDigest,
            field: "topUpAnchorRef.anchorDigest"
        )
        self.topUpOperationID = Data(topUpOperationID)
        self.anchorDigest = Data(anchorDigest)
    }
}

/// Canonical finality proof returned by Torii for one applied top-up.
///
/// Consensus and Merkle internals intentionally remain opaque to wallet code;
/// the native verifier decodes the typed Rust contract and rejects any
/// non-canonical archive before the branch can become spendable.
public struct KagemushaTopUpFinalityProofArchive: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityProofMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpFinalityProof")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            field: "topUpFinalityProof"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Content-addressed validator-roster trust artifact prefetched while online.
///
/// The artifact is opaque to application code. Native verification validates
/// its exact authenticated SHA-256, chain id, activation windows, ordered BLS
/// keys, PoPs, and generation.
public struct KagemushaTopUpFinalityRosterArtifactArchive: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityRosterMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "topUpFinalityRosterArtifact"
            )
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
            field: "topUpFinalityRosterArtifact"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Canonical authenticated V3 release manifest passed opaquely to the native
/// artifact loader. Application code never derives proof parameters from it.
public struct KagemushaRecursiveSpendArtifactManifestArchive: Equatable, Sendable {
    public let noritoArchive: Data
    public let sha256: Data

    public init(noritoArchive: Data, expectedSHA256: Data) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedSHA256,
            field: "artifactManifest.sha256"
        )
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.artifactManifestWireName,
            field: "artifactManifest"
        )
        guard Data(SHA256.hash(data: noritoArchive)) == expectedSHA256 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactManifest.sha256")
        }
        self.noritoArchive = Data(noritoArchive)
        self.sha256 = Data(expectedSHA256)
    }
}

public struct KagemushaRecursiveSpendInputBranch: Equatable, Sendable {
    public let bundleDigest: Data
    public let inputNote: KagemushaSpendableNoteDescriptor
    public let branchClaims: [KagemushaRecursiveSpendBranchClaim]
    public let inputRoot: Data
    public let proofStepCount: UInt32
    public let peerHopCount: UInt32

    init(
        bundleDigest: Data,
        inputNote: KagemushaSpendableNoteDescriptor,
        branchClaims: [KagemushaRecursiveSpendBranchClaim],
        inputRoot: Data,
        proofStepCount: UInt32,
        peerHopCount: UInt32
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            bundleDigest,
            field: "input.bundleDigest"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            inputRoot,
            field: "input.inputRoot"
        )
        try KagemushaRecursiveSpend.validateBranchClaims(branchClaims)
        guard proofStepCount > 0,
              peerHopCount <= UInt32(KagemushaRecursiveSpendBranchPath.maximumDepth) else {
            throw KagemushaRecursiveSpendError.invalidField("input.hopCount")
        }
        self.bundleDigest = Data(bundleDigest)
        self.inputNote = inputNote
        self.branchClaims = branchClaims
        self.inputRoot = Data(inputRoot)
        self.proofStepCount = proofStepCount
        self.peerHopCount = peerHopCount
    }
}

/// Typed native-factory request for a split intent. Parent provenance is not
/// accepted from Swift; it is derived from the opaque bundle archives.
public struct KagemushaRecursiveSpendSplitIntentBuildRequest: Equatable, Sendable {
    public let previousBundles: [KagemushaRecursiveSpendBundle]
    public let outputArtifactGeneration: String
    public let transferAmount: KagemushaScaledAmount
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    public let changeOutput: KagemushaSpendableNoteDescriptor?
    public let recipientRequestDigest: Data
    public let operationID: Data

    public init(
        previousBundles: [KagemushaRecursiveSpendBundle],
        outputArtifactGeneration: String,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptor,
        changeOutput: KagemushaSpendableNoteDescriptor? = nil,
        recipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        operationID: Data
    ) throws {
        guard (1...2).contains(previousBundles.count) else {
            throw KagemushaRecursiveSpendError.invalidField("previousBundles")
        }
        for (previous, current) in zip(previousBundles, previousBundles.dropFirst()) {
            guard previous.summary.bundleDigest.lexicographicallyPrecedes(
                current.summary.bundleDigest
            ) else {
                throw KagemushaRecursiveSpendError.invalidField("previousBundles.order")
            }
        }
        try KagemushaRecursiveSpend.requirePortableText(
            outputArtifactGeneration,
            field: "outputArtifactGeneration"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        let request = recipientRequest.request.payload
        guard request.amount == transferAmount,
              request.recipientOutput == recipientOutput,
              previousBundles.allSatisfy({
                  $0.summary.assetDefinitionID == request.assetDefinitionID
                    && $0.summary.amount.scale == request.amount.scale
              }) else {
            throw KagemushaRecursiveSpendError.invalidField("recipientRequest")
        }
        self.previousBundles = previousBundles
        self.outputArtifactGeneration = outputArtifactGeneration
        self.transferAmount = transferAmount
        self.recipientOutput = recipientOutput
        self.changeOutput = changeOutput
        self.recipientRequestDigest = Data(recipientRequest.digest)
        self.operationID = Data(operationID)
    }

    public func build() throws -> KagemushaRecursiveSpendSplitIntent {
        let requestArchive = try KagemushaRecursiveSpendCodecs
            .encodeSplitIntentBuildRequest(self)
        guard let intentArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBuildSplitIntentV2(requestArchive: requestArchive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        let intent = try KagemushaRecursiveSpendCodecs.decodeSplitIntent(intentArchive)
        guard intent.outputArtifactGeneration == outputArtifactGeneration,
              intent.transferAmount == transferAmount,
              intent.recipientOutput == recipientOutput,
              intent.changeOutput == changeOutput,
              intent.recipientRequestDigest == recipientRequestDigest,
              intent.operationID == operationID,
              intent.inputs.map(\.bundleDigest) == previousBundles.map(\.summary.bundleDigest)
        else {
            throw KagemushaRecursiveSpendError.invalidArchive("splitIntent.factoryBinding")
        }
        return intent
    }
}

public struct KagemushaRecursiveSpendSplitIntent: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputs: [KagemushaRecursiveSpendInputBranch]
    public let topUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef]
    public let assetScale: UInt32
    public let lineageMode: KagemushaRecursiveSpendLineageMode
    public let outputArtifactGeneration: String
    public let transferAmount: KagemushaScaledAmount
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    public let changeOutput: KagemushaSpendableNoteDescriptor?
    public let recipientRequestDigest: Data
    public let operationID: Data

    init(
        chainID: String,
        assetDefinitionID: String,
        inputs: [KagemushaRecursiveSpendInputBranch],
        topUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef],
        assetScale: UInt32,
        lineageMode: KagemushaRecursiveSpendLineageMode,
        outputArtifactGeneration: String,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptor,
        changeOutput: KagemushaSpendableNoteDescriptor?,
        recipientRequestDigest: Data,
        operationID: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            recipientRequestDigest,
            field: "recipientRequestDigest"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        guard (1...2).contains(inputs.count),
              (1...2).contains(topUpAnchorRefs.count),
              assetScale == transferAmount.scale,
              recipientOutput.amount == transferAmount else {
            throw KagemushaRecursiveSpendError.invalidField("split.context")
        }
        try KagemushaRecursiveSpend.requirePortableText(
            outputArtifactGeneration,
            field: "outputArtifactGeneration"
        )
        for (previous, current) in zip(inputs, inputs.dropFirst()) {
            guard previous.bundleDigest.lexicographicallyPrecedes(current.bundleDigest) else {
                throw KagemushaRecursiveSpendError.invalidField("split.inputs.order")
            }
        }
        for (previous, current) in zip(topUpAnchorRefs, topUpAnchorRefs.dropFirst()) {
            guard previous.topUpOperationID.lexicographicallyPrecedes(
                current.topUpOperationID
            ) else {
                throw KagemushaRecursiveSpendError.invalidField("split.topUpAnchorRefs.order")
            }
        }
        guard Set(topUpAnchorRefs.map(\.anchorDigest)).count == topUpAnchorRefs.count else {
            throw KagemushaRecursiveSpendError.invalidField(
                "split.topUpAnchorRefs.identity"
            )
        }
        let notes = inputs.map(\.inputNote)
            + [recipientOutput]
            + (changeOutput.map { [$0] } ?? [])
        guard notes.allSatisfy({
            $0.chainID == chainID
                && $0.assetDefinitionID == assetDefinitionID
                && $0.amount.scale == assetScale
        }) else {
            throw KagemushaRecursiveSpendError.invalidField("split.context")
        }
        var inputAtomicUnits = "0"
        for input in inputs {
            inputAtomicUnits = Self.add(inputAtomicUnits, input.inputNote.amount.atomicUnits)
            _ = try KagemushaScaledAmount(atomicUnits: inputAtomicUnits, scale: assetScale)
        }
        let consumedClaims = inputs.flatMap(\.branchClaims).sorted {
            $0.path.canonicallyPrecedes($1.path)
        }
        try KagemushaRecursiveSpend.validateBranchClaims(consumedClaims)
        let referencedLineageRoots = Set(topUpAnchorRefs.map(\.anchorDigest))
        let claimedLineageRoots = Set(consumedClaims.map(\.path.lineageRoot))
        guard referencedLineageRoots == claimedLineageRoots,
              inputs.allSatisfy({ input in
                  input.branchClaims.allSatisfy({ claim in
                      claim.path.depth != 0
                        || (referencedLineageRoots.contains(claim.path.lineageRoot)
                            && input.proofStepCount == 1
                            && input.peerHopCount == 0)
                  })
              }) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "split.topUpAnchorRefs.identity"
            )
        }
        if let changeOutput {
            guard transferAmount.atomicUnits != inputAtomicUnits,
                  Self.add(transferAmount.atomicUnits, changeOutput.amount.atomicUnits)
                    == inputAtomicUnits else {
                throw KagemushaRecursiveSpendError.invalidField("changeOutput.amount")
            }
        } else if transferAmount.atomicUnits != inputAtomicUnits {
            throw KagemushaRecursiveSpendError.invalidField("changeOutput")
        }
        let material = notes.flatMap { [$0.noteCommitment, $0.spendNullifier] }
        guard Set(material).count == material.count else {
            throw KagemushaRecursiveSpendError.invalidField("split.noteMaterial")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.inputs = inputs
        self.topUpAnchorRefs = topUpAnchorRefs
        self.assetScale = assetScale
        self.lineageMode = lineageMode
        self.outputArtifactGeneration = outputArtifactGeneration
        self.transferAmount = transferAmount
        self.recipientOutput = recipientOutput
        self.changeOutput = changeOutput
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.operationID = Data(operationID)
    }

    private static func add(_ lhs: String, _ rhs: String) -> String {
        let left = Array(lhs.utf8.reversed())
        let right = Array(rhs.utf8.reversed())
        var output: [UInt8] = []
        var carry = 0
        for index in 0..<max(left.count, right.count) {
            let a = index < left.count ? Int(left[index] - 48) : 0
            let b = index < right.count ? Int(right[index] - 48) : 0
            let sum = a + b + carry
            output.append(UInt8(sum % 10) + 48)
            carry = sum / 10
        }
        if carry > 0 { output.append(UInt8(carry) + 48) }
        let value = String(decoding: output.reversed(), as: UTF8.self)
        return String(value.drop(while: { $0 == "0" })).isEmpty
            ? "0"
            : String(value.drop(while: { $0 == "0" }))
    }
}

public struct KagemushaRecursiveSpendBundleSummary: Equatable, Sendable {
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let hopCount: UInt32
    public let branchClaims: [KagemushaRecursiveSpendBranchClaim]
    public let artifactGeneration: String
    public let verifierKeyID: String
    public let lineageMode: KagemushaRecursiveSpendLineageMode
    public let bundleDigest: Data
}

/// A proof-carrying bundle whose accumulator and proof bytes remain opaque.
/// Wallet code receives only the validated typed summary above.
public struct KagemushaRecursiveSpendBundle: Equatable, Sendable {
    public let archive: Data
    public let summary: KagemushaRecursiveSpendBundleSummary

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "bundle"
        )
        guard let summaryArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBundleSummaryV2(bundleArchive: noritoArchive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        self.archive = Data(noritoArchive)
        self.summary = try KagemushaRecursiveSpendCodecs.decodeBundleSummary(summaryArchive)
    }

    init(archive: Data, summary: KagemushaRecursiveSpendBundleSummary) {
        self.archive = Data(archive)
        self.summary = summary
    }
}

public struct KagemushaRecursiveSpendAppendInput: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundle
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let previousRecursiveProofOpenEnvelopesArchive: Data

    public init(
        previousBundle: KagemushaRecursiveSpendBundle,
        previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        previousRecursiveProofOpenEnvelopesArchive: Data = Data()
    ) throws {
        switch previousBundle.summary.lineageMode {
        case .reserved:
            guard previousLineageVerifierRecord != nil,
                  !previousRecursiveProofOpenEnvelopesArchive.isEmpty else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "previousInput.reservedWitness"
                )
            }
        case .semantic:
            guard previousLineageVerifierRecord == nil,
                  previousRecursiveProofOpenEnvelopesArchive.isEmpty else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "previousInput.semanticWitness"
                )
            }
        }
        self.previousBundle = previousBundle
        self.previousLineageVerifierRecord = previousLineageVerifierRecord
        self.previousRecursiveProofOpenEnvelopesArchive = Data(
            previousRecursiveProofOpenEnvelopesArchive
        )
    }
}

public struct KagemushaRecursiveSpendAppendRequest: Equatable, Sendable {
    public let previousInputs: [KagemushaRecursiveSpendAppendInput]
    public let recordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let split: KagemushaRecursiveSpendSplitIntent
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReference?
    public let outputProofCircuitID: String
    public let blockHeight: UInt64

    public init(
        previousInputs: [KagemushaRecursiveSpendAppendInput],
        recordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        split: KagemushaRecursiveSpendSplitIntent,
        lineageArtifact: KagemushaRecursiveSpendArtifactReference? = nil,
        blockHeight: UInt64
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            recordBundle,
            schema: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            field: "recordBundle"
        )
        guard let first = previousInputs.first,
              !pallasOpenEnvelopesArchive.isEmpty,
              previousInputs.count == split.inputs.count,
              (1...2).contains(previousInputs.count),
              blockHeight > 0,
              zip(previousInputs, split.inputs).allSatisfy({ previous, input in
                  previous.previousBundle.summary.amount == input.inputNote.amount
                    && previous.previousBundle.summary.noteCommitment
                        == input.inputNote.noteCommitment
                    && previous.previousBundle.summary.spendNullifier
                        == input.inputNote.spendNullifier
                    && previous.previousBundle.summary.branchClaims == input.branchClaims
                    && previous.previousBundle.summary.bundleDigest == input.bundleDigest
                    && previous.previousBundle.summary.hopCount == input.peerHopCount
              }) else {
            throw KagemushaRecursiveSpendError.invalidField("appendRequest")
        }
        let outputProofCircuitID: String
        switch (first.previousBundle.summary.lineageMode, lineageArtifact) {
        case let (.reserved, .some(artifact))
            where artifact.role == .lineageAppendProver
                && split.lineageMode == .reserved
                && split.outputArtifactGeneration == artifact.generation
                && previousInputs.allSatisfy({
                    $0.previousBundle.summary.lineageMode == .reserved
                }):
            outputProofCircuitID = KagemushaRecursiveSpend.reservedAppendCircuitID
        case (.semantic, nil)
            where split.lineageMode == .semantic
                && previousInputs.allSatisfy({
                $0.previousBundle.summary.lineageMode == .semantic
            }):
            outputProofCircuitID = KagemushaRecursiveSpend.semanticCircuitID
        default:
            throw KagemushaRecursiveSpendError.invalidField("lineageArtifact")
        }
        self.previousInputs = previousInputs
        self.recordBundle = Data(recordBundle)
        self.pallasOpenEnvelopesArchive = Data(pallasOpenEnvelopesArchive)
        self.split = split
        self.lineageArtifact = lineageArtifact
        self.outputProofCircuitID = outputProofCircuitID
        self.blockHeight = blockHeight
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeAppendRequest(self)
    }
}

public struct KagemushaRecursiveSpendSplitResult: Equatable, Sendable {
    public let split: KagemushaRecursiveSpendSplitIntent
    public let splitBindingDigest: Data
    public let recipientBundle: KagemushaRecursiveSpendBundle
    public let changeBundle: KagemushaRecursiveSpendBundle?
    public let archive: Data

    init(
        split: KagemushaRecursiveSpendSplitIntent,
        splitBindingDigest: Data,
        recipientBundle: KagemushaRecursiveSpendBundle,
        changeBundle: KagemushaRecursiveSpendBundle?,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            splitBindingDigest,
            field: "splitBindingDigest"
        )
        let expectedHopCount = (split.inputs.map(\.peerHopCount).max() ?? 0) + 1
        guard recipientBundle.summary.amount == split.transferAmount,
              recipientBundle.summary.noteCommitment == split.recipientOutput.noteCommitment,
              recipientBundle.summary.lineageMode == split.lineageMode,
              recipientBundle.summary.artifactGeneration == split.outputArtifactGeneration,
              recipientBundle.summary.hopCount == expectedHopCount else {
            throw KagemushaRecursiveSpendError.invalidField("recipientBundle")
        }
        switch (split.changeOutput, changeBundle) {
        case (nil, nil):
            break
        case let (.some(change), .some(bundle)):
            guard bundle.summary.amount == change.amount,
                  bundle.summary.noteCommitment == change.noteCommitment,
                  bundle.summary.lineageMode == split.lineageMode,
                  bundle.summary.artifactGeneration == split.outputArtifactGeneration,
                  bundle.summary.hopCount == expectedHopCount,
                  bundle.archive != recipientBundle.archive else {
                throw KagemushaRecursiveSpendError.invalidField("changeBundle")
            }
        default:
            throw KagemushaRecursiveSpendError.invalidField("changeBundle")
        }
        self.split = split
        self.splitBindingDigest = Data(splitBindingDigest)
        self.recipientBundle = recipientBundle
        self.changeBundle = changeBundle
        self.archive = Data(archive)
    }
}

/// Recipient-only transport projection of a local split result.
///
/// Sender change is never part of this archive. The native decoder validates
/// the embedded recipient transition before exposing the opaque bundle. The
/// archive contains only `recipientBundle`; the public identity properties are
/// derived from that bundle's proof-bound recipient `PeerSplit` transition.
public struct KagemushaRecursiveSpendPeerPayment: Equatable, Sendable {
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let recipientBundle: KagemushaRecursiveSpendBundle
    public let archive: Data

    init(
        recipientBundle: KagemushaRecursiveSpendBundle,
        archive: Data
    ) throws {
        let identity = try KagemushaRecursiveSpendCodecs
            .recipientPeerSplitIdentity(from: recipientBundle.archive)
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.peerPaymentWireName,
            field: "peerPayment"
        )
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPayment.size")
        }
        self.operationID = identity.operationID
        self.recipientRequestDigest = identity.recipientRequestDigest
        self.recipientBundle = recipientBundle
        self.archive = Data(archive)
    }

    public static func create(
        recipientBundle: KagemushaRecursiveSpendBundle
    ) throws -> Self {
        let archive = try KagemushaRecursiveSpendCodecs.encodePeerPayment(
            recipientBundle: recipientBundle
        )
        return try Self(recipientBundle: recipientBundle, archive: archive)
    }

    public static func recipientOnly(
        from result: KagemushaRecursiveSpendSplitResult
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentFromSplitV2(
                splitResultArchive: result.archive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try decode(archive)
    }

    public static func decode(_ archive: Data) throws -> Self {
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPayment.size")
        }
        guard let canonical = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentValidateV2(paymentArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard canonical == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPayment.canonical")
        }
        return try KagemushaRecursiveSpendCodecs.decodePeerPayment(canonical)
    }

    public func noritoEncoded() -> Data {
        archive
    }
}

public struct KagemushaRecursiveSpendVerifyRequest: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundle
    public let recipientRequest: KagemushaRecipientPaymentRequest
    public let maximumHops: UInt32
    public let artifactGeneration: String
    public let verifiedAtMilliseconds: UInt64

    public init(
        bundle: KagemushaRecursiveSpendBundle,
        recipientRequest: KagemushaRecipientPaymentRequest,
        maximumHops: UInt32,
        verifiedAtMilliseconds: UInt64
    ) throws {
        guard maximumHops > 0,
              maximumHops <= 64,
              bundle.summary.hopCount <= maximumHops,
              verifiedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("verifyRequest")
        }
        self.bundle = bundle
        self.recipientRequest = recipientRequest
        self.maximumHops = maximumHops
        self.artifactGeneration = bundle.summary.artifactGeneration
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeVerifyRequest(self)
    }
}

public struct KagemushaRecursiveSpendLineageNode: Equatable, Sendable {
    public let resultBundleDigest: Data
    public let parentBundleDigests: [Data]
    public let proofStepCount: UInt32
    public let verifiedAtBlockHeight: UInt64
    public let transitionArchive: Data

    public init(
        resultBundleDigest: Data,
        parentBundleDigests: [Data],
        proofStepCount: UInt32,
        verifiedAtBlockHeight: UInt64,
        transitionArchive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            resultBundleDigest,
            field: "lineageNode.resultBundleDigest"
        )
        guard parentBundleDigests.count <= 2,
              parentBundleDigests.allSatisfy({ digest in
                  digest.count == 32 && digest.contains(where: { $0 != 0 })
              }),
              zip(parentBundleDigests, parentBundleDigests.dropFirst()).allSatisfy({ pair in
                  pair.0.lexicographicallyPrecedes(pair.1)
              }),
              proofStepCount > 0,
              proofStepCount <= KagemushaRecursiveSpend.semanticMaximumHops + 1,
              verifiedAtBlockHeight > 0,
              !transitionArchive.isEmpty,
              transitionArchive.count
                <= KagemushaRecursiveSpend.semanticLineageMaximumNodeArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidField("lineageNode")
        }
        self.resultBundleDigest = Data(resultBundleDigest)
        self.parentBundleDigests = parentBundleDigests.map { Data($0) }
        self.proofStepCount = proofStepCount
        self.verifiedAtBlockHeight = verifiedAtBlockHeight
        self.transitionArchive = Data(transitionArchive)
    }
}

public struct KagemushaRecursiveSpendLineageWitness: Equatable, Sendable {
    public let nodes: [KagemushaRecursiveSpendLineageNode]
    public let finalBundleDigest: Data

    public init(
        nodes: [KagemushaRecursiveSpendLineageNode],
        finalBundleDigest: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            finalBundleDigest,
            field: "finalBundleDigest"
        )
        try Self.validateCanonicalDAG(nodes: nodes, finalBundleDigest: finalBundleDigest)
        self.nodes = nodes
        self.finalBundleDigest = Data(finalBundleDigest)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeLineageWitness(self)
    }

    private static func validateCanonicalDAG(
        nodes: [KagemushaRecursiveSpendLineageNode],
        finalBundleDigest: Data
    ) throws {
        guard !nodes.isEmpty,
              nodes.count <= KagemushaRecursiveSpend.semanticLineageMaximumNodes else {
            throw KagemushaRecursiveSpendError.invalidField("lineageWitness.nodes")
        }

        var nodeIndexes: [Data: Int] = [:]
        var childCounts: [Data: Int] = [:]
        var previousStep: UInt32?
        var previousDigest: Data?
        var rootCount = 0
        var totalArchiveBytes = 0

        for (index, node) in nodes.enumerated() {
            guard nodeIndexes[node.resultBundleDigest] == nil else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "lineageWitness.nodes.resultBundleDigest.duplicate"
                )
            }
            if let previousStep, let previousDigest {
                guard previousStep < node.proofStepCount
                        || (previousStep == node.proofStepCount
                            && previousDigest.lexicographicallyPrecedes(
                                node.resultBundleDigest
                            )) else {
                    throw KagemushaRecursiveSpendError.invalidField(
                        "lineageWitness.nodes.order"
                    )
                }
            }
            previousStep = node.proofStepCount
            previousDigest = node.resultBundleDigest

            let (nextTotal, overflow) = totalArchiveBytes.addingReportingOverflow(
                node.transitionArchive.count
            )
            guard !overflow,
                  nextTotal
                    <= KagemushaRecursiveSpend.semanticLineageMaximumTotalArchiveBytes else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "lineageWitness.transitionArchive.totalBytes"
                )
            }
            totalArchiveBytes = nextTotal

            let expectedStep: UInt32
            var maximumParentVerificationHeight: UInt64 = 0
            if node.parentBundleDigests.isEmpty {
                rootCount += 1
                expectedStep = 1
            } else {
                var maximumParentStep: UInt32 = 0
                for parent in node.parentBundleDigests {
                    guard let parentIndex = nodeIndexes[parent],
                          let childCount = childCounts[parent] else {
                        throw KagemushaRecursiveSpendError.invalidField(
                            "lineageWitness.nodes.parentBundleDigests.missing"
                        )
                    }
                    maximumParentStep = max(
                        maximumParentStep,
                        nodes[parentIndex].proofStepCount
                    )
                    maximumParentVerificationHeight = max(
                        maximumParentVerificationHeight,
                        nodes[parentIndex].verifiedAtBlockHeight
                    )
                    childCounts[parent] = childCount + 1
                }
                let (step, overflow) = maximumParentStep.addingReportingOverflow(1)
                guard !overflow else {
                    throw KagemushaRecursiveSpendError.invalidField(
                        "lineageWitness.nodes.proofStepCount"
                    )
                }
                expectedStep = step
            }
            guard node.proofStepCount == expectedStep else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "lineageWitness.nodes.proofStepCount"
                )
            }
            guard node.verifiedAtBlockHeight >= maximumParentVerificationHeight else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "lineageWitness.nodes.verifiedAtBlockHeight"
                )
            }
            nodeIndexes[node.resultBundleDigest] = index
            childCounts[node.resultBundleDigest] = 0
        }

        guard (1...2).contains(rootCount) else {
            throw KagemushaRecursiveSpendError.invalidField("lineageWitness.nodes.roots")
        }
        let sinks = childCounts.compactMap { digest, count in count == 0 ? digest : nil }
        guard sinks.count == 1,
              sinks[0] == finalBundleDigest,
              nodes.last?.resultBundleDigest == finalBundleDigest else {
            throw KagemushaRecursiveSpendError.invalidField("lineageWitness.nodes.sink")
        }

        var closure = Set<Data>()
        var pending = [finalBundleDigest]
        while let digest = pending.popLast() {
            guard closure.insert(digest).inserted else { continue }
            guard let index = nodeIndexes[digest] else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "lineageWitness.nodes.ancestorClosure"
                )
            }
            pending.append(contentsOf: nodes[index].parentBundleDigests)
        }
        guard closure.count == nodes.count else {
            throw KagemushaRecursiveSpendError.invalidField(
                "lineageWitness.nodes.ancestorClosure"
            )
        }
    }
}

public struct KagemushaRecursiveSpendVerifyResult: Equatable, Sendable {
    public let valid: Bool
    public let chainAdmissible: Bool
    public let lineageRedeemable: Bool
    public let witnesslessRedemptionSupported: Bool
    public let lineageMode: KagemushaRecursiveSpendLineageMode
    public let summary: KagemushaRecursiveSpendBundleSummary
    public let recipientRequestDigest: Data
    public let requestOutputBindingDigest: Data
    public let verifierKeyID: String
    public let verifierCircuitID: String
    public let verifierActivationHeight: UInt64?
    public let verifierWithdrawHeight: UInt64?
    public let verifiedAtBlockHeight: UInt64
    public let verifiedAtMilliseconds: UInt64
    public let verifiedLineageWitness: KagemushaRecursiveSpendLineageWitness?
}

public struct KagemushaReceiverAcknowledgementPayload: Equatable, Sendable {
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let recipientCommitment: Data
    public let acceptedAtMilliseconds: UInt64
    public let receiverDeviceID: String
    public let receiverKeyReference: Data
    public let receiverPublicKey: KagemushaPublicKey
    public let archive: Data

    init(
        operationID: Data,
        recipientRequestDigest: Data,
        paymentBundleDigest: Data,
        recipientCommitment: Data,
        acceptedAtMilliseconds: UInt64,
        receiverDeviceID: String,
        receiverKeyReference: Data,
        receiverPublicKey: KagemushaPublicKey,
        archive: Data
    ) throws {
        for (field, value) in [
            ("operationID", operationID),
            ("recipientRequestDigest", recipientRequestDigest),
            ("paymentBundleDigest", paymentBundleDigest),
            ("recipientCommitment", recipientCommitment),
            ("receiverKeyReference", receiverKeyReference),
        ] {
            try KagemushaRecursiveSpend.requireNonzeroFixed32(value, field: field)
        }
        guard acceptedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("acceptedAtMilliseconds")
        }
        try KagemushaRecursiveSpend.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementPayloadWireName,
            field: "acknowledgementPayload"
        )
        self.operationID = Data(operationID)
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.paymentBundleDigest = Data(paymentBundleDigest)
        self.recipientCommitment = Data(recipientCommitment)
        self.acceptedAtMilliseconds = acceptedAtMilliseconds
        self.receiverDeviceID = receiverDeviceID
        self.receiverKeyReference = Data(receiverKeyReference)
        self.receiverPublicKey = receiverPublicKey
        self.archive = Data(archive)
    }

    public func signingBytes() throws -> Data {
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaReceiverAcknowledgementSigningBytesV2(payloadArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }
}

public struct KagemushaReceiverAcknowledgement: Equatable, Sendable {
    public let payload: KagemushaReceiverAcknowledgementPayload
    public let signature: Data
    public let archive: Data

    public static func prepare(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPayment,
        acceptedAtMilliseconds: UInt64
    ) throws -> KagemushaReceiverAcknowledgementPayload {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementPayloadV2(
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive,
            acceptedAtMilliseconds: acceptedAtMilliseconds
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeAcknowledgementPayload(archive)
    }

    public static func create(
        payload: KagemushaReceiverAcknowledgementPayload,
        signature: Data,
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPayment
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementCreateV2(
            payloadArchive: payload.archive,
            signature: signature,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try Self(payload: payload, signature: signature, archive: archive)
    }

    init(payload: KagemushaReceiverAcknowledgementPayload, signature: Data, archive: Data) throws {
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("acknowledgement.signature")
        }
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementWireName,
            field: "acknowledgement"
        )
        self.payload = payload
        self.signature = Data(signature)
        self.archive = Data(archive)
    }

    /// Sender-side commit gate. Inputs must remain reserved until this succeeds
    /// and the application confirms the receiver key's registered-device lineage.
    public func verifiedForSender(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPayment
    ) throws -> KagemushaReceiverAcknowledgementVerifyResult {
        guard let result = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementVerifyV2(
            acknowledgementArchive: archive,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeAcknowledgementVerifyResult(result)
    }
}

public struct KagemushaReceiverAcknowledgementVerifyResult: Equatable, Sendable {
    public let valid: Bool
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let acknowledgementDigest: Data
}

public struct KagemushaUnshieldPublicInputsBinding: Equatable, Sendable {
    public let inputCommitments: [Data]
    public let nullifiers: [Data]
    public let changeOutputCommitment: Data
    public let root: Data
    public let publicAmount: Data
    public let assetTag: Data
    public let chainTag: Data

    public init(
        inputCommitments: [Data],
        nullifiers: [Data],
        changeOutputCommitment: Data,
        root: Data,
        publicAmount: Data,
        assetTag: Data,
        chainTag: Data
    ) throws {
        guard inputCommitments.count == 2, nullifiers.count == 2 else {
            throw KagemushaRecursiveSpendError.invalidField("unshieldPublicInputs")
        }
        for (field, values) in [
            ("inputCommitments", inputCommitments),
            ("nullifiers", nullifiers),
        ] {
            guard values.allSatisfy({ $0.count == 32 }) else {
                throw KagemushaRecursiveSpendError.invalidField(field)
            }
        }
        for (field, value) in [
            ("changeOutputCommitment", changeOutputCommitment),
            ("root", root),
            ("publicAmount", publicAmount),
            ("assetTag", assetTag),
            ("chainTag", chainTag),
        ] where value.count != 32 {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
        self.inputCommitments = inputCommitments.map { Data($0) }
        self.nullifiers = nullifiers.map { Data($0) }
        self.changeOutputCommitment = Data(changeOutputCommitment)
        self.root = Data(root)
        self.publicAmount = Data(publicAmount)
        self.assetTag = Data(assetTag)
        self.chainTag = Data(chainTag)
    }
}

/// Typed native-factory request for a redemption intent. Swift cannot supply
/// parent claims, anchors, roots, counts, or bundle identity.
public struct KagemushaRecursiveSpendRedemptionIntentBuildRequest: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundle
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOutput: KagemushaSpendableNoteDescriptor?
    public let changeArtifactGeneration: String?
    public let unshieldPublicInputs: KagemushaUnshieldPublicInputsBinding
    public let unshieldPublicInputsDigest: Data
    public let operationID: Data

    public init(
        previousBundle: KagemushaRecursiveSpendBundle,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptor? = nil,
        changeArtifactGeneration: String? = nil,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBinding,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            unshieldPublicInputsDigest,
            field: "unshieldPublicInputsDigest"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        guard publicAmount.scale == previousBundle.summary.amount.scale,
              KagemushaScaledAmount.compareAtomicUnits(
                  publicAmount.atomicUnits,
                  previousBundle.summary.amount.atomicUnits
              ) != .orderedDescending else {
            throw KagemushaRecursiveSpendError.invalidField("publicAmount")
        }
        switch (
            changeOutput,
            changeArtifactGeneration,
            publicAmount.atomicUnits == previousBundle.summary.amount.atomicUnits
        ) {
        case (nil, nil, true): break
        case let (.some(change), .some(generation), false):
            try KagemushaRecursiveSpend.requirePortableText(
                generation,
                field: "changeArtifactGeneration"
            )
            guard change.assetDefinitionID == previousBundle.summary.assetDefinitionID,
                  change.amount.scale == publicAmount.scale,
                  KagemushaRecursiveSpendSplitIntent.addForValidation(
                      publicAmount.atomicUnits,
                      change.amount.atomicUnits
                  ) == previousBundle.summary.amount.atomicUnits,
                  change.noteCommitment == unshieldPublicInputs.changeOutputCommitment else {
                throw KagemushaRecursiveSpendError.invalidField("changeOutput")
            }
        default:
            throw KagemushaRecursiveSpendError.invalidField("changeOutput")
        }
        self.previousBundle = previousBundle
        self.recipient = recipient
        self.publicAmount = publicAmount
        self.changeOutput = changeOutput
        self.changeArtifactGeneration = changeArtifactGeneration
        self.unshieldPublicInputs = unshieldPublicInputs
        self.unshieldPublicInputsDigest = Data(unshieldPublicInputsDigest)
        self.operationID = Data(operationID)
    }

    public func build() throws -> KagemushaRecursiveSpendRedemptionIntent {
        let requestArchive = try KagemushaRecursiveSpendCodecs
            .encodeRedemptionIntentBuildRequest(self)
        guard let intentArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBuildRedemptionIntentV2(
                requestArchive: requestArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        let intent = try KagemushaRecursiveSpendCodecs
            .decodeRedemptionIntent(intentArchive)
        guard intent.parentBundleDigest == previousBundle.summary.bundleDigest,
              intent.inputNote.assetDefinitionID == previousBundle.summary.assetDefinitionID,
              intent.inputNote.amount == previousBundle.summary.amount,
              intent.inputNote.noteCommitment == previousBundle.summary.noteCommitment,
              intent.inputNote.spendNullifier == previousBundle.summary.spendNullifier,
              intent.parentBranchClaims == previousBundle.summary.branchClaims,
              intent.parentPeerHopCount == previousBundle.summary.hopCount,
              intent.recipient == recipient,
              intent.publicAmount == publicAmount,
              intent.changeOutput == changeOutput,
              intent.changeArtifactGeneration == changeArtifactGeneration,
              intent.unshieldPublicInputs == unshieldPublicInputs,
              intent.unshieldPublicInputsDigest == unshieldPublicInputsDigest,
              intent.operationID == operationID else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "redemptionIntent.factoryBinding"
            )
        }
        return intent
    }
}

public struct KagemushaRecursiveSpendRedemptionIntent: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputNote: KagemushaSpendableNoteDescriptor
    public let parentBranchClaims: [KagemushaRecursiveSpendBranchClaim]
    public let parentTopUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef]
    public let parentProofStepCount: UInt32
    public let parentPeerHopCount: UInt32
    public let parentBundleDigest: Data
    public let inputRoot: Data
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOutput: KagemushaSpendableNoteDescriptor?
    public let changeArtifactGeneration: String?
    public let unshieldPublicInputs: KagemushaUnshieldPublicInputsBinding
    public let unshieldPublicInputsDigest: Data
    public let operationID: Data

    init(
        chainID: String,
        assetDefinitionID: String,
        inputNote: KagemushaSpendableNoteDescriptor,
        parentBranchClaims: [KagemushaRecursiveSpendBranchClaim],
        parentTopUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef],
        parentProofStepCount: UInt32,
        parentPeerHopCount: UInt32,
        parentBundleDigest: Data,
        inputRoot: Data,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptor?,
        changeArtifactGeneration: String?,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBinding,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            parentBundleDigest,
            field: "parentBundleDigest"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(inputRoot, field: "inputRoot")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            unshieldPublicInputsDigest,
            field: "unshieldPublicInputsDigest"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        try KagemushaRecursiveSpend.validateBranchClaims(parentBranchClaims)
        guard (1...2).contains(parentTopUpAnchorRefs.count),
              parentProofStepCount > 0,
              parentPeerHopCount <= UInt32(KagemushaRecursiveSpendBranchPath.maximumDepth),
              inputNote.chainID == chainID,
              inputNote.assetDefinitionID == assetDefinitionID,
              publicAmount.scale == inputNote.amount.scale else {
            throw KagemushaRecursiveSpendError.invalidField("redemptionIntent")
        }
        for (previous, current) in zip(
            parentTopUpAnchorRefs,
            parentTopUpAnchorRefs.dropFirst()
        ) {
            guard previous.topUpOperationID.lexicographicallyPrecedes(
                current.topUpOperationID
            ) else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "parentTopUpAnchorRefs.order"
                )
            }
        }
        switch (changeOutput, changeArtifactGeneration) {
        case (nil, nil):
            guard publicAmount.atomicUnits == inputNote.amount.atomicUnits,
                  unshieldPublicInputs.changeOutputCommitment == Data(repeating: 0, count: 32)
            else {
                throw KagemushaRecursiveSpendError.invalidField("publicAmount")
            }
        case let (.some(change), .some(generation)):
            try KagemushaRecursiveSpend.requirePortableText(
                generation,
                field: "changeArtifactGeneration"
            )
            guard change.chainID == chainID,
                  change.assetDefinitionID == assetDefinitionID,
                  change.amount.scale == publicAmount.scale,
                  KagemushaRecursiveSpendSplitIntent.addForValidation(
                      publicAmount.atomicUnits,
                      change.amount.atomicUnits
                  ) == inputNote.amount.atomicUnits,
                  change.noteCommitment == unshieldPublicInputs.changeOutputCommitment else {
                throw KagemushaRecursiveSpendError.invalidField("changeOutput")
            }
        default:
            throw KagemushaRecursiveSpendError.invalidField("changeOutput")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.inputNote = inputNote
        self.parentBranchClaims = parentBranchClaims
        self.parentTopUpAnchorRefs = parentTopUpAnchorRefs
        self.parentProofStepCount = parentProofStepCount
        self.parentPeerHopCount = parentPeerHopCount
        self.parentBundleDigest = parentBundleDigest
        self.inputRoot = inputRoot
        self.recipient = recipient
        self.publicAmount = publicAmount
        self.changeOutput = changeOutput
        self.changeArtifactGeneration = changeArtifactGeneration
        self.unshieldPublicInputs = unshieldPublicInputs
        self.unshieldPublicInputsDigest = unshieldPublicInputsDigest
        self.operationID = operationID
    }
}

public struct KagemushaRecursiveSpendRedeemChangeBranch: Equatable, Sendable {
    public let output: KagemushaSpendableNoteDescriptor
    public let branchClaims: [KagemushaRecursiveSpendBranchClaim]
    public let bundle: KagemushaRecursiveSpendBundle
}

public struct KagemushaRecursiveSpendRedeemChangeBuildRequest: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundle
    public let previousRecursiveProofOpenEnvelopesArchive: Data
    public let unshieldRecordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let redemption: KagemushaRecursiveSpendRedemptionIntent
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReference
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef
    public let blockHeight: UInt64

    public init(
        previousBundle: KagemushaRecursiveSpendBundle,
        previousRecursiveProofOpenEnvelopesArchive: Data,
        unshieldRecordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        redemption: KagemushaRecursiveSpendRedemptionIntent,
        lineageArtifact: KagemushaRecursiveSpendArtifactReference,
        previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef,
        blockHeight: UInt64
    ) throws {
        guard !previousRecursiveProofOpenEnvelopesArchive.isEmpty,
              !pallasOpenEnvelopesArchive.isEmpty,
              lineageArtifact.role == .redeemChangeProver,
              lineageArtifact.generation == redemption.changeArtifactGeneration,
              redemption.parentBundleDigest == previousBundle.summary.bundleDigest,
              redemption.changeOutput != nil,
              blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("redeemChangeBuildRequest")
        }
        try KagemushaRecursiveSpend.requireArchive(
            unshieldRecordBundle,
            schema: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            field: "unshieldRecordBundle"
        )
        self.previousBundle = previousBundle
        self.previousRecursiveProofOpenEnvelopesArchive =
            Data(previousRecursiveProofOpenEnvelopesArchive)
        self.unshieldRecordBundle = Data(unshieldRecordBundle)
        self.pallasOpenEnvelopesArchive = Data(pallasOpenEnvelopesArchive)
        self.redemption = redemption
        self.lineageArtifact = lineageArtifact
        self.previousLineageVerifierRecord = previousLineageVerifierRecord
        self.blockHeight = blockHeight
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeRedeemChangeBuildRequest(self)
    }
}

public struct KagemushaRecursiveSpendRedeemChangeBuildResult: Equatable, Sendable {
    public let changeBranch: KagemushaRecursiveSpendRedeemChangeBranch
    public let transitionBindingDigest: Data
    public let publicStatementDigest: Data
}

public struct KagemushaRecursiveSpendRedeemUnsigned: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundle
    public let recipient: String
    public let amount: KagemushaScaledAmount
    public let redeemProof: Data
    public let redemption: KagemushaRecursiveSpendRedemptionIntent
    public let lineageWitness: KagemushaRecursiveSpendLineageWitness?
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef
    public let offlineChange: KagemushaRecursiveSpendRedeemChangeBranch?
    public let blockHeight: UInt64
    public let operationID: Data

    public init(
        bundle: KagemushaRecursiveSpendBundle,
        recipient: String,
        amount: KagemushaScaledAmount,
        redeemProof: Data,
        redemption: KagemushaRecursiveSpendRedemptionIntent,
        lineageWitness: KagemushaRecursiveSpendLineageWitness?,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef,
        offlineChange: KagemushaRecursiveSpendRedeemChangeBranch? = nil,
        blockHeight: UInt64,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpend.requireArchive(
            redeemProof,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "redeemProof"
        )
        guard blockHeight > 0,
              recipient == redemption.recipient,
              amount == redemption.publicAmount,
              operationID == redemption.operationID,
              redemption.parentBundleDigest == bundle.summary.bundleDigest,
              redemption.inputNote.assetDefinitionID == bundle.summary.assetDefinitionID,
              redemption.inputNote.amount == bundle.summary.amount,
              redemption.inputNote.noteCommitment == bundle.summary.noteCommitment,
              redemption.inputNote.spendNullifier == bundle.summary.spendNullifier,
              redemption.parentBranchClaims == bundle.summary.branchClaims,
              redemption.parentPeerHopCount == bundle.summary.hopCount,
              (redemption.changeOutput == nil) == (offlineChange == nil) else {
            throw KagemushaRecursiveSpendError.invalidField("redeemUnsigned")
        }
        if let offlineChange {
            guard offlineChange.output == redemption.changeOutput,
                  offlineChange.bundle.summary.artifactGeneration
                    == redemption.changeArtifactGeneration else {
                throw KagemushaRecursiveSpendError.invalidField("offlineChange")
            }
        }
        switch (bundle.summary.lineageMode, lineageWitness) {
        case (.reserved, nil):
            break
        case (.semantic, .some)
            where bundle.summary.hopCount <= KagemushaRecursiveSpend.semanticMaximumHops:
            break
        default:
            throw KagemushaRecursiveSpendError.invalidField("lineageWitness")
        }
        self.bundle = bundle
        self.recipient = recipient
        self.amount = amount
        self.redeemProof = Data(redeemProof)
        self.redemption = redemption
        self.lineageWitness = lineageWitness
        self.lineageVerifierRecord = lineageVerifierRecord
        self.offlineChange = offlineChange
        self.blockHeight = blockHeight
        self.operationID = Data(operationID)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeRedeemUnsigned(self)
    }

    public func authorizationPayloadDigest() throws -> Data {
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemUnsignedPayloadDigestV2(
                unsignedArchive: noritoEncoded()
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            digest,
            field: "redeemUnsigned.payloadDigest"
        )
        return digest
    }

    public func finalize(
        authorization: KagemushaRequestAuthorization
    ) throws -> KagemushaRecursiveSpendRedeemRequest {
        let unsignedArchive = try noritoEncoded()
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == (try authorizationPayloadDigest()) else {
            throw KagemushaRecursiveSpendError.invalidField("authorization")
        }
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemFinalizeRequestV2(
                unsignedArchive: unsignedArchive,
                authorizationArchive: authorization.archive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendRedeemRequest(
            unsigned: self,
            authorization: authorization,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecursiveSpendRedeemRequest: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendRedeemUnsigned
    public let authorization: KagemushaRequestAuthorization
    public let archive: Data

    public var bundle: KagemushaRecursiveSpendBundle { unsigned.bundle }
    public var recipient: String { unsigned.recipient }
    public var amount: KagemushaScaledAmount { unsigned.amount }
    public var redeemProof: Data { unsigned.redeemProof }
    public var redemption: KagemushaRecursiveSpendRedemptionIntent { unsigned.redemption }
    public var lineageWitness: KagemushaRecursiveSpendLineageWitness? {
        unsigned.lineageWitness
    }
    public var lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef {
        unsigned.lineageVerifierRecord
    }
    public var offlineChange: KagemushaRecursiveSpendRedeemChangeBranch? {
        unsigned.offlineChange
    }
    public var blockHeight: UInt64 { unsigned.blockHeight }
    public var operationID: Data { unsigned.operationID }

    init(
        unsigned: KagemushaRecursiveSpendRedeemUnsigned,
        authorization: KagemushaRequestAuthorization,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            field: "redeemRequest"
        )
        self.unsigned = unsigned
        self.authorization = authorization
        self.archive = Data(archive)
        guard try KagemushaRecursiveSpendCodecs.encodeRedeemRequest(self) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("redeemRequest.canonical")
        }
    }

    public func noritoEncoded() -> Data { archive }
}

public struct KagemushaRecursiveSpendRedeemResult: Equatable, Sendable {
    public let redeemRequestArchive: Data
    public let offlineChangeBundle: KagemushaRecursiveSpendBundle?
    public let operationID: Data
}

/// Owns one ABI-18 V3 streaming handle. `write` accepts chunks of the complete
/// published `KRV3KEY` file and never exposes or parses its header or payload.
/// Native finalization re-parses and authenticates the held file descriptor.
public final class KagemushaRecursiveSpendArtifactIngest: @unchecked Sendable {
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    public let artifactSHA256: Data
    private var handle: UInt64?
    private var finalized = false
    private let lock = NSLock()

    public init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        expectedArtifactSHA256: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedArtifactSHA256,
            field: "artifact.sha256"
        )
        guard let handle = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendArtifactBeginV3(
                manifestArchive: manifest.noritoArchive,
                expectedManifestSHA256: manifest.sha256,
                expectedArtifactSHA256: expectedArtifactSHA256
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        self.manifest = manifest
        self.artifactSHA256 = Data(expectedArtifactSHA256)
        self.handle = handle
    }

    deinit {
        lock.lock()
        let active = handle
        handle = nil
        lock.unlock()
        if let active {
            _ = try? NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactCancelV3(handle: active)
        }
    }

    public func write(_ chunk: Data) throws {
        guard !chunk.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.chunk")
        }
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.handle")
        }
        guard !finalized else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
        }
        do {
            guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactWriteV3(
                handle: handle,
                chunk: chunk
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
        } catch {
            _ = try? NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactCancelV3(handle: handle)
            self.handle = nil
            throw error
        }
    }

    public func finalize() throws {
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.handle")
        }
        guard !finalized else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
        }
        do {
            guard try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactFinalizeV3(handle: handle) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            finalized = true
        } catch {
            // Native finalization removes a corrupt spool. Clear the Swift
            // owner as well so cancellation remains idempotent.
            _ = try? NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactCancelV3(handle: handle)
            self.handle = nil
            throw error
        }
    }

    public func cancel() throws {
        lock.lock()
        defer { lock.unlock() }
        guard let active = handle else { return }
        guard try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendArtifactCancelV3(handle: active) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        handle = nil
        finalized = false
    }

    fileprivate func finalizedHandle(
        for expectedManifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard manifest == expectedManifest else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.manifest")
        }
        guard finalized, let handle else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
        }
        return handle
    }

    fileprivate func relinquishInstalledHandle(_ expectedHandle: UInt64) {
        lock.lock()
        defer { lock.unlock() }
        if handle == expectedHandle, finalized {
            handle = nil
            finalized = false
        }
    }
}

/// Coordinates a complete six-file V3 release installation.
///
/// Each artifact is still streamed independently, but `install()` is the only
/// operation that transfers ownership to the prover. Native code revalidates
/// all six anonymous files and either consumes every finalized handle or none.
public final class KagemushaRecursiveSpendArtifactInstallSessionV3: @unchecked Sendable {
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    private var artifacts: [Data: KagemushaRecursiveSpendArtifactIngest] = [:]
    private var installed = false
    private var closed = false
    private let lock = NSLock()

    public init(manifest: KagemushaRecursiveSpendArtifactManifestArchive) {
        self.manifest = manifest
    }

    deinit {
        lock.lock()
        let pending = installed ? [] : Array(artifacts.values)
        artifacts.removeAll()
        closed = true
        lock.unlock()
        for artifact in pending {
            try? artifact.cancel()
        }
    }

    /// Start one manifest-selected artifact stream. Native begin rejects a
    /// digest not present exactly once in the canonical manifest.
    public func beginArtifact(
        expectedArtifactSHA256: Data
    ) throws -> KagemushaRecursiveSpendArtifactIngest {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedArtifactSHA256,
            field: "artifact.sha256"
        )
        lock.lock()
        defer { lock.unlock() }
        guard !closed, !installed, artifacts.count < 6 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactSet.state")
        }
        guard artifacts[expectedArtifactSHA256] == nil else {
            throw KagemushaRecursiveSpendError.invalidField("artifactSet.duplicate")
        }
        let artifact = try KagemushaRecursiveSpendArtifactIngest(
            manifest: manifest,
            expectedArtifactSHA256: expectedArtifactSHA256
        )
        artifacts[Data(expectedArtifactSHA256)] = artifact
        return artifact
    }

    /// Atomically transfer one finalized handle for each of the six manifest
    /// roles into the active native generation.
    public func install() throws {
        lock.lock()
        defer { lock.unlock() }
        guard !closed, !installed, artifacts.count == 6 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactSet.count")
        }
        let orderedArtifacts = artifacts
            .sorted { $0.key.lexicographicallyPrecedes($1.key) }
            .map(\.value)
        let handles = try orderedArtifacts.map { try $0.finalizedHandle(for: manifest) }
        guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactSetInstallV3(
            manifestArchive: manifest.noritoArchive,
            expectedManifestSHA256: manifest.sha256,
            handles: handles
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        for (artifact, handle) in zip(orderedArtifacts, handles) {
            artifact.relinquishInstalledHandle(handle)
        }
        artifacts.removeAll()
        installed = true
    }

    public func isInstalled() throws -> Bool {
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendArtifactSetIsInstalledV3(
                manifestArchive: manifest.noritoArchive,
                expectedManifestSHA256: manifest.sha256
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return result
    }

    /// Cancel only pending streams. An installed generation remains active
    /// until `uninstall()` is explicitly requested.
    public func cancel() throws {
        lock.lock()
        guard !installed else {
            lock.unlock()
            return
        }
        let pending = Array(artifacts.values)
        artifacts.removeAll()
        closed = true
        lock.unlock()
        var firstError: Error?
        for artifact in pending {
            do {
                try artifact.cancel()
            } catch where firstError == nil {
                firstError = error
            } catch {}
        }
        if let firstError { throw firstError }
    }

    /// Release this exact installed generation. The native digest guard makes
    /// a stale session incapable of removing a newer generation.
    public func uninstall() throws {
        lock.lock()
        defer { lock.unlock() }
        guard !closed else { return }
        // The native digest guard is the source of truth. This deliberately
        // supports reconstructing a coordinator after an app-layer owner was
        // lost while the process stayed alive; an explicit uninstall can then
        // release the exact active generation without being able to remove a
        // newer one.
        guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactSetUninstallV3(
            expectedManifestSHA256: manifest.sha256
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        installed = false
        closed = true
    }
}

private extension KagemushaRecursiveSpendSplitIntent {
    static func addForValidation(_ lhs: String, _ rhs: String) -> String {
        add(lhs, rhs)
    }
}
