import CryptoKit
import Foundation

public enum ToriiOfflineAmountError: LocalizedError, Equatable {
    case invalidAmount(String)
    case negativeResult

    public var errorDescription: String? {
        switch self {
        case .invalidAmount(let value):
            return "Invalid offline cash amount: \(value)"
        case .negativeResult:
            return "Offline cash amount arithmetic produced a negative result."
        }
    }
}

public enum ToriiOfflineTransferDirection: String, Codable, Sendable {
    case incoming
    case outgoing
}

public struct ToriiOfflineDeviceBinding: Codable, Sendable, Equatable {
    public let platform: String
    public let attestationKeyId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let attestationReportBase64: String
    public let iosTeamId: String?
    public let iosBundleId: String?
    public let iosEnvironment: String?

    public init(
        platform: String,
        attestationKeyId: String,
        deviceId: String,
        offlinePublicKey: String,
        attestationReportBase64: String,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil
    ) {
        self.platform = platform
        self.attestationKeyId = attestationKeyId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.attestationReportBase64 = attestationReportBase64
        self.iosTeamId = iosTeamId
        self.iosBundleId = iosBundleId
        self.iosEnvironment = iosEnvironment
    }

    private enum CodingKeys: String, CodingKey {
        case platform
        case attestationKeyId = "attestation_key_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case attestationReportBase64 = "attestation_report_base64"
        case iosTeamId = "ios_team_id"
        case iosBundleId = "ios_bundle_id"
        case iosEnvironment = "ios_environment"
    }
}

public struct ToriiOfflineDeviceProof: Codable, Sendable, Equatable {
    public let platform: String
    public let attestationKeyId: String
    public let challengeHashHex: String
    public let assertionBase64: String
    public let counter: UInt64?

    public init(
        platform: String,
        attestationKeyId: String,
        challengeHashHex: String,
        assertionBase64: String,
        counter: UInt64? = nil
    ) {
        self.platform = platform
        self.attestationKeyId = attestationKeyId
        self.challengeHashHex = challengeHashHex
        self.assertionBase64 = assertionBase64
        self.counter = counter
    }

    private enum CodingKeys: String, CodingKey {
        case platform
        case attestationKeyId = "attestation_key_id"
        case challengeHashHex = "challenge_hash_hex"
        case assertionBase64 = "assertion_base64"
        case counter
    }
}

public struct ToriiOfflineSpendAuthorization: Codable, Sendable, Equatable, Identifiable {
    public let authorizationId: String
    public let lineageId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let verdictId: String
    public let policyMaxBalance: String
    public let policyMaxTxValue: String
    public let issuedAtMs: UInt64
    public let refreshAtMs: UInt64
    public let expiresAtMs: UInt64
    public let appAttestKeyId: String
    public let issuerSignatureBase64: String

    public var id: String { authorizationId }

    public init(
        authorizationId: String,
        lineageId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        verdictId: String,
        policyMaxBalance: String,
        policyMaxTxValue: String,
        issuedAtMs: UInt64,
        refreshAtMs: UInt64,
        expiresAtMs: UInt64,
        appAttestKeyId: String,
        issuerSignatureBase64: String
    ) {
        self.authorizationId = authorizationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.verdictId = verdictId
        self.policyMaxBalance = policyMaxBalance
        self.policyMaxTxValue = policyMaxTxValue
        self.issuedAtMs = issuedAtMs
        self.refreshAtMs = refreshAtMs
        self.expiresAtMs = expiresAtMs
        self.appAttestKeyId = appAttestKeyId
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public func isExpired(nowMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs()) -> Bool {
        nowMs >= expiresAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case authorizationId = "authorization_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case verdictId = "verdict_id"
        case policyMaxBalance = "max_balance"
        case policyMaxTxValue = "max_tx_value"
        case issuedAtMs = "issued_at_ms"
        case refreshAtMs = "refresh_at_ms"
        case expiresAtMs = "expires_at_ms"
        case appAttestKeyId = "app_attest_key_id"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

public struct ToriiOfflineCashState: Codable, Sendable, Equatable, Identifiable {
    public let lineageId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let assetDefinitionId: String
    public let balance: String
    public let lockedBalance: String
    public let serverRevision: UInt64
    public let serverStateHash: String
    public let pendingLocalRevision: UInt64
    public let authorization: ToriiOfflineSpendAuthorization
    public let issuerSignatureBase64: String

    public var id: String { lineageId }

    public init(
        lineageId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        assetDefinitionId: String,
        balance: String,
        lockedBalance: String,
        serverRevision: UInt64,
        serverStateHash: String,
        pendingLocalRevision: UInt64,
        authorization: ToriiOfflineSpendAuthorization,
        issuerSignatureBase64: String
    ) {
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.assetDefinitionId = assetDefinitionId
        self.balance = balance
        self.lockedBalance = lockedBalance
        self.serverRevision = serverRevision
        self.serverStateHash = serverStateHash
        self.pendingLocalRevision = pendingLocalRevision
        self.authorization = authorization
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    private enum CodingKeys: String, CodingKey {
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case assetDefinitionId = "asset_definition_id"
        case balance
        case lockedBalance = "locked_balance"
        case serverRevision = "server_revision"
        case serverStateHash = "server_state_hash"
        case pendingLocalRevision = "pending_local_revision"
        case authorization
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

public struct ToriiOfflineRevocationBundle: Codable, Sendable, Equatable {
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let verdictIds: [String]
    public let issuerSignatureBase64: String

    public init(
        issuedAtMs: UInt64,
        expiresAtMs: UInt64,
        verdictIds: [String],
        issuerSignatureBase64: String
    ) {
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.verdictIds = verdictIds
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public func isExpired(nowMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs()) -> Bool {
        nowMs >= expiresAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case verdictIds = "verdict_ids"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

public struct ToriiOfflineTransferReceipt: Codable, Sendable, Equatable, Identifiable {
    public let version: Int
    public let transferId: String
    public let direction: ToriiOfflineTransferDirection
    public let lineageId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let preBalance: String
    public let postBalance: String
    public let preLockedBalance: String
    public let postLockedBalance: String
    public let preStateHash: String
    public let postStateHash: String
    public let localRevision: UInt64
    public let counterpartyLineageId: String
    public let counterpartyAccountId: String
    public let counterpartyDeviceId: String
    public let counterpartyOfflinePublicKey: String
    public let amount: String
    public let authorization: ToriiOfflineSpendAuthorization?
    public let deviceProof: ToriiOfflineDeviceProof
    public let sourcePayload: String?
    public let senderSignatureBase64: String
    public let createdAtMs: UInt64

    public var id: String { "\(lineageId):\(localRevision)" }

    public init(
        version: Int = 1,
        transferId: String,
        direction: ToriiOfflineTransferDirection,
        lineageId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        preBalance: String,
        postBalance: String,
        preLockedBalance: String,
        postLockedBalance: String,
        preStateHash: String,
        postStateHash: String,
        localRevision: UInt64,
        counterpartyLineageId: String,
        counterpartyAccountId: String,
        counterpartyDeviceId: String,
        counterpartyOfflinePublicKey: String,
        amount: String,
        authorization: ToriiOfflineSpendAuthorization?,
        deviceProof: ToriiOfflineDeviceProof,
        sourcePayload: String?,
        senderSignatureBase64: String,
        createdAtMs: UInt64
    ) {
        self.version = version
        self.transferId = transferId
        self.direction = direction
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.preBalance = preBalance
        self.postBalance = postBalance
        self.preLockedBalance = preLockedBalance
        self.postLockedBalance = postLockedBalance
        self.preStateHash = preStateHash
        self.postStateHash = postStateHash
        self.localRevision = localRevision
        self.counterpartyLineageId = counterpartyLineageId
        self.counterpartyAccountId = counterpartyAccountId
        self.counterpartyDeviceId = counterpartyDeviceId
        self.counterpartyOfflinePublicKey = counterpartyOfflinePublicKey
        self.amount = amount
        self.authorization = authorization
        self.deviceProof = deviceProof
        self.sourcePayload = sourcePayload
        self.senderSignatureBase64 = senderSignatureBase64
        self.createdAtMs = createdAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case transferId = "transfer_id"
        case direction
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case preBalance = "pre_balance"
        case postBalance = "post_balance"
        case preLockedBalance = "pre_locked_balance"
        case postLockedBalance = "post_locked_balance"
        case preStateHash = "pre_state_hash"
        case postStateHash = "post_state_hash"
        case localRevision = "local_revision"
        case counterpartyLineageId = "counterparty_lineage_id"
        case counterpartyAccountId = "counterparty_account_id"
        case counterpartyDeviceId = "counterparty_device_id"
        case counterpartyOfflinePublicKey = "counterparty_offline_public_key"
        case amount
        case authorization
        case deviceProof = "device_proof"
        case sourcePayload = "source_payload"
        case senderSignatureBase64 = "sender_signature_base64"
        case createdAtMs = "created_at_ms"
    }
}

public struct ToriiOfflineOutgoingTransferPayload: Codable, Sendable, Equatable {
    public let version: Int
    public let anchor: ToriiOfflineCashState
    public let ancestryReceipts: [ToriiOfflineTransferReceipt]
    public let receipt: ToriiOfflineTransferReceipt

    public init(
        version: Int = 1,
        anchor: ToriiOfflineCashState,
        ancestryReceipts: [ToriiOfflineTransferReceipt],
        receipt: ToriiOfflineTransferReceipt
    ) {
        self.version = version
        self.anchor = anchor
        self.ancestryReceipts = ancestryReceipts
        self.receipt = receipt
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case anchor
        case ancestryReceipts = "ancestry_receipts"
        case receipt
    }
}

public struct ToriiOfflineCashSetupRequest: Codable, Sendable, Equatable {
    public let accountId: String
    public let assetDefinitionId: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        accountId: String,
        assetDefinitionId: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof
    ) {
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
    }

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineCashLoadRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let lineageId: String?
    public let accountId: String
    public let assetDefinitionId: String
    public let amount: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        operationId: String,
        lineageId: String?,
        accountId: String,
        assetDefinitionId: String,
        amount: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof
    ) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = amount
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineCashRefreshRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let lineageId: String
    public let accountId: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof

    public init(
        operationId: String,
        lineageId: String,
        accountId: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof
    ) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
    }
}

public struct ToriiOfflineCashSyncRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let lineageId: String
    public let accountId: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof
    public let receipts: [ToriiOfflineTransferReceipt]

    public init(
        operationId: String,
        lineageId: String,
        accountId: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof,
        receipts: [ToriiOfflineTransferReceipt]
    ) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
        self.receipts = receipts
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
        case receipts
    }
}

public struct ToriiOfflineCashRedeemRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let lineageId: String
    public let accountId: String
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let deviceProof: ToriiOfflineDeviceProof
    public let amount: String
    public let receipts: [ToriiOfflineTransferReceipt]

    public init(
        operationId: String,
        lineageId: String,
        accountId: String,
        deviceBinding: ToriiOfflineDeviceBinding,
        deviceProof: ToriiOfflineDeviceProof,
        amount: String,
        receipts: [ToriiOfflineTransferReceipt]
    ) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceBinding = deviceBinding
        self.deviceProof = deviceProof
        self.amount = amount
        self.receipts = receipts
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case deviceBinding = "device_binding"
        case deviceProof = "device_proof"
        case amount
        case receipts
    }
}

public struct ToriiOfflineCashEnvelope: Codable, Sendable, Equatable {
    public let lineageState: ToriiOfflineCashState

    public init(lineageState: ToriiOfflineCashState) {
        self.lineageState = lineageState
    }

    private enum CodingKeys: String, CodingKey {
        case lineageState = "lineage_state"
    }
}

public enum ToriiOfflineCashCodec {
    public enum Error: LocalizedError, Equatable {
        case invalidSignature
        case invalidPublicKey
        case invalidSignatureEncoding

        public var errorDescription: String? {
            switch self {
            case .invalidSignature:
                return "Offline cash signature is invalid."
            case .invalidPublicKey:
                return "Offline cash public key is invalid."
            case .invalidSignatureEncoding:
                return "Offline cash signature encoding is invalid."
            }
        }
    }

    public static func currentTimestampMs(date: Date = Date()) -> UInt64 {
        UInt64((date.timeIntervalSince1970 * 1000).rounded())
    }

    public static func canonicalData<T: Encodable>(_ value: T) throws -> Data {
        let encoder = JSONEncoder()
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting = [.sortedKeys]
        }
        return try encoder.encode(value)
    }

    public static func hashHex<T: Encodable>(_ value: T) throws -> String {
        let digest = SHA256.hash(data: try canonicalData(value))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    public static func canonicalAmountString(_ rawValue: String) throws -> String {
        let parsed = try parseAmount(rawValue)
        return NSDecimalNumber(decimal: parsed).stringValue
    }

    public static func addAmounts(_ lhs: String, _ rhs: String) throws -> String {
        let result = try parseAmount(lhs) + parseAmount(rhs)
        return NSDecimalNumber(decimal: result).stringValue
    }

    public static func subtractAmounts(_ lhs: String, _ rhs: String) throws -> String {
        let result = try parseAmount(lhs) - parseAmount(rhs)
        if NSDecimalNumber(decimal: result).compare(NSDecimalNumber.zero) == .orderedAscending {
            throw ToriiOfflineAmountError.negativeResult
        }
        return NSDecimalNumber(decimal: result).stringValue
    }

    public static func compareAmounts(_ lhs: String, _ rhs: String) throws -> ComparisonResult {
        let left = NSDecimalNumber(decimal: try parseAmount(lhs))
        let right = NSDecimalNumber(decimal: try parseAmount(rhs))
        return left.compare(right)
    }

    public static func nextLocalStateHash(
        lineageId: String,
        previousStateHash: String,
        transferId: String,
        direction: ToriiOfflineTransferDirection,
        counterpartyLineageId: String,
        amount: String,
        localRevision: UInt64,
        postBalance: String,
        postLockedBalance: String
    ) throws -> String {
        try hashHex(
            LocalStateHashPayload(
                lineageId: lineageId,
                previousStateHash: previousStateHash,
                transferId: transferId,
                direction: direction.rawValue,
                counterpartyLineageId: counterpartyLineageId,
                amount: canonicalAmountString(amount),
                localRevision: localRevision,
                postBalance: canonicalAmountString(postBalance),
                postLockedBalance: canonicalAmountString(postLockedBalance)
            )
        )
    }

    public static func verifyIssuerSignature(
        authorization: ToriiOfflineSpendAuthorization,
        issuerPublicKeyBase64: String
    ) throws {
        try verifySignature(
            payload: authorizationUnsignedPayload(authorization),
            signatureBase64: authorization.issuerSignatureBase64,
            publicKeyBase64: issuerPublicKeyBase64
        )
    }

    public static func verifyIssuerSignature(
        lineageState: ToriiOfflineCashState,
        issuerPublicKeyBase64: String
    ) throws {
        try verifySignature(
            payload: lineageStateUnsignedPayload(lineageState),
            signatureBase64: lineageState.issuerSignatureBase64,
            publicKeyBase64: issuerPublicKeyBase64
        )
    }

    public static func verifyIssuerSignature(
        revocationBundle: ToriiOfflineRevocationBundle,
        issuerPublicKeyBase64: String
    ) throws {
        try verifySignature(
            payload: revocationBundleUnsignedPayload(revocationBundle),
            signatureBase64: revocationBundle.issuerSignatureBase64,
            publicKeyBase64: issuerPublicKeyBase64
        )
    }

    public static func verifyReceiptSignature(_ receipt: ToriiOfflineTransferReceipt) throws {
        try verifySignature(
            payload: transferReceiptUnsignedPayload(receipt),
            signatureBase64: receipt.senderSignatureBase64,
            publicKeyBase64: receipt.offlinePublicKey
        )
    }

    public static func authorizationUnsignedPayload(_ authorization: ToriiOfflineSpendAuthorization) throws -> Data {
        try canonicalData(
            AuthorizationUnsignedPayload(
                authorizationId: authorization.authorizationId,
                lineageId: authorization.lineageId,
                accountId: authorization.accountId,
                deviceId: authorization.deviceId,
                offlinePublicKey: authorization.offlinePublicKey,
                verdictId: authorization.verdictId,
                policyMaxBalance: try canonicalAmountString(authorization.policyMaxBalance),
                policyMaxTxValue: try canonicalAmountString(authorization.policyMaxTxValue),
                issuedAtMs: authorization.issuedAtMs,
                refreshAtMs: authorization.refreshAtMs,
                expiresAtMs: authorization.expiresAtMs,
                appAttestKeyId: authorization.appAttestKeyId
            )
        )
    }

    public static func lineageStateUnsignedPayload(_ lineageState: ToriiOfflineCashState) throws -> Data {
        try canonicalData(
            CashStateUnsignedPayload(
                lineageId: lineageState.lineageId,
                accountId: lineageState.accountId,
                deviceId: lineageState.deviceId,
                offlinePublicKey: lineageState.offlinePublicKey,
                assetDefinitionId: lineageState.assetDefinitionId,
                balance: try canonicalAmountString(lineageState.balance),
                lockedBalance: try canonicalAmountString(lineageState.lockedBalance),
                serverRevision: lineageState.serverRevision,
                serverStateHash: lineageState.serverStateHash,
                pendingLocalRevision: lineageState.pendingLocalRevision,
                authorizationId: lineageState.authorization.authorizationId
            )
        )
    }

    public static func revocationBundleUnsignedPayload(_ bundle: ToriiOfflineRevocationBundle) throws -> Data {
        try canonicalData(
            RevocationBundleUnsignedPayload(
                issuedAtMs: bundle.issuedAtMs,
                expiresAtMs: bundle.expiresAtMs,
                verdictIds: bundle.verdictIds.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.sorted()
            )
        )
    }

    public static func transferReceiptUnsignedPayload(_ receipt: ToriiOfflineTransferReceipt) throws -> Data {
        try canonicalData(
            TransferReceiptUnsignedPayload(
                version: receipt.version,
                transferId: receipt.transferId,
                direction: receipt.direction.rawValue,
                lineageId: receipt.lineageId,
                accountId: receipt.accountId,
                deviceId: receipt.deviceId,
                offlinePublicKey: receipt.offlinePublicKey,
                preBalance: try canonicalAmountString(receipt.preBalance),
                postBalance: try canonicalAmountString(receipt.postBalance),
                preLockedBalance: try canonicalAmountString(receipt.preLockedBalance),
                postLockedBalance: try canonicalAmountString(receipt.postLockedBalance),
                preStateHash: receipt.preStateHash,
                postStateHash: receipt.postStateHash,
                localRevision: receipt.localRevision,
                counterpartyLineageId: receipt.counterpartyLineageId,
                counterpartyAccountId: receipt.counterpartyAccountId,
                counterpartyDeviceId: receipt.counterpartyDeviceId,
                counterpartyOfflinePublicKey: receipt.counterpartyOfflinePublicKey,
                amount: try canonicalAmountString(receipt.amount),
                authorization: receipt.authorization,
                deviceProof: receipt.deviceProof,
                sourcePayload: receipt.sourcePayload,
                createdAtMs: receipt.createdAtMs
            )
        )
    }
}

private extension ToriiOfflineCashCodec {
    struct AuthorizationUnsignedPayload: Encodable {
        let authorizationId: String
        let lineageId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let verdictId: String
        let policyMaxBalance: String
        let policyMaxTxValue: String
        let issuedAtMs: UInt64
        let refreshAtMs: UInt64
        let expiresAtMs: UInt64
        let appAttestKeyId: String

        enum CodingKeys: String, CodingKey {
            case authorizationId = "authorization_id"
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case deviceId = "device_id"
            case offlinePublicKey = "offline_public_key"
            case verdictId = "verdict_id"
            case policyMaxBalance = "max_balance"
            case policyMaxTxValue = "max_tx_value"
            case issuedAtMs = "issued_at_ms"
            case refreshAtMs = "refresh_at_ms"
            case expiresAtMs = "expires_at_ms"
            case appAttestKeyId = "app_attest_key_id"
        }
    }

    struct CashStateUnsignedPayload: Encodable {
        let lineageId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let assetDefinitionId: String
        let balance: String
        let lockedBalance: String
        let serverRevision: UInt64
        let serverStateHash: String
        let pendingLocalRevision: UInt64
        let authorizationId: String

        enum CodingKeys: String, CodingKey {
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case deviceId = "device_id"
            case offlinePublicKey = "offline_public_key"
            case assetDefinitionId = "asset_definition_id"
            case balance
            case lockedBalance = "locked_balance"
            case serverRevision = "server_revision"
            case serverStateHash = "server_state_hash"
            case pendingLocalRevision = "pending_local_revision"
            case authorizationId = "authorization_id"
        }
    }

    struct RevocationBundleUnsignedPayload: Encodable {
        let issuedAtMs: UInt64
        let expiresAtMs: UInt64
        let verdictIds: [String]

        enum CodingKeys: String, CodingKey {
            case issuedAtMs = "issued_at_ms"
            case expiresAtMs = "expires_at_ms"
            case verdictIds = "verdict_ids"
        }
    }

    struct TransferReceiptUnsignedPayload: Encodable {
        let version: Int
        let transferId: String
        let direction: String
        let lineageId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let preBalance: String
        let postBalance: String
        let preLockedBalance: String
        let postLockedBalance: String
        let preStateHash: String
        let postStateHash: String
        let localRevision: UInt64
        let counterpartyLineageId: String
        let counterpartyAccountId: String
        let counterpartyDeviceId: String
        let counterpartyOfflinePublicKey: String
        let amount: String
        let authorization: ToriiOfflineSpendAuthorization?
        let deviceProof: ToriiOfflineDeviceProof
        let sourcePayload: String?
        let createdAtMs: UInt64

        enum CodingKeys: String, CodingKey {
            case version
            case transferId = "transfer_id"
            case direction
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case deviceId = "device_id"
            case offlinePublicKey = "offline_public_key"
            case preBalance = "pre_balance"
            case postBalance = "post_balance"
            case preLockedBalance = "pre_locked_balance"
            case postLockedBalance = "post_locked_balance"
            case preStateHash = "pre_state_hash"
            case postStateHash = "post_state_hash"
            case localRevision = "local_revision"
            case counterpartyLineageId = "counterparty_lineage_id"
            case counterpartyAccountId = "counterparty_account_id"
            case counterpartyDeviceId = "counterparty_device_id"
            case counterpartyOfflinePublicKey = "counterparty_offline_public_key"
            case amount
            case authorization
            case deviceProof = "device_proof"
            case sourcePayload = "source_payload"
            case createdAtMs = "created_at_ms"
        }
    }

    struct LocalStateHashPayload: Encodable {
        let lineageId: String
        let previousStateHash: String
        let transferId: String
        let direction: String
        let counterpartyLineageId: String
        let amount: String
        let localRevision: UInt64
        let postBalance: String
        let postLockedBalance: String

        enum CodingKeys: String, CodingKey {
            case lineageId = "lineage_id"
            case previousStateHash = "previous_state_hash"
            case transferId = "transfer_id"
            case direction
            case counterpartyLineageId = "counterparty_lineage_id"
            case amount
            case localRevision = "local_revision"
            case postBalance = "post_balance"
            case postLockedBalance = "post_locked_balance"
        }
    }

    static func parseAmount(_ rawValue: String) throws -> Decimal {
        let normalized = rawValue
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: ",", with: "")
        guard !normalized.isEmpty,
              let value = Decimal(string: normalized, locale: Locale(identifier: "en_US_POSIX")) else {
            throw ToriiOfflineAmountError.invalidAmount(rawValue)
        }
        return value
    }

    static func verifySignature(
        payload: Data,
        signatureBase64: String,
        publicKeyBase64: String
    ) throws {
        guard let publicKeyData = Data(base64Encoded: publicKeyBase64) else {
            throw Error.invalidPublicKey
        }
        guard let signature = Data(base64Encoded: signatureBase64) else {
            throw Error.invalidSignatureEncoding
        }
        do {
            if publicKeyData.count == 32 {
                let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: publicKeyData)
                guard publicKey.isValidSignature(signature, for: payload) else {
                    throw Error.invalidSignature
                }
                return
            }

            let publicKey = try P256.Signing.PublicKey(x963Representation: publicKeyData)
            let ecdsaSignature = try P256.Signing.ECDSASignature(derRepresentation: signature)
            guard publicKey.isValidSignature(ecdsaSignature, for: payload) else {
                throw Error.invalidSignature
            }
        } catch let error as Error {
            throw error
        } catch {
            throw Error.invalidPublicKey
        }
    }
}
