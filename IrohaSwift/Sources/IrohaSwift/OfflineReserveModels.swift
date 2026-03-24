import CryptoKit
import Foundation

public enum ToriiOfflineAmountError: LocalizedError, Equatable {
    case invalidAmount(String)
    case negativeResult

    public var errorDescription: String? {
        switch self {
        case .invalidAmount(let value):
            return "Invalid offline reserve amount: \(value)"
        case .negativeResult:
            return "Offline reserve amount arithmetic produced a negative result."
        }
    }
}

public enum ToriiOfflineTransferDirection: String, Codable, Sendable {
    case incoming
    case outgoing
}

public struct ToriiOfflineDeviceAttestation: Codable, Sendable, Equatable {
    public let keyId: String
    public let counter: UInt64
    public let assertionBase64: String
    public let challengeHashHex: String

    public init(
        keyId: String,
        counter: UInt64,
        assertionBase64: String,
        challengeHashHex: String
    ) {
        self.keyId = keyId
        self.counter = counter
        self.assertionBase64 = assertionBase64
        self.challengeHashHex = challengeHashHex
    }

    private enum CodingKeys: String, CodingKey {
        case keyId = "key_id"
        case counter
        case assertionBase64 = "assertion_base64"
        case challengeHashHex = "challenge_hash_hex"
    }
}

public struct ToriiOfflineSpendAuthorization: Codable, Sendable, Equatable, Identifiable {
    public let authorizationId: String
    public let reserveId: String
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
        reserveId: String,
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
        self.reserveId = reserveId
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

    public func isExpired(nowMs: UInt64 = ToriiOfflineReserveCodec.currentTimestampMs()) -> Bool {
        nowMs >= expiresAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case authorizationId = "authorization_id"
        case reserveId = "reserve_id"
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

public struct ToriiOfflineReserveState: Codable, Sendable, Equatable, Identifiable {
    public let reserveId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let balance: String
    public let serverRevision: UInt64
    public let serverStateHash: String
    public let pendingLocalRevision: UInt64
    public let authorization: ToriiOfflineSpendAuthorization
    public let issuerSignatureBase64: String

    public var id: String { reserveId }

    public init(
        reserveId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        balance: String,
        serverRevision: UInt64,
        serverStateHash: String,
        pendingLocalRevision: UInt64,
        authorization: ToriiOfflineSpendAuthorization,
        issuerSignatureBase64: String
    ) {
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.balance = balance
        self.serverRevision = serverRevision
        self.serverStateHash = serverStateHash
        self.pendingLocalRevision = pendingLocalRevision
        self.authorization = authorization
        self.issuerSignatureBase64 = issuerSignatureBase64
    }

    private enum CodingKeys: String, CodingKey {
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case balance
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

    public func isExpired(nowMs: UInt64 = ToriiOfflineReserveCodec.currentTimestampMs()) -> Bool {
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
    public let reserveId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let preBalance: String
    public let postBalance: String
    public let preStateHash: String
    public let postStateHash: String
    public let localRevision: UInt64
    public let counterpartyReserveId: String
    public let counterpartyAccountId: String
    public let counterpartyDeviceId: String
    public let counterpartyOfflinePublicKey: String
    public let amount: String
    public let authorization: ToriiOfflineSpendAuthorization?
    public let attestation: ToriiOfflineDeviceAttestation
    public let sourcePayload: String?
    public let senderSignatureBase64: String
    public let createdAtMs: UInt64

    public var id: String { "\(reserveId):\(localRevision)" }

    public init(
        version: Int = 1,
        transferId: String,
        direction: ToriiOfflineTransferDirection,
        reserveId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        preBalance: String,
        postBalance: String,
        preStateHash: String,
        postStateHash: String,
        localRevision: UInt64,
        counterpartyReserveId: String,
        counterpartyAccountId: String,
        counterpartyDeviceId: String,
        counterpartyOfflinePublicKey: String,
        amount: String,
        authorization: ToriiOfflineSpendAuthorization?,
        attestation: ToriiOfflineDeviceAttestation,
        sourcePayload: String?,
        senderSignatureBase64: String,
        createdAtMs: UInt64
    ) {
        self.version = version
        self.transferId = transferId
        self.direction = direction
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.preBalance = preBalance
        self.postBalance = postBalance
        self.preStateHash = preStateHash
        self.postStateHash = postStateHash
        self.localRevision = localRevision
        self.counterpartyReserveId = counterpartyReserveId
        self.counterpartyAccountId = counterpartyAccountId
        self.counterpartyDeviceId = counterpartyDeviceId
        self.counterpartyOfflinePublicKey = counterpartyOfflinePublicKey
        self.amount = amount
        self.authorization = authorization
        self.attestation = attestation
        self.sourcePayload = sourcePayload
        self.senderSignatureBase64 = senderSignatureBase64
        self.createdAtMs = createdAtMs
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case transferId = "transfer_id"
        case direction
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case preBalance = "pre_balance"
        case postBalance = "post_balance"
        case preStateHash = "pre_state_hash"
        case postStateHash = "post_state_hash"
        case localRevision = "local_revision"
        case counterpartyReserveId = "counterparty_reserve_id"
        case counterpartyAccountId = "counterparty_account_id"
        case counterpartyDeviceId = "counterparty_device_id"
        case counterpartyOfflinePublicKey = "counterparty_offline_public_key"
        case amount
        case authorization
        case attestation
        case sourcePayload = "source_payload"
        case senderSignatureBase64 = "sender_signature_base64"
        case createdAtMs = "created_at_ms"
    }
}

public struct ToriiOfflineOutgoingTransferPayload: Codable, Sendable, Equatable {
    public let version: Int
    public let anchor: ToriiOfflineReserveState
    public let ancestryReceipts: [ToriiOfflineTransferReceipt]
    public let receipt: ToriiOfflineTransferReceipt

    public init(
        version: Int = 1,
        anchor: ToriiOfflineReserveState,
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

public struct ToriiOfflineReserveSetupRequest: Codable, Sendable, Equatable {
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let appAttestKeyId: String
    public let attestation: ToriiOfflineDeviceAttestation

    public init(
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        appAttestKeyId: String,
        attestation: ToriiOfflineDeviceAttestation
    ) {
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.appAttestKeyId = appAttestKeyId
        self.attestation = attestation
    }

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case appAttestKeyId = "app_attest_key_id"
        case attestation
    }
}

public struct ToriiOfflineReserveTopUpRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let reserveId: String?
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let appAttestKeyId: String
    public let amount: String
    public let attestation: ToriiOfflineDeviceAttestation

    public init(
        operationId: String,
        reserveId: String?,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        appAttestKeyId: String,
        amount: String,
        attestation: ToriiOfflineDeviceAttestation
    ) {
        self.operationId = operationId
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.appAttestKeyId = appAttestKeyId
        self.amount = amount
        self.attestation = attestation
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case appAttestKeyId = "app_attest_key_id"
        case amount
        case attestation
    }
}

public struct ToriiOfflineReserveRenewRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let reserveId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let appAttestKeyId: String
    public let attestation: ToriiOfflineDeviceAttestation

    public init(
        operationId: String,
        reserveId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        appAttestKeyId: String,
        attestation: ToriiOfflineDeviceAttestation
    ) {
        self.operationId = operationId
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.appAttestKeyId = appAttestKeyId
        self.attestation = attestation
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case appAttestKeyId = "app_attest_key_id"
        case attestation
    }
}

public struct ToriiOfflineReserveSyncRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let reserveId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let receipts: [ToriiOfflineTransferReceipt]

    public init(
        operationId: String,
        reserveId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        receipts: [ToriiOfflineTransferReceipt]
    ) {
        self.operationId = operationId
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.receipts = receipts
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case receipts
    }
}

public struct ToriiOfflineReserveDefundRequest: Codable, Sendable, Equatable {
    public let operationId: String
    public let reserveId: String
    public let accountId: String
    public let deviceId: String
    public let offlinePublicKey: String
    public let amount: String
    public let receipts: [ToriiOfflineTransferReceipt]

    public init(
        operationId: String,
        reserveId: String,
        accountId: String,
        deviceId: String,
        offlinePublicKey: String,
        amount: String,
        receipts: [ToriiOfflineTransferReceipt]
    ) {
        self.operationId = operationId
        self.reserveId = reserveId
        self.accountId = accountId
        self.deviceId = deviceId
        self.offlinePublicKey = offlinePublicKey
        self.amount = amount
        self.receipts = receipts
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case reserveId = "reserve_id"
        case accountId = "account_id"
        case deviceId = "device_id"
        case offlinePublicKey = "offline_public_key"
        case amount
        case receipts
    }
}

public struct ToriiOfflineReserveEnvelope: Codable, Sendable, Equatable {
    public let reserveState: ToriiOfflineReserveState

    public init(reserveState: ToriiOfflineReserveState) {
        self.reserveState = reserveState
    }

    private enum CodingKeys: String, CodingKey {
        case reserveState = "reserve_state"
    }
}

public enum ToriiOfflineReserveCodec {
    public enum Error: LocalizedError, Equatable {
        case invalidSignature
        case invalidPublicKey
        case invalidSignatureEncoding

        public var errorDescription: String? {
            switch self {
            case .invalidSignature:
                return "Offline reserve signature is invalid."
            case .invalidPublicKey:
                return "Offline reserve public key is invalid."
            case .invalidSignatureEncoding:
                return "Offline reserve signature encoding is invalid."
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
        reserveId: String,
        previousStateHash: String,
        transferId: String,
        direction: ToriiOfflineTransferDirection,
        counterpartyReserveId: String,
        amount: String,
        localRevision: UInt64,
        postBalance: String
    ) throws -> String {
        try hashHex(
            LocalStateHashPayload(
                reserveId: reserveId,
                previousStateHash: previousStateHash,
                transferId: transferId,
                direction: direction.rawValue,
                counterpartyReserveId: counterpartyReserveId,
                amount: canonicalAmountString(amount),
                localRevision: localRevision,
                postBalance: canonicalAmountString(postBalance)
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
        reserveState: ToriiOfflineReserveState,
        issuerPublicKeyBase64: String
    ) throws {
        try verifySignature(
            payload: reserveStateUnsignedPayload(reserveState),
            signatureBase64: reserveState.issuerSignatureBase64,
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
                reserveId: authorization.reserveId,
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

    public static func reserveStateUnsignedPayload(_ reserveState: ToriiOfflineReserveState) throws -> Data {
        try canonicalData(
            ReserveStateUnsignedPayload(
                reserveId: reserveState.reserveId,
                accountId: reserveState.accountId,
                deviceId: reserveState.deviceId,
                offlinePublicKey: reserveState.offlinePublicKey,
                balance: try canonicalAmountString(reserveState.balance),
                serverRevision: reserveState.serverRevision,
                serverStateHash: reserveState.serverStateHash,
                pendingLocalRevision: reserveState.pendingLocalRevision,
                authorizationId: reserveState.authorization.authorizationId
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
                reserveId: receipt.reserveId,
                accountId: receipt.accountId,
                deviceId: receipt.deviceId,
                offlinePublicKey: receipt.offlinePublicKey,
                preBalance: try canonicalAmountString(receipt.preBalance),
                postBalance: try canonicalAmountString(receipt.postBalance),
                preStateHash: receipt.preStateHash,
                postStateHash: receipt.postStateHash,
                localRevision: receipt.localRevision,
                counterpartyReserveId: receipt.counterpartyReserveId,
                counterpartyAccountId: receipt.counterpartyAccountId,
                counterpartyDeviceId: receipt.counterpartyDeviceId,
                counterpartyOfflinePublicKey: receipt.counterpartyOfflinePublicKey,
                amount: try canonicalAmountString(receipt.amount),
                authorization: receipt.authorization,
                attestation: receipt.attestation,
                sourcePayload: receipt.sourcePayload,
                createdAtMs: receipt.createdAtMs
            )
        )
    }
}

private extension ToriiOfflineReserveCodec {
    struct AuthorizationUnsignedPayload: Encodable {
        let authorizationId: String
        let reserveId: String
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
    }

    struct ReserveStateUnsignedPayload: Encodable {
        let reserveId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let balance: String
        let serverRevision: UInt64
        let serverStateHash: String
        let pendingLocalRevision: UInt64
        let authorizationId: String
    }

    struct RevocationBundleUnsignedPayload: Encodable {
        let issuedAtMs: UInt64
        let expiresAtMs: UInt64
        let verdictIds: [String]
    }

    struct TransferReceiptUnsignedPayload: Encodable {
        let version: Int
        let transferId: String
        let direction: String
        let reserveId: String
        let accountId: String
        let deviceId: String
        let offlinePublicKey: String
        let preBalance: String
        let postBalance: String
        let preStateHash: String
        let postStateHash: String
        let localRevision: UInt64
        let counterpartyReserveId: String
        let counterpartyAccountId: String
        let counterpartyDeviceId: String
        let counterpartyOfflinePublicKey: String
        let amount: String
        let authorization: ToriiOfflineSpendAuthorization?
        let attestation: ToriiOfflineDeviceAttestation
        let sourcePayload: String?
        let createdAtMs: UInt64
    }

    struct LocalStateHashPayload: Encodable {
        let reserveId: String
        let previousStateHash: String
        let transferId: String
        let direction: String
        let counterpartyReserveId: String
        let amount: String
        let localRevision: UInt64
        let postBalance: String
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
            let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: publicKeyData)
            guard publicKey.isValidSignature(signature, for: payload) else {
                throw Error.invalidSignature
            }
        } catch let error as Error {
            throw error
        } catch {
            throw Error.invalidPublicKey
        }
    }
}
