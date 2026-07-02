import Foundation

public enum ToriiOfflineCashAPI {
    public enum Endpoint: String, Sendable {
        case keyRefill = "/v1/offline/v2/keys/refill"
        case noteIssue = "/v1/offline/v2/notes/issue"
        case noteRedeem = "/v1/offline/v2/notes/redeem"
        case audit = "/v1/offline/v2/audit"
        case revocationBundle = "/v1/offline/revocations/bundle"
        case telemetry = "/v1/offline/telemetry"

        public var path: String { rawValue }
    }

    public static func canonicalBody<T: Encodable>(_ request: T) throws -> Data {
        try ToriiOfflineCashCodec.canonicalData(request)
    }

    public static func idempotencyKey(for request: ToriiOfflineKeyRefillRequest) -> String {
        request.operationId
    }

    public static func idempotencyKey(for request: ToriiOfflineNoteIssueSettlementRequest) -> String {
        request.operationId
    }

    public static func idempotencyKey(for request: ToriiOfflineNoteRedeemSettlementRequest) -> String {
        request.operationId
    }

    public static func idempotencyKey(for request: ToriiOfflineAuditRequest) -> String {
        request.operationId
    }
}

public enum ToriiOfflineCashOperationKind: String, Codable, Equatable, Sendable {
    case load
    case refresh
    case sync
    case redeem
}

private enum ToriiOfflineCashAPIModelValidation {
    static func requireExactNonEmptyText(_ value: String, field: String) throws {
        guard !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw OfflineNotePayloadError.invalidField(field)
        }
    }

    static func optionalExactNonEmptyText(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        try requireExactNonEmptyText(value, field: field)
        return value
    }

    static func canonicalNonNegativeAmount(_ value: String, field: String) throws -> String {
        let canonical = try ToriiOfflineCashCodec.canonicalAmountString(value)
        guard !canonical.hasPrefix("-") else {
            throw OfflineNotePayloadError.invalidField(field)
        }
        return canonical
    }

    static func optionalCanonicalNonNegativeAmount(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        return try canonicalNonNegativeAmount(value, field: field)
    }

    static func requireCanonicalSignatureBase64(_ value: String, field: String) throws {
        guard let signature = OfflineNoteTextPayloadEncoding.decodeExactBase64(value),
              signature.count == 64 else {
            throw OfflineNotePayloadError.invalidField(field)
        }
    }

    static func requireEmptyOrHashHex(_ value: String, field: String) throws {
        guard !value.isEmpty else { return }
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(value, field: field)
    }

    static func optionalHashHex(_ value: String?, field: String) throws -> String? {
        guard let value else { return nil }
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(value, field: field)
        return value
    }
}

public struct ToriiOfflineSettlementProof: Codable, Equatable, Sendable {
    public let operationId: String
    public let kind: ToriiOfflineCashOperationKind
    public let accountId: String
    public let deviceId: String
    public let assetDefinitionId: String
    public let amount: String
    public let preBalance: String
    public let postBalance: String
    public let entryHash: String
    public let chainTxHash: String
    public let blockHeight: UInt64
    public let issuedAtMs: UInt64
    public let noteCommitment: String?
    public let issuerSignatureBase64: String

    public init(
        operationId: String,
        kind: ToriiOfflineCashOperationKind,
        accountId: String,
        deviceId: String,
        assetDefinitionId: String,
        amount: String,
        preBalance: String,
        postBalance: String,
        entryHash: String,
        chainTxHash: String,
        blockHeight: UInt64,
        issuedAtMs: UInt64,
        noteCommitment: String? = nil,
        issuerSignatureBase64: String
    ) throws {
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(accountId, field: "account_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(deviceId, field: "device_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            assetDefinitionId,
            field: "asset_definition_id"
        )
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(entryHash, field: "entry_hash")
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(chainTxHash, field: "chain_tx_hash")
        try ToriiOfflineCashAPIModelValidation.requireCanonicalSignatureBase64(
            issuerSignatureBase64,
            field: "issuer_signature_base64"
        )
        self.operationId = operationId
        self.kind = kind
        self.accountId = accountId
        self.deviceId = deviceId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            amount,
            field: "amount"
        )
        self.preBalance = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            preBalance,
            field: "pre_balance"
        )
        self.postBalance = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            postBalance,
            field: "post_balance"
        )
        self.entryHash = entryHash
        self.chainTxHash = chainTxHash
        self.blockHeight = blockHeight
        self.issuedAtMs = issuedAtMs
        if let noteCommitment {
            _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
                noteCommitment,
                field: "note_commitment"
            )
            self.noteCommitment = noteCommitment
        } else {
            self.noteCommitment = nil
        }
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            kind: container.decode(ToriiOfflineCashOperationKind.self, forKey: .kind),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            amount: container.decode(String.self, forKey: .amount),
            preBalance: container.decode(String.self, forKey: .preBalance),
            postBalance: container.decode(String.self, forKey: .postBalance),
            entryHash: container.decode(String.self, forKey: .entryHash),
            chainTxHash: container.decode(String.self, forKey: .chainTxHash),
            blockHeight: container.decode(UInt64.self, forKey: .blockHeight),
            issuedAtMs: container.decode(UInt64.self, forKey: .issuedAtMs),
            noteCommitment: container.decodeIfPresent(String.self, forKey: .noteCommitment),
            issuerSignatureBase64: container.decode(String.self, forKey: .issuerSignatureBase64)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case kind
        case accountId = "account_id"
        case deviceId = "device_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case preBalance = "pre_balance"
        case postBalance = "post_balance"
        case entryHash = "entry_hash"
        case chainTxHash = "chain_tx_hash"
        case blockHeight = "block_height"
        case issuedAtMs = "issued_at_ms"
        case noteCommitment = "note_commitment"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

public struct ToriiOfflineKeyRefillRequest: Codable, Equatable, Sendable {
    public let operationId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public var attestationKeyId: String
    public let assetDefinitionId: String
    public let existingLineageId: String?
    public let lineageState: ToriiOfflineCashState?
    public let localRevision: UInt64
    public let localStateHash: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let keyCertificateBindings: [ToriiOfflineDeviceBinding]
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        operationId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        attestationKeyId: String,
        assetDefinitionId: String,
        existingLineageId: String?,
        lineageState: ToriiOfflineCashState? = nil,
        localRevision: UInt64,
        localStateHash: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        keyCertificateBindings: [ToriiOfflineDeviceBinding]? = nil,
        deviceProof: ToriiOfflineDeviceProof
    ) throws {
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(accountId, field: "account_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(deviceId, field: "device_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            offlinePublicKey,
            field: "offline_public_key"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            attestationKeyId,
            field: "attestation_key_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            assetDefinitionId,
            field: "asset_definition_id"
        )
        if let existingLineageId {
            try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
                existingLineageId,
                field: "existing_lineage_id"
            )
        }
        try ToriiOfflineCashAPIModelValidation.requireEmptyOrHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.attestationKeyId = attestationKeyId
        self.assetDefinitionId = assetDefinitionId
        self.existingLineageId = existingLineageId
        self.lineageState = lineageState
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.deviceBinding = deviceBinding
        self.keyCertificateBindings = keyCertificateBindings ?? [deviceBinding]
        self.deviceProof = deviceProof
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            offlinePublicKey: container.decode(String.self, forKey: .offlinePublicKey),
            attestationKeyId: container.decode(String.self, forKey: .attestationKeyId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            existingLineageId: container.decodeIfPresent(String.self, forKey: .existingLineageId),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            localRevision: container.decode(UInt64.self, forKey: .localRevision),
            localStateHash: container.decode(String.self, forKey: .localStateHash),
            deviceBinding: container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding),
            keyCertificateBindings: container.decodeIfPresent(
                [ToriiOfflineDeviceBinding].self,
                forKey: .keyCertificateBindings
            ),
            deviceProof: container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(operationId, forKey: .operationId)
        try container.encode(accountId, forKey: .accountId)
        try container.encode(deviceId, forKey: .deviceId)
        try container.encode(offlinePublicKey, forKey: .offlinePublicKey)
        try container.encode(attestationKeyId, forKey: .attestationKeyId)
        try container.encode(assetDefinitionId, forKey: .assetDefinitionId)
        try container.encodeIfPresent(existingLineageId, forKey: .existingLineageId)
        try container.encodeIfPresent(lineageState, forKey: .lineageState)
        try container.encode(localRevision, forKey: .localRevision)
        try container.encode(localStateHash, forKey: .localStateHash)
        try container.encode(deviceBinding, forKey: .deviceBinding)
        try container.encode(keyCertificateBindings, forKey: .keyCertificateBindings)
        try container.encode(deviceProof, forKey: .deviceProof)
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case attestationKeyId = "attestation_key_id"
        case assetDefinitionId = "asset_definition_id"
        case existingLineageId = "existing_lineage_id"
        case lineageState = "lineage_state"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case deviceBinding = "device_binding"
        case keyCertificateBindings = "key_certificate_bindings"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineKeyRefillResponse: Codable, Equatable, Sendable {
    public let operationId: String?
    public let lineageState: ToriiOfflineCashState?
    public let keyCertificate: OfflineCompactKeyCertificate?
    public let keyCertificates: [OfflineCompactKeyCertificate]?

    public init(
        operationId: String? = nil,
        lineageState: ToriiOfflineCashState? = nil,
        keyCertificate: OfflineCompactKeyCertificate? = nil,
        keyCertificates: [OfflineCompactKeyCertificate]? = nil
    ) throws {
        self.operationId = try ToriiOfflineCashAPIModelValidation.optionalExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        self.lineageState = lineageState
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decodeIfPresent(String.self, forKey: .operationId),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            keyCertificate: container.decodeIfPresent(
                OfflineCompactKeyCertificate.self,
                forKey: .keyCertificate
            ),
            keyCertificates: container.decodeIfPresent(
                [OfflineCompactKeyCertificate].self,
                forKey: .keyCertificates
            )
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case lineageState = "lineage_state"
        case keyCertificate = "key_certificate"
        case keyCertificates = "key_certificates"
    }
}

public struct ToriiOfflineNoteIssueSettlementRequest: Codable, Equatable, Sendable {
    public let operationId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let lineageId: String
    public let assetDefinitionId: String
    public let amount: String
    public let noteCommitment: String
    public let lineageState: ToriiOfflineCashState?
    public let localBalance: String
    public let localRevision: UInt64
    public let localStateHash: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let keyCertificateBindings: [ToriiOfflineDeviceBinding]?
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        operationId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        noteCommitment: String,
        lineageState: ToriiOfflineCashState? = nil,
        localBalance: String,
        localRevision: UInt64,
        localStateHash: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        keyCertificateBindings: [ToriiOfflineDeviceBinding]? = nil,
        deviceProof: ToriiOfflineDeviceProof
    ) throws {
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(accountId, field: "account_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(deviceId, field: "device_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            offlinePublicKey,
            field: "offline_public_key"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(lineageId, field: "lineage_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            assetDefinitionId,
            field: "asset_definition_id"
        )
        let canonicalAmount = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            amount,
            field: "amount"
        )
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
            noteCommitment,
            field: "note_commitment"
        )
        let canonicalLocalBalance = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            localBalance,
            field: "local_balance"
        )
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.lineageId = lineageId
        self.assetDefinitionId = assetDefinitionId
        self.amount = canonicalAmount
        self.noteCommitment = noteCommitment
        self.lineageState = lineageState
        self.localBalance = canonicalLocalBalance
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.deviceBinding = deviceBinding
        self.keyCertificateBindings = keyCertificateBindings ?? [deviceBinding]
        self.deviceProof = deviceProof
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            offlinePublicKey: container.decode(String.self, forKey: .offlinePublicKey),
            lineageId: container.decode(String.self, forKey: .lineageId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            amount: container.decode(String.self, forKey: .amount),
            noteCommitment: container.decode(String.self, forKey: .noteCommitment),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            localBalance: container.decode(String.self, forKey: .localBalance),
            localRevision: container.decode(UInt64.self, forKey: .localRevision),
            localStateHash: container.decode(String.self, forKey: .localStateHash),
            deviceBinding: container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding),
            keyCertificateBindings: container.decodeIfPresent(
                [ToriiOfflineDeviceBinding].self,
                forKey: .keyCertificateBindings
            ),
            deviceProof: container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case lineageId = "lineage_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case noteCommitment = "note_commitment"
        case lineageState = "lineage_state"
        case localBalance = "local_balance"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case deviceBinding = "device_binding"
        case keyCertificateBindings = "key_certificate_bindings"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineNoteIssueSettlementResponse: Codable, Equatable, Sendable {
    public let operationId: String?
    public let settlement: ToriiOfflineSettlementProof
    public let issuedNoteCommitment: String?
    public let lineageState: ToriiOfflineCashState?
    public let localBalance: String?
    public let lockedBalance: String?
    public let localRevision: UInt64?
    public let localStateHash: String?
    public let keyCertificate: OfflineCompactKeyCertificate?
    public let keyCertificates: [OfflineCompactKeyCertificate]?

    public init(
        operationId: String? = nil,
        settlement: ToriiOfflineSettlementProof,
        issuedNoteCommitment: String? = nil,
        lineageState: ToriiOfflineCashState? = nil,
        localBalance: String? = nil,
        lockedBalance: String? = nil,
        localRevision: UInt64? = nil,
        localStateHash: String? = nil,
        keyCertificate: OfflineCompactKeyCertificate? = nil,
        keyCertificates: [OfflineCompactKeyCertificate]? = nil
    ) throws {
        self.operationId = try ToriiOfflineCashAPIModelValidation.optionalExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        self.settlement = settlement
        if let issuedNoteCommitment {
            _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
                issuedNoteCommitment,
                field: "issued_note_commitment"
            )
            self.issuedNoteCommitment = issuedNoteCommitment
        } else {
            self.issuedNoteCommitment = nil
        }
        self.lineageState = lineageState
        self.localBalance = try ToriiOfflineCashAPIModelValidation.optionalCanonicalNonNegativeAmount(
            localBalance,
            field: "local_balance"
        )
        self.lockedBalance = try ToriiOfflineCashAPIModelValidation.optionalCanonicalNonNegativeAmount(
            lockedBalance,
            field: "locked_balance"
        )
        self.localRevision = localRevision
        self.localStateHash = try ToriiOfflineCashAPIModelValidation.optionalHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decodeIfPresent(String.self, forKey: .operationId),
            settlement: container.decode(ToriiOfflineSettlementProof.self, forKey: .settlement),
            issuedNoteCommitment: container.decodeIfPresent(
                String.self,
                forKey: .issuedNoteCommitment
            ),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            localBalance: container.decodeIfPresent(String.self, forKey: .localBalance),
            lockedBalance: container.decodeIfPresent(String.self, forKey: .lockedBalance),
            localRevision: container.decodeIfPresent(UInt64.self, forKey: .localRevision),
            localStateHash: container.decodeIfPresent(String.self, forKey: .localStateHash),
            keyCertificate: container.decodeIfPresent(
                OfflineCompactKeyCertificate.self,
                forKey: .keyCertificate
            ),
            keyCertificates: container.decodeIfPresent(
                [OfflineCompactKeyCertificate].self,
                forKey: .keyCertificates
            )
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case settlement
        case issuedNoteCommitment = "issued_note_commitment"
        case lineageState = "lineage_state"
        case localBalance = "local_balance"
        case lockedBalance = "locked_balance"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case keyCertificate = "key_certificate"
        case keyCertificates = "key_certificates"
    }
}

public struct ToriiOfflineRedemptionProof: Codable, Equatable, Sendable {
    public let sourceNoteCommitment: String
    public let inputNullifiers: [String]
    public let senderKeyCertificate: OfflineCompactKeyCertificate
    public let recipientAccountId: String
    public let assetDefinitionId: String
    public let amount: String
    public let recursiveProof: OfflineRecursiveProof

    public init(
        sourceNoteCommitment: String,
        inputNullifiers: [String],
        senderKeyCertificate: OfflineCompactKeyCertificate,
        recipientAccountId: String,
        assetDefinitionId: String,
        amount: String,
        recursiveProof: OfflineRecursiveProof
    ) throws {
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
            sourceNoteCommitment,
            field: "source_note_commitment"
        )
        guard !inputNullifiers.isEmpty else {
            throw OfflineNotePayloadError.invalidField("input_nullifiers")
        }
        for inputNullifier in inputNullifiers {
            _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
                inputNullifier,
                field: "input_nullifiers"
            )
        }
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            recipientAccountId,
            field: "recipient_account_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            assetDefinitionId,
            field: "asset_definition_id"
        )
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.senderKeyCertificate = senderKeyCertificate
        self.recipientAccountId = recipientAccountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            amount,
            field: "amount"
        )
        self.recursiveProof = recursiveProof
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            sourceNoteCommitment: container.decode(String.self, forKey: .sourceNoteCommitment),
            inputNullifiers: container.decode([String].self, forKey: .inputNullifiers),
            senderKeyCertificate: container.decode(
                OfflineCompactKeyCertificate.self,
                forKey: .senderKeyCertificate
            ),
            recipientAccountId: container.decode(String.self, forKey: .recipientAccountId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            amount: container.decode(String.self, forKey: .amount),
            recursiveProof: container.decode(OfflineRecursiveProof.self, forKey: .recursiveProof)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifiers = "input_nullifiers"
        case senderKeyCertificate = "sender_key_certificate"
        case recipientAccountId = "recipient_account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case recursiveProof = "recursive_proof"
    }
}

public struct ToriiOfflineNoteRedeemSettlementRequest: Codable, Equatable, Sendable {
    public let operationId: String
    public let accountId: String
    public let deviceId: String
    public let lineageId: String
    public let assetDefinitionId: String
    public let amount: String
    public let localBalance: String
    public let localRevision: UInt64
    public let localStateHash: String
    public let pendingReceipts: [ToriiOfflineTransferReceipt]
    public let paymentTokens: [OfflinePaymentToken]
    public let paymentTokensNoritoBase64: [String]
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof
    public let redemption: ToriiOfflineRedemptionProof

    public init(
        operationId: String,
        accountId: String,
        deviceId: String,
        lineageId: String,
        assetDefinitionId: String,
        amount: String,
        localBalance: String,
        localRevision: UInt64,
        localStateHash: String,
        pendingReceipts: [ToriiOfflineTransferReceipt],
        paymentTokens: [OfflinePaymentToken] = [],
        paymentTokensNoritoBase64: [String] = [],
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof,
        redemption: ToriiOfflineRedemptionProof
    ) throws {
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(accountId, field: "account_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(deviceId, field: "device_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(lineageId, field: "lineage_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            assetDefinitionId,
            field: "asset_definition_id"
        )
        let canonicalAmount = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            amount,
            field: "amount"
        )
        let canonicalLocalBalance = try ToriiOfflineCashAPIModelValidation.canonicalNonNegativeAmount(
            localBalance,
            field: "local_balance"
        )
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.lineageId = lineageId
        self.assetDefinitionId = assetDefinitionId
        self.amount = canonicalAmount
        self.localBalance = canonicalLocalBalance
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.pendingReceipts = pendingReceipts
        self.paymentTokens = paymentTokens
        self.paymentTokensNoritoBase64 = paymentTokensNoritoBase64
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
        self.redemption = redemption
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            lineageId: container.decode(String.self, forKey: .lineageId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            amount: container.decode(String.self, forKey: .amount),
            localBalance: container.decode(String.self, forKey: .localBalance),
            localRevision: container.decode(UInt64.self, forKey: .localRevision),
            localStateHash: container.decode(String.self, forKey: .localStateHash),
            pendingReceipts: container.decode([ToriiOfflineTransferReceipt].self, forKey: .pendingReceipts),
            paymentTokens: container.decodeIfPresent(
                [OfflinePaymentToken].self,
                forKey: .paymentTokens
            ) ?? [],
            paymentTokensNoritoBase64: container.decodeIfPresent(
                [String].self,
                forKey: .paymentTokensNoritoBase64
            ) ?? [],
            deviceBinding: container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding),
            deviceProof: container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof),
            redemption: container.decode(ToriiOfflineRedemptionProof.self, forKey: .redemption)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case lineageId = "lineage_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case localBalance = "local_balance"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case pendingReceipts = "pending_receipts"
        case paymentTokens = "payment_tokens"
        case paymentTokensNoritoBase64 = "payment_tokens_norito_base64"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
        case redemption
    }
}

public struct ToriiOfflineNoteRedeemSettlementResponse: Codable, Equatable, Sendable {
    public let operationId: String?
    public let settlement: ToriiOfflineSettlementProof
    public let lineageState: ToriiOfflineCashState?
    public let localBalance: String?
    public let lockedBalance: String?
    public let localRevision: UInt64?
    public let localStateHash: String?
    public let acceptedReceiptIds: [String]?
    public let keyCertificate: OfflineCompactKeyCertificate?
    public let keyCertificates: [OfflineCompactKeyCertificate]?

    public init(
        operationId: String? = nil,
        settlement: ToriiOfflineSettlementProof,
        lineageState: ToriiOfflineCashState? = nil,
        localBalance: String? = nil,
        lockedBalance: String? = nil,
        localRevision: UInt64? = nil,
        localStateHash: String? = nil,
        acceptedReceiptIds: [String]? = nil,
        keyCertificate: OfflineCompactKeyCertificate? = nil,
        keyCertificates: [OfflineCompactKeyCertificate]? = nil
    ) throws {
        self.operationId = try ToriiOfflineCashAPIModelValidation.optionalExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        self.settlement = settlement
        self.lineageState = lineageState
        self.localBalance = try ToriiOfflineCashAPIModelValidation.optionalCanonicalNonNegativeAmount(
            localBalance,
            field: "local_balance"
        )
        self.lockedBalance = try ToriiOfflineCashAPIModelValidation.optionalCanonicalNonNegativeAmount(
            lockedBalance,
            field: "locked_balance"
        )
        self.localRevision = localRevision
        self.localStateHash = try ToriiOfflineCashAPIModelValidation.optionalHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.acceptedReceiptIds = try acceptedReceiptIds?.map { receiptId in
            try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
                receiptId,
                field: "accepted_receipt_ids"
            )
            return receiptId
        }
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decodeIfPresent(String.self, forKey: .operationId),
            settlement: container.decode(ToriiOfflineSettlementProof.self, forKey: .settlement),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            localBalance: container.decodeIfPresent(String.self, forKey: .localBalance),
            lockedBalance: container.decodeIfPresent(String.self, forKey: .lockedBalance),
            localRevision: container.decodeIfPresent(UInt64.self, forKey: .localRevision),
            localStateHash: container.decodeIfPresent(String.self, forKey: .localStateHash),
            acceptedReceiptIds: container.decodeIfPresent([String].self, forKey: .acceptedReceiptIds),
            keyCertificate: container.decodeIfPresent(
                OfflineCompactKeyCertificate.self,
                forKey: .keyCertificate
            ),
            keyCertificates: container.decodeIfPresent(
                [OfflineCompactKeyCertificate].self,
                forKey: .keyCertificates
            )
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case settlement
        case lineageState = "lineage_state"
        case localBalance = "local_balance"
        case lockedBalance = "locked_balance"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case acceptedReceiptIds = "accepted_receipt_ids"
        case keyCertificate = "key_certificate"
        case keyCertificates = "key_certificates"
    }
}

public struct ToriiOfflineAuditRequest: Codable, Equatable, Sendable {
    public let operationId: String
    public let accountId: String
    public let deviceId: String
    public let lineageId: String
    public let localRevision: UInt64
    public let localStateHash: String
    public let receipts: [ToriiOfflineTransferReceipt]
    public let paymentTokens: [OfflinePaymentToken]
    public let paymentTokensNoritoBase64: [String]
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        operationId: String,
        accountId: String,
        deviceId: String,
        lineageId: String,
        localRevision: UInt64,
        localStateHash: String,
        receipts: [ToriiOfflineTransferReceipt],
        paymentTokens: [OfflinePaymentToken],
        paymentTokensNoritoBase64: [String],
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof
    ) throws {
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(accountId, field: "account_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(deviceId, field: "device_id")
        try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(lineageId, field: "lineage_id")
        _ = try OfflineNoteTextPayloadEncoding.requireHashHex(
            localStateHash,
            field: "local_state_hash"
        )
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.lineageId = lineageId
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.receipts = receipts
        self.paymentTokens = paymentTokens
        self.paymentTokensNoritoBase64 = paymentTokensNoritoBase64
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            lineageId: container.decode(String.self, forKey: .lineageId),
            localRevision: container.decode(UInt64.self, forKey: .localRevision),
            localStateHash: container.decode(String.self, forKey: .localStateHash),
            receipts: container.decode([ToriiOfflineTransferReceipt].self, forKey: .receipts),
            paymentTokens: container.decodeIfPresent(
                [OfflinePaymentToken].self,
                forKey: .paymentTokens
            ) ?? [],
            paymentTokensNoritoBase64: container.decodeIfPresent(
                [String].self,
                forKey: .paymentTokensNoritoBase64
            ) ?? [],
            deviceBinding: container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding),
            deviceProof: container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case lineageId = "lineage_id"
        case localRevision = "local_revision"
        case localStateHash = "local_state_hash"
        case receipts
        case paymentTokens = "payment_tokens"
        case paymentTokensNoritoBase64 = "payment_tokens_norito_base64"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineAuditResponse: Codable, Equatable, Sendable {
    public let operationId: String?
    public let acceptedReceiptIds: [String]?
    public let lineageState: ToriiOfflineCashState?
    public let keyCertificate: OfflineCompactKeyCertificate?
    public let keyCertificates: [OfflineCompactKeyCertificate]?

    public init(
        operationId: String? = nil,
        acceptedReceiptIds: [String]? = nil,
        lineageState: ToriiOfflineCashState? = nil,
        keyCertificate: OfflineCompactKeyCertificate? = nil,
        keyCertificates: [OfflineCompactKeyCertificate]? = nil
    ) throws {
        self.operationId = try ToriiOfflineCashAPIModelValidation.optionalExactNonEmptyText(
            operationId,
            field: "operation_id"
        )
        self.acceptedReceiptIds = try acceptedReceiptIds?.map { receiptId in
            try ToriiOfflineCashAPIModelValidation.requireExactNonEmptyText(
                receiptId,
                field: "accepted_receipt_ids"
            )
            return receiptId
        }
        self.lineageState = lineageState
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decodeIfPresent(String.self, forKey: .operationId),
            acceptedReceiptIds: container.decodeIfPresent([String].self, forKey: .acceptedReceiptIds),
            lineageState: container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState),
            keyCertificate: container.decodeIfPresent(
                OfflineCompactKeyCertificate.self,
                forKey: .keyCertificate
            ),
            keyCertificates: container.decodeIfPresent(
                [OfflineCompactKeyCertificate].self,
                forKey: .keyCertificates
            )
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case acceptedReceiptIds = "accepted_receipt_ids"
        case lineageState = "lineage_state"
        case keyCertificate = "key_certificate"
        case keyCertificates = "key_certificates"
    }
}
