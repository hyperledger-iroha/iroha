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
    public let assertionPublicKey: String?
    public let attestationReportBase64: String
    public let attestationReceipt: ToriiOfflineAttestationReceipt?
    public let iosTeamId: String?
    public let iosBundleId: String?
    public let iosEnvironment: String?

    public init(
        platform: String,
        attestationKeyId: String,
        deviceId: String,
        offlinePublicKey: String,
        assertionPublicKey: String? = nil,
        attestationReportBase64: String,
        attestationReceipt: ToriiOfflineAttestationReceipt? = nil,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil
    ) {
        self.platform = platform
        self.attestationKeyId = attestationKeyId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.assertionPublicKey = assertionPublicKey
        self.attestationReportBase64 = attestationReportBase64
        self.attestationReceipt = attestationReceipt
        self.iosTeamId = iosTeamId
        self.iosBundleId = iosBundleId
        self.iosEnvironment = iosEnvironment
    }

    private enum CodingKeys: String, CodingKey {
        case platform
        case attestationKeyId = "attestation_key_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case assertionPublicKey = "assertion_public_key"
        case attestationReportBase64 = "attestation_report_base64"
        case attestationReceipt = "attestation_receipt"
        case iosTeamId = "ios_team_id"
        case iosBundleId = "ios_bundle_id"
        case iosEnvironment = "ios_environment"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(platform, forKey: .platform)
        try container.encode(attestationKeyId, forKey: .attestationKeyId)
        try container.encode(deviceId, forKey: .deviceId)
        try container.encode(offlinePublicKey, forKey: .offlinePublicKey)
        try container.encodeIfPresent(assertionPublicKey, forKey: .assertionPublicKey)
        try container.encode(attestationReportBase64, forKey: .attestationReportBase64)
        try container.encodeIfPresent(attestationReceipt, forKey: .attestationReceipt)
        try container.encodeIfPresent(iosTeamId, forKey: .iosTeamId)
        try container.encodeIfPresent(iosBundleId, forKey: .iosBundleId)
        try container.encodeIfPresent(iosEnvironment, forKey: .iosEnvironment)
    }
}

public struct ToriiOfflineAttestationReceipt: Codable, Sendable, Equatable {
    public let version: UInt64
    public let platform: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKeyBase64: String
    public let assertionPublicKeyBase64: String
    public let assertionScheme: String
    public let assertionKeyAlgorithm: String
    public let attestationKeyId: String
    public let hardwareOneUse: Bool
    public let attestationReportHashHex: String
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let signatureBase64: String

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKeyBase64 = "offline_public_key_base64"
        case assertionPublicKeyBase64 = "assertion_public_key_base64"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case attestationKeyId = "attestation_key_id"
        case hardwareOneUse = "hardware_one_use"
        case attestationReportHashHex = "attestation_report_hash_hex"
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case signatureBase64 = "signature_base64"
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
    public let verdictId: String
    public let policyMaxBalance: String
    public let policyMaxTxValue: String
    public let issuedAtMs: UInt64
    public let refreshAtMs: UInt64
    public let expiresAtMs: UInt64
    public let deviceBinding: ToriiOfflineDeviceBinding
    public let issuerSignatureBase64: String

    public var id: String { authorizationId }
    public var deviceId: String { deviceBinding.deviceId }
    public var offlinePublicKey: String { deviceBinding.offlinePublicKey }
    public var attestationKeyId: String { deviceBinding.attestationKeyId }

    public init(
        authorizationId: String,
        lineageId: String,
        accountId: String,
        verdictId: String,
        policyMaxBalance: String,
        policyMaxTxValue: String,
        issuedAtMs: UInt64,
        refreshAtMs: UInt64,
        expiresAtMs: UInt64,
        deviceBinding: ToriiOfflineDeviceBinding,
        issuerSignatureBase64: String
    ) throws {
        self.authorizationId = authorizationId
        self.lineageId = lineageId
        self.accountId = accountId
        self.verdictId = verdictId
        self.policyMaxBalance = try ToriiOfflineCashCodec.canonicalAmountString(policyMaxBalance)
        self.policyMaxTxValue = try ToriiOfflineCashCodec.canonicalAmountString(policyMaxTxValue)
        self.issuedAtMs = issuedAtMs
        self.refreshAtMs = refreshAtMs
        self.expiresAtMs = expiresAtMs
        self.deviceBinding = deviceBinding
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            authorizationId: container.decode(String.self, forKey: .authorizationId),
            lineageId: container.decode(String.self, forKey: .lineageId),
            accountId: container.decode(String.self, forKey: .accountId),
            verdictId: container.decode(String.self, forKey: .verdictId),
            policyMaxBalance: container.decode(String.self, forKey: .policyMaxBalance),
            policyMaxTxValue: container.decode(String.self, forKey: .policyMaxTxValue),
            issuedAtMs: container.decode(UInt64.self, forKey: .issuedAtMs),
            refreshAtMs: container.decode(UInt64.self, forKey: .refreshAtMs),
            expiresAtMs: container.decode(UInt64.self, forKey: .expiresAtMs),
            deviceBinding: container.decode(ToriiOfflineDeviceBinding.self, forKey: .deviceBinding),
            issuerSignatureBase64: container.decode(String.self, forKey: .issuerSignatureBase64)
        )
    }

    public func isExpired(nowMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs()) -> Bool {
        nowMs >= expiresAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case authorizationId = "authorization_id"
        case lineageId = "lineage_id"
        case accountId = "account_id"
        case verdictId = "verdict_id"
        case policyMaxBalance = "max_balance"
        case policyMaxTxValue = "max_tx_value"
        case issuedAtMs = "issued_at_ms"
        case refreshAtMs = "refresh_at_ms"
        case expiresAtMs = "expires_at_ms"
        case deviceBinding = "device_binding"
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
    ) throws {
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.assetDefinitionId = assetDefinitionId
        self.balance = try ToriiOfflineCashCodec.canonicalAmountString(balance)
        self.lockedBalance = try ToriiOfflineCashCodec.canonicalAmountString(lockedBalance)
        self.serverRevision = serverRevision
        self.serverStateHash = serverStateHash
        self.pendingLocalRevision = pendingLocalRevision
        self.authorization = authorization
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            lineageId: container.decode(String.self, forKey: .lineageId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            offlinePublicKey: container.decode(String.self, forKey: .offlinePublicKey),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            balance: container.decode(String.self, forKey: .balance),
            lockedBalance: container.decode(String.self, forKey: .lockedBalance),
            serverRevision: container.decode(UInt64.self, forKey: .serverRevision),
            serverStateHash: container.decode(String.self, forKey: .serverStateHash),
            pendingLocalRevision: container.decode(UInt64.self, forKey: .pendingLocalRevision),
            authorization: container.decode(ToriiOfflineSpendAuthorization.self, forKey: .authorization),
            issuerSignatureBase64: container.decode(String.self, forKey: .issuerSignatureBase64)
        )
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
    public let blacklistedAccountIds: [String]
    public let assetSendLimits: [ToriiOfflineAssetSendLimit]
    public let issuerSignatureBase64: String

    public init(
        issuedAtMs: UInt64,
        expiresAtMs: UInt64,
        verdictIds: [String],
        blacklistedAccountIds: [String] = [],
        assetSendLimits: [ToriiOfflineAssetSendLimit] = [],
        issuerSignatureBase64: String
    ) {
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.verdictIds = verdictIds
        self.blacklistedAccountIds = blacklistedAccountIds
        self.assetSendLimits = assetSendLimits
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        issuedAtMs = try container.decode(UInt64.self, forKey: .issuedAtMs)
        expiresAtMs = try container.decode(UInt64.self, forKey: .expiresAtMs)
        verdictIds = try container.decode([String].self, forKey: .verdictIds)
        blacklistedAccountIds = try container.decodeIfPresent([String].self, forKey: .blacklistedAccountIds) ?? []
        assetSendLimits = try container.decodeIfPresent([ToriiOfflineAssetSendLimit].self, forKey: .assetSendLimits) ?? []
        issuerSignatureBase64 = try container.decode(String.self, forKey: .issuerSignatureBase64)
    }

    public func isExpired(nowMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs()) -> Bool {
        nowMs >= expiresAtMs
    }

    public func blacklistsAccount(_ accountId: String) -> Bool {
        let normalized = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        return blacklistedAccountIds.contains {
            $0.trimmingCharacters(in: .whitespacesAndNewlines) == normalized
        }
    }

    public func revokesVerdict(_ verdictId: String?) -> Bool {
        let normalized = verdictId?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !normalized.isEmpty else { return false }
        return verdictIds.contains {
            $0.trimmingCharacters(in: .whitespacesAndNewlines).caseInsensitiveCompare(normalized) == .orderedSame
        }
    }

    public func sendLimit(assetDefinitionId: String) -> ToriiOfflineAssetSendLimit? {
        let normalized = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return nil }
        return assetSendLimits.first {
            $0.assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
                .caseInsensitiveCompare(normalized) == .orderedSame
        }
    }

    private enum CodingKeys: String, CodingKey {
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case verdictIds = "verdict_ids"
        case blacklistedAccountIds = "blacklisted_account_ids"
        case assetSendLimits = "asset_send_limits"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

public struct ToriiOfflineAssetSendLimit: Codable, Sendable, Equatable {
    public let assetDefinitionId: String
    public let dailySendLimit: String
    public let monthlySendLimit: String

    public init(
        assetDefinitionId: String,
        dailySendLimit: String,
        monthlySendLimit: String
    ) throws {
        self.assetDefinitionId = assetDefinitionId
        self.dailySendLimit = try ToriiOfflineCashCodec.canonicalAmountString(dailySendLimit)
        self.monthlySendLimit = try ToriiOfflineCashCodec.canonicalAmountString(monthlySendLimit)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            dailySendLimit: container.decode(String.self, forKey: .dailySendLimit),
            monthlySendLimit: container.decode(String.self, forKey: .monthlySendLimit)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case assetDefinitionId = "asset_definition_id"
        case dailySendLimit = "daily_send_limit"
        case monthlySendLimit = "monthly_send_limit"
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
        senderSignatureBase64: String,
        createdAtMs: UInt64
    ) throws {
        self.version = version
        self.transferId = transferId
        self.direction = direction
        self.lineageId = lineageId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.preBalance = try ToriiOfflineCashCodec.canonicalAmountString(preBalance)
        self.postBalance = try ToriiOfflineCashCodec.canonicalAmountString(postBalance)
        self.preLockedBalance = try ToriiOfflineCashCodec.canonicalAmountString(preLockedBalance)
        self.postLockedBalance = try ToriiOfflineCashCodec.canonicalAmountString(postLockedBalance)
        self.preStateHash = preStateHash
        self.postStateHash = postStateHash
        self.localRevision = localRevision
        self.counterpartyLineageId = counterpartyLineageId
        self.counterpartyAccountId = counterpartyAccountId
        self.counterpartyDeviceId = counterpartyDeviceId
        self.counterpartyOfflinePublicKey = counterpartyOfflinePublicKey
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.authorization = authorization
        self.deviceProof = deviceProof
        self.senderSignatureBase64 = senderSignatureBase64
        self.createdAtMs = createdAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decodeIfPresent(Int.self, forKey: .version) ?? 1,
            transferId: container.decode(String.self, forKey: .transferId),
            direction: container.decode(ToriiOfflineTransferDirection.self, forKey: .direction),
            lineageId: container.decode(String.self, forKey: .lineageId),
            accountId: container.decode(String.self, forKey: .accountId),
            deviceId: container.decode(String.self, forKey: .deviceId),
            offlinePublicKey: container.decode(String.self, forKey: .offlinePublicKey),
            preBalance: container.decode(String.self, forKey: .preBalance),
            postBalance: container.decode(String.self, forKey: .postBalance),
            preLockedBalance: container.decode(String.self, forKey: .preLockedBalance),
            postLockedBalance: container.decode(String.self, forKey: .postLockedBalance),
            preStateHash: container.decode(String.self, forKey: .preStateHash),
            postStateHash: container.decode(String.self, forKey: .postStateHash),
            localRevision: container.decode(UInt64.self, forKey: .localRevision),
            counterpartyLineageId: container.decode(String.self, forKey: .counterpartyLineageId),
            counterpartyAccountId: container.decode(String.self, forKey: .counterpartyAccountId),
            counterpartyDeviceId: container.decode(String.self, forKey: .counterpartyDeviceId),
            counterpartyOfflinePublicKey: container.decode(String.self, forKey: .counterpartyOfflinePublicKey),
            amount: container.decode(String.self, forKey: .amount),
            authorization: container.decodeIfPresent(ToriiOfflineSpendAuthorization.self, forKey: .authorization),
            deviceProof: container.decode(ToriiOfflineDeviceProof.self, forKey: .deviceProof),
            senderSignatureBase64: container.decode(String.self, forKey: .senderSignatureBase64),
            createdAtMs: container.decode(UInt64.self, forKey: .createdAtMs)
        )
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
        case senderSignatureBase64 = "sender_signature_base64"
        case createdAtMs = "created_at_ms"
    }
}

public struct ToriiOfflineCashEnvelope: Codable, Sendable, Equatable {
    public let lineageState: ToriiOfflineCashState

    public init(
        lineageState: ToriiOfflineCashState
    ) {
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
            if #available(iOS 13.0, macOS 10.15, *) {
                encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
            } else {
                encoder.outputFormatting = [.sortedKeys]
            }
        }
        return try encoder.encode(value)
    }

    public static func hashHex<T: Encodable>(_ value: T) throws -> String {
        let digest = SHA256.hash(data: try canonicalData(value))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    /// Canonicalizes an amount string to match the Rust `Numeric::Display` format.
    /// The parser is strict: it rejects malformed inputs and mantissas that exceed
    /// the same 64-byte signed bound enforced by Norito numeric encoding.
    public static func canonicalAmountString(_ rawValue: String) throws -> String {
        do {
            return try OfflineNorito.parseNumeric(rawValue).canonicalString
        } catch {
            throw ToriiOfflineAmountError.invalidAmount(rawValue)
        }
    }

    public static func addAmounts(_ lhs: String, _ rhs: String) throws -> String {
        let left = try parseAmount(lhs)
        let right = try parseAmount(rhs)
        return try left.adding(right, maxBytes: OfflineNorito.maxBigIntBytes).canonicalString
    }

    public static func subtractAmounts(_ lhs: String, _ rhs: String) throws -> String {
        let left = try parseAmount(lhs)
        let right = try parseAmount(rhs)
        let result = try left.subtracting(right, maxBytes: OfflineNorito.maxBigIntBytes)
        if result.isNegative {
            throw ToriiOfflineAmountError.negativeResult
        }
        return result.canonicalString
    }

    public static func compareAmounts(_ lhs: String, _ rhs: String) throws -> ComparisonResult {
        let left = try parseAmount(lhs)
        let right = try parseAmount(rhs)
        return left.compared(to: right)
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
            payload: cashTransferReceiptUnsignedPayload(receipt),
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
                verdictId: authorization.verdictId,
                policyMaxBalance: try canonicalAmountString(authorization.policyMaxBalance),
                policyMaxTxValue: try canonicalAmountString(authorization.policyMaxTxValue),
                issuedAtMs: authorization.issuedAtMs,
                refreshAtMs: authorization.refreshAtMs,
                expiresAtMs: authorization.expiresAtMs,
                deviceBinding: authorization.deviceBinding
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
                verdictIds: bundle.verdictIds.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.sorted(),
                blacklistedAccountIds: bundle.blacklistedAccountIds
                    .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .sorted(),
                assetSendLimits: try bundle.assetSendLimits
                    .sorted(by: { $0.assetDefinitionId < $1.assetDefinitionId })
                    .map { limit in
                        RevocationBundleAssetSendLimit(
                            assetDefinitionId: limit.assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines),
                            dailySendLimit: try canonicalAmountString(limit.dailySendLimit),
                            monthlySendLimit: try canonicalAmountString(limit.monthlySendLimit)
                        )
                    }
            )
        )
    }

    public static func cashTransferReceiptUnsignedPayload(_ receipt: ToriiOfflineTransferReceipt) throws -> Data {
        return try canonicalData(
            CashTransferReceiptUnsignedPayload(
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
                authorization: try receipt.authorization.map { authorization in
                    CashTransferReceiptAuthorizationPayload(
                        authorizationId: authorization.authorizationId,
                        lineageId: authorization.lineageId,
                        accountId: authorization.accountId,
                        verdictId: authorization.verdictId,
                        policyMaxBalance: try canonicalAmountString(authorization.policyMaxBalance),
                        policyMaxTxValue: try canonicalAmountString(authorization.policyMaxTxValue),
                        issuedAtMs: authorization.issuedAtMs,
                        refreshAtMs: authorization.refreshAtMs,
                        expiresAtMs: authorization.expiresAtMs,
                        deviceBinding: authorization.deviceBinding,
                        issuerSignatureBase64: authorization.issuerSignatureBase64
                    )
                },
                attestation: TransferReceiptAttestationPayload(
                    keyId: receipt.deviceProof.attestationKeyId,
                    counter: receipt.deviceProof.counter ?? 0,
                    assertionBase64: receipt.deviceProof.assertionBase64,
                    challengeHashHex: receipt.deviceProof.challengeHashHex
                ),
                createdAtMs: receipt.createdAtMs
            )
        )
    }

    public static func cashTransferReceiptLineageHashHex(_ receipt: ToriiOfflineTransferReceipt) throws -> String {
        let digest = SHA256.hash(
            data: try canonicalData(
                CashTransferReceiptLineageHashPayload(
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
                    authorization: try receipt.authorization.map { authorization in
                        CashTransferReceiptLineageAuthorizationPayload(
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
                            deviceBinding: authorization.deviceBinding,
                            issuerSignatureBase64: authorization.issuerSignatureBase64
                        )
                    },
                    attestation: TransferReceiptAttestationPayload(
                        keyId: receipt.deviceProof.attestationKeyId,
                        counter: receipt.deviceProof.counter ?? 0,
                        assertionBase64: receipt.deviceProof.assertionBase64,
                        challengeHashHex: receipt.deviceProof.challengeHashHex
                    ),
                    senderSignatureBase64: receipt.senderSignatureBase64,
                    createdAtMs: receipt.createdAtMs
                )
            )
        )
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}

private extension ToriiOfflineCashCodec {
    struct AuthorizationUnsignedPayload: Encodable {
        let authorizationId: String
        let lineageId: String
        let accountId: String
        let verdictId: String
        let policyMaxBalance: String
        let policyMaxTxValue: String
        let issuedAtMs: UInt64
        let refreshAtMs: UInt64
        let expiresAtMs: UInt64
        let deviceBinding: ToriiOfflineDeviceBinding

        enum CodingKeys: String, CodingKey {
            case authorizationId = "authorization_id"
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case verdictId = "verdict_id"
            case policyMaxBalance = "max_balance"
            case policyMaxTxValue = "max_tx_value"
            case issuedAtMs = "issued_at_ms"
            case refreshAtMs = "refresh_at_ms"
            case expiresAtMs = "expires_at_ms"
            case deviceBinding = "device_binding"
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
        let blacklistedAccountIds: [String]
        let assetSendLimits: [RevocationBundleAssetSendLimit]

        enum CodingKeys: String, CodingKey {
            case issuedAtMs = "issued_at_ms"
            case expiresAtMs = "expires_at_ms"
            case verdictIds = "verdict_ids"
            case blacklistedAccountIds = "blacklisted_account_ids"
            case assetSendLimits = "asset_send_limits"
        }
    }

    struct RevocationBundleAssetSendLimit: Encodable {
        let assetDefinitionId: String
        let dailySendLimit: String
        let monthlySendLimit: String

        enum CodingKeys: String, CodingKey {
            case assetDefinitionId = "asset_definition_id"
            case dailySendLimit = "daily_send_limit"
            case monthlySendLimit = "monthly_send_limit"
        }
    }

    struct CashTransferReceiptAuthorizationPayload: Encodable {
        let authorizationId: String
        let lineageId: String
        let accountId: String
        let verdictId: String
        let policyMaxBalance: String
        let policyMaxTxValue: String
        let issuedAtMs: UInt64
        let refreshAtMs: UInt64
        let expiresAtMs: UInt64
        let deviceBinding: ToriiOfflineDeviceBinding
        let issuerSignatureBase64: String

        enum CodingKeys: String, CodingKey {
            case authorizationId = "authorization_id"
            case lineageId = "lineage_id"
            case accountId = "account_id"
            case verdictId = "verdict_id"
            case policyMaxBalance = "max_balance"
            case policyMaxTxValue = "max_tx_value"
            case issuedAtMs = "issued_at_ms"
            case refreshAtMs = "refresh_at_ms"
            case expiresAtMs = "expires_at_ms"
            case deviceBinding = "device_binding"
            case issuerSignatureBase64 = "issuer_signature_base64"
        }
    }

    struct CashTransferReceiptUnsignedPayload: Encodable {
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
        let authorization: CashTransferReceiptAuthorizationPayload?
        let attestation: TransferReceiptAttestationPayload
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
            case attestation
            case createdAtMs = "created_at_ms"
        }
    }

    struct CashTransferReceiptLineageAuthorizationPayload: Encodable {
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
        let deviceBinding: ToriiOfflineDeviceBinding
        let issuerSignatureBase64: String

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
            case deviceBinding = "device_binding"
            case issuerSignatureBase64 = "issuer_signature_base64"
        }
    }

    struct CashTransferReceiptLineageHashPayload: Encodable {
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
        let authorization: CashTransferReceiptLineageAuthorizationPayload?
        let attestation: TransferReceiptAttestationPayload
        let senderSignatureBase64: String
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
            case attestation
            case senderSignatureBase64 = "sender_signature_base64"
            case createdAtMs = "created_at_ms"
        }
    }

    struct TransferReceiptAttestationPayload: Encodable {
        let keyId: String
        let counter: UInt64
        let assertionBase64: String
        let challengeHashHex: String

        enum CodingKeys: String, CodingKey {
            case keyId = "key_id"
            case counter
            case assertionBase64 = "assertion_base64"
            case challengeHashHex = "challenge_hash_hex"
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

    static func parseAmount(_ rawValue: String) throws -> OfflineCanonicalNumeric {
        do {
            return try OfflineNorito.parseCanonicalNumeric(rawValue)
        } catch {
            throw ToriiOfflineAmountError.invalidAmount(rawValue)
        }
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
