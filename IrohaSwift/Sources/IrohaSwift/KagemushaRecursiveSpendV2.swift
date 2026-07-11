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
            return "The ABI-17 Kagemusha recursive spend V2 bridge is unavailable."
        case .proofBackendUnavailable:
            return "Kagemusha recursive spend V2 is unavailable until the branch-safe proof backend is linked."
        }
    }
}

/// Availability, canonical wire names, and high-level native entrypoints for
/// exact-amount branch-safe recursive offline cash.
public enum KagemushaRecursiveSpendV2 {
    public static let requiredNativeBridgeAbiVersion: UInt32 = 17
    /// This remains false until init/append/verify/redeem all call the audited
    /// V2 circuit and chain implementation in the same source revision.
    public static let isProofBackendAvailable = false

    public static let scaledAmountWireName = wire("KagemushaScaledAmountV2")
    public static let noteWireName = wire("KagemushaSpendableNoteDescriptorV2")
    public static let branchPathWireName = wire("KagemushaRecursiveSpendBranchPathV2")
    public static let recipientRequestPayloadWireName =
        wire("KagemushaRecipientPaymentRequestSigningPayloadV2")
    public static let recipientRequestWireName = wire("KagemushaRecipientPaymentRequestV2")
    public static let authorizationWireName = wire("KagemushaRequestAuthorizationV2")
    public static let artifactReferenceWireName =
        wire("KagemushaRecursiveSpendArtifactReferenceV2")
    public static let initRequestWireName = wire("KagemushaRecursiveSpendInitRequestV2")
    public static let topUpRequestWireName = wire("KagemushaRecursiveSpendTopUpRequestV2")
    public static let topUpAnchorWireName = wire("KagemushaRecursiveSpendTopUpAnchorV2")
    public static let splitIntentWireName = wire("KagemushaRecursiveSpendSplitIntentV2")
    public static let appendRequestWireName = wire("KagemushaRecursiveSpendAppendRequestV2")
    public static let branchWireName = wire("KagemushaRecursiveSpendBranchV2")
    public static let lineageModeWireName = wire("KagemushaRecursiveSpendLineageModeV2")
    public static let bundleWireName = wire("KagemushaRecursiveSpendBundleV2")
    public static let bundleSummaryWireName = wire("KagemushaRecursiveSpendBundleSummaryV2")
    public static let splitResultWireName = wire("KagemushaRecursiveSpendSplitResultV2")
    public static let verifyRequestWireName = wire("KagemushaRecursiveSpendVerifyRequestV2")
    public static let verifyResultWireName = wire("KagemushaRecursiveSpendVerifyResultV2")
    public static let lineageWitnessWireName =
        wire("KagemushaRecursiveSpendLineageWitnessV2")
    public static let acknowledgementPayloadWireName =
        wire("KagemushaReceiverAcknowledgementPayloadV2")
    public static let acknowledgementWireName = wire("KagemushaReceiverAcknowledgementV2")
    public static let acknowledgementVerifyResultWireName =
        wire("KagemushaReceiverAcknowledgementVerifyResultV2")
    public static let redeemRequestWireName = wire("KagemushaRecursiveSpendRedeemRequestV2")
    public static let redeemResultWireName = wire("KagemushaRecursiveSpendRedeemResultV2")
    public static let redemptionIntentWireName =
        wire("KagemushaRecursiveSpendRedemptionIntentV2")
    public static let unshieldBindingWireName = wire("KagemushaUnshieldPublicInputsBindingV2")
    public static let redeemChangeBranchWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBranchV2")
    public static let redeemChangeBuildRequestWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBuildRequestV2")
    public static let redeemChangeBuildResultWireName =
        wire("KagemushaRecursiveSpendRedeemChangeBuildResultV2")

    public static let reservedInitCircuitID = "kagemusha-recursive-spend-reserved-init-v2"
    public static let reservedAppendCircuitID = "kagemusha-recursive-spend-reserved-append-v2"
    public static let reservedRedeemChangeCircuitID =
        "kagemusha-recursive-spend-reserved-redeem-change-v2"
    public static let lineageArtifactType = "KagemushaRecursiveSpendLineageKeyArtifactsV2"
    public static let maximumPeerTextEnvelopeBytes = 12 * 1024
    /// Largest raw archive whose unpadded base64url representation plus the
    /// six-byte `PKK2?.` prefix still fits the 12 KiB transport envelope.
    public static let maximumPeerArchiveBytes = 9_211
    public static let maximumAuthorizationTTLMilliseconds: UInt64 = 5 * 60 * 1_000

    public static let requiredProofSymbols = [
        "connect_norito_kagemusha_recursive_spend_init_v2",
        "connect_norito_kagemusha_recursive_spend_topup_v2",
        "connect_norito_kagemusha_recursive_spend_append_v2",
        "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
        "connect_norito_kagemusha_recursive_spend_verify_v2",
        "connect_norito_kagemusha_recursive_spend_redeem_v2",
    ]

    public static let requiredProtocolSymbols = [
        "connect_norito_kagemusha_receiver_key_reference_v2",
        "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
        "connect_norito_kagemusha_recipient_payment_request_create_v2",
        "connect_norito_kagemusha_recipient_payment_request_verify_v2",
        "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
        "connect_norito_kagemusha_request_authorization_create_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v2",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v2",
        "connect_norito_kagemusha_recursive_spend_artifact_write_v2",
        "connect_norito_kagemusha_recursive_spend_artifact_finalize_v2",
        "connect_norito_kagemusha_recursive_spend_artifact_cancel_v2",
    ]

    /// Backward-compatible inventory name used by existing readiness checks.
    public static let requiredNativeSymbols = requiredProofSymbols + requiredProtocolSymbols

    public static func ensureProofBackendAvailable() throws {
        guard isProofBackendAvailable else {
            throw KagemushaRecursiveSpendV2Error.proofBackendUnavailable
        }
    }

    public static var isNativeStubAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendV2StubAvailable
    }

    public static func initSpend(
        request: KagemushaRecursiveSpendInitRequestV2,
        anchor: KagemushaRecursiveSpendTopUpAnchorV2
    ) throws -> Data {
        let requestArchive = try request.noritoEncoded()
        guard request.operationID == anchor.topUpOperationID,
              request.currentNote == anchor.currentNote,
              request.amount == anchor.amount,
              request.lineageArtifact.generation == anchor.artifactGeneration else {
            throw KagemushaRecursiveSpendV2Error.invalidField("topUpAnchor")
        }
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendInitV2(
                requestArchive: requestArchive,
                topUpAnchorArchive: anchor.archive
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

    public static func appendSpend(
        requestArchive: Data,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequestV2,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data {
        try requireArchive(requestArchive, schema: appendRequestWireName, field: "requestArchive")
        guard verifiedAtMilliseconds == signedRecipientRequest.verifiedAtMilliseconds else {
            throw KagemushaRecursiveSpendV2Error.invalidField("verifiedAtMilliseconds")
        }
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
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendVerifyV2(requestArchive: requestArchive)
        }
    }

    public static func proveRedeemChange(
        request: KagemushaRecursiveSpendRedeemChangeBuildRequestV2
    ) throws -> KagemushaRecursiveSpendRedeemChangeBuildResultV2 {
        let archive = try request.noritoEncoded()
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
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendRedeemV2(requestArchive: requestArchive)
        }
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

    static func requirePortableText(_ value: String, field: String, maximum: Int = 128) throws {
        guard !value.isEmpty,
              value.count <= maximum,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw KagemushaRecursiveSpendV2Error.invalidField(field)
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
        self.appAttestEvidenceSHA256 = appAttestEvidenceSHA256.map { Data(bytes: $0) }
        self.appAttestEvidence = appAttestEvidence.map { Data(bytes: $0) }
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
    public let initRequest: KagemushaRecursiveSpendInitRequest
    public let amount: KagemushaScaledAmount
    public let currentNote: KagemushaSpendableNoteDescriptorV2
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2
    public let operationID: Data

    public init(
        initRequest: KagemushaRecursiveSpendInitRequest,
        amount: KagemushaScaledAmount,
        currentNote: KagemushaSpendableNoteDescriptorV2,
        lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2,
        operationID: Data
    ) throws {
        guard initRequest.currentNote.amount == amount.atomicUnits,
              currentNote.amount == amount,
              lineageArtifact.role == .lineageInitProver else {
            throw KagemushaRecursiveSpendV2Error.invalidField("initRequest")
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        self.initRequest = initRequest
        self.amount = amount
        self.currentNote = currentNote
        self.lineageArtifact = lineageArtifact
        self.operationID = Data(operationID)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeInitRequest(self)
    }
}

public struct KagemushaRecursiveSpendTopUpRequestV2: Equatable, Sendable {
    public let assetID: String
    public let initRequest: KagemushaRecursiveSpendInitRequestV2
    public let authorization: KagemushaRequestAuthorizationV2

    public init(
        assetID: String,
        initRequest: KagemushaRecursiveSpendInitRequestV2,
        authorization: KagemushaRequestAuthorizationV2
    ) throws {
        let canonicalAssetID = try KagemushaRecursiveSpendRequestCodecs.canonicalAssetId(
            assetID,
            field: "assetID"
        )
        guard authorization.fields.operationID == initRequest.operationID else {
            throw KagemushaRecursiveSpendV2Error.invalidField("authorization.operationID")
        }
        self.assetID = canonicalAssetID
        self.initRequest = initRequest
        self.authorization = authorization
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeTopUpRequest(self)
    }
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
        guard version == 1,
              assetScale == amount.scale,
              currentNote.amount == amount,
              !topUpAnchorNullifiers.isEmpty,
              finalizedHeight > 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidField("topUpAnchor")
        }
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
        self.version = version
        self.chainID = chainID
        self.payer = payer
        self.assetID = assetID
        self.assetScale = assetScale
        self.amount = amount
        self.initialRoot = Data(initialRoot)
        self.finalizedRoot = Data(finalizedRoot)
        self.topUpAnchorNullifiers = topUpAnchorNullifiers.map { Data(bytes: $0) }
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
}

public struct KagemushaRecursiveSpendSplitIntentV2: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputNote: KagemushaSpendableNoteDescriptorV2
    public let parentBranchPath: KagemushaRecursiveSpendBranchPathV2
    public let assetScale: UInt32
    public let transferAmount: KagemushaScaledAmount
    public let recipientOutput: KagemushaSpendableNoteDescriptorV2
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let recipientRequestDigest: Data
    public let parentLineageDigest: Data
    public let operationID: Data

    public init(
        inputNote: KagemushaSpendableNoteDescriptorV2,
        parentBranchPath: KagemushaRecursiveSpendBranchPathV2,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        changeOutput: KagemushaSpendableNoteDescriptorV2? = nil,
        recipientRequest: KagemushaVerifiedRecipientPaymentRequestV2,
        parentLineageDigest: Data,
        operationID: Data
    ) throws {
        try self.init(
            chainID: inputNote.chainID,
            assetDefinitionID: inputNote.assetDefinitionID,
            inputNote: inputNote,
            parentBranchPath: parentBranchPath,
            assetScale: inputNote.amount.scale,
            transferAmount: transferAmount,
            recipientOutput: recipientOutput,
            changeOutput: changeOutput,
            recipientRequestDigest: recipientRequest.digest,
            parentLineageDigest: parentLineageDigest,
            operationID: operationID
        )
        let request = recipientRequest.request.payload
        guard request.chainID == chainID,
              request.assetDefinitionID == assetDefinitionID,
              request.amount == transferAmount,
              request.recipientOutput == recipientOutput else {
            throw KagemushaRecursiveSpendV2Error.invalidField("recipientRequest")
        }
    }

    init(
        chainID: String,
        assetDefinitionID: String,
        inputNote: KagemushaSpendableNoteDescriptorV2,
        parentBranchPath: KagemushaRecursiveSpendBranchPathV2,
        assetScale: UInt32,
        transferAmount: KagemushaScaledAmount,
        recipientOutput: KagemushaSpendableNoteDescriptorV2,
        changeOutput: KagemushaSpendableNoteDescriptorV2?,
        recipientRequestDigest: Data,
        parentLineageDigest: Data,
        operationID: Data
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            recipientRequestDigest,
            field: "recipientRequestDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            parentLineageDigest,
            field: "parentLineageDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        let notes = [inputNote, recipientOutput] + (changeOutput.map { [$0] } ?? [])
        guard assetScale == transferAmount.scale,
              notes.allSatisfy({
                  $0.chainID == chainID
                    && $0.assetDefinitionID == assetDefinitionID
                    && $0.amount.scale == assetScale
              }),
              recipientOutput.amount == transferAmount else {
            throw KagemushaRecursiveSpendV2Error.invalidField("split.context")
        }
        if let changeOutput {
            guard transferAmount.atomicUnits != inputNote.amount.atomicUnits,
                  Self.add(transferAmount.atomicUnits, changeOutput.amount.atomicUnits)
                    == inputNote.amount.atomicUnits else {
                throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput.amount")
            }
        } else if transferAmount.atomicUnits != inputNote.amount.atomicUnits {
            throw KagemushaRecursiveSpendV2Error.invalidField("changeOutput")
        }
        let material = notes.flatMap { [$0.noteCommitment, $0.spendNullifier] }
        guard Set(material).count == material.count else {
            throw KagemushaRecursiveSpendV2Error.invalidField("split.noteMaterial")
        }
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.inputNote = inputNote
        self.parentBranchPath = parentBranchPath
        self.assetScale = assetScale
        self.transferAmount = transferAmount
        self.recipientOutput = recipientOutput
        self.changeOutput = changeOutput
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.parentLineageDigest = Data(parentLineageDigest)
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
        return String(decoding: output.reversed(), as: UTF8.self)
    }
}

public struct KagemushaRecursiveSpendBundleSummaryV2: Equatable, Sendable {
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let hopCount: UInt32
    public let branchPath: KagemushaRecursiveSpendBranchPathV2
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

public struct KagemushaRecursiveSpendAppendRequestV2: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV2
    public let recordBundle: Data
    public let pallasOpenEnvelopesArchive: Data
    public let split: KagemushaRecursiveSpendSplitIntentV2
    public let lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2
    public let outputProofCircuitID: String
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let previousProofOpenEnvelopesArchive: Data
    public let blockHeight: UInt64?

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV2,
        recordBundle: Data,
        pallasOpenEnvelopesArchive: Data,
        split: KagemushaRecursiveSpendSplitIntentV2,
        lineageArtifact: KagemushaRecursiveSpendArtifactReferenceV2,
        previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        previousProofOpenEnvelopesArchive: Data = Data(),
        blockHeight: UInt64? = nil
    ) throws {
        _ = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        guard !pallasOpenEnvelopesArchive.isEmpty,
              previousBundle.summary.amount == split.inputNote.amount,
              previousBundle.summary.noteCommitment == split.inputNote.noteCommitment,
              previousBundle.summary.spendNullifier == split.inputNote.spendNullifier,
              previousBundle.summary.branchPath == split.parentBranchPath,
              previousBundle.summary.bundleDigest == split.parentLineageDigest,
              lineageArtifact.role == .lineageAppendProver,
              lineageArtifact.generation == previousBundle.summary.artifactGeneration else {
            throw KagemushaRecursiveSpendV2Error.invalidField("appendRequest")
        }
        self.previousBundle = previousBundle
        self.recordBundle = Data(recordBundle)
        self.pallasOpenEnvelopesArchive = Data(pallasOpenEnvelopesArchive)
        self.split = split
        self.lineageArtifact = lineageArtifact
        self.outputProofCircuitID = KagemushaRecursiveSpendV2.reservedAppendCircuitID
        self.previousLineageVerifierRecord = previousLineageVerifierRecord
        self.previousProofOpenEnvelopesArchive = Data(previousProofOpenEnvelopesArchive)
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

    init(
        split: KagemushaRecursiveSpendSplitIntentV2,
        splitBindingDigest: Data,
        recipientBundle: KagemushaRecursiveSpendBundleV2,
        changeBundle: KagemushaRecursiveSpendBundleV2?
    ) throws {
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            splitBindingDigest,
            field: "splitBindingDigest"
        )
        guard recipientBundle.summary.amount == split.transferAmount,
              recipientBundle.summary.noteCommitment == split.recipientOutput.noteCommitment else {
            throw KagemushaRecursiveSpendV2Error.invalidField("recipientBundle")
        }
        switch (split.changeOutput, changeBundle) {
        case (nil, nil):
            break
        case let (.some(change), .some(bundle)):
            guard bundle.summary.amount == change.amount,
                  bundle.summary.noteCommitment == change.noteCommitment,
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

public struct KagemushaRecursiveSpendLineageWitnessV2: Equatable, Sendable {
    public let transitionArchives: [Data]
    public let finalBundleDigest: Data

    public init(transitionArchives: [Data], finalBundleDigest: Data) throws {
        guard !transitionArchives.isEmpty,
              transitionArchives.count <= 128,
              transitionArchives.allSatisfy({
                  !$0.isEmpty && $0.count <= KagemushaRecursiveSpendV2.maximumPeerArchiveBytes
              }) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("transitionArchives")
        }
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            finalBundleDigest,
            field: "finalBundleDigest"
        )
        self.transitionArchives = transitionArchives.map { Data(bytes: $0) }
        self.finalBundleDigest = Data(finalBundleDigest)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeLineageWitness(self)
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
        recipientBundle: KagemushaRecursiveSpendBundleV2,
        acceptedAtMilliseconds: UInt64
    ) throws -> KagemushaReceiverAcknowledgementPayloadV2 {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementPayloadV2(
            requestArchive: request.archive,
            recipientBundleArchive: recipientBundle.archive,
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
        recipientBundle: KagemushaRecursiveSpendBundleV2
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementCreateV2(
            payloadArchive: payload.archive,
            signature: signature,
            requestArchive: request.archive,
            recipientBundleArchive: recipientBundle.archive
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
        recipientBundle: KagemushaRecursiveSpendBundleV2
    ) throws -> KagemushaReceiverAcknowledgementVerifyResultV2 {
        guard let result = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementVerifyV2(
            acknowledgementArchive: archive,
            requestArchive: request.archive,
            recipientBundleArchive: recipientBundle.archive
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
        self.inputCommitments = inputCommitments.map { Data(bytes: $0) }
        self.nullifiers = nullifiers.map { Data(bytes: $0) }
        self.changeOutputCommitment = Data(changeOutputCommitment)
        self.root = Data(root)
        self.publicAmount = Data(publicAmount)
        self.assetTag = Data(assetTag)
        self.chainTag = Data(chainTag)
    }
}

public struct KagemushaRecursiveSpendRedemptionIntentV2: Equatable, Sendable {
    public let chainID: String
    public let assetDefinitionID: String
    public let inputNote: KagemushaSpendableNoteDescriptorV2
    public let parentBranchPath: KagemushaRecursiveSpendBranchPathV2
    public let parentBundleDigest: Data
    public let inputRoot: Data
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOutput: KagemushaSpendableNoteDescriptorV2?
    public let unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2
    public let unshieldPublicInputsDigest: Data
    public let operationID: Data

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV2,
        chainID: String,
        inputNote: KagemushaSpendableNoteDescriptorV2,
        inputRoot: Data,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptorV2? = nil,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) throws {
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(inputRoot, field: "inputRoot")
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(
            unshieldPublicInputsDigest,
            field: "unshieldPublicInputsDigest"
        )
        try KagemushaRecursiveSpendV2.requireNonzeroFixed32(operationID, field: "operationID")
        guard inputNote.chainID == chainID,
              inputNote.assetDefinitionID == previousBundle.summary.assetDefinitionID,
              inputNote.amount == previousBundle.summary.amount,
              inputNote.noteCommitment == previousBundle.summary.noteCommitment,
              inputNote.spendNullifier == previousBundle.summary.spendNullifier,
              publicAmount.scale == previousBundle.summary.amount.scale,
              KagemushaScaledAmount.compareAtomicUnits(
                  publicAmount.atomicUnits,
                  previousBundle.summary.amount.atomicUnits
              ) != .orderedDescending else {
            throw KagemushaRecursiveSpendV2Error.invalidField("publicAmount")
        }
        switch (changeOutput, publicAmount.atomicUnits == previousBundle.summary.amount.atomicUnits) {
        case (nil, true): break
        case let (.some(change), false):
            guard change.chainID == chainID,
                  change.assetDefinitionID == inputNote.assetDefinitionID,
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
        self.chainID = chainID
        self.assetDefinitionID = previousBundle.summary.assetDefinitionID
        self.inputNote = inputNote
        self.parentBranchPath = previousBundle.summary.branchPath
        self.parentBundleDigest = previousBundle.summary.bundleDigest
        self.inputRoot = Data(inputRoot)
        self.recipient = recipient
        self.publicAmount = publicAmount
        self.changeOutput = changeOutput
        self.unshieldPublicInputs = unshieldPublicInputs
        self.unshieldPublicInputsDigest = Data(unshieldPublicInputsDigest)
        self.operationID = Data(operationID)
    }

    init(
        chainID: String,
        assetDefinitionID: String,
        inputNote: KagemushaSpendableNoteDescriptorV2,
        parentBranchPath: KagemushaRecursiveSpendBranchPathV2,
        parentBundleDigest: Data,
        inputRoot: Data,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOutput: KagemushaSpendableNoteDescriptorV2?,
        unshieldPublicInputs: KagemushaUnshieldPublicInputsBindingV2,
        unshieldPublicInputsDigest: Data,
        operationID: Data
    ) {
        self.chainID = chainID
        self.assetDefinitionID = assetDefinitionID
        self.inputNote = inputNote
        self.parentBranchPath = parentBranchPath
        self.parentBundleDigest = parentBundleDigest
        self.inputRoot = inputRoot
        self.recipient = recipient
        self.publicAmount = publicAmount
        self.changeOutput = changeOutput
        self.unshieldPublicInputs = unshieldPublicInputs
        self.unshieldPublicInputsDigest = unshieldPublicInputsDigest
        self.operationID = operationID
    }
}

public struct KagemushaRecursiveSpendRedeemChangeBranchV2: Equatable, Sendable {
    public let output: KagemushaSpendableNoteDescriptorV2
    public let branchPath: KagemushaRecursiveSpendBranchPathV2
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
              lineageArtifact.generation == previousBundle.summary.artifactGeneration,
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

public struct KagemushaRecursiveSpendRedeemRequestV2: Equatable, Sendable {
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
    public let authorization: KagemushaRequestAuthorizationV2

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
        operationID: Data,
        authorization: KagemushaRequestAuthorizationV2
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
              authorization.fields.operationID == operationID,
              (redemption.changeOutput == nil) == (offlineChange == nil) else {
            throw KagemushaRecursiveSpendV2Error.invalidField("redeemRequest")
        }
        if let offlineChange, offlineChange.output != redemption.changeOutput {
            throw KagemushaRecursiveSpendV2Error.invalidField("offlineChange")
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
        self.authorization = authorization
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendV2Codecs.encodeRedeemRequest(self)
    }
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

private extension KagemushaRecursiveSpendSplitIntentV2 {
    static func addForValidation(_ lhs: String, _ rhs: String) -> String {
        add(lhs, rhs)
    }
}
