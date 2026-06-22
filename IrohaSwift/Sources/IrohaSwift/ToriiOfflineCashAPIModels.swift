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
        self.operationId = operationId
        self.kind = kind
        self.accountId = accountId
        self.deviceId = deviceId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.preBalance = try ToriiOfflineCashCodec.canonicalAmountString(preBalance)
        self.postBalance = try ToriiOfflineCashCodec.canonicalAmountString(postBalance)
        self.entryHash = entryHash
        self.chainTxHash = chainTxHash
        self.blockHeight = blockHeight
        self.issuedAtMs = issuedAtMs
        self.noteCommitment = noteCommitment?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
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
    public var appAttestKeyId: String
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
        appAttestKeyId: String,
        assetDefinitionId: String,
        existingLineageId: String?,
        lineageState: ToriiOfflineCashState? = nil,
        localRevision: UInt64,
        localStateHash: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        keyCertificateBindings: [ToriiOfflineDeviceBinding]? = nil,
        deviceProof: ToriiOfflineDeviceProof
    ) {
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.appAttestKeyId = appAttestKeyId
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
        operationId = try container.decode(String.self, forKey: .operationId)
        accountId = try container.decode(String.self, forKey: .accountId)
        deviceId = try container.decode(String.self, forKey: .deviceId)
        offlinePublicKey = try container.decode(String.self, forKey: .offlinePublicKey)
        assetDefinitionId = try container.decode(String.self, forKey: .assetDefinitionId)
        existingLineageId = try container.decodeIfPresent(String.self, forKey: .existingLineageId)
        lineageState = try container.decodeIfPresent(ToriiOfflineCashState.self, forKey: .lineageState)
        localRevision = try container.decode(UInt64.self, forKey: .localRevision)
        localStateHash = try container.decode(String.self, forKey: .localStateHash)
        deviceBinding = try container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding)
        keyCertificateBindings = try container.decodeIfPresent(
            [ToriiOfflineDeviceBinding].self,
            forKey: .keyCertificateBindings
        ) ?? [deviceBinding]
        deviceProof = try container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof)
        appAttestKeyId = try container.decodeIfPresent(String.self, forKey: .attestationKeyId)
            ?? container.decodeIfPresent(String.self, forKey: .appAttestKeyId)
            ?? deviceBinding.attestationKeyId
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(operationId, forKey: .operationId)
        try container.encode(accountId, forKey: .accountId)
        try container.encode(deviceId, forKey: .deviceId)
        try container.encode(offlinePublicKey, forKey: .offlinePublicKey)
        try container.encode(appAttestKeyId, forKey: .appAttestKeyId)
        try container.encode(appAttestKeyId, forKey: .attestationKeyId)
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
        case appAttestKeyId = "app_attest_key_id"
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
    ) {
        self.operationId = operationId
        self.lineageState = lineageState
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
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
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.lineageId = lineageId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.noteCommitment = noteCommitment.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        self.lineageState = lineageState
        self.localBalance = try ToriiOfflineCashCodec.canonicalAmountString(localBalance)
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.deviceBinding = deviceBinding
        self.keyCertificateBindings = keyCertificateBindings ?? [deviceBinding]
        self.deviceProof = deviceProof
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
    ) {
        self.operationId = operationId
        self.settlement = settlement
        self.issuedNoteCommitment = issuedNoteCommitment
        self.lineageState = lineageState
        self.localBalance = localBalance
        self.lockedBalance = lockedBalance
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
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
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.senderKeyCertificate = senderKeyCertificate
        self.recipientAccountId = recipientAccountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.recursiveProof = recursiveProof
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
        self.operationId = operationId
        self.accountId = accountId
        self.deviceId = deviceId
        self.lineageId = lineageId
        self.assetDefinitionId = assetDefinitionId
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.localBalance = try ToriiOfflineCashCodec.canonicalAmountString(localBalance)
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.pendingReceipts = pendingReceipts
        self.paymentTokens = paymentTokens
        self.paymentTokensNoritoBase64 = paymentTokensNoritoBase64
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
        self.redemption = redemption
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
    ) {
        self.operationId = operationId
        self.settlement = settlement
        self.lineageState = lineageState
        self.localBalance = localBalance
        self.lockedBalance = lockedBalance
        self.localRevision = localRevision
        self.localStateHash = localStateHash
        self.acceptedReceiptIds = acceptedReceiptIds
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
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
    ) {
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
    ) {
        self.operationId = operationId
        self.acceptedReceiptIds = acceptedReceiptIds
        self.lineageState = lineageState
        self.keyCertificate = keyCertificate
        self.keyCertificates = keyCertificates
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case acceptedReceiptIds = "accepted_receipt_ids"
        case lineageState = "lineage_state"
        case keyCertificate = "key_certificate"
        case keyCertificates = "key_certificates"
    }
}
