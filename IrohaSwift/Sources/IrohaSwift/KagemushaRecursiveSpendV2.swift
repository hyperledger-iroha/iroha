import CryptoKit
import Foundation

public enum KagemushaRecursiveSpendV2Error: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)
    case nativeBridgeUnavailable
    case proofBackendUnavailable

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
        }
    }
}

/// Availability, canonical wire names, and high-level native entrypoints for
/// exact-amount branch-safe recursive offline cash.
public enum KagemushaRecursiveSpendV2 {
    public static let requiredNativeBridgeAbiVersion: UInt32 = 18
    public static let artifactManifestSchema =
        "kagemusha.offline.recursive_spend.artifact_manifest.v3"
    public static let mode = "recursive_spend_v1"
    public static let pastaCycleBackend = "halo2/ipa-pasta-cycle-v1"
    public static let pastaCycleTranscript = "kagemusha-pasta-cycle-poseidon-v1"
    public static let pastaCycleProofEnvelopeVersion: UInt16 = 1
    public static let stateBoundaryVersion: UInt16 = 1
    public static let transitionEqCircuitID =
        "kagemusha-recursive-spend-transition-eq-v1"
    public static let stateEpCircuitID = "kagemusha-recursive-spend-state-ep-v1"
    public static let releaseMaximumProofBytes = 4_096
    public static let artifactMaximumFileBytes = 256 * 1024 * 1024
    /// This remains false until init/append/verify/redeem all call the audited
    /// V2 circuit and chain implementation in the same source revision.
    public static let isProofBackendAvailable = false

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
    public static let initRequestWireName = wire("KagemushaRecursiveSpendInitRequestV2")
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
    ]

    /// Complete native-symbol inventory required by V2 readiness checks.
    public static let requiredNativeSymbols = requiredProofSymbols + requiredProtocolSymbols

    public static func ensureProofBackendAvailable() throws {
        guard isProofBackendAvailable else {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
        }
    }

    public static var isNativeStubAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendV2StubAvailable
            && NoritoNativeBridge.shared.hasKagemushaRecursiveSpendV2Symbols(
                requiredNativeSymbols
            )
    }

    /// Exact local production capability; Torii readiness remains an additional requirement.
    public static var isProductionAvailable: Bool {
        isProofBackendAvailable && isNativeStubAvailable
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
        proofBackendAvailable && nativeStubAvailable ? .recursiveSpendV1 : nil
    }

    public static func initSpend(
        request: KagemushaRecursiveSpendInitRequestV2
    ) throws -> Data {
        let requestArchive = try request.noritoEncoded()
        try ensureProofBackendAvailable()
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendInitV2(
                requestArchive: requestArchive
            ) else {
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
        }
    }

    public static func topUpSpend(requestArchive: Data) throws -> Data {
        try callSingleArchive(requestArchive, schema: topUpRequestWireName) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendTopUpV2(requestArchive: requestArchive)
        }
    }

    public static func topUpSpend(
        request: KagemushaRecursiveSpendTopUpRequestV2
    ) throws -> Data {
        try topUpSpend(requestArchive: request.noritoEncoded())
    }

    /// Verify chain finality before admitting an initialized top-up branch to
    /// the local spendable set.
    public static func verifyTopUpFinality(
        proof: KagemushaTopUpFinalityProofArchiveV2,
        rosterArtifact: KagemushaTopUpFinalityRosterArtifactArchiveV2,
        expectedRosterSHA256: Data
    ) throws {
        try requireNonzeroFixed32(
            expectedRosterSHA256,
            field: "topUpFinalityRosterArtifact.sha256"
        )
        guard try NoritoNativeBridge.shared.kagemushaTopUpFinalityVerifyV2(
            proofArchive: proof.noritoArchive,
            rosterArtifactArchive: rosterArtifact.noritoArchive,
            expectedRosterSHA256: expectedRosterSHA256
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
    }

    public static func appendSpend(
        requestArchive: Data,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequestV2,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data {
        try requireArchive(requestArchive, schema: appendRequestWireName, field: "requestArchive")
        guard verifiedAtMilliseconds == signedRecipientRequest.verifiedAtMilliseconds else {
            throw KagemushaRecursiveSpendV2Error.invalidField("verifiedAtMilliseconds")
        }
        try ensureProofBackendAvailable()
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppendV2(
                requestArchive: requestArchive,
                recipientRequestArchive: signedRecipientRequest.request.archive,
                verifiedAtMilliseconds: verifiedAtMilliseconds
            ) else {
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
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
        request: KagemushaRecursiveSpendRedeemChangeBuildRequestV2
    ) throws -> KagemushaRecursiveSpendRedeemChangeBuildResultV2 {
        let archive = try request.noritoEncoded()
        try ensureProofBackendAvailable()
        do {
            guard let result = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendRedeemChangeV2(requestArchive: archive) else {
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
            }
            return try KagemushaRecursiveSpendV2Codecs.decodeRedeemChangeBuildResult(result)
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
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
        request: KagemushaRecursiveSpendRedeemRequestV2
    ) throws -> Data {
        try redeemSpend(requestArchive: request.noritoEncoded())
    }

    static func requireArchive(_ archive: Data, schema: String, field: String) throws {
        guard !archive.isEmpty,
              archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              !frame.payload.isEmpty else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
    }

    static func requireNonzeroFixed32(_ value: Data, field: String) throws {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField(field)
        }
    }

    static func transitionTag(for transitionBinding: Data) throws -> Data {
        try requireNonzeroFixed32(transitionBinding, field: "transitionBinding")
        var preimage = Data(transitionTagDomain.utf8)
        preimage.append(0)
        preimage.append(transitionBinding)
        let tag = Data(SHA256.hash(data: preimage).prefix(transitionTagBytes))
        guard tag.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("transitionTag")
        }
        return tag
    }

    static func requirePortableText(_ value: String, field: String, maximum: Int = 128) throws {
        guard !value.isEmpty,
              value.count <= maximum,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw KagemushaRecursiveSpendV2Error.invalidField(field)
        }
    }

    static func validateBranchClaims(
        _ claims: [KagemushaRecursiveSpendBranchClaimV2]
    ) throws {
        guard (1...maximumBranchClaims).contains(claims.count) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("branchClaims")
        }
        for index in claims.indices where index > claims.startIndex {
            let claim = claims[index]
            guard claims[index - 1].path.canonicallyPrecedes(claim.path) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("branchClaims.order")
            }
            guard !claims[..<index].contains(where: { $0.path.conflicts(with: claim.path) }) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("branchClaims.conflict")
            }
            for previous in claims[..<index] {
                guard previous.path.lineageRoot == claim.path.lineageRoot else { continue }
                let sharedDepth = min(previous.path.depth, claim.path.depth)
                for parentDepth in 0..<sharedDepth
                    where previous.path.hasSamePrefix(as: claim.path, depth: parentDepth)
                {
                    guard previous.transitionTags[Int(parentDepth)]
                        == claim.transitionTags[Int(parentDepth)] else {
                        throw KagemushaRecursiveSpendV2Error.invalidField(
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
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
            }
            return output
        } catch NativeBridgeError.kagemushaRecursiveSpendV2Unavailable {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
        }
    }
}

public struct KagemushaPublicKeyV2: Equatable, Hashable, Sendable {
    public let algorithm: UInt8
    public let payload: Data

    public init(algorithm: UInt8 = 0, payload: Data) throws {
        guard !payload.isEmpty, payload.count <= 8_192 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("receiverPublicKey")
        }
        if algorithm == 0, payload.count != 32 {
            throw KagemushaRecursiveSpendV2Error.invalidField("receiverPublicKey.ed25519")
        }
        self.algorithm = algorithm
        self.payload = Data(payload)
    }

    public func receiverKeyReference() throws -> Data {
        guard let reference = try NoritoNativeBridge.shared.kagemushaReceiverKeyReferenceV2(
            algorithm: algorithm,
            publicKey: payload
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            reference,
            field: "recipientKeyReference"
        )
        return reference
    }
}

public struct KagemushaSpendableNoteDescriptorV2: Equatable, Hashable, Sendable {
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
        try KagemushaRecursiveSpendV2.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendV2Error.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            noteCommitment,
            field: "noteCommitment"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            spendNullifier,
            field: "spendNullifier"
        )
        guard noteCommitment != spendNullifier else {
            throw KagemushaRecursiveSpendV2Error.invalidField("spendNullifier")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.noteCommitment = Data(noteCommitment)
        self.spendNullifier = Data(spendNullifier)
        self.amount = amount
    }
}

public struct KagemushaRecipientOutputDerivationRequestV2: Equatable, Sendable {
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
        try KagemushaRecursiveSpendV2.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendV2Error.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(requestID, field: "requestID")
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.amount = amount
        self.requestID = Data(requestID)
    }

    public func derive(
        receiverSpendSecret: Data
    ) throws -> KagemushaRecipientOutputDerivationResultV2 {
        guard receiverSpendSecret.count == 32,
              receiverSpendSecret.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("receiverSpendSecret")
        }
        let requestArchive = try KagemushaRecursiveSpendV2Codecs
            .encodeRecipientOutputDerivationRequest(self)
        guard let resultArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientOutputDeriveV2(
                requestArchive: requestArchive,
                receiverSpendSecret: receiverSpendSecret
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendV2Codecs.decodeRecipientOutputDerivationResult(
            resultArchive,
            request: self
        )
    }
}

public struct KagemushaRecipientOutputDerivationResultV2: Equatable, Sendable {
    public let recipientOutput: KagemushaSpendableNoteDescriptorV2
    public let recipientOutputProverMaterial: Data

    init(
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        recipientOutputProverMaterial: Data,
        request: KagemushaRecipientOutputDerivationRequestV2
    ) throws {
        guard recipientOutput.chainID == request.chainID,
              recipientOutput.assetDefinitionID == request.assetDefinitionID,
              recipientOutput.amount == request.amount,
              !recipientOutputProverMaterial.isEmpty,
              recipientOutputProverMaterial.count <= 4 * 1_024 else {
            throw KagemushaRecursiveSpendV2Error.invalidField(
                "recipientOutputProverMaterial"
            )
        }
        self.recipientOutput = recipientOutput
        self.recipientOutputProverMaterial = Data(recipientOutputProverMaterial)
    }
}

public enum KagemushaRecursiveSpendBranchV2: UInt32, Equatable, Sendable {
    case recipient = 0
    case change = 1
}

public enum KagemushaRecursiveSpendLineageModeV2: UInt32, Equatable, Sendable {
    case reserved = 0
    case semantic = 1
}

public struct KagemushaRecursiveSpendBranchPathV2: Equatable, Hashable, Sendable {
    public static let maximumDepth: UInt8 = 64
    public let lineageRoot: Data
    public let depth: UInt8
    public let pathBits: Data

    public init(lineageRoot: Data, depth: UInt8, pathBits: Data) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(lineageRoot, field: "lineageRoot")
        guard depth <= Self.maximumDepth, pathBits.count == 8 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("branchPath")
        }
        let unused = 64 - Int(depth)
        if unused > 0 {
            let value = pathBits.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
            let mask = unused == 64 ? UInt64.max : (UInt64(1) << UInt64(unused)) - 1
            guard value & mask == 0 else {
                throw KagemushaRecursiveSpendV2Error.invalidField("branchPath.pathBits")
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
public struct KagemushaRecursiveSpendBranchClaimV2: Equatable, Hashable, Sendable {
    public let path: KagemushaRecursiveSpendBranchPathV2
    public let transitionTags: [Data]

    public init(
        path: KagemushaRecursiveSpendBranchPathV2,
        transitionTags: [Data]
    ) throws {
        guard transitionTags.count == Int(path.depth),
              transitionTags.allSatisfy({
                  $0.count == KagemushaRecursiveSpendV2.transitionTagBytes
                    && $0.contains(where: { $0 != 0 })
              }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField(
                "branchClaim.transitionTags"
            )
        }
        self.path = path
        self.transitionTags = transitionTags.map { Data($0) }
    }

    public static func root(lineageRoot: Data) throws -> Self {
        try Self(
            path: KagemushaRecursiveSpendBranchPathV2.root(lineageRoot),
            transitionTags: []
        )
    }
}

public enum KagemushaRecursiveSpendArtifactRoleV2: UInt32, Equatable, Sendable {
    case transferProver = 0
    case unshieldProver = 1
    case lineageInitProver = 2
    case lineageAppendProver = 3
    case redeemChangeProver = 4

    var nativeExpectedRole: UInt32? {
        switch self {
        case .lineageInitProver: return 3
        case .lineageAppendProver: return 4
        case .redeemChangeProver: return 5
        default: return nil
        }
    }
}

public struct KagemushaRecursiveSpendArtifactReferenceV2: Equatable, Sendable {
    public let role: KagemushaRecursiveSpendArtifactRoleV2
    public let generation: String
    public let circuitID: String
    public let artifactType: String
    public let sizeBytes: UInt64
    public let sha256: Data

    public init(
        role: KagemushaRecursiveSpendArtifactRoleV2,
        generation: String,
        circuitID: String,
        artifactType: String = KagemushaRecursiveSpendV2.lineageArtifactType,
        sizeBytes: UInt64,
        sha256: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requirePortableText(generation, field: "generation")
        try KagemushaRecursiveSpendV2.requirePortableText(circuitID, field: "circuitID")
        guard sizeBytes > 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("sizeBytes")
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(sha256, field: "sha256")
        switch role {
        case .lineageInitProver:
            guard circuitID == KagemushaRecursiveSpendV2.reservedInitCircuitID,
                  artifactType == KagemushaRecursiveSpendV2.lineageArtifactType else {
                throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact")
            }
        case .lineageAppendProver:
            guard circuitID == KagemushaRecursiveSpendV2.reservedAppendCircuitID,
                  artifactType == KagemushaRecursiveSpendV2.lineageArtifactType else {
                throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact")
            }
        case .redeemChangeProver:
            guard circuitID == KagemushaRecursiveSpendV2.reservedRedeemChangeCircuitID,
                  artifactType == KagemushaRecursiveSpendV2.lineageArtifactType else {
                throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact")
            }
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact.role")
        }
        self.role = role
        self.generation = generation
        self.circuitID = circuitID
        self.artifactType = artifactType
        self.sizeBytes = sizeBytes
        self.sha256 = Data(sha256)
    }
}

public struct KagemushaRecipientPaymentRequestSigningPayloadV2: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let recipient: String
    public let recipientKeyReference: Data
    public let receiverDeviceID: String
    public let receiverPublicKey: KagemushaPublicKeyV2
    public let requestID: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let recipientOutput: KagemushaSpendableNoteDescriptorV2
    public let recipientOutputProverMaterial: Data

    public init(
        chainID: String,
        assetDefinitionID: String,
        amount: KagemushaScaledAmount,
        recipient: String,
        recipientKeyReference: Data,
        receiverDeviceID: String,
        receiverPublicKey: KagemushaPublicKeyV2,
        requestID: Data,
        issuedAtMilliseconds: UInt64,
        expiresAtMilliseconds: UInt64,
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        recipientOutputProverMaterial: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requirePortableText(chainID, field: "chainID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendV2Error.invalidField("assetDefinitionID")
        }
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            recipientKeyReference,
            field: "recipientKeyReference"
        )
        try KagemushaRecursiveSpendV2.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(requestID, field: "requestID")
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpendV2.maximumAuthorizationTTLMilliseconds,
              recipientOutput.chainID == chainID,
              recipientOutput.assetDefinitionID == assetDefinitionID,
              recipientOutput.amount == amount,
              !recipientOutputProverMaterial.isEmpty,
              recipientOutputProverMaterial.count <= 4 * 1024 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("recipientRequest")
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
        let archive = try KagemushaRecursiveSpendV2Codecs.encodeRecipientRequestPayload(self)
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestSigningBytesV2(payloadArchive: archive) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return bytes
    }

    public func signed(signature: Data) throws -> KagemushaRecipientPaymentRequestV2 {
        let payloadArchive = try KagemushaRecursiveSpendV2Codecs.encodeRecipientRequestPayload(self)
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestCreateV2(
                payloadArchive: payloadArchive,
                signature: signature
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecipientPaymentRequestV2(
            payload: self,
            signature: signature,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecipientPaymentRequestV2: Equatable, Sendable {
    public let payload: KagemushaRecipientPaymentRequestSigningPayloadV2
    public let signature: Data
    public let archive: Data

    init(
        payload: KagemushaRecipientPaymentRequestSigningPayloadV2,
        signature: Data,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.recipientRequestWireName,
            field: "recipientRequest"
        )
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendV2Error.invalidField("signature")
        }
        self.payload = payload
        self.signature = Data(signature)
        self.archive = Data(archive)
    }

    public func verified(atMilliseconds: UInt64) throws -> KagemushaVerifiedRecipientPaymentRequestV2 {
        guard let digest = try NoritoNativeBridge.shared.kagemushaRecipientPaymentRequestVerifyV2(
            requestArchive: archive,
            verifiedAtMilliseconds: atMilliseconds
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(digest, field: "requestDigest")
        return KagemushaVerifiedRecipientPaymentRequestV2(
            request: self,
            digest: digest,
            verifiedAtMilliseconds: atMilliseconds
        )
    }
}

public struct KagemushaVerifiedRecipientPaymentRequestV2: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequestV2
    public let digest: Data
    public let verifiedAtMilliseconds: UInt64

    init(
        request: KagemushaRecipientPaymentRequestV2,
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
public struct KagemushaRequestAuthorizationFieldsV2: Equatable, Sendable {
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
        try KagemushaRecursiveSpendV2.requirePortableText(deviceID, field: "deviceID")
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(nonce, field: "nonce")
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(payloadDigest, field: "payloadDigest")
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpendV2.maximumAuthorizationTTLMilliseconds else {
            throw KagemushaRecursiveSpendV2Error.invalidField("authorization.expiry")
        }
        switch (appAttestEvidenceSHA256, appAttestEvidence) {
        case (nil, nil):
            break
        case let (.some(digest), .some(evidence)):
            try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
                digest,
                field: "appAttestEvidenceSHA256"
            )
            guard !evidence.isEmpty, evidence.count <= 16 * 1024 else {
                throw KagemushaRecursiveSpendV2Error.invalidField("appAttestEvidence")
            }
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("appAttestEvidence")
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
        let template = try KagemushaRecursiveSpendV2Codecs.encodeAuthorizationTemplate(self)
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaRequestAuthorizationSigningBytesV2(templateArchive: template) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return bytes
    }

    public func signed(signature: Data) throws -> KagemushaRequestAuthorizationV2 {
        let template = try KagemushaRecursiveSpendV2Codecs.encodeAuthorizationTemplate(self)
        guard let archive = try NoritoNativeBridge.shared.kagemushaRequestAuthorizationCreateV2(
            templateArchive: template,
            signature: signature
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRequestAuthorizationV2(
            fields: self,
            signature: signature,
            archive: archive
        )
    }
}

public struct KagemushaRequestAuthorizationV2: Equatable, Sendable {
    public let fields: KagemushaRequestAuthorizationFieldsV2
    public let signature: Data
    public let archive: Data

    init(fields: KagemushaRequestAuthorizationFieldsV2, signature: Data, archive: Data) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.authorizationWireName,
            field: "authorization"
        )
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendV2Error.invalidField("authorization.signature")
        }
        self.fields = fields
        self.signature = Data(signature)
        self.archive = Data(archive)
    }
}

public struct KagemushaRecursiveSpendInitRequestV2: Equatable, Sendable {
    public let topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV2
    public let recordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let lineageMode: KagemushaRecursiveSpendLineageModeV2
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2?

    public init(
        topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV2,
        recordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        lineageMode: KagemushaRecursiveSpendLineageModeV2,
        lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2? = nil
    ) throws {
        let checkedTransfer = try KagemushaRecursiveSpendRequestCodecs.v2TransferFragmentSummary(
            recordBundle: recordBundle,
            pallasOpenEnvelopes: pallasOpenEnvelopesArchive,
            atBlockHeight: topUpAnchor.finalizedHeight
        )
        guard checkedTransfer.chainID == topUpAnchor.chainID,
              checkedTransfer.assetDefinitionID == topUpAnchor.currentNote.assetDefinitionID,
              checkedTransfer.rootBefore == topUpAnchor.initialRoot,
              checkedTransfer.rootAfter == topUpAnchor.finalizedRoot,
              checkedTransfer.inputNullifiers == topUpAnchor.topUpAnchorNullifiers,
              checkedTransfer.outputCommitments == [topUpAnchor.currentNote.noteCommitment],
              checkedTransfer.verifierKeyID == topUpAnchor.transferVerifierID,
              checkedTransfer.verifierKeyCommitment
                == topUpAnchor.transferVerifierCommitment,
              lineageArtifact.map({ topUpAnchor.artifactGeneration == $0.generation }) ?? true else {
            throw KagemushaRecursiveSpendV2Error.invalidField(
                "topUpAnchor.transferEvidence"
            )
        }
        switch (lineageMode, lineageArtifact) {
        case let (.reserved, .some(artifact)) where artifact.role == .lineageInitProver:
            break
        case (.semantic, nil):
            break
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact")
        }
        self.topUpAnchor = topUpAnchor
        self.recordBundle = Data(recordBundle)
        self.pallasOpenEnvelopesArchive = Data(pallasOpenEnvelopesArchive)
        self.lineageMode = lineageMode
        self.lineageArtifact = lineageArtifact
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeInitRequest(self)
    }

    public static func decode(_ archive: Data) throws -> Self {
        try KagemushaRecursiveSpendV2Codecs.decodeInitRequest(archive)
    }
}

public struct KagemushaRecursiveSpendTopUpUnsignedV2: Equatable, Sendable {
    public let assetID: String
    public let amount: KagemushaScaledAmount
    public let currentNote: KagemushaSpendableNoteDescriptorV2
    public let recordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let artifactGeneration: String
    public let operationID: Data

    public init(
        assetID: String,
        amount: KagemushaScaledAmount,
        currentNote: KagemushaSpendableNoteDescriptorV2,
        recordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        artifactGeneration: String,
        operationID: Data
    ) throws {
        let canonicalAssetID = try KagemushaRecursiveSpendRequestCodecs.canonicalAssetId(
            assetID,
            field: "assetID"
        )
        let carrier = try KagemushaRecursiveSpendableNoteDescriptor(
            noteCommitment: currentNote.noteCommitment,
            spendNullifier: currentNote.spendNullifier,
            amount: amount.atomicUnits
        )
        let topUpEvidence = try KagemushaRecursiveSpendTopUpInitRequest(
            recordBundle: recordBundle,
            pallasOpenEnvelopes: pallasOpenEnvelopesArchive,
            currentNote: carrier
        )
        let evidenceArchive = try KagemushaRecursiveSpendRequestCodecs.encodeTopUpInitRequest(
            topUpEvidence
        )
        let evidenceSummary = try KagemushaRecursiveSpendRequestCodecs.topUpInitRequestSummary(
            evidenceArchive
        )
        try KagemushaRecursiveSpendV2.requirePortableText(
            artifactGeneration,
            field: "artifactGeneration"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        guard currentNote.amount == amount,
              evidenceSummary.assetDefinitionId == currentNote.assetDefinitionID,
              evidenceSummary.amount == amount.atomicUnits else {
            throw KagemushaRecursiveSpendV2Error.invalidField("topUpUnsigned")
        }
        self.assetID = canonicalAssetID
        self.amount = amount
        self.currentNote = currentNote
        self.recordBundle = Data(recordBundle)
        self.pallasOpenEnvelopesArchive = Data(pallasOpenEnvelopesArchive)
        self.artifactGeneration = artifactGeneration
        self.operationID = Data(operationID)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeTopUpUnsigned(self)
    }

    public func authorizationPayloadDigest() throws -> Data {
        let archive = try noritoEncoded()
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpUnsignedPayloadDigestV2(
                unsignedArchive: archive
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            digest,
            field: "topUpUnsigned.payloadDigest"
        )
        return digest
    }

    public func finalize(
        authorization: KagemushaRequestAuthorizationV2
    ) throws -> KagemushaRecursiveSpendTopUpRequestV2 {
        let unsignedArchive = try noritoEncoded()
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == (try authorizationPayloadDigest()) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("authorization")
        }
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpFinalizeRequestV2(
                unsignedArchive: unsignedArchive,
                authorizationArchive: authorization.archive
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendTopUpRequestV2(
            unsigned: self,
            authorization: authorization,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecursiveSpendTopUpRequestV2: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendTopUpUnsignedV2
    public let authorization: KagemushaRequestAuthorizationV2
    public let archive: Data

    public var assetID: String { unsigned.assetID }
    public var amount: KagemushaScaledAmount { unsigned.amount }
    public var currentNote: KagemushaSpendableNoteDescriptorV2 { unsigned.currentNote }
    public var recordBundle: Data { unsigned.recordBundle }
    public var pallasOpenEnvelopesArchive: Data { unsigned.pallasOpenEnvelopesArchive }
    public var artifactGeneration: String { unsigned.artifactGeneration }
    public var operationID: Data { unsigned.operationID }

    init(
        unsigned: KagemushaRecursiveSpendTopUpUnsignedV2,
        authorization: KagemushaRequestAuthorizationV2,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.topUpRequestWireName,
            field: "topUpRequest"
        )
        self.unsigned = unsigned
        self.authorization = authorization
        self.archive = Data(archive)
        guard try KagemushaRecursiveSpendV2Codecs.encodeTopUpRequest(self) == archive else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("topUpRequest.canonical")
        }
    }

    public func noritoEncoded() -> Data { archive }
}

/// Immutable chain-finality receipt consumed by the local init prover. A
/// wallet must never construct hop-0 cash from the pre-finality top-up request.
public struct KagemushaRecursiveSpendTopUpAnchorV2: Equatable, Sendable {
    public let version: UInt16
    public let chainID: String
    public let payer: String
    public let assetID: String
    public let assetScale: UInt32
    public let amount: KagemushaScaledAmount
    public let initialRoot: Data
    public let finalizedRoot: Data
    public let topUpAnchorNullifiers: [Data]
    public let currentNote: KagemushaSpendableNoteDescriptorV2
    public let topUpOperationID: Data
    public let transferVerifierID: String
    public let transferVerifierCommitment: Data
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
        topUpAnchorNullifiers: [Data],
        currentNote: KagemushaSpendableNoteDescriptorV2,
        topUpOperationID: Data,
        transferVerifierID: String,
        transferVerifierCommitment: Data,
        artifactGeneration: String,
        finalizedHeight: UInt64,
        finalizedTransactionHash: Data,
        anchorDigest: Data,
        archive: Data
    ) throws {
        let canonicalAssetID: String
        do {
            canonicalAssetID = try KagemushaRecursiveSpendRequestCodecs.canonicalAssetId(
                assetID,
                field: "assetID"
            )
        } catch {
            throw KagemushaRecursiveSpendV2Error.invalidField("topUpAnchor")
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
              (1...KagemushaRecursiveSpendV2.maximumInputNullifiers)
                .contains(topUpAnchorNullifiers.count),
              initialRoot != finalizedRoot,
              !topUpAnchorNullifiers.contains(currentNote.noteCommitment),
              !topUpAnchorNullifiers.contains(currentNote.spendNullifier),
              finalizedHeight > 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("topUpAnchor")
        }
        try KagemushaRecursiveSpendV2.requirePortableText(chainID, field: "chainID")
        try KagemushaRecursiveSpendV2.requirePortableText(payer, field: "payer")
        try KagemushaRecursiveSpendV2.requirePortableText(assetID, field: "assetID")
        try KagemushaRecursiveSpendV2.requirePortableText(
            transferVerifierID,
            field: "transferVerifierID"
        )
        try KagemushaRecursiveSpendV2.requirePortableText(
            artifactGeneration,
            field: "artifactGeneration"
        )
        for (field, value) in [
            ("initialRoot", initialRoot),
            ("finalizedRoot", finalizedRoot),
            ("topUpOperationID", topUpOperationID),
            ("transferVerifierCommitment", transferVerifierCommitment),
            ("finalizedTransactionHash", finalizedTransactionHash),
            ("anchorDigest", anchorDigest),
        ] {
            try KagemushaRecursiveSpendV2.requireNonzeroFixed32(value, field: field)
        }
        try topUpAnchorNullifiers.forEach {
            try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
                $0,
                field: "topUpAnchorNullifiers"
            )
        }
        for index in topUpAnchorNullifiers.indices where index > topUpAnchorNullifiers.startIndex {
            guard topUpAnchorNullifiers[index - 1]
                .lexicographicallyPrecedes(topUpAnchorNullifiers[index]) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("topUpAnchorNullifiers.order")
            }
        }
        self.version = version
        self.chainID = chainID
        self.payer = payer
        self.assetID = assetID
        self.assetScale = assetScale
        self.amount = amount
        self.initialRoot = Data(initialRoot)
        self.finalizedRoot = Data(finalizedRoot)
        self.topUpAnchorNullifiers = topUpAnchorNullifiers.map { Data($0) }
        self.currentNote = currentNote
        self.topUpOperationID = Data(topUpOperationID)
        self.transferVerifierID = transferVerifierID
        self.transferVerifierCommitment = Data(transferVerifierCommitment)
        self.artifactGeneration = artifactGeneration
        self.finalizedHeight = finalizedHeight
        self.finalizedTransactionHash = Data(finalizedTransactionHash)
        self.anchorDigest = Data(anchorDigest)
        self.archive = Data(archive)
    }

    public static func decode(_ archive: Data) throws -> Self {
        try KagemushaRecursiveSpendV2Codecs.decodeTopUpAnchor(archive)
    }

    public func compactReference() throws -> KagemushaRecursiveSpendTopUpAnchorRefV2 {
        try KagemushaRecursiveSpendTopUpAnchorRefV2(
            topUpOperationID: topUpOperationID,
            anchorDigest: anchorDigest
        )
    }
}

/// Compact chain-resolvable top-up identity carried by peer bundles.
public struct KagemushaRecursiveSpendTopUpAnchorRefV2: Equatable, Hashable, Sendable {
    public let topUpOperationID: Data
    public let anchorDigest: Data

    public init(topUpOperationID: Data, anchorDigest: Data) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            topUpOperationID,
            field: "topUpAnchorRef.topUpOperationID"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
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
public struct KagemushaTopUpFinalityProofArchiveV2: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpendV2.topUpFinalityProofWireName,
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
public struct KagemushaTopUpFinalityRosterArtifactArchiveV2: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpendV2.topUpFinalityRosterArtifactWireName,
            field: "topUpFinalityRosterArtifact"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Canonical authenticated V3 release manifest passed opaquely to the native
/// artifact loader. Application code never derives proof parameters from it.
public struct KagemushaRecursiveSpendArtifactManifestArchiveV3: Equatable, Sendable {
    public let noritoArchive: Data
    public let sha256: Data

    public init(noritoArchive: Data, expectedSHA256: Data) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            expectedSHA256,
            field: "artifactManifest.sha256"
        )
        try KagemushaRecursiveSpendV2.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpendV2.artifactManifestWireName,
            field: "artifactManifest"
        )
        guard Data(SHA256.hash(data: noritoArchive)) == expectedSHA256 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifactManifest.sha256")
        }
        self.noritoArchive = Data(noritoArchive)
        self.sha256 = Data(expectedSHA256)
    }
}

public struct KagemushaRecursiveSpendInputBranchV2: Equatable, Sendable {
    public let bundleDigest: Data
    public let inputNote: KagemushaSpendableNoteDescriptorV2
    public let branchClaims: [KagemushaRecursiveSpendBranchClaimV2]
    public let inputRoot: Data
    public let proofStepCount: UInt32
    public let peerHopCount: UInt32

    init(
        bundleDigest: Data,
        inputNote: KagemushaSpendableNoteDescriptorV2,
        branchClaims: [KagemushaRecursiveSpendBranchClaimV2],
        inputRoot: Data,
        proofStepCount: UInt32,
        peerHopCount: UInt32
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            bundleDigest,
            field: "input.bundleDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            inputRoot,
            field: "input.inputRoot"
        )
        try KagemushaRecursiveSpendV2.validateBranchClaims(branchClaims)
        guard proofStepCount > 0,
              peerHopCount <= UInt32(KagemushaRecursiveSpendBranchPathV2.maximumDepth) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("input.hopCount")
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
public struct KagemushaRecursiveSpendSplitIntentBuildRequestV2: Equatable, Sendable {
    public let previousBundles: [KagemushaRecursiveSpendBundleV2]
    public let outputArtifactGeneration: String
    public let transferAmount: KagemushaScaledAmount
    public let recipientOutput: KagemushaSpendableNoteDescriptorV2
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let recipientRequestDigest: Data
    public let operationID: Data

    public init(
        previousBundles: [KagemushaRecursiveSpendBundleV2],
        outputArtifactGeneration: String,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        changeOutput: KagemushaSpendableNoteDescriptorV2? = nil,
        recipientRequest: KagemushaVerifiedRecipientPaymentRequestV2,
        operationID: Data
    ) throws {
        guard (1...2).contains(previousBundles.count) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("previousBundles")
        }
        for (previous, current) in zip(previousBundles, previousBundles.dropFirst()) {
            guard previous.summary.bundleDigest.lexicographicallyPrecedes(
                current.summary.bundleDigest
            ) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("previousBundles.order")
            }
        }
        try KagemushaRecursiveSpendV2.requirePortableText(
            outputArtifactGeneration,
            field: "outputArtifactGeneration"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        let request = recipientRequest.request.payload
        guard request.amount == transferAmount,
              request.recipientOutput == recipientOutput,
              previousBundles.allSatisfy({
                  $0.summary.assetDefinitionID == request.assetDefinitionID
                    && $0.summary.amount.scale == request.amount.scale
              }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("recipientRequest")
        }
        self.previousBundles = previousBundles
        self.outputArtifactGeneration = outputArtifactGeneration
        self.transferAmount = transferAmount
        self.recipientOutput = recipientOutput
        self.changeOutput = changeOutput
        self.recipientRequestDigest = Data(recipientRequest.digest)
        self.operationID = Data(operationID)
    }

    public func build() throws -> KagemushaRecursiveSpendSplitIntentV2 {
        let requestArchive = try KagemushaRecursiveSpendV2Codecs
            .encodeSplitIntentBuildRequest(self)
        guard let intentArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBuildSplitIntentV2(requestArchive: requestArchive) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        let intent = try KagemushaRecursiveSpendV2Codecs.decodeSplitIntent(intentArchive)
        guard intent.outputArtifactGeneration == outputArtifactGeneration,
              intent.transferAmount == transferAmount,
              intent.recipientOutput == recipientOutput,
              intent.changeOutput == changeOutput,
              intent.recipientRequestDigest == recipientRequestDigest,
              intent.operationID == operationID,
              intent.inputs.map(\.bundleDigest) == previousBundles.map(\.summary.bundleDigest)
        else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("splitIntent.factoryBinding")
        }
        return intent
    }
}

public struct KagemushaRecursiveSpendSplitIntentV2: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputs: [KagemushaRecursiveSpendInputBranchV2]
    public let topUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRefV2]
    public let assetScale: UInt32
    public let lineageMode: KagemushaRecursiveSpendLineageModeV2
    public let outputArtifactGeneration: String
    public let transferAmount: KagemushaScaledAmount
    public let recipientOutput: KagemushaSpendableNoteDescriptorV2
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let recipientRequestDigest: Data
    public let operationID: Data

    init(
        chainID: String,
        assetDefinitionID: String,
        inputs: [KagemushaRecursiveSpendInputBranchV2],
        topUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRefV2],
        assetScale: UInt32,
        lineageMode: KagemushaRecursiveSpendLineageModeV2,
        outputArtifactGeneration: String,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        changeOutput: KagemushaSpendableNoteDescriptorV2?,
        recipientRequestDigest: Data,
        operationID: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            recipientRequestDigest,
            field: "recipientRequestDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        guard (1...2).contains(inputs.count),
              (1...2).contains(topUpAnchorRefs.count),
              assetScale == transferAmount.scale,
              recipientOutput.amount == transferAmount else {
            throw KagemushaRecursiveSpendV2Error.invalidField("split.context")
        }
        try KagemushaRecursiveSpendV2.requirePortableText(
            outputArtifactGeneration,
            field: "outputArtifactGeneration"
        )
        for (previous, current) in zip(inputs, inputs.dropFirst()) {
            guard previous.bundleDigest.lexicographicallyPrecedes(current.bundleDigest) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("split.inputs.order")
            }
        }
        for (previous, current) in zip(topUpAnchorRefs, topUpAnchorRefs.dropFirst()) {
            guard previous.topUpOperationID.lexicographicallyPrecedes(
                current.topUpOperationID
            ) else {
                throw KagemushaRecursiveSpendV2Error.invalidField("split.topUpAnchorRefs.order")
            }
        }
        guard Set(topUpAnchorRefs.map(\.anchorDigest)).count == topUpAnchorRefs.count else {
            throw KagemushaRecursiveSpendV2Error.invalidField(
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
            throw KagemushaRecursiveSpendV2Error.invalidField("split.context")
        }
        var inputAtomicUnits = "0"
        for input in inputs {
            inputAtomicUnits = Self.add(inputAtomicUnits, input.inputNote.amount.atomicUnits)
            _ = try KagemushaScaledAmount(atomicUnits: inputAtomicUnits, scale: assetScale)
        }
        let consumedClaims = inputs.flatMap(\.branchClaims).sorted {
            $0.path.canonicallyPrecedes($1.path)
        }
        try KagemushaRecursiveSpendV2.validateBranchClaims(consumedClaims)
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
            throw KagemushaRecursiveSpendV2Error.invalidField(
                "split.topUpAnchorRefs.identity"
            )
        }
        if let changeOutput {
            guard transferAmount.atomicUnits != inputAtomicUnits,
                  Self.add(transferAmount.atomicUnits, changeOutput.amount.atomicUnits)
                    == inputAtomicUnits else {
                throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput.amount")
            }
        } else if transferAmount.atomicUnits != inputAtomicUnits {
            throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
        }
        let material = notes.flatMap { [$0.noteCommitment, $0.spendNullifier] }
        guard Set(material).count == material.count else {
            throw KagemushaRecursiveSpendV2Error.invalidField("split.noteMaterial")
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

public struct KagemushaRecursiveSpendBundleSummaryV2: Equatable, Sendable {
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let hopCount: UInt32
    public let branchClaims: [KagemushaRecursiveSpendBranchClaimV2]
    public let artifactGeneration: String
    public let verifierKeyID: String
    public let lineageMode: KagemushaRecursiveSpendLineageModeV2
    public let bundleDigest: Data
}

/// A proof-carrying bundle whose accumulator and proof bytes remain opaque.
/// Wallet code receives only the validated typed summary above.
public struct KagemushaRecursiveSpendBundleV2: Equatable, Sendable {
    public let archive: Data
    public let summary: KagemushaRecursiveSpendBundleSummaryV2

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "bundle"
        )
        guard let summaryArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBundleSummaryV2(bundleArchive: noritoArchive) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        self.archive = Data(noritoArchive)
        self.summary = try KagemushaRecursiveSpendV2Codecs.decodeBundleSummary(summaryArchive)
    }

    init(archive: Data, summary: KagemushaRecursiveSpendBundleSummaryV2) {
        self.archive = Data(archive)
        self.summary = summary
    }
}

public struct KagemushaRecursiveSpendAppendInputV2: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV2
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let previousRecursiveProofOpenEnvelopesArchive: Data

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV2,
        previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        previousRecursiveProofOpenEnvelopesArchive: Data = Data()
    ) throws {
        switch previousBundle.summary.lineageMode {
        case .reserved:
            guard previousLineageVerifierRecord != nil,
                  !previousRecursiveProofOpenEnvelopesArchive.isEmpty else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "previousInput.reservedWitness"
                )
            }
        case .semantic:
            guard previousLineageVerifierRecord == nil,
                  previousRecursiveProofOpenEnvelopesArchive.isEmpty else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
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

public struct KagemushaRecursiveSpendAppendRequestV2: Equatable, Sendable {
    public let previousInputs: [KagemushaRecursiveSpendAppendInputV2]
    public let recordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let split: KagemushaRecursiveSpendSplitIntentV2
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2?
    public let outputProofCircuitID: String
    public let blockHeight: UInt64

    public init(
        previousInputs: [KagemushaRecursiveSpendAppendInputV2],
        recordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        split: KagemushaRecursiveSpendSplitIntentV2,
        lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2? = nil,
        blockHeight: UInt64
    ) throws {
        _ = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
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
            throw KagemushaRecursiveSpendV2Error.invalidField("appendRequest")
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
            outputProofCircuitID = KagemushaRecursiveSpendV2.reservedAppendCircuitID
        case (.semantic, nil)
            where split.lineageMode == .semantic
                && previousInputs.allSatisfy({
                $0.previousBundle.summary.lineageMode == .semantic
            }):
            outputProofCircuitID = KagemushaRecursiveSpendV2.semanticCircuitID
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageArtifact")
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
        try KagemushaRecursiveSpendV2Codecs.encodeAppendRequest(self)
    }
}

public struct KagemushaRecursiveSpendSplitResultV2: Equatable, Sendable {
    public let split: KagemushaRecursiveSpendSplitIntentV2
    public let splitBindingDigest: Data
    public let recipientBundle: KagemushaRecursiveSpendBundleV2
    public let changeBundle: KagemushaRecursiveSpendBundleV2?
    public let archive: Data

    init(
        split: KagemushaRecursiveSpendSplitIntentV2,
        splitBindingDigest: Data,
        recipientBundle: KagemushaRecursiveSpendBundleV2,
        changeBundle: KagemushaRecursiveSpendBundleV2?,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            splitBindingDigest,
            field: "splitBindingDigest"
        )
        let expectedHopCount = (split.inputs.map(\.peerHopCount).max() ?? 0) + 1
        guard recipientBundle.summary.amount == split.transferAmount,
              recipientBundle.summary.noteCommitment == split.recipientOutput.noteCommitment,
              recipientBundle.summary.lineageMode == split.lineageMode,
              recipientBundle.summary.artifactGeneration == split.outputArtifactGeneration,
              recipientBundle.summary.hopCount == expectedHopCount else {
            throw KagemushaRecursiveSpendV2Error.invalidField("recipientBundle")
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
                throw KagemushaRecursiveSpendV2Error.invalidField("changeBundle")
            }
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("changeBundle")
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
public struct KagemushaRecursiveSpendPeerPaymentV2: Equatable, Sendable {
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let recipientBundle: KagemushaRecursiveSpendBundleV2
    public let archive: Data

    init(
        recipientBundle: KagemushaRecursiveSpendBundleV2,
        archive: Data
    ) throws {
        let identity = try KagemushaRecursiveSpendV2Codecs
            .recipientPeerSplitIdentity(from: recipientBundle.archive)
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.peerPaymentWireName,
            field: "peerPayment"
        )
        guard archive.count <= KagemushaRecursiveSpendV2.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("peerPayment.size")
        }
        self.operationID = identity.operationID
        self.recipientRequestDigest = identity.recipientRequestDigest
        self.recipientBundle = recipientBundle
        self.archive = Data(archive)
    }

    public static func create(
        recipientBundle: KagemushaRecursiveSpendBundleV2
    ) throws -> Self {
        let archive = try KagemushaRecursiveSpendV2Codecs.encodePeerPayment(
            recipientBundle: recipientBundle
        )
        return try Self(recipientBundle: recipientBundle, archive: archive)
    }

    public static func recipientOnly(
        from result: KagemushaRecursiveSpendSplitResultV2
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentFromSplitV2(
                splitResultArchive: result.archive
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try decode(archive)
    }

    public static func decode(_ archive: Data) throws -> Self {
        guard archive.count <= KagemushaRecursiveSpendV2.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("peerPayment.size")
        }
        guard let canonical = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentValidateV2(paymentArchive: archive) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        guard canonical == archive else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("peerPayment.canonical")
        }
        return try KagemushaRecursiveSpendV2Codecs.decodePeerPayment(canonical)
    }

    public func noritoEncoded() -> Data {
        archive
    }
}

public struct KagemushaRecursiveSpendVerifyRequestV2: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV2
    public let recipientRequest: KagemushaRecipientPaymentRequestV2
    public let maximumHops: UInt32
    public let artifactGeneration: String
    public let verifiedAtMilliseconds: UInt64
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let blockHeight: UInt64?

    public init(
        bundle: KagemushaRecursiveSpendBundleV2,
        recipientRequest: KagemushaRecipientPaymentRequestV2,
        maximumHops: UInt32,
        verifiedAtMilliseconds: UInt64,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        blockHeight: UInt64? = nil
    ) throws {
        guard maximumHops > 0,
              maximumHops <= 64,
              bundle.summary.hopCount <= maximumHops,
              verifiedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("verifyRequest")
        }
        self.bundle = bundle
        self.recipientRequest = recipientRequest
        self.maximumHops = maximumHops
        self.artifactGeneration = bundle.summary.artifactGeneration
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
        self.lineageVerifierRecord = lineageVerifierRecord
        self.blockHeight = blockHeight
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeVerifyRequest(self)
    }
}

public struct KagemushaRecursiveSpendLineageNodeV2: Equatable, Sendable {
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
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
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
              proofStepCount <= KagemushaRecursiveSpendV2.semanticMaximumHops + 1,
              verifiedAtBlockHeight > 0,
              !transitionArchive.isEmpty,
              transitionArchive.count
                <= KagemushaRecursiveSpendV2.semanticLineageMaximumNodeArchiveBytes else {
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageNode")
        }
        self.resultBundleDigest = Data(resultBundleDigest)
        self.parentBundleDigests = parentBundleDigests.map { Data($0) }
        self.proofStepCount = proofStepCount
        self.verifiedAtBlockHeight = verifiedAtBlockHeight
        self.transitionArchive = Data(transitionArchive)
    }
}

public struct KagemushaRecursiveSpendLineageWitnessV2: Equatable, Sendable {
    public let nodes: [KagemushaRecursiveSpendLineageNodeV2]
    public let finalBundleDigest: Data

    public init(
        nodes: [KagemushaRecursiveSpendLineageNodeV2],
        finalBundleDigest: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            finalBundleDigest,
            field: "finalBundleDigest"
        )
        try Self.validateCanonicalDAG(nodes: nodes, finalBundleDigest: finalBundleDigest)
        self.nodes = nodes
        self.finalBundleDigest = Data(finalBundleDigest)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeLineageWitness(self)
    }

    private static func validateCanonicalDAG(
        nodes: [KagemushaRecursiveSpendLineageNodeV2],
        finalBundleDigest: Data
    ) throws {
        guard !nodes.isEmpty,
              nodes.count <= KagemushaRecursiveSpendV2.semanticLineageMaximumNodes else {
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageWitness.nodes")
        }

        var nodeIndexes: [Data: Int] = [:]
        var childCounts: [Data: Int] = [:]
        var previousStep: UInt32?
        var previousDigest: Data?
        var rootCount = 0
        var totalArchiveBytes = 0

        for (index, node) in nodes.enumerated() {
            guard nodeIndexes[node.resultBundleDigest] == nil else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "lineageWitness.nodes.resultBundleDigest.duplicate"
                )
            }
            if let previousStep, let previousDigest {
                guard previousStep < node.proofStepCount
                        || (previousStep == node.proofStepCount
                            && previousDigest.lexicographicallyPrecedes(
                                node.resultBundleDigest
                            )) else {
                    throw KagemushaRecursiveSpendV2Error.invalidField(
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
                    <= KagemushaRecursiveSpendV2.semanticLineageMaximumTotalArchiveBytes else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
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
                        throw KagemushaRecursiveSpendV2Error.invalidField(
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
                    throw KagemushaRecursiveSpendV2Error.invalidField(
                        "lineageWitness.nodes.proofStepCount"
                    )
                }
                expectedStep = step
            }
            guard node.proofStepCount == expectedStep else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "lineageWitness.nodes.proofStepCount"
                )
            }
            guard node.verifiedAtBlockHeight >= maximumParentVerificationHeight else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "lineageWitness.nodes.verifiedAtBlockHeight"
                )
            }
            nodeIndexes[node.resultBundleDigest] = index
            childCounts[node.resultBundleDigest] = 0
        }

        guard (1...2).contains(rootCount) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageWitness.nodes.roots")
        }
        let sinks = childCounts.compactMap { digest, count in count == 0 ? digest : nil }
        guard sinks.count == 1,
              sinks[0] == finalBundleDigest,
              nodes.last?.resultBundleDigest == finalBundleDigest else {
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageWitness.nodes.sink")
        }

        var closure = Set<Data>()
        var pending = [finalBundleDigest]
        while let digest = pending.popLast() {
            guard closure.insert(digest).inserted else { continue }
            guard let index = nodeIndexes[digest] else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "lineageWitness.nodes.ancestorClosure"
                )
            }
            pending.append(contentsOf: nodes[index].parentBundleDigests)
        }
        guard closure.count == nodes.count else {
            throw KagemushaRecursiveSpendV2Error.invalidField(
                "lineageWitness.nodes.ancestorClosure"
            )
        }
    }
}

public struct KagemushaRecursiveSpendVerifyResultV2: Equatable, Sendable {
    public let valid: Bool
    public let chainAdmissible: Bool
    public let lineageRedeemable: Bool
    public let witnesslessRedemptionSupported: Bool
    public let lineageMode: KagemushaRecursiveSpendLineageModeV2
    public let summary: KagemushaRecursiveSpendBundleSummaryV2
    public let recipientRequestDigest: Data
    public let requestOutputBindingDigest: Data
    public let verifierKeyID: String
    public let verifierCircuitID: String
    public let verifierActivationHeight: UInt64?
    public let verifierWithdrawHeight: UInt64?
    public let verifiedAtBlockHeight: UInt64
    public let verifiedAtMilliseconds: UInt64
    public let verifiedLineageWitness: KagemushaRecursiveSpendLineageWitnessV2?
}

public struct KagemushaReceiverAcknowledgementPayloadV2: Equatable, Sendable {
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let recipientCommitment: Data
    public let acceptedAtMilliseconds: UInt64
    public let receiverDeviceID: String
    public let receiverKeyReference: Data
    public let receiverPublicKey: KagemushaPublicKeyV2
    public let archive: Data

    init(
        operationID: Data,
        recipientRequestDigest: Data,
        paymentBundleDigest: Data,
        recipientCommitment: Data,
        acceptedAtMilliseconds: UInt64,
        receiverDeviceID: String,
        receiverKeyReference: Data,
        receiverPublicKey: KagemushaPublicKeyV2,
        archive: Data
    ) throws {
        for (field, value) in [
            ("operationID", operationID),
            ("recipientRequestDigest", recipientRequestDigest),
            ("paymentBundleDigest", paymentBundleDigest),
            ("recipientCommitment", recipientCommitment),
            ("receiverKeyReference", receiverKeyReference),
        ] {
            try KagemushaRecursiveSpendV2.requireNonzeroFixed32(value, field: field)
        }
        guard acceptedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("acceptedAtMilliseconds")
        }
        try KagemushaRecursiveSpendV2.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.acknowledgementPayloadWireName,
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
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return bytes
    }
}

public struct KagemushaReceiverAcknowledgementV2: Equatable, Sendable {
    public let payload: KagemushaReceiverAcknowledgementPayloadV2
    public let signature: Data
    public let archive: Data

    public static func prepare(
        request: KagemushaRecipientPaymentRequestV2,
        payment: KagemushaRecursiveSpendPeerPaymentV2,
        acceptedAtMilliseconds: UInt64
    ) throws -> KagemushaReceiverAcknowledgementPayloadV2 {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementPayloadV2(
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive,
            acceptedAtMilliseconds: acceptedAtMilliseconds
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendV2Codecs.decodeAcknowledgementPayload(archive)
    }

    public static func create(
        payload: KagemushaReceiverAcknowledgementPayloadV2,
        signature: Data,
        request: KagemushaRecipientPaymentRequestV2,
        payment: KagemushaRecursiveSpendPeerPaymentV2
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementCreateV2(
            payloadArchive: payload.archive,
            signature: signature,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try Self(payload: payload, signature: signature, archive: archive)
    }

    init(payload: KagemushaReceiverAcknowledgementPayloadV2, signature: Data, archive: Data) throws {
        guard !signature.isEmpty else {
            throw KagemushaRecursiveSpendV2Error.invalidField("acknowledgement.signature")
        }
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.acknowledgementWireName,
            field: "acknowledgement"
        )
        self.payload = payload
        self.signature = Data(signature)
        self.archive = Data(archive)
    }

    /// Sender-side commit gate. Inputs must remain reserved until this succeeds
    /// and the application confirms the receiver key's registered-device lineage.
    public func verifiedForSender(
        request: KagemushaRecipientPaymentRequestV2,
        payment: KagemushaRecursiveSpendPeerPaymentV2
    ) throws -> KagemushaReceiverAcknowledgementVerifyResultV2 {
        guard let result = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementVerifyV2(
            acknowledgementArchive: archive,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendV2Codecs.decodeAcknowledgementVerifyResult(result)
    }
}

public struct KagemushaReceiverAcknowledgementVerifyResultV2: Equatable, Sendable {
    public let valid: Bool
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let acknowledgementDigest: Data
}

public struct KagemushaUnshieldPublicInputsBindingV2: Equatable, Sendable {
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
            throw KagemushaRecursiveSpendV2Error.invalidField("unshieldPublicInputs")
        }
        for (field, values) in [
            ("inputCommitments", inputCommitments),
            ("nullifiers", nullifiers),
        ] {
            guard values.allSatisfy({ $0.count == 32 }) else {
                throw KagemushaRecursiveSpendV2Error.invalidField(field)
            }
        }
        for (field, value) in [
            ("changeOutputCommitment", changeOutputCommitment),
            ("root", root),
            ("publicAmount", publicAmount),
            ("assetTag", assetTag),
            ("chainTag", chainTag),
        ] where value.count != 32 {
            throw KagemushaRecursiveSpendV2Error.invalidField(field)
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
public struct KagemushaRecursiveSpendRedemptionIntentBuildRequestV2: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV2
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let changeArtifactGeneration: String?
    public let unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2
    public let unshieldPublicInputsDigest: Data
    public let operationID: Data

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV2,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptorV2? = nil,
        changeArtifactGeneration: String? = nil,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            unshieldPublicInputsDigest,
            field: "unshieldPublicInputsDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        guard publicAmount.scale == previousBundle.summary.amount.scale,
              KagemushaScaledAmount.compareAtomicUnits(
                  publicAmount.atomicUnits,
                  previousBundle.summary.amount.atomicUnits
              ) != .orderedDescending else {
            throw KagemushaRecursiveSpendV2Error.invalidField("publicAmount")
        }
        switch (
            changeOutput,
            changeArtifactGeneration,
            publicAmount.atomicUnits == previousBundle.summary.amount.atomicUnits
        ) {
        case (nil, nil, true): break
        case let (.some(change), .some(generation), false):
            try KagemushaRecursiveSpendV2.requirePortableText(
                generation,
                field: "changeArtifactGeneration"
            )
            guard change.assetDefinitionID == previousBundle.summary.assetDefinitionID,
                  change.amount.scale == publicAmount.scale,
                  KagemushaRecursiveSpendSplitIntentV2.addForValidation(
                      publicAmount.atomicUnits,
                      change.amount.atomicUnits
                  ) == previousBundle.summary.amount.atomicUnits,
                  change.noteCommitment == unshieldPublicInputs.changeOutputCommitment else {
                throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
            }
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
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

    public func build() throws -> KagemushaRecursiveSpendRedemptionIntentV2 {
        let requestArchive = try KagemushaRecursiveSpendV2Codecs
            .encodeRedemptionIntentBuildRequest(self)
        guard let intentArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBuildRedemptionIntentV2(
                requestArchive: requestArchive
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        let intent = try KagemushaRecursiveSpendV2Codecs
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive(
                "redemptionIntent.factoryBinding"
            )
        }
        return intent
    }
}

public struct KagemushaRecursiveSpendRedemptionIntentV2: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputNote: KagemushaSpendableNoteDescriptorV2
    public let parentBranchClaims: [KagemushaRecursiveSpendBranchClaimV2]
    public let parentTopUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRefV2]
    public let parentProofStepCount: UInt32
    public let parentPeerHopCount: UInt32
    public let parentBundleDigest: Data
    public let inputRoot: Data
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let changeArtifactGeneration: String?
    public let unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2
    public let unshieldPublicInputsDigest: Data
    public let operationID: Data

    init(
        chainID: String,
        assetDefinitionID: String,
        inputNote: KagemushaSpendableNoteDescriptorV2,
        parentBranchClaims: [KagemushaRecursiveSpendBranchClaimV2],
        parentTopUpAnchorRefs: [KagemushaRecursiveSpendTopUpAnchorRefV2],
        parentProofStepCount: UInt32,
        parentPeerHopCount: UInt32,
        parentBundleDigest: Data,
        inputRoot: Data,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptorV2?,
        changeArtifactGeneration: String?,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            parentBundleDigest,
            field: "parentBundleDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(inputRoot, field: "inputRoot")
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            unshieldPublicInputsDigest,
            field: "unshieldPublicInputsDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        try KagemushaRecursiveSpendV2.validateBranchClaims(parentBranchClaims)
        guard (1...2).contains(parentTopUpAnchorRefs.count),
              parentProofStepCount > 0,
              parentPeerHopCount <= UInt32(KagemushaRecursiveSpendBranchPathV2.maximumDepth),
              inputNote.chainID == chainID,
              inputNote.assetDefinitionID == assetDefinitionID,
              publicAmount.scale == inputNote.amount.scale else {
            throw KagemushaRecursiveSpendV2Error.invalidField("redemptionIntent")
        }
        for (previous, current) in zip(
            parentTopUpAnchorRefs,
            parentTopUpAnchorRefs.dropFirst()
        ) {
            guard previous.topUpOperationID.lexicographicallyPrecedes(
                current.topUpOperationID
            ) else {
                throw KagemushaRecursiveSpendV2Error.invalidField(
                    "parentTopUpAnchorRefs.order"
                )
            }
        }
        switch (changeOutput, changeArtifactGeneration) {
        case (nil, nil):
            guard publicAmount.atomicUnits == inputNote.amount.atomicUnits,
                  unshieldPublicInputs.changeOutputCommitment == Data(repeating: 0, count: 32)
            else {
                throw KagemushaRecursiveSpendV2Error.invalidField("publicAmount")
            }
        case let (.some(change), .some(generation)):
            try KagemushaRecursiveSpendV2.requirePortableText(
                generation,
                field: "changeArtifactGeneration"
            )
            guard change.chainID == chainID,
                  change.assetDefinitionID == assetDefinitionID,
                  change.amount.scale == publicAmount.scale,
                  KagemushaRecursiveSpendSplitIntentV2.addForValidation(
                      publicAmount.atomicUnits,
                      change.amount.atomicUnits
                  ) == inputNote.amount.atomicUnits,
                  change.noteCommitment == unshieldPublicInputs.changeOutputCommitment else {
                throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
            }
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
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

public struct KagemushaRecursiveSpendRedeemChangeBranchV2: Equatable, Sendable {
    public let output: KagemushaSpendableNoteDescriptorV2
    public let branchClaims: [KagemushaRecursiveSpendBranchClaimV2]
    public let bundle: KagemushaRecursiveSpendBundleV2
}

public struct KagemushaRecursiveSpendRedeemChangeBuildRequestV2: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV2
    public let previousRecursiveProofOpenEnvelopesArchive: Data
    public let unshieldRecordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let redemption: KagemushaRecursiveSpendRedemptionIntentV2
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef
    public let blockHeight: UInt64

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV2,
        previousRecursiveProofOpenEnvelopesArchive: Data,
        unshieldRecordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        redemption: KagemushaRecursiveSpendRedemptionIntentV2,
        lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2,
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
            throw KagemushaRecursiveSpendV2Error.invalidField("redeemChangeBuildRequest")
        }
        _ = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            unshieldRecordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
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
        try KagemushaRecursiveSpendV2Codecs.encodeRedeemChangeBuildRequest(self)
    }
}

public struct KagemushaRecursiveSpendRedeemChangeBuildResultV2: Equatable, Sendable {
    public let changeBranch: KagemushaRecursiveSpendRedeemChangeBranchV2
    public let transitionBindingDigest: Data
    public let publicStatementDigest: Data
}

public struct KagemushaRecursiveSpendRedeemUnsignedV2: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV2
    public let recipient: String
    public let amount: KagemushaScaledAmount
    public let redeemProof: Data
    public let redemption: KagemushaRecursiveSpendRedemptionIntentV2
    public let lineageWitness: KagemushaRecursiveSpendLineageWitnessV2?
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef
    public let offlineChange: KagemushaRecursiveSpendRedeemChangeBranchV2?
    public let blockHeight: UInt64
    public let operationID: Data

    public init(
        bundle: KagemushaRecursiveSpendBundleV2,
        recipient: String,
        amount: KagemushaScaledAmount,
        redeemProof: Data,
        redemption: KagemushaRecursiveSpendRedemptionIntentV2,
        lineageWitness: KagemushaRecursiveSpendLineageWitnessV2?,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef,
        offlineChange: KagemushaRecursiveSpendRedeemChangeBranchV2? = nil,
        blockHeight: UInt64,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpendV2.requireArchive(
            redeemProof,
            schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName,
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
            throw KagemushaRecursiveSpendV2Error.invalidField("redeemUnsigned")
        }
        if let offlineChange {
            guard offlineChange.output == redemption.changeOutput,
                  offlineChange.bundle.summary.artifactGeneration
                    == redemption.changeArtifactGeneration else {
                throw KagemushaRecursiveSpendV2Error.invalidField("offlineChange")
            }
        }
        switch (bundle.summary.lineageMode, lineageWitness) {
        case (.reserved, nil):
            break
        case (.semantic, .some)
            where bundle.summary.hopCount <= KagemushaRecursiveSpendV2.semanticMaximumHops:
            break
        default:
            throw KagemushaRecursiveSpendV2Error.invalidField("lineageWitness")
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
        try KagemushaRecursiveSpendV2Codecs.encodeRedeemUnsigned(self)
    }

    public func authorizationPayloadDigest() throws -> Data {
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemUnsignedPayloadDigestV2(
                unsignedArchive: noritoEncoded()
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            digest,
            field: "redeemUnsigned.payloadDigest"
        )
        return digest
    }

    public func finalize(
        authorization: KagemushaRequestAuthorizationV2
    ) throws -> KagemushaRecursiveSpendRedeemRequestV2 {
        let unsignedArchive = try noritoEncoded()
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == (try authorizationPayloadDigest()) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("authorization")
        }
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemFinalizeRequestV2(
                unsignedArchive: unsignedArchive,
                authorizationArchive: authorization.archive
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendRedeemRequestV2(
            unsigned: self,
            authorization: authorization,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecursiveSpendRedeemRequestV2: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendRedeemUnsignedV2
    public let authorization: KagemushaRequestAuthorizationV2
    public let archive: Data

    public var bundle: KagemushaRecursiveSpendBundleV2 { unsigned.bundle }
    public var recipient: String { unsigned.recipient }
    public var amount: KagemushaScaledAmount { unsigned.amount }
    public var redeemProof: Data { unsigned.redeemProof }
    public var redemption: KagemushaRecursiveSpendRedemptionIntentV2 { unsigned.redemption }
    public var lineageWitness: KagemushaRecursiveSpendLineageWitnessV2? {
        unsigned.lineageWitness
    }
    public var lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef {
        unsigned.lineageVerifierRecord
    }
    public var offlineChange: KagemushaRecursiveSpendRedeemChangeBranchV2? {
        unsigned.offlineChange
    }
    public var blockHeight: UInt64 { unsigned.blockHeight }
    public var operationID: Data { unsigned.operationID }

    init(
        unsigned: KagemushaRecursiveSpendRedeemUnsignedV2,
        authorization: KagemushaRequestAuthorizationV2,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireArchive(
            archive,
            schema: KagemushaRecursiveSpendV2.redeemRequestWireName,
            field: "redeemRequest"
        )
        self.unsigned = unsigned
        self.authorization = authorization
        self.archive = Data(archive)
        guard try KagemushaRecursiveSpendV2Codecs.encodeRedeemRequest(self) == archive else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("redeemRequest.canonical")
        }
    }

    public func noritoEncoded() -> Data { archive }
}

public struct KagemushaRecursiveSpendRedeemResultV2: Equatable, Sendable {
    public let redeemRequestArchive: Data
    public let offlineChangeBundle: KagemushaRecursiveSpendBundleV2?
    public let operationID: Data
}

/// Owns one native streaming handle. Chunks are written directly to the Rust
/// spool; the Swift wrapper never concatenates the complete artifact.
public final class KagemushaRecursiveSpendArtifactIngestV2: @unchecked Sendable {
    public let reference: KagemushaRecursiveSpendArtifactReferenceV2
    private var handle: UInt64?
    private let lock = NSLock()

    public init(reference: KagemushaRecursiveSpendArtifactReferenceV2) throws {
        guard let expectedRole = reference.role.nativeExpectedRole else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.role")
        }
        let archive = try KagemushaRecursiveSpendV2Codecs.encodeArtifactReference(reference)
        guard let handle = try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactBeginV2(
            referenceArchive: archive,
            expectedRole: expectedRole
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        self.reference = reference
        self.handle = handle
    }

    deinit {
        lock.lock()
        let active = handle
        handle = nil
        lock.unlock()
        if let active {
            _ = try? NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactCancelV2(handle: active)
        }
    }

    public func write(_ chunk: Data) throws {
        guard !chunk.isEmpty else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.chunk")
        }
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.handle")
        }
        guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactWriteV2(
            handle: handle,
            chunk: chunk
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
    }

    public func finalize() throws {
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.handle")
        }
        guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactFinalizeV2(
            handle: handle
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        // The finalized handle remains owned so deinit/cancel releases the
        // local package after the caller finishes all proofs for this session.
    }

    public func cancel() throws {
        lock.lock()
        defer { lock.unlock() }
        guard let active = handle else { return }
        guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactCancelV2(
            handle: active
        ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        handle = nil
    }
}

/// Owns one ABI-18 V3 streaming handle. `write` accepts chunks of the complete
/// published `KRV3KEY` file and never exposes or parses its header or payload.
/// Native finalization re-parses and authenticates the held file descriptor.
public final class KagemushaRecursiveSpendArtifactIngestV3: @unchecked Sendable {
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchiveV3
    public let artifactSHA256: Data
    private var handle: UInt64?
    private let lock = NSLock()

    public init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchiveV3,
        expectedArtifactSHA256: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            expectedArtifactSHA256,
            field: "artifact.sha256"
        )
        guard let handle = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendArtifactBeginV3(
                manifestArchive: manifest.noritoArchive,
                expectedManifestSHA256: manifest.sha256,
                expectedArtifactSHA256: expectedArtifactSHA256
            ) else {
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
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
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.chunk")
        }
        lock.lock()
        defer { lock.unlock() }
        guard let handle else {
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.handle")
        }
        do {
            guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactWriteV3(
                handle: handle,
                chunk: chunk
            ) else {
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
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
            throw KagemushaRecursiveSpendV2Error.invalidField("artifact.handle")
        }
        do {
            guard try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactFinalizeV3(handle: handle) else {
                throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
            }
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
            throw KagemushaRecursiveSpendV2Error.nativeBridgeUnavailable
        }
        handle = nil
    }
}

private extension KagemushaRecursiveSpendSplitIntentV2 {
    static func addForValidation(_ lhs: String, _ rhs: String) -> String {
        add(lhs, rhs)
    }
}
