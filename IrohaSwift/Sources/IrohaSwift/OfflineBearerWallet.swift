import CryptoKit
import Foundation

public enum OfflineBearerV2Crypto {
    public static let ed25519 = "ed25519"
    public static let ecdsaP256SHA256 = "ecdsa_p256_sha256"
    public static let rawEd25519PublicKey = "raw_ed25519"
    public static let x963P256PublicKey = "x963_uncompressed_p256"
}

/// Capabilities reported by a hardware-backed Offline Bearer purse provider.
public struct OfflineBearerSecureElementCapabilities: Equatable, Sendable {
    public let hardwareBacked: Bool
    public let statefulPurse: Bool
    public let hardwareClass: String
    public let attestationKeyId: String?
    public let signatureAlgorithm: String
    public let publicKeyEncoding: String
    public let rollbackResistantState: Bool
    public let attestationEvidence: Data

    public init(hardwareBacked: Bool,
                statefulPurse: Bool,
                hardwareClass: String,
                attestationKeyId: String? = nil,
                signatureAlgorithm: String = OfflineBearerV2Crypto.ed25519,
                publicKeyEncoding: String = OfflineBearerV2Crypto.rawEd25519PublicKey,
                rollbackResistantState: Bool = false,
                attestationEvidence: Data = Data()) throws {
        guard !hardwareClass.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineBearerPolicyError("hardwareClass must not be blank")
        }
        try requireSupportedSignatureAlgorithm(signatureAlgorithm)
        try requireSupportedPublicKeyEncoding(publicKeyEncoding)
        self.hardwareBacked = hardwareBacked
        self.statefulPurse = statefulPurse
        self.hardwareClass = hardwareClass
        self.attestationKeyId = attestationKeyId
        self.signatureAlgorithm = signatureAlgorithm
        self.publicKeyEncoding = publicKeyEncoding
        self.rollbackResistantState = rollbackResistantState
        self.attestationEvidence = attestationEvidence
    }
}

/// V2 certificate binding an Offline Bearer purse to issuer-approved hardware.
public struct OfflineBearerCertificateV2: Equatable, Sendable {
    public let certificateId: String
    public let chainId: String
    public let issuerId: String
    public let purseId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let deviceId: String
    public let keyId: String
    public let hardwareClass: String
    public let signatureAlgorithm: String
    public let publicKeyEncoding: String
    public let publicKey: Data
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policyId: String
    public let policyHashHex: String
    public let issuerSignature: Data

    public init(certificateId: String,
                chainId: String,
                issuerId: String,
                purseId: String,
                accountId: String,
                assetDefinitionId: String,
                deviceId: String,
                keyId: String,
                hardwareClass: String,
                signatureAlgorithm: String = OfflineBearerV2Crypto.ed25519,
                publicKeyEncoding: String = OfflineBearerV2Crypto.rawEd25519PublicKey,
                publicKey: Data,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                policyId: String,
                policyHashHex: String,
                issuerSignature: Data) throws {
        try requireNonBlank(certificateId, "certificateId")
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(issuerId, "issuerId")
        try requireNonBlank(purseId, "purseId")
        try requireNonBlank(accountId, "accountId")
        try requireNonBlank(assetDefinitionId, "assetDefinitionId")
        try requireNonBlank(deviceId, "deviceId")
        try requireNonBlank(keyId, "keyId")
        try requireNonBlank(hardwareClass, "hardwareClass")
        try requireSupportedSignatureAlgorithm(signatureAlgorithm)
        try requireSupportedPublicKeyEncoding(publicKeyEncoding)
        guard !publicKey.isEmpty else { throw OfflineBearerPolicyError("publicKey must not be empty") }
        guard expiresAtMs > issuedAtMs else {
            throw OfflineBearerPolicyError("expiresAtMs must be after issuedAtMs")
        }
        try requireNonBlank(policyId, "policyId")
        try requireHexLike(policyHashHex, "policyHashHex")
        guard !issuerSignature.isEmpty else {
            throw OfflineBearerPolicyError("issuerSignature must not be empty")
        }
        self.certificateId = certificateId
        self.chainId = chainId
        self.issuerId = issuerId
        self.purseId = purseId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.deviceId = deviceId
        self.keyId = keyId
        self.hardwareClass = hardwareClass
        self.signatureAlgorithm = signatureAlgorithm
        self.publicKeyEncoding = publicKeyEncoding
        self.publicKey = publicKey
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.policyId = policyId
        self.policyHashHex = policyHashHex
        self.issuerSignature = issuerSignature
    }
}

/// Signed policy bundle used by correct apps to gate offline bearer acceptance.
public struct OfflineBearerAssetSendLimitV2: Equatable, Sendable {
    public let assetDefinitionId: String
    public let maxTransactionAmount: String
    public let dailySendLimit: String
    public let monthlySendLimit: String

    public init(assetDefinitionId: String,
                maxTransactionAmount: String,
                dailySendLimit: String,
                monthlySendLimit: String) throws {
        try requireNonBlank(assetDefinitionId, "assetDefinitionId")
        let canonicalMaxTransaction = try ToriiOfflineCashCodec.canonicalAmountString(maxTransactionAmount)
        let canonicalDaily = try ToriiOfflineCashCodec.canonicalAmountString(dailySendLimit)
        let canonicalMonthly = try ToriiOfflineCashCodec.canonicalAmountString(monthlySendLimit)
        try requirePositiveAmount(canonicalMaxTransaction, "maxTransactionAmount")
        try requirePositiveAmount(canonicalDaily, "dailySendLimit")
        try requirePositiveAmount(canonicalMonthly, "monthlySendLimit")
        self.assetDefinitionId = assetDefinitionId
        self.maxTransactionAmount = canonicalMaxTransaction
        self.dailySendLimit = canonicalDaily
        self.monthlySendLimit = canonicalMonthly
    }
}

public struct OfflineBearerPolicyBundleV2: Equatable, Sendable {
    public static let defaultMaxCertificateAgeMs: UInt64 = 24 * 60 * 60 * 1_000
    public static let defaultMaxPolicyAgeMs: UInt64 = 12 * 60 * 60 * 1_000
    public static let defaultMaxTokenAgeMs: UInt64 = 5 * 60 * 1_000

    public let policyId: String
    public let policyHashHex: String
    public let issuerId: String
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let maxCertificateAgeMs: UInt64
    public let maxPolicyAgeMs: UInt64
    public let maxTokenAgeMs: UInt64
    public let maxOfflineBalance: String
    public let maxTransactionAmount: String
    public let allowedHardwareClasses: Set<String>
    public let blacklistedAccountIds: Set<String>
    public let blacklistedDeviceIds: Set<String>
    public let blacklistedKeyIds: Set<String>
    public let policyEpoch: UInt64
    public let policySource: String
    public let revokedCertificateIds: Set<String>
    public let revokedTransferIds: Set<String>
    public let assetSendLimits: [OfflineBearerAssetSendLimitV2]
    public let signatureAlgorithm: String
    public let issuerSignature: Data

    public init(policyId: String,
                policyHashHex: String,
                issuerId: String,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                maxCertificateAgeMs: UInt64 = Self.defaultMaxCertificateAgeMs,
                maxPolicyAgeMs: UInt64 = Self.defaultMaxPolicyAgeMs,
                maxTokenAgeMs: UInt64 = Self.defaultMaxTokenAgeMs,
                maxOfflineBalance: String,
                maxTransactionAmount: String,
                allowedHardwareClasses: Set<String>,
                blacklistedAccountIds: Set<String> = [],
                blacklistedDeviceIds: Set<String> = [],
                blacklistedKeyIds: Set<String> = [],
                signatureAlgorithm: String = OfflineBearerV2Crypto.ed25519,
                issuerSignature: Data = Data([1]),
                policyEpoch: UInt64 = 0,
                policySource: String = "middleware",
                revokedCertificateIds: Set<String> = [],
                revokedTransferIds: Set<String> = [],
                assetSendLimits: [OfflineBearerAssetSendLimitV2] = []) throws {
        try requireNonBlank(policyId, "policyId")
        try requireHexLike(policyHashHex, "policyHashHex")
        try requireNonBlank(issuerId, "issuerId")
        guard expiresAtMs > issuedAtMs else {
            throw OfflineBearerPolicyError("expiresAtMs must be after issuedAtMs")
        }
        guard maxCertificateAgeMs > 0, maxPolicyAgeMs > 0, maxTokenAgeMs > 0 else {
            throw OfflineBearerPolicyError("policy time limits must be positive")
        }
        let normalizedHardware = normalizedSet(allowedHardwareClasses)
        guard !normalizedHardware.isEmpty else {
            throw OfflineBearerPolicyError("allowedHardwareClasses must not be empty")
        }
        let canonicalMaxBalance = try ToriiOfflineCashCodec.canonicalAmountString(maxOfflineBalance)
        let canonicalMaxTx = try ToriiOfflineCashCodec.canonicalAmountString(maxTransactionAmount)
        try requirePositiveAmount(canonicalMaxBalance, "maxOfflineBalance")
        try requirePositiveAmount(canonicalMaxTx, "maxTransactionAmount")
        try requireNonBlank(policySource, "policySource")
        try requireSupportedSignatureAlgorithm(signatureAlgorithm)
        guard !issuerSignature.isEmpty else {
            throw OfflineBearerPolicyError("issuerSignature must not be empty")
        }
        self.policyId = policyId
        self.policyHashHex = policyHashHex
        self.issuerId = issuerId
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.maxCertificateAgeMs = maxCertificateAgeMs
        self.maxPolicyAgeMs = maxPolicyAgeMs
        self.maxTokenAgeMs = maxTokenAgeMs
        self.maxOfflineBalance = canonicalMaxBalance
        self.maxTransactionAmount = canonicalMaxTx
        self.allowedHardwareClasses = normalizedHardware
        self.blacklistedAccountIds = normalizedSet(blacklistedAccountIds)
        self.blacklistedDeviceIds = normalizedSet(blacklistedDeviceIds)
        self.blacklistedKeyIds = normalizedSet(blacklistedKeyIds)
        self.policyEpoch = policyEpoch
        self.policySource = policySource
        self.revokedCertificateIds = normalizedSet(revokedCertificateIds)
        self.revokedTransferIds = normalizedSet(revokedTransferIds)
        self.assetSendLimits = assetSendLimits
        self.signatureAlgorithm = signatureAlgorithm
        self.issuerSignature = issuerSignature
    }
}

/// Current value and sequence tracked by a stateful Offline Bearer purse.
public struct OfflineBearerPurseStateV2: Equatable, Sendable {
    public let chainId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let purseId: String
    public let balance: String
    public let sequence: UInt64
    public let policyHashHex: String
    public let updatedAtMs: UInt64

    public init(chainId: String,
                accountId: String,
                assetDefinitionId: String,
                purseId: String,
                balance: String,
                sequence: UInt64,
                policyHashHex: String,
                updatedAtMs: UInt64) throws {
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(accountId, "accountId")
        try requireNonBlank(assetDefinitionId, "assetDefinitionId")
        try requireNonBlank(purseId, "purseId")
        let canonicalBalance = try ToriiOfflineCashCodec.canonicalAmountString(balance)
        try requireNonNegativeAmount(canonicalBalance, "balance")
        try requireHexLike(policyHashHex, "policyHashHex")
        self.chainId = chainId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.purseId = purseId
        self.balance = canonicalBalance
        self.sequence = sequence
        self.policyHashHex = policyHashHex
        self.updatedAtMs = updatedAtMs
    }
}

/// Recipient challenge for a v2 offline bearer payment.
public struct OfflineBearerReceiveRequestV2: Equatable, Sendable {
    public static let version = 2

    public let version: Int
    public let chainId: String
    public let paymentRequestId: String
    public let recipientCertificate: OfflineBearerCertificateV2
    public let assetDefinitionId: String
    public let amount: String
    public let createdAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policyHashHex: String
    public let signatureAlgorithm: String
    public let challengeSignature: Data

    public init(version: Int = Self.version,
                chainId: String,
                paymentRequestId: String,
                recipientCertificate: OfflineBearerCertificateV2,
                assetDefinitionId: String,
                amount: String,
                createdAtMs: UInt64,
                expiresAtMs: UInt64,
                policyHashHex: String,
                signatureAlgorithm: String? = nil,
                challengeSignature: Data) throws {
        guard version == Self.version else { throw OfflineBearerPolicyError("unsupported receive request version") }
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(paymentRequestId, "paymentRequestId")
        try requireNonBlank(assetDefinitionId, "assetDefinitionId")
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        try requirePositiveAmount(canonicalAmount, "amount")
        guard expiresAtMs > createdAtMs else {
            throw OfflineBearerPolicyError("expiresAtMs must be after createdAtMs")
        }
        try requireHexLike(policyHashHex, "policyHashHex")
        let effectiveAlgorithm = signatureAlgorithm ?? recipientCertificate.signatureAlgorithm
        try requireSupportedSignatureAlgorithm(effectiveAlgorithm)
        guard !challengeSignature.isEmpty else {
            throw OfflineBearerPolicyError("challengeSignature must not be empty")
        }
        self.version = version
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.recipientCertificate = recipientCertificate
        self.assetDefinitionId = assetDefinitionId
        self.amount = canonicalAmount
        self.createdAtMs = createdAtMs
        self.expiresAtMs = expiresAtMs
        self.policyHashHex = policyHashHex
        self.signatureAlgorithm = effectiveAlgorithm
        self.challengeSignature = challengeSignature
    }
}

/// Constant-size sender debit receipt transferred to the recipient.
public struct OfflineBearerDebitReceiptV2: Equatable, Sendable {
    public static let version = 2

    public let version: Int
    public let transferId: String
    public let chainId: String
    public let paymentRequestId: String
    public let senderCertificate: OfflineBearerCertificateV2
    public let recipientCertificate: OfflineBearerCertificateV2
    public let assetDefinitionId: String
    public let amount: String
    public let senderPreBalance: String
    public let senderPostBalance: String
    public let senderSequence: UInt64
    public let createdAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policyHashHex: String
    public let receiveChallengeSignature: Data
    public let signatureAlgorithm: String
    public let debitSignature: Data

    public init(version: Int = Self.version,
                transferId: String,
                chainId: String,
                paymentRequestId: String,
                senderCertificate: OfflineBearerCertificateV2,
                recipientCertificate: OfflineBearerCertificateV2,
                assetDefinitionId: String,
                amount: String,
                senderPreBalance: String,
                senderPostBalance: String,
                senderSequence: UInt64,
                createdAtMs: UInt64,
                expiresAtMs: UInt64,
                policyHashHex: String,
                receiveChallengeSignature: Data,
                signatureAlgorithm: String? = nil,
                debitSignature: Data) throws {
        guard version == Self.version else { throw OfflineBearerPolicyError("unsupported debit receipt version") }
        try requireNonBlank(transferId, "transferId")
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(paymentRequestId, "paymentRequestId")
        try requireNonBlank(assetDefinitionId, "assetDefinitionId")
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        try requirePositiveAmount(canonicalAmount, "amount")
        let canonicalPre = try ToriiOfflineCashCodec.canonicalAmountString(senderPreBalance)
        let canonicalPost = try ToriiOfflineCashCodec.canonicalAmountString(senderPostBalance)
        try requireNonNegativeAmount(canonicalPre, "senderPreBalance")
        try requireNonNegativeAmount(canonicalPost, "senderPostBalance")
        guard senderSequence > 0, expiresAtMs > createdAtMs else {
            throw OfflineBearerPolicyError("debit receipt sequence/time is invalid")
        }
        try requireHexLike(policyHashHex, "policyHashHex")
        let effectiveAlgorithm = signatureAlgorithm ?? senderCertificate.signatureAlgorithm
        try requireSupportedSignatureAlgorithm(effectiveAlgorithm)
        guard !receiveChallengeSignature.isEmpty, !debitSignature.isEmpty else {
            throw OfflineBearerPolicyError("debit receipt signatures must not be empty")
        }
        self.version = version
        self.transferId = transferId
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.senderCertificate = senderCertificate
        self.recipientCertificate = recipientCertificate
        self.assetDefinitionId = assetDefinitionId
        self.amount = canonicalAmount
        self.senderPreBalance = canonicalPre
        self.senderPostBalance = canonicalPost
        self.senderSequence = senderSequence
        self.createdAtMs = createdAtMs
        self.expiresAtMs = expiresAtMs
        self.policyHashHex = policyHashHex
        self.receiveChallengeSignature = receiveChallengeSignature
        self.signatureAlgorithm = effectiveAlgorithm
        self.debitSignature = debitSignature
    }
}

/// Recipient-side credit receipt retained for later settlement.
public struct OfflineBearerCreditReceiptV2: Equatable, Sendable {
    public static let version = 2

    public let version: Int
    public let transferId: String
    public let chainId: String
    public let recipientCertificate: OfflineBearerCertificateV2
    public let amount: String
    public let recipientPreBalance: String
    public let recipientPostBalance: String
    public let recipientSequence: UInt64
    public let acceptedAtMs: UInt64
    public let signatureAlgorithm: String
    public let creditSignature: Data

    public init(version: Int = Self.version,
                transferId: String,
                chainId: String,
                recipientCertificate: OfflineBearerCertificateV2,
                amount: String,
                recipientPreBalance: String,
                recipientPostBalance: String,
                recipientSequence: UInt64,
                acceptedAtMs: UInt64,
                signatureAlgorithm: String? = nil,
                creditSignature: Data) throws {
        guard version == Self.version else { throw OfflineBearerPolicyError("unsupported credit receipt version") }
        try requireNonBlank(transferId, "transferId")
        try requireNonBlank(chainId, "chainId")
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        let canonicalPre = try ToriiOfflineCashCodec.canonicalAmountString(recipientPreBalance)
        let canonicalPost = try ToriiOfflineCashCodec.canonicalAmountString(recipientPostBalance)
        try requirePositiveAmount(canonicalAmount, "amount")
        try requireNonNegativeAmount(canonicalPre, "recipientPreBalance")
        try requireNonNegativeAmount(canonicalPost, "recipientPostBalance")
        guard recipientSequence > 0, !creditSignature.isEmpty else {
            throw OfflineBearerPolicyError("credit receipt sequence/signature is invalid")
        }
        let effectiveAlgorithm = signatureAlgorithm ?? recipientCertificate.signatureAlgorithm
        try requireSupportedSignatureAlgorithm(effectiveAlgorithm)
        self.version = version
        self.transferId = transferId
        self.chainId = chainId
        self.recipientCertificate = recipientCertificate
        self.amount = canonicalAmount
        self.recipientPreBalance = canonicalPre
        self.recipientPostBalance = canonicalPost
        self.recipientSequence = recipientSequence
        self.acceptedAtMs = acceptedAtMs
        self.signatureAlgorithm = effectiveAlgorithm
        self.creditSignature = creditSignature
    }
}

/// Compact online settlement batch exported from a local v2 purse journal.
public struct OfflineBearerSettlementBatchV2: Equatable, Sendable {
    public static let version = 2

    public let version: Int
    public let chainId: String
    public let purseId: String
    public let debitReceipts: [OfflineBearerDebitReceiptV2]
    public let creditReceipts: [OfflineBearerCreditReceiptV2]

    public init(version: Int = Self.version,
                chainId: String,
                purseId: String,
                debitReceipts: [OfflineBearerDebitReceiptV2],
                creditReceipts: [OfflineBearerCreditReceiptV2]) throws {
        guard version == Self.version else { throw OfflineBearerPolicyError("unsupported settlement batch version") }
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(purseId, "purseId")
        self.version = version
        self.chainId = chainId
        self.purseId = purseId
        self.debitReceipts = debitReceipts
        self.creditReceipts = creditReceipts
    }
}

public protocol OfflineBearerPolicyProvider {
    func currentPolicy() throws -> OfflineBearerPolicyBundleV2
}

public struct StaticOfflineBearerPolicyProvider: OfflineBearerPolicyProvider {
    private let policy: OfflineBearerPolicyBundleV2

    public init(policy: OfflineBearerPolicyBundleV2) {
        self.policy = policy
    }

    public func currentPolicy() throws -> OfflineBearerPolicyBundleV2 {
        policy
    }
}

public protocol OfflineBearerSecureElement: AnyObject {
    func capabilities() throws -> OfflineBearerSecureElementCapabilities
    func currentCertificate() throws -> OfflineBearerCertificateV2?
    func currentState() throws -> OfflineBearerPurseStateV2?
    func installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) throws
    func createReceiveRequest(paymentRequestId: String,
                              amount: String,
                              createdAtMs: UInt64,
                              expiresAtMs: UInt64,
                              policyHashHex: String) throws -> OfflineBearerReceiveRequestV2
    func debit(request: OfflineBearerReceiveRequestV2,
               transferId: String,
               createdAtMs: UInt64,
               expiresAtMs: UInt64) throws -> OfflineBearerDebitReceiptV2
    func credit(receipt: OfflineBearerDebitReceiptV2,
                acceptedAtMs: UInt64) throws -> OfflineBearerCreditReceiptV2
    func exportSettlementBatch(maxReceipts: Int) throws -> OfflineBearerSettlementBatchV2
    func pruneSettled(transferIds: Set<String>) throws
}

public final class UnsupportedOfflineBearerSecureElement: OfflineBearerSecureElement {
    private let hardwareClass: String

    public init(hardwareClass: String = "unsupported") {
        self.hardwareClass = hardwareClass
    }

    public func capabilities() throws -> OfflineBearerSecureElementCapabilities {
        try OfflineBearerSecureElementCapabilities(
            hardwareBacked: false,
            statefulPurse: false,
            hardwareClass: hardwareClass
        )
    }

    public func currentCertificate() throws -> OfflineBearerCertificateV2? { nil }
    public func currentState() throws -> OfflineBearerPurseStateV2? { nil }
    public func installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) throws {
        throw unsupported()
    }
    public func createReceiveRequest(paymentRequestId: String,
                                     amount: String,
                                     createdAtMs: UInt64,
                                     expiresAtMs: UInt64,
                                     policyHashHex: String) throws -> OfflineBearerReceiveRequestV2 {
        throw unsupported()
    }
    public func debit(request: OfflineBearerReceiveRequestV2,
                      transferId: String,
                      createdAtMs: UInt64,
                      expiresAtMs: UInt64) throws -> OfflineBearerDebitReceiptV2 {
        throw unsupported()
    }
    public func credit(receipt: OfflineBearerDebitReceiptV2,
                       acceptedAtMs: UInt64) throws -> OfflineBearerCreditReceiptV2 {
        throw unsupported()
    }
    public func exportSettlementBatch(maxReceipts: Int) throws -> OfflineBearerSettlementBatchV2 {
        throw unsupported()
    }
    public func pruneSettled(transferIds: Set<String>) throws {
        throw unsupported()
    }

    private func unsupported() -> OfflineBearerPolicyError {
        OfflineBearerPolicyError("Offline Bearer value is disabled on unsupported hardware")
    }
}

public struct OfflineBearerPolicyError: Error, LocalizedError, Equatable {
    public let reason: String

    public init(_ reason: String) {
        self.reason = reason
    }

    public var errorDescription: String? { reason }
}

public enum OfflineBearerV2TextCodecError: Error, LocalizedError, Equatable {
    case invalidField(String)
    case invalidPrefix

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Offline Bearer v2 field \(field) is invalid."
        case .invalidPrefix:
            return "Offline Bearer v2 text prefix missing."
        }
    }
}

public enum OfflineBearerV2TextCodec {
    public enum PayloadKind: Equatable, Sendable {
        case receiveRequest
        case payment
        case ack
    }

    public static let receiveRequestTextPrefix = "wallet-offline-bearer-receive:"
    public static let paymentTextPrefix = "wallet-offline-bearer-payment:"
    public static let ackTextPrefix = "wallet-offline-bearer-ack:"

    private static let policyType = "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2"
    private static let certificateType = "iroha_data_model::offline::model::OfflineBearerCertificateV2"
    private static let receiveRequestType = "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2"
    private static let debitReceiptType = "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2"
    private static let creditReceiptType = "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2"
    private static let settlementBatchType = "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2"

    public static func encodePolicyBundleNorito(_ policy: OfflineBearerPolicyBundleV2) throws -> Data {
        noritoEncode(typeName: policyType, payload: try encodePolicyPayload(policy), flags: NoritoHeader.compactLen)
    }

    public static func decodePolicyBundleNorito(_ payload: Data) throws -> OfflineBearerPolicyBundleV2 {
        var reader = try payloadReader(payload, typeName: policyType)
        let policy = try decodePolicy(&reader)
        try requireNoTrailingBytes(reader, "policy")
        return policy
    }

    public static func encodeCertificateNorito(_ certificate: OfflineBearerCertificateV2) throws -> Data {
        noritoEncode(typeName: certificateType, payload: try encodeCertificatePayload(certificate), flags: NoritoHeader.compactLen)
    }

    public static func decodeCertificateNorito(_ payload: Data) throws -> OfflineBearerCertificateV2 {
        var reader = try payloadReader(payload, typeName: certificateType)
        let certificate = try decodeCertificate(&reader)
        try requireNoTrailingBytes(reader, "certificate")
        return certificate
    }

    public static func encodeReceiveRequestNorito(_ request: OfflineBearerReceiveRequestV2) throws -> Data {
        noritoEncode(typeName: receiveRequestType, payload: try encodeReceiveRequestPayload(request), flags: NoritoHeader.compactLen)
    }

    public static func decodeReceiveRequestNorito(_ payload: Data) throws -> OfflineBearerReceiveRequestV2 {
        var reader = try payloadReader(payload, typeName: receiveRequestType)
        let request = try decodeReceiveRequest(&reader)
        try requireNoTrailingBytes(reader, "receive_request")
        return request
    }

    public static func encodeDebitReceiptNorito(_ receipt: OfflineBearerDebitReceiptV2) throws -> Data {
        noritoEncode(typeName: debitReceiptType, payload: try encodeDebitReceiptPayload(receipt), flags: NoritoHeader.compactLen)
    }

    public static func decodeDebitReceiptNorito(_ payload: Data) throws -> OfflineBearerDebitReceiptV2 {
        var reader = try payloadReader(payload, typeName: debitReceiptType)
        let receipt = try decodeDebitReceipt(&reader)
        try requireNoTrailingBytes(reader, "debit_receipt")
        return receipt
    }

    public static func encodeCreditReceiptNorito(_ receipt: OfflineBearerCreditReceiptV2) throws -> Data {
        noritoEncode(typeName: creditReceiptType, payload: try encodeCreditReceiptPayload(receipt), flags: NoritoHeader.compactLen)
    }

    public static func decodeCreditReceiptNorito(_ payload: Data) throws -> OfflineBearerCreditReceiptV2 {
        var reader = try payloadReader(payload, typeName: creditReceiptType)
        let receipt = try decodeCreditReceipt(&reader)
        try requireNoTrailingBytes(reader, "credit_receipt")
        return receipt
    }

    public static func encodeSettlementBatchNorito(_ batch: OfflineBearerSettlementBatchV2) throws -> Data {
        noritoEncode(typeName: settlementBatchType, payload: try encodeSettlementBatchPayload(batch), flags: NoritoHeader.compactLen)
    }

    public static func decodeSettlementBatchNorito(_ payload: Data) throws -> OfflineBearerSettlementBatchV2 {
        var reader = try payloadReader(payload, typeName: settlementBatchType)
        let batch = try decodeSettlementBatch(&reader)
        try requireNoTrailingBytes(reader, "settlement_batch")
        return batch
    }

    public static func encodeReceiveRequestText(_ request: OfflineBearerReceiveRequestV2) throws -> String {
        receiveRequestTextPrefix + base64UrlEncode(try encodeReceiveRequestNorito(request))
    }

    public static func decodeReceiveRequestText(_ text: String) throws -> OfflineBearerReceiveRequestV2 {
        try decodeReceiveRequestNorito(decodeTextPayload(text, prefix: receiveRequestTextPrefix))
    }

    public static func encodePaymentText(_ receipt: OfflineBearerDebitReceiptV2) throws -> String {
        paymentTextPrefix + base64UrlEncode(try encodeDebitReceiptNorito(receipt))
    }

    public static func decodePaymentText(_ text: String) throws -> OfflineBearerDebitReceiptV2 {
        try decodeDebitReceiptNorito(decodeTextPayload(text, prefix: paymentTextPrefix))
    }

    public static func encodeAckText(_ receipt: OfflineBearerCreditReceiptV2) throws -> String {
        ackTextPrefix + base64UrlEncode(try encodeCreditReceiptNorito(receipt))
    }

    public static func decodeAckText(_ text: String) throws -> OfflineBearerCreditReceiptV2 {
        try decodeCreditReceiptNorito(decodeTextPayload(text, prefix: ackTextPrefix))
    }

    public static func payloadKind(_ text: String) -> PayloadKind? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(receiveRequestTextPrefix) {
            return .receiveRequest
        }
        if trimmed.hasPrefix(paymentTextPrefix) {
            return .payment
        }
        if trimmed.hasPrefix(ackTextPrefix) {
            return .ack
        }
        return nil
    }

    private static func encodePolicyPayload(_ policy: OfflineBearerPolicyBundleV2) throws -> Data {
        structPayload([
            stringPayload(policy.policyId),
            stringPayload(policy.policyHashHex),
            stringPayload(policy.issuerId),
            u64Payload(policy.issuedAtMs),
            u64Payload(policy.expiresAtMs),
            u64Payload(policy.maxCertificateAgeMs),
            u64Payload(policy.maxPolicyAgeMs),
            u64Payload(policy.maxTokenAgeMs),
            stringPayload(policy.maxOfflineBalance),
            stringPayload(policy.maxTransactionAmount),
            try stringListPayload(policy.allowedHardwareClasses.sorted()),
            try stringListPayload(policy.blacklistedAccountIds.sorted()),
            try stringListPayload(policy.blacklistedDeviceIds.sorted()),
            try stringListPayload(policy.blacklistedKeyIds.sorted()),
            stringPayload(policy.signatureAlgorithm),
            bytesVecPayload(policy.issuerSignature),
            u64Payload(policy.policyEpoch),
            stringPayload(policy.policySource),
            try stringListPayload(policy.revokedCertificateIds.sorted()),
            try stringListPayload(policy.revokedTransferIds.sorted()),
            writeList(policy.assetSendLimits) { limit in
                structPayload([
                    stringPayload(limit.assetDefinitionId),
                    stringPayload(limit.maxTransactionAmount),
                    stringPayload(limit.dailySendLimit),
                    stringPayload(limit.monthlySendLimit),
                ])
            },
        ])
    }

    private static func decodePolicy(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerPolicyBundleV2 {
        try OfflineBearerPolicyBundleV2(
            policyId: readField(&reader, "policy_id", readString),
            policyHashHex: readField(&reader, "policy_hash_hex", readString),
            issuerId: readField(&reader, "issuer_id", readString),
            issuedAtMs: readField(&reader, "issued_at_ms") { try $0.readUInt64LE() },
            expiresAtMs: readField(&reader, "expires_at_ms") { try $0.readUInt64LE() },
            maxCertificateAgeMs: readField(&reader, "max_certificate_age_ms") { try $0.readUInt64LE() },
            maxPolicyAgeMs: readField(&reader, "max_policy_age_ms") { try $0.readUInt64LE() },
            maxTokenAgeMs: readField(&reader, "max_token_age_ms") { try $0.readUInt64LE() },
            maxOfflineBalance: readField(&reader, "max_offline_balance", readString),
            maxTransactionAmount: readField(&reader, "max_transaction_amount", readString),
            allowedHardwareClasses: Set(readField(&reader, "allowed_hardware_classes", readStringList)),
            blacklistedAccountIds: Set(readField(&reader, "blacklisted_account_ids", readStringList)),
            blacklistedDeviceIds: Set(readField(&reader, "blacklisted_device_ids", readStringList)),
            blacklistedKeyIds: Set(readField(&reader, "blacklisted_key_ids", readStringList)),
            signatureAlgorithm: readField(&reader, "signature_algorithm", readString),
            issuerSignature: readField(&reader, "issuer_signature", readBytesVec),
            policyEpoch: readField(&reader, "policy_epoch") { try $0.readUInt64LE() },
            policySource: readField(&reader, "policy_source", readString),
            revokedCertificateIds: Set(readField(&reader, "revoked_certificate_ids", readStringList)),
            revokedTransferIds: Set(readField(&reader, "revoked_transfer_ids", readStringList)),
            assetSendLimits: readField(&reader, "asset_send_limits") { child in
                try readList(&child, decodeAssetSendLimit)
            }
        )
    }

    private static func decodeAssetSendLimit(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerAssetSendLimitV2 {
        try OfflineBearerAssetSendLimitV2(
            assetDefinitionId: readField(&reader, "asset_definition_id", readString),
            maxTransactionAmount: readField(&reader, "max_transaction_amount", readString),
            dailySendLimit: readField(&reader, "daily_send_limit", readString),
            monthlySendLimit: readField(&reader, "monthly_send_limit", readString)
        )
    }

    private static func encodeCertificatePayload(_ certificate: OfflineBearerCertificateV2) throws -> Data {
        structPayload([
            stringPayload(certificate.certificateId),
            stringPayload(certificate.chainId),
            stringPayload(certificate.issuerId),
            stringPayload(certificate.purseId),
            stringPayload(certificate.accountId),
            stringPayload(certificate.assetDefinitionId),
            stringPayload(certificate.deviceId),
            stringPayload(certificate.keyId),
            stringPayload(certificate.hardwareClass),
            stringPayload(certificate.signatureAlgorithm),
            stringPayload(certificate.publicKeyEncoding),
            bytesVecPayload(certificate.publicKey),
            u64Payload(certificate.issuedAtMs),
            u64Payload(certificate.expiresAtMs),
            stringPayload(certificate.policyId),
            stringPayload(certificate.policyHashHex),
            bytesVecPayload(certificate.issuerSignature),
        ])
    }

    private static func decodeCertificate(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerCertificateV2 {
        try OfflineBearerCertificateV2(
            certificateId: readField(&reader, "certificate_id", readString),
            chainId: readField(&reader, "chain_id", readString),
            issuerId: readField(&reader, "issuer_id", readString),
            purseId: readField(&reader, "purse_id", readString),
            accountId: readField(&reader, "account_id", readString),
            assetDefinitionId: readField(&reader, "asset_definition_id", readString),
            deviceId: readField(&reader, "device_id", readString),
            keyId: readField(&reader, "key_id", readString),
            hardwareClass: readField(&reader, "hardware_class", readString),
            signatureAlgorithm: readField(&reader, "signature_algorithm", readString),
            publicKeyEncoding: readField(&reader, "public_key_encoding", readString),
            publicKey: readField(&reader, "public_key", readBytesVec),
            issuedAtMs: readField(&reader, "issued_at_ms") { try $0.readUInt64LE() },
            expiresAtMs: readField(&reader, "expires_at_ms") { try $0.readUInt64LE() },
            policyId: readField(&reader, "policy_id", readString),
            policyHashHex: readField(&reader, "policy_hash_hex", readString),
            issuerSignature: readField(&reader, "issuer_signature", readBytesVec)
        )
    }

    private static func encodeReceiveRequestPayload(_ request: OfflineBearerReceiveRequestV2) throws -> Data {
        structPayload([
            try u16Payload(request.version),
            stringPayload(request.chainId),
            stringPayload(request.paymentRequestId),
            try encodeCertificatePayload(request.recipientCertificate),
            stringPayload(request.assetDefinitionId),
            stringPayload(request.amount),
            u64Payload(request.createdAtMs),
            u64Payload(request.expiresAtMs),
            stringPayload(request.policyHashHex),
            stringPayload(request.signatureAlgorithm),
            bytesVecPayload(request.challengeSignature),
        ])
    }

    private static func decodeReceiveRequest(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerReceiveRequestV2 {
        try OfflineBearerReceiveRequestV2(
            version: Int(readField(&reader, "version") { try $0.readUInt16LE() }),
            chainId: readField(&reader, "chain_id", readString),
            paymentRequestId: readField(&reader, "payment_request_id", readString),
            recipientCertificate: readField(&reader, "recipient_certificate", decodeCertificate),
            assetDefinitionId: readField(&reader, "asset_definition_id", readString),
            amount: readField(&reader, "amount", readString),
            createdAtMs: readField(&reader, "created_at_ms") { try $0.readUInt64LE() },
            expiresAtMs: readField(&reader, "expires_at_ms") { try $0.readUInt64LE() },
            policyHashHex: readField(&reader, "policy_hash_hex", readString),
            signatureAlgorithm: readField(&reader, "signature_algorithm", readString),
            challengeSignature: readField(&reader, "challenge_signature", readBytesVec)
        )
    }

    private static func encodeDebitReceiptPayload(_ receipt: OfflineBearerDebitReceiptV2) throws -> Data {
        structPayload([
            try u16Payload(receipt.version),
            stringPayload(receipt.transferId),
            stringPayload(receipt.chainId),
            stringPayload(receipt.paymentRequestId),
            try encodeCertificatePayload(receipt.senderCertificate),
            try encodeCertificatePayload(receipt.recipientCertificate),
            stringPayload(receipt.assetDefinitionId),
            stringPayload(receipt.amount),
            stringPayload(receipt.senderPreBalance),
            stringPayload(receipt.senderPostBalance),
            u64Payload(receipt.senderSequence),
            u64Payload(receipt.createdAtMs),
            u64Payload(receipt.expiresAtMs),
            stringPayload(receipt.policyHashHex),
            bytesVecPayload(receipt.receiveChallengeSignature),
            stringPayload(receipt.signatureAlgorithm),
            bytesVecPayload(receipt.debitSignature),
        ])
    }

    private static func decodeDebitReceipt(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerDebitReceiptV2 {
        try OfflineBearerDebitReceiptV2(
            version: Int(readField(&reader, "version") { try $0.readUInt16LE() }),
            transferId: readField(&reader, "transfer_id", readString),
            chainId: readField(&reader, "chain_id", readString),
            paymentRequestId: readField(&reader, "payment_request_id", readString),
            senderCertificate: readField(&reader, "sender_certificate", decodeCertificate),
            recipientCertificate: readField(&reader, "recipient_certificate", decodeCertificate),
            assetDefinitionId: readField(&reader, "asset_definition_id", readString),
            amount: readField(&reader, "amount", readString),
            senderPreBalance: readField(&reader, "sender_pre_balance", readString),
            senderPostBalance: readField(&reader, "sender_post_balance", readString),
            senderSequence: readField(&reader, "sender_sequence") { try $0.readUInt64LE() },
            createdAtMs: readField(&reader, "created_at_ms") { try $0.readUInt64LE() },
            expiresAtMs: readField(&reader, "expires_at_ms") { try $0.readUInt64LE() },
            policyHashHex: readField(&reader, "policy_hash_hex", readString),
            receiveChallengeSignature: readField(&reader, "receive_challenge_signature", readBytesVec),
            signatureAlgorithm: readField(&reader, "signature_algorithm", readString),
            debitSignature: readField(&reader, "debit_signature", readBytesVec)
        )
    }

    private static func encodeCreditReceiptPayload(_ receipt: OfflineBearerCreditReceiptV2) throws -> Data {
        structPayload([
            try u16Payload(receipt.version),
            stringPayload(receipt.transferId),
            stringPayload(receipt.chainId),
            try encodeCertificatePayload(receipt.recipientCertificate),
            stringPayload(receipt.amount),
            stringPayload(receipt.recipientPreBalance),
            stringPayload(receipt.recipientPostBalance),
            u64Payload(receipt.recipientSequence),
            u64Payload(receipt.acceptedAtMs),
            stringPayload(receipt.signatureAlgorithm),
            bytesVecPayload(receipt.creditSignature),
        ])
    }

    private static func decodeCreditReceipt(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerCreditReceiptV2 {
        try OfflineBearerCreditReceiptV2(
            version: Int(readField(&reader, "version") { try $0.readUInt16LE() }),
            transferId: readField(&reader, "transfer_id", readString),
            chainId: readField(&reader, "chain_id", readString),
            recipientCertificate: readField(&reader, "recipient_certificate", decodeCertificate),
            amount: readField(&reader, "amount", readString),
            recipientPreBalance: readField(&reader, "recipient_pre_balance", readString),
            recipientPostBalance: readField(&reader, "recipient_post_balance", readString),
            recipientSequence: readField(&reader, "recipient_sequence") { try $0.readUInt64LE() },
            acceptedAtMs: readField(&reader, "accepted_at_ms") { try $0.readUInt64LE() },
            signatureAlgorithm: readField(&reader, "signature_algorithm", readString),
            creditSignature: readField(&reader, "credit_signature", readBytesVec)
        )
    }

    private static func encodeSettlementBatchPayload(_ batch: OfflineBearerSettlementBatchV2) throws -> Data {
        structPayload([
            try u16Payload(batch.version),
            stringPayload(batch.chainId),
            stringPayload(batch.purseId),
            try writeList(batch.debitReceipts, encodeDebitReceiptPayload),
            try writeList(batch.creditReceipts, encodeCreditReceiptPayload),
        ])
    }

    private static func decodeSettlementBatch(_ reader: inout OfflineNoritoReader) throws -> OfflineBearerSettlementBatchV2 {
        try OfflineBearerSettlementBatchV2(
            version: Int(readField(&reader, "version") { try $0.readUInt16LE() }),
            chainId: readField(&reader, "chain_id", readString),
            purseId: readField(&reader, "purse_id", readString),
            debitReceipts: readField(&reader, "debit_receipts") { child in
                try readList(&child, decodeDebitReceipt)
            },
            creditReceipts: readField(&reader, "credit_receipts") { child in
                try readList(&child, decodeCreditReceipt)
            }
        )
    }

    private static func payloadReader(_ payload: Data, typeName: String) throws -> OfflineNoritoReader {
        guard let frame = noritoDecodeFrame(payload) else {
            throw OfflineBearerV2TextCodecError.invalidField("payload")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: typeName) else {
            throw OfflineBearerV2TextCodecError.invalidField("schema")
        }
        guard frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw OfflineBearerV2TextCodecError.invalidField("layout")
        }
        return OfflineNoritoReader(data: frame.payload)
    }

    private static func requireNoTrailingBytes(_ reader: OfflineNoritoReader, _ field: String) throws {
        guard reader.remaining() == 0 else {
            throw OfflineBearerV2TextCodecError.invalidField("\(field).trailing_bytes")
        }
    }

    private static func readField<T>(_ reader: inout OfflineNoritoReader,
                                     _ field: String,
                                     _ decode: (inout OfflineNoritoReader) throws -> T) throws -> T {
        var child = OfflineNoritoReader(data: try reader.readCompactField())
        let value = try decode(&child)
        guard child.remaining() == 0 else {
            throw OfflineBearerV2TextCodecError.invalidField(field)
        }
        return value
    }

    private static func structPayload(_ fields: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        for field in fields {
            writer.writeField(field)
        }
        return writer.data
    }

    private static func stringPayload(_ value: String) -> Data {
        OfflineCompactNorito.encodeString(value)
    }

    private static func bytesVecPayload(_ value: Data) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private static func stringListPayload(_ values: [String]) throws -> Data {
        writeList(values, stringPayload)
    }

    private static func writeList<T>(_ values: [T], _ encode: (T) throws -> Data) rethrows -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(values.count))
        for value in values {
            let payload = try encode(value)
            var element = OfflineCompactNoritoWriter()
            element.writeField(payload)
            writer.writeBytes(element.data)
        }
        return writer.data
    }

    private static func readList<T>(_ reader: inout OfflineNoritoReader,
                                    _ decode: (inout OfflineNoritoReader) throws -> T) throws -> [T] {
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineBearerV2TextCodecError.invalidField("list")
        }
        var values: [T] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<Int(count) {
            var child = OfflineNoritoReader(data: try reader.readCompactField())
            values.append(try decode(&child))
            guard child.remaining() == 0 else {
                throw OfflineBearerV2TextCodecError.invalidField("list.element")
            }
        }
        return values
    }

    private static func readStringList(_ reader: inout OfflineNoritoReader) throws -> [String] {
        try readList(&reader, readString)
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineBearerV2TextCodecError.invalidField("string")
        }
        guard let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8) else {
            throw OfflineBearerV2TextCodecError.invalidField("string")
        }
        return value
    }

    private static func readBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineBearerV2TextCodecError.invalidField("bytes")
        }
        return try reader.readBytes(Int(length))
    }

    private static func u16Payload(_ value: Int) throws -> Data {
        guard value >= 0, value <= Int(UInt16.max) else {
            throw OfflineBearerV2TextCodecError.invalidField("u16")
        }
        return OfflineCompactNorito.encodeUInt16(UInt16(value))
    }

    private static func u64Payload(_ value: UInt64) -> Data {
        OfflineCompactNorito.encodeUInt64(value)
    }

    private static func base64UrlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    private static func base64UrlDecode(_ value: String) -> Data? {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !value.contains("="),
              value.unicodeScalars.allSatisfy({ scalar in
                  let byte = scalar.value
                  return (65...90).contains(byte)
                      || (97...122).contains(byte)
                      || (48...57).contains(byte)
                      || byte == 45
                      || byte == 95
              }) else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        normalized.append(String(repeating: "=", count: (4 - normalized.count % 4) % 4))
        return Data(base64Encoded: normalized)
    }

    private static func decodeTextPayload(_ text: String, prefix: String) throws -> Data {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(prefix) else {
            throw OfflineBearerV2TextCodecError.invalidPrefix
        }
        guard let payload = base64UrlDecode(String(trimmed.dropFirst(prefix.count))) else {
            throw OfflineBearerV2TextCodecError.invalidField("payload")
        }
        return payload
    }
}

public enum OfflineBearerV2Payloads {
    private static let policyPayloadType = "iroha_data_model::offline::model::OfflineBearerPolicyBundlePayloadV2"
    private static let policyType = "iroha_data_model::offline::model::OfflineBearerPolicyBundleV2"
    private static let certificatePayloadType = "iroha_data_model::offline::model::OfflineBearerCertificatePayloadV2"
    private static let certificateType = "iroha_data_model::offline::model::OfflineBearerCertificateV2"
    private static let receiveRequestPayloadType = "iroha_data_model::offline::model::OfflineBearerReceiveRequestPayloadV2"
    private static let receiveRequestType = "iroha_data_model::offline::model::OfflineBearerReceiveRequestV2"
    private static let debitReceiptPayloadType = "iroha_data_model::offline::model::OfflineBearerDebitReceiptPayloadV2"
    private static let debitReceiptType = "iroha_data_model::offline::model::OfflineBearerDebitReceiptV2"
    private static let creditReceiptPayloadType = "iroha_data_model::offline::model::OfflineBearerCreditReceiptPayloadV2"
    private static let creditReceiptType = "iroha_data_model::offline::model::OfflineBearerCreditReceiptV2"
    private static let settlementBatchPayloadType = "iroha_data_model::offline::model::OfflineBearerSettlementBatchPayloadV2"
    private static let settlementBatchType = "iroha_data_model::offline::model::OfflineBearerSettlementBatchV2"

    private static let policyDomain = "iroha:offline-bearer-v2:policy-bundle"
    private static let certificateDomain = "iroha:offline-bearer-v2:certificate"
    private static let receiveRequestDomain = "iroha:offline-bearer-v2:receive-request"
    private static let debitReceiptDomain = "iroha:offline-bearer-v2:debit-receipt"
    private static let creditReceiptDomain = "iroha:offline-bearer-v2:credit-receipt"
    private static let settlementBatchDomain = "iroha:offline-bearer-v2:settlement-batch"

    public static func policyUnsignedPayload(_ policy: OfflineBearerPolicyBundleV2) throws -> Data {
        frame(typeName: policyPayloadType, payload: try policyPayload(policy, includeSignature: false, includeDomain: true))
    }

    public static func certificateUnsignedPayload(_ certificate: OfflineBearerCertificateV2) throws -> Data {
        frame(typeName: certificatePayloadType, payload: try certificatePayload(certificate, includeSignature: false, includeDomain: true))
    }

    public static func receiveRequestUnsignedPayload(_ request: OfflineBearerReceiveRequestV2) throws -> Data {
        frame(
            typeName: receiveRequestPayloadType,
            payload: structPayload([
                stringPayload(receiveRequestDomain),
                try u16Payload(request.version),
                stringPayload(request.chainId),
                stringPayload(request.paymentRequestId),
                try hashPayload(signedCertificateHash(request.recipientCertificate)),
                stringPayload(request.assetDefinitionId),
                stringPayload(request.amount),
                u64Payload(request.createdAtMs),
                u64Payload(request.expiresAtMs),
                stringPayload(request.policyHashHex),
                stringPayload(request.signatureAlgorithm),
            ])
        )
    }

    public static func debitReceiptUnsignedPayload(_ receipt: OfflineBearerDebitReceiptV2) throws -> Data {
        frame(
            typeName: debitReceiptPayloadType,
            payload: structPayload([
                stringPayload(debitReceiptDomain),
                try u16Payload(receipt.version),
                stringPayload(receipt.transferId),
                stringPayload(receipt.chainId),
                stringPayload(receipt.paymentRequestId),
                try hashPayload(signedCertificateHash(receipt.senderCertificate)),
                try hashPayload(signedCertificateHash(receipt.recipientCertificate)),
                stringPayload(receipt.assetDefinitionId),
                stringPayload(receipt.amount),
                stringPayload(receipt.senderPreBalance),
                stringPayload(receipt.senderPostBalance),
                u64Payload(receipt.senderSequence),
                u64Payload(receipt.createdAtMs),
                u64Payload(receipt.expiresAtMs),
                stringPayload(receipt.policyHashHex),
                bytesVecPayload(receipt.receiveChallengeSignature),
                stringPayload(receipt.signatureAlgorithm),
            ])
        )
    }

    public static func creditReceiptUnsignedPayload(_ receipt: OfflineBearerCreditReceiptV2) throws -> Data {
        frame(
            typeName: creditReceiptPayloadType,
            payload: structPayload([
                stringPayload(creditReceiptDomain),
                try u16Payload(receipt.version),
                stringPayload(receipt.transferId),
                stringPayload(receipt.chainId),
                try hashPayload(signedCertificateHash(receipt.recipientCertificate)),
                stringPayload(receipt.amount),
                stringPayload(receipt.recipientPreBalance),
                stringPayload(receipt.recipientPostBalance),
                u64Payload(receipt.recipientSequence),
                u64Payload(receipt.acceptedAtMs),
                stringPayload(receipt.signatureAlgorithm),
            ])
        )
    }

    public static func settlementBatchUnsignedPayload(_ batch: OfflineBearerSettlementBatchV2) throws -> Data {
        frame(
            typeName: settlementBatchPayloadType,
            payload: structPayload([
                stringPayload(settlementBatchDomain),
                try u16Payload(batch.version),
                stringPayload(batch.chainId),
                stringPayload(batch.purseId),
                try hashVecPayload(batch.debitReceipts.map { try signedDebitReceiptHash($0) }),
                try hashVecPayload(batch.creditReceipts.map { try signedCreditReceiptHash($0) }),
            ])
        )
    }

    private static func policyPayload(_ policy: OfflineBearerPolicyBundleV2,
                                      includeSignature: Bool,
                                      includeDomain: Bool) throws -> Data {
        var fields: [Data] = []
        if includeDomain {
            fields.append(stringPayload(policyDomain))
        }
        fields.append(contentsOf: [
            stringPayload(policy.policyId),
            stringPayload(policy.policyHashHex),
            stringPayload(policy.issuerId),
            u64Payload(policy.issuedAtMs),
            u64Payload(policy.expiresAtMs),
            u64Payload(policy.maxCertificateAgeMs),
            u64Payload(policy.maxPolicyAgeMs),
            u64Payload(policy.maxTokenAgeMs),
            stringPayload(policy.maxOfflineBalance),
            stringPayload(policy.maxTransactionAmount),
            stringVecPayload(policy.allowedHardwareClasses),
            stringVecPayload(policy.blacklistedAccountIds),
            stringVecPayload(policy.blacklistedDeviceIds),
            stringVecPayload(policy.blacklistedKeyIds),
            stringPayload(policy.signatureAlgorithm),
        ])
        if includeSignature {
            fields.append(bytesVecPayload(policy.issuerSignature))
        }
        fields.append(contentsOf: [
            u64Payload(policy.policyEpoch),
            stringPayload(policy.policySource),
            stringVecPayload(policy.revokedCertificateIds),
            stringVecPayload(policy.revokedTransferIds),
            try assetSendLimitVecPayload(policy.assetSendLimits),
        ])
        return structPayload(fields)
    }

    private static func certificatePayload(_ certificate: OfflineBearerCertificateV2,
                                           includeSignature: Bool,
                                           includeDomain: Bool) throws -> Data {
        var fields: [Data] = []
        if includeDomain {
            fields.append(stringPayload(certificateDomain))
        }
        fields.append(contentsOf: [
            stringPayload(certificate.certificateId),
            stringPayload(certificate.chainId),
            stringPayload(certificate.issuerId),
            stringPayload(certificate.purseId),
            stringPayload(certificate.accountId),
            stringPayload(certificate.assetDefinitionId),
            stringPayload(certificate.deviceId),
            stringPayload(certificate.keyId),
            stringPayload(certificate.hardwareClass),
            stringPayload(certificate.signatureAlgorithm),
            stringPayload(certificate.publicKeyEncoding),
            bytesVecPayload(certificate.publicKey),
            u64Payload(certificate.issuedAtMs),
            u64Payload(certificate.expiresAtMs),
            stringPayload(certificate.policyId),
            stringPayload(certificate.policyHashHex),
        ])
        if includeSignature {
            fields.append(bytesVecPayload(certificate.issuerSignature))
        }
        return structPayload(fields)
    }

    private static func receiveRequestPayload(_ request: OfflineBearerReceiveRequestV2,
                                              includeSignature: Bool) throws -> Data {
        var fields: [Data] = [
            try u16Payload(request.version),
            stringPayload(request.chainId),
            stringPayload(request.paymentRequestId),
            try certificatePayload(request.recipientCertificate, includeSignature: true, includeDomain: false),
            stringPayload(request.assetDefinitionId),
            stringPayload(request.amount),
            u64Payload(request.createdAtMs),
            u64Payload(request.expiresAtMs),
            stringPayload(request.policyHashHex),
            stringPayload(request.signatureAlgorithm),
        ]
        if includeSignature {
            fields.append(bytesVecPayload(request.challengeSignature))
        }
        return structPayload(fields)
    }

    private static func debitReceiptPayload(_ receipt: OfflineBearerDebitReceiptV2,
                                            includeSignature: Bool) throws -> Data {
        var fields: [Data] = [
            try u16Payload(receipt.version),
            stringPayload(receipt.transferId),
            stringPayload(receipt.chainId),
            stringPayload(receipt.paymentRequestId),
            try certificatePayload(receipt.senderCertificate, includeSignature: true, includeDomain: false),
            try certificatePayload(receipt.recipientCertificate, includeSignature: true, includeDomain: false),
            stringPayload(receipt.assetDefinitionId),
            stringPayload(receipt.amount),
            stringPayload(receipt.senderPreBalance),
            stringPayload(receipt.senderPostBalance),
            u64Payload(receipt.senderSequence),
            u64Payload(receipt.createdAtMs),
            u64Payload(receipt.expiresAtMs),
            stringPayload(receipt.policyHashHex),
            bytesVecPayload(receipt.receiveChallengeSignature),
            stringPayload(receipt.signatureAlgorithm),
        ]
        if includeSignature {
            fields.append(bytesVecPayload(receipt.debitSignature))
        }
        return structPayload(fields)
    }

    private static func creditReceiptPayload(_ receipt: OfflineBearerCreditReceiptV2,
                                             includeSignature: Bool) throws -> Data {
        var fields: [Data] = [
            try u16Payload(receipt.version),
            stringPayload(receipt.transferId),
            stringPayload(receipt.chainId),
            try certificatePayload(receipt.recipientCertificate, includeSignature: true, includeDomain: false),
            stringPayload(receipt.amount),
            stringPayload(receipt.recipientPreBalance),
            stringPayload(receipt.recipientPostBalance),
            u64Payload(receipt.recipientSequence),
            u64Payload(receipt.acceptedAtMs),
            stringPayload(receipt.signatureAlgorithm),
        ]
        if includeSignature {
            fields.append(bytesVecPayload(receipt.creditSignature))
        }
        return structPayload(fields)
    }

    private static func settlementBatchPayload(_ batch: OfflineBearerSettlementBatchV2) throws -> Data {
        structPayload([
            try u16Payload(batch.version),
            stringPayload(batch.chainId),
            stringPayload(batch.purseId),
            try vecPayload(batch.debitReceipts.map { try debitReceiptPayload($0, includeSignature: true) }),
            try vecPayload(batch.creditReceipts.map { try creditReceiptPayload($0, includeSignature: true) }),
        ])
    }

    private static func signedCertificateHash(_ certificate: OfflineBearerCertificateV2) throws -> Data {
        IrohaHash.hash(frame(
            typeName: certificateType,
            payload: try certificatePayload(certificate, includeSignature: true, includeDomain: false)
        ))
    }

    private static func signedDebitReceiptHash(_ receipt: OfflineBearerDebitReceiptV2) throws -> Data {
        IrohaHash.hash(frame(
            typeName: debitReceiptType,
            payload: try debitReceiptPayload(receipt, includeSignature: true)
        ))
    }

    private static func signedCreditReceiptHash(_ receipt: OfflineBearerCreditReceiptV2) throws -> Data {
        IrohaHash.hash(frame(
            typeName: creditReceiptType,
            payload: try creditReceiptPayload(receipt, includeSignature: true)
        ))
    }

    private static func stringVecPayload(_ values: Set<String>) -> Data {
        let normalized = values
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
            .sorted()
        return vecPayload(normalized.map(stringPayload))
    }

    private static func assetSendLimitVecPayload(_ limits: [OfflineBearerAssetSendLimitV2]) throws -> Data {
        vecPayload(
            limits.sorted { $0.assetDefinitionId < $1.assetDefinitionId }.map { limit in
                structPayload([
                    stringPayload(limit.assetDefinitionId),
                    stringPayload(limit.maxTransactionAmount),
                    stringPayload(limit.dailySendLimit),
                    stringPayload(limit.monthlySendLimit),
                ])
            }
        )
    }

    private static func hashVecPayload(_ values: [Data]) throws -> Data {
        try vecPayload(values.map(hashPayload))
    }

    private static func vecPayload(_ values: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeLength(UInt64(values.count))
        for value in values {
            writer.writeField(value)
        }
        return writer.data
    }

    private static func structPayload(_ fields: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        for field in fields {
            writer.writeField(field)
        }
        return writer.data
    }

    private static func stringPayload(_ value: String) -> Data {
        OfflineCompactNorito.encodeString(value)
    }

    private static func bytesVecPayload(_ value: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeLength(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private static func hashPayload(_ value: Data) throws -> Data {
        guard value.count == 32 else {
            throw OfflineBearerPolicyError("Offline Bearer hash fields must be 32 bytes")
        }
        return value
    }

    private static func u16Payload(_ value: Int) throws -> Data {
        guard value >= 0, value <= Int(UInt16.max) else {
            throw OfflineBearerPolicyError("u16 value is out of range")
        }
        return OfflineCompactNorito.encodeUInt16(UInt16(value))
    }

    private static func u64Payload(_ value: UInt64) -> Data {
        OfflineCompactNorito.encodeUInt64(value)
    }

    private static func frame(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: NoritoHeader.compactLen)
    }
}

private extension SHA256Digest {
    var data: Data { Data(self) }
}

public protocol OfflineBearerSignatureVerifying {
    func verifyPolicy(_ policy: OfflineBearerPolicyBundleV2) throws
    func verifyCertificate(_ certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2) throws
    func verifyReceiveRequest(_ request: OfflineBearerReceiveRequestV2) throws
    func verifyDebitReceipt(_ receipt: OfflineBearerDebitReceiptV2) throws
    func verifyCreditReceipt(_ receipt: OfflineBearerCreditReceiptV2) throws
}

public struct RejectingOfflineBearerSignatureVerifier: OfflineBearerSignatureVerifying {
    public init() {}

    public func verifyPolicy(_ policy: OfflineBearerPolicyBundleV2) throws {
        throw OfflineBearerPolicyError("Offline Bearer issuer signature verifier is not configured")
    }

    public func verifyCertificate(_ certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2) throws {
        throw OfflineBearerPolicyError("Offline Bearer issuer signature verifier is not configured")
    }

    public func verifyReceiveRequest(_ request: OfflineBearerReceiveRequestV2) throws {
        throw OfflineBearerPolicyError("Offline Bearer device signature verifier is not configured")
    }

    public func verifyDebitReceipt(_ receipt: OfflineBearerDebitReceiptV2) throws {
        throw OfflineBearerPolicyError("Offline Bearer device signature verifier is not configured")
    }

    public func verifyCreditReceipt(_ receipt: OfflineBearerCreditReceiptV2) throws {
        throw OfflineBearerPolicyError("Offline Bearer device signature verifier is not configured")
    }
}

public struct OfflineBearerSignatureVerifier: OfflineBearerSignatureVerifying {
    private let trustedIssuerPublicKeys: [Data]

    public init(trustedIssuerPublicKeys: [Data]) {
        self.trustedIssuerPublicKeys = trustedIssuerPublicKeys
    }

    public func verifyPolicy(_ policy: OfflineBearerPolicyBundleV2) throws {
        try verifyIssuerSignature(
            algorithm: policy.signatureAlgorithm,
            payload: OfflineBearerV2Payloads.policyUnsignedPayload(policy),
            signature: policy.issuerSignature,
            message: "Offline Bearer policy issuer signature is invalid."
        )
    }

    public func verifyCertificate(_ certificate: OfflineBearerCertificateV2, policy: OfflineBearerPolicyBundleV2) throws {
        guard certificate.issuerId == policy.issuerId,
              certificate.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame else {
            throw OfflineBearerPolicyError("Offline Bearer certificate does not match policy")
        }
        try verifyIssuerSignature(
            algorithm: policy.signatureAlgorithm,
            payload: OfflineBearerV2Payloads.certificateUnsignedPayload(certificate),
            signature: certificate.issuerSignature,
            message: "Offline Bearer certificate issuer signature is invalid."
        )
    }

    public func verifyReceiveRequest(_ request: OfflineBearerReceiveRequestV2) throws {
        guard request.signatureAlgorithm == request.recipientCertificate.signatureAlgorithm else {
            throw OfflineBearerPolicyError("Offline Bearer receive request algorithm does not match certificate")
        }
        try verifyDeviceSignature(
            algorithm: request.signatureAlgorithm,
            publicKey: request.recipientCertificate.publicKey,
            payload: OfflineBearerV2Payloads.receiveRequestUnsignedPayload(request),
            signature: request.challengeSignature,
            message: "Offline Bearer receive request signature is invalid."
        )
    }

    public func verifyDebitReceipt(_ receipt: OfflineBearerDebitReceiptV2) throws {
        guard receipt.signatureAlgorithm == receipt.senderCertificate.signatureAlgorithm else {
            throw OfflineBearerPolicyError("Offline Bearer debit receipt algorithm does not match certificate")
        }
        try verifyDeviceSignature(
            algorithm: receipt.signatureAlgorithm,
            publicKey: receipt.senderCertificate.publicKey,
            payload: OfflineBearerV2Payloads.debitReceiptUnsignedPayload(receipt),
            signature: receipt.debitSignature,
            message: "Offline Bearer debit receipt signature is invalid."
        )
    }

    public func verifyCreditReceipt(_ receipt: OfflineBearerCreditReceiptV2) throws {
        guard receipt.signatureAlgorithm == receipt.recipientCertificate.signatureAlgorithm else {
            throw OfflineBearerPolicyError("Offline Bearer credit receipt algorithm does not match certificate")
        }
        try verifyDeviceSignature(
            algorithm: receipt.signatureAlgorithm,
            publicKey: receipt.recipientCertificate.publicKey,
            payload: OfflineBearerV2Payloads.creditReceiptUnsignedPayload(receipt),
            signature: receipt.creditSignature,
            message: "Offline Bearer credit receipt signature is invalid."
        )
    }

    private func verifyIssuerSignature(algorithm: String, payload: Data, signature: Data, message: String) throws {
        guard trustedIssuerPublicKeys.contains(where: {
            verifySignature(algorithm: algorithm, publicKey: $0, payload: payload, signature: signature)
        }) else {
            throw OfflineBearerPolicyError(message)
        }
    }

    private func verifyDeviceSignature(algorithm: String, publicKey: Data, payload: Data, signature: Data, message: String) throws {
        guard verifySignature(algorithm: algorithm, publicKey: publicKey, payload: payload, signature: signature) else {
            throw OfflineBearerPolicyError(message)
        }
    }

    private func verifySignature(algorithm: String, publicKey: Data, payload: Data, signature: Data) -> Bool {
        switch algorithm {
        case OfflineBearerV2Crypto.ed25519:
            guard publicKey.count == 32 else { return false }
            return (try? Curve25519.Signing.PublicKey(rawRepresentation: publicKey)
                .isValidSignature(signature, for: payload)) == true
        case OfflineBearerV2Crypto.ecdsaP256SHA256:
            guard let key = try? P256.Signing.PublicKey(x963Representation: publicKey),
                  let ecdsaSignature = try? P256.Signing.ECDSASignature(derRepresentation: signature) else {
                return false
            }
            return key.isValidSignature(ecdsaSignature, for: payload)
        default:
            return false
        }
    }
}

/// Verifies exported Offline Bearer settlement batches before online submission.
public enum OfflineBearerSettlementBatchVerifier {
    public static func verify(_ batch: OfflineBearerSettlementBatchV2,
                              policy: OfflineBearerPolicyBundleV2,
                              signatureVerifier: OfflineBearerSignatureVerifying,
                              now: UInt64 = UInt64(Date().timeIntervalSince1970 * 1000)) throws {
        try signatureVerifier.verifyPolicy(policy)
        try requirePolicyFresh(policy, now: now)
        guard policyHashMatches(batch, policy: policy) else {
            throw OfflineBearerPolicyError("settlement batch policy hash does not match current policy")
        }
        let creditTransferIds = Set(batch.creditReceipts.map(\.transferId))
        for receipt in batch.debitReceipts {
            try verifyDebitReceipt(batch: batch,
                                   receipt: receipt,
                                   policy: policy,
                                   signatureVerifier: signatureVerifier,
                                   now: now,
                                   creditTransferIds: creditTransferIds)
        }
        let debitByTransferId = Dictionary(uniqueKeysWithValues: batch.debitReceipts.map { ($0.transferId, $0) })
        for receipt in batch.creditReceipts {
            try verifyCreditReceipt(batch: batch,
                                    receipt: receipt,
                                    policy: policy,
                                    signatureVerifier: signatureVerifier,
                                    now: now)
            guard let debit = debitByTransferId[receipt.transferId] else {
                throw OfflineBearerPolicyError("settlement credit is missing its accepted debit receipt")
            }
            guard receipt.amount == debit.amount else {
                throw OfflineBearerPolicyError("settlement credit amount does not match debit receipt")
            }
            guard receipt.chainId == debit.chainId else {
                throw OfflineBearerPolicyError("settlement credit chainId does not match debit receipt")
            }
            guard receipt.recipientCertificate.purseId == debit.recipientCertificate.purseId else {
                throw OfflineBearerPolicyError("settlement credit recipient purse does not match debit receipt")
            }
            guard receipt.acceptedAtMs <= debit.expiresAtMs else {
                throw OfflineBearerPolicyError("settlement credit was accepted after debit receipt expiry")
            }
        }
    }

    private static func policyHashMatches(_ batch: OfflineBearerSettlementBatchV2,
                                          policy: OfflineBearerPolicyBundleV2) -> Bool {
        batch.debitReceipts.allSatisfy {
            $0.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame
        } && batch.creditReceipts.allSatisfy {
            $0.recipientCertificate.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame
        }
    }

    private static func verifyDebitReceipt(batch: OfflineBearerSettlementBatchV2,
                                           receipt: OfflineBearerDebitReceiptV2,
                                           policy: OfflineBearerPolicyBundleV2,
                                           signatureVerifier: OfflineBearerSignatureVerifying,
                                           now: UInt64,
                                           creditTransferIds: Set<String>) throws {
        guard receipt.chainId == batch.chainId else {
            throw OfflineBearerPolicyError("debit receipt chainId does not match settlement batch")
        }
        let senderExport = receipt.senderCertificate.purseId == batch.purseId
        let recipientExport = receipt.recipientCertificate.purseId == batch.purseId
        guard senderExport || recipientExport else {
            throw OfflineBearerPolicyError("debit receipt purse does not match settlement batch")
        }
        if recipientExport && !senderExport {
            guard creditTransferIds.contains(receipt.transferId) else {
                throw OfflineBearerPolicyError("receiver settlement batch must include a credit receipt for its accepted debit")
            }
        }
        guard receipt.createdAtMs <= now else {
            throw OfflineBearerPolicyError("debit receipt is from the future")
        }
        guard receipt.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame else {
            throw OfflineBearerPolicyError("debit receipt policy hash does not match current policy")
        }
        guard !policy.revokedTransferIds.contains(receipt.transferId) else {
            throw OfflineBearerPolicyError("Offline Bearer transfer is revoked")
        }
        guard !policy.revokedTransferIds.contains(receipt.paymentRequestId) else {
            throw OfflineBearerPolicyError("Offline Bearer receive request is revoked")
        }
        guard receipt.senderCertificate.assetDefinitionId == receipt.assetDefinitionId else {
            throw OfflineBearerPolicyError("sender certificate asset does not match debit receipt")
        }
        guard receipt.recipientCertificate.assetDefinitionId == receipt.assetDefinitionId else {
            throw OfflineBearerPolicyError("recipient certificate asset does not match debit receipt")
        }
        try requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        try requireAmountAtMost(
            receipt.amount,
            maxTransactionAmount(for: receipt.assetDefinitionId, policy: policy),
            "amount exceeds offline asset transaction policy"
        )
        guard try ToriiOfflineCashCodec.subtractAmounts(receipt.senderPreBalance, receipt.amount) == receipt.senderPostBalance else {
            throw OfflineBearerPolicyError("debit receipt balance transition is invalid")
        }
        try enforceCertificatePolicy(receipt.senderCertificate,
                                     policy: policy,
                                     eventTimeMs: receipt.createdAtMs,
                                     signatureVerifier: signatureVerifier)
        try enforceCertificatePolicy(receipt.recipientCertificate,
                                     policy: policy,
                                     eventTimeMs: receipt.createdAtMs,
                                     signatureVerifier: signatureVerifier)
        try signatureVerifier.verifyDebitReceipt(receipt)
    }

    private static func verifyCreditReceipt(batch: OfflineBearerSettlementBatchV2,
                                            receipt: OfflineBearerCreditReceiptV2,
                                            policy: OfflineBearerPolicyBundleV2,
                                            signatureVerifier: OfflineBearerSignatureVerifying,
                                            now: UInt64) throws {
        guard receipt.chainId == batch.chainId else {
            throw OfflineBearerPolicyError("credit receipt chainId does not match settlement batch")
        }
        guard receipt.recipientCertificate.purseId == batch.purseId else {
            throw OfflineBearerPolicyError("credit receipt recipient purse does not match settlement batch")
        }
        guard receipt.acceptedAtMs <= now else {
            throw OfflineBearerPolicyError("credit receipt is from the future")
        }
        guard !policy.revokedTransferIds.contains(receipt.transferId) else {
            throw OfflineBearerPolicyError("Offline Bearer transfer is revoked")
        }
        try requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        try requireAmountAtMost(
            receipt.amount,
            maxTransactionAmount(for: receipt.recipientCertificate.assetDefinitionId, policy: policy),
            "amount exceeds offline asset transaction policy"
        )
        guard try ToriiOfflineCashCodec.addAmounts(receipt.recipientPreBalance, receipt.amount) == receipt.recipientPostBalance else {
            throw OfflineBearerPolicyError("credit receipt balance transition is invalid")
        }
        try enforceCertificatePolicy(receipt.recipientCertificate,
                                     policy: policy,
                                     eventTimeMs: receipt.acceptedAtMs,
                                     signatureVerifier: signatureVerifier)
        try signatureVerifier.verifyCreditReceipt(receipt)
    }

    private static func enforceCertificatePolicy(_ certificate: OfflineBearerCertificateV2,
                                                 policy: OfflineBearerPolicyBundleV2,
                                                 eventTimeMs: UInt64,
                                                 signatureVerifier: OfflineBearerSignatureVerifying) throws {
        try requirePolicyFresh(policy, now: eventTimeMs)
        guard certificate.issuedAtMs <= eventTimeMs, certificate.expiresAtMs > eventTimeMs else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is not currently valid")
        }
        guard eventTimeMs - certificate.issuedAtMs <= policy.maxCertificateAgeMs else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is too old")
        }
        guard certificate.issuerId == policy.issuerId,
              certificate.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame,
              policy.allowedHardwareClasses.contains(certificate.hardwareClass)
        else {
            throw OfflineBearerPolicyError("Offline Bearer certificate does not match policy")
        }
        try requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm)
        try requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding)
        guard !policy.blacklistedAccountIds.contains(certificate.accountId),
              !policy.blacklistedDeviceIds.contains(certificate.deviceId),
              !policy.blacklistedKeyIds.contains(certificate.keyId),
              !policy.revokedCertificateIds.contains(certificate.certificateId),
              !policy.revokedCertificateIds.contains(certificate.keyId)
        else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is blacklisted")
        }
        try signatureVerifier.verifyCertificate(certificate, policy: policy)
    }

    private static func requirePolicyFresh(_ policy: OfflineBearerPolicyBundleV2, now: UInt64) throws {
        guard policy.issuedAtMs <= now, policy.expiresAtMs > now else {
            throw OfflineBearerPolicyError("Offline Bearer policy is not currently valid")
        }
        guard now - policy.issuedAtMs <= policy.maxPolicyAgeMs else {
            throw OfflineBearerPolicyError("Offline Bearer policy is too old")
        }
    }
}

public final class OfflineBearerWallet {
    private let chainId: String
    private let accountId: String
    private let secureElement: OfflineBearerSecureElement
    private let policyProvider: OfflineBearerPolicyProvider
    private let signatureVerifier: OfflineBearerSignatureVerifying
    private let idGenerator: OfflineNoteIdGenerator
    private let clock: () -> UInt64

    public init(chainId: String,
                accountId: String,
                secureElement: OfflineBearerSecureElement,
                policyProvider: OfflineBearerPolicyProvider,
                signatureVerifier: OfflineBearerSignatureVerifying = RejectingOfflineBearerSignatureVerifier(),
                idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
                clock: @escaping () -> UInt64 = { UInt64(Date().timeIntervalSince1970 * 1000) }) throws {
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(accountId, "accountId")
        self.chainId = chainId
        self.accountId = accountId
        self.secureElement = secureElement
        self.policyProvider = policyProvider
        self.signatureVerifier = signatureVerifier
        self.idGenerator = idGenerator
        self.clock = clock
    }

    public func currentState() throws -> OfflineBearerPurseStateV2? {
        try secureElement.currentState()
    }

    public func installLoadedPurse(certificate: OfflineBearerCertificateV2,
                                   state: OfflineBearerPurseStateV2) throws {
        let policy = try currentVerifiedPolicy()
        let now = clock()
        try requireHardwareUsable(policy: policy)
        guard certificate.chainId == chainId,
              certificate.accountId == accountId,
              state.chainId == chainId,
              state.accountId == accountId,
              state.purseId == certificate.purseId,
              state.assetDefinitionId == certificate.assetDefinitionId,
              state.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame,
              certificate.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame
        else {
            throw OfflineBearerPolicyError("Offline Bearer purse install does not match wallet or policy")
        }
        try enforceCertificatePolicy(certificate, policy: policy, now: now)
        try requireAmountAtMost(state.balance, policy.maxOfflineBalance, "offline purse balance exceeds policy limit")
        try secureElement.installPurse(certificate: certificate, state: state)
    }

    public func prepareReceive(assetDefinitionId: String,
                               amount: String,
                               ttlMs: UInt64? = nil) throws -> OfflineBearerReceiveRequestV2 {
        let policy = try currentVerifiedPolicy()
        let now = clock()
        try requireHardwareUsable(policy: policy)
        try requirePolicyFresh(policy, now: now)
        let certificate = try requireCurrentCertificate()
        let state = try requireCurrentState()
        try enforceCertificatePolicy(certificate, policy: policy, now: now)
        guard assetDefinitionId == state.assetDefinitionId else {
            throw OfflineBearerPolicyError("assetDefinitionId does not match purse asset")
        }
        let canonicalAmount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        try requirePositiveAmount(canonicalAmount, "amount")
        try requireAmountAtMost(canonicalAmount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        try requireAmountAtMost(
            canonicalAmount,
            maxTransactionAmount(for: assetDefinitionId, policy: policy),
            "amount exceeds offline asset transaction policy"
        )
        let request = try secureElement.createReceiveRequest(
            paymentRequestId: idGenerator.nextId(prefix: "offline-bearer-request"),
            amount: canonicalAmount,
            createdAtMs: now,
            expiresAtMs: safeAdd(now, min(ttlMs ?? policy.maxTokenAgeMs, policy.maxTokenAgeMs)),
            policyHashHex: policy.policyHashHex
        )
        try signatureVerifier.verifyReceiveRequest(request)
        return request
    }

    public func pay(_ request: OfflineBearerReceiveRequestV2,
                    ttlMs: UInt64? = nil) throws -> OfflineBearerDebitReceiptV2 {
        let policy = try currentVerifiedPolicy()
        let now = clock()
        try requireHardwareUsable(policy: policy)
        try requirePolicyFresh(policy, now: now)
        try validateReceiveRequest(request, policy: policy, now: now)
        let senderCertificate = try requireCurrentCertificate()
        try enforceCertificatePolicy(senderCertificate, policy: policy, now: now)
        guard senderCertificate.assetDefinitionId == request.assetDefinitionId else {
            throw OfflineBearerPolicyError("sender purse asset does not match receive request")
        }
        try requireAmountAtMost(request.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        try requireAmountAtMost(
            request.amount,
            maxTransactionAmount(for: request.assetDefinitionId, policy: policy),
            "amount exceeds offline asset transaction policy"
        )
        let receipt = try secureElement.debit(
            request: request,
            transferId: idGenerator.nextId(prefix: "offline-bearer-transfer"),
            createdAtMs: now,
            expiresAtMs: safeAdd(now, min(ttlMs ?? policy.maxTokenAgeMs, policy.maxTokenAgeMs))
        )
        try signatureVerifier.verifyDebitReceipt(receipt)
        return receipt
    }

    public func accept(_ receipt: OfflineBearerDebitReceiptV2) throws -> OfflineBearerCreditReceiptV2 {
        let policy = try currentVerifiedPolicy()
        let now = clock()
        try requireHardwareUsable(policy: policy)
        try requirePolicyFresh(policy, now: now)
        try validateDebitReceipt(receipt, policy: policy, now: now)
        let certificate = try requireCurrentCertificate()
        let state = try requireCurrentState()
        guard receipt.recipientCertificate.purseId == certificate.purseId else {
            throw OfflineBearerPolicyError("debit receipt is not addressed to this purse")
        }
        guard state.purseId == certificate.purseId, state.accountId == accountId else {
            throw OfflineBearerPolicyError("current purse state does not match wallet certificate")
        }
        guard state.assetDefinitionId == receipt.assetDefinitionId else {
            throw OfflineBearerPolicyError("current purse asset does not match debit receipt")
        }
        guard state.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame else {
            throw OfflineBearerPolicyError("current purse policy hash does not match policy")
        }
        try enforceCertificatePolicy(certificate, policy: policy, now: now)
        try requireAmountAtMost(
            ToriiOfflineCashCodec.addAmounts(state.balance, receipt.amount),
            policy.maxOfflineBalance,
            "offline purse balance exceeds policy limit"
        )
        let credit = try secureElement.credit(receipt: receipt, acceptedAtMs: now)
        try signatureVerifier.verifyCreditReceipt(credit)
        return credit
    }

    public func exportSettlementBatch(maxReceipts: Int = 256) throws -> OfflineBearerSettlementBatchV2 {
        try requireHardwareUsable(policy: try currentVerifiedPolicy())
        guard maxReceipts > 0 else { throw OfflineBearerPolicyError("maxReceipts must be positive") }
        let batch = try secureElement.exportSettlementBatch(maxReceipts: maxReceipts)
        for receipt in batch.debitReceipts {
            try signatureVerifier.verifyDebitReceipt(receipt)
        }
        for receipt in batch.creditReceipts {
            try signatureVerifier.verifyCreditReceipt(receipt)
        }
        return batch
    }

    public func pruneSettled(transferIds: Set<String>) throws {
        try requireHardwareUsable(policy: try currentVerifiedPolicy())
        try secureElement.pruneSettled(transferIds: transferIds)
    }

    private func validateReceiveRequest(_ request: OfflineBearerReceiveRequestV2,
                                        policy: OfflineBearerPolicyBundleV2,
                                        now: UInt64) throws {
        guard request.chainId == chainId else { throw OfflineBearerPolicyError("receive request chainId mismatch") }
        guard request.expiresAtMs > now, request.createdAtMs <= now else {
            throw OfflineBearerPolicyError("receive request is not currently valid")
        }
        guard now - request.createdAtMs <= policy.maxTokenAgeMs else {
            throw OfflineBearerPolicyError("receive request is too old")
        }
        guard request.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame else {
            throw OfflineBearerPolicyError("receive request policy hash mismatch")
        }
        guard !policy.revokedTransferIds.contains(request.paymentRequestId) else {
            throw OfflineBearerPolicyError("Offline Bearer receive request is revoked")
        }
        try enforceCertificatePolicy(request.recipientCertificate, policy: policy, now: now)
        try signatureVerifier.verifyReceiveRequest(request)
    }

    private func validateDebitReceipt(_ receipt: OfflineBearerDebitReceiptV2,
                                      policy: OfflineBearerPolicyBundleV2,
                                      now: UInt64) throws {
        guard receipt.chainId == chainId else { throw OfflineBearerPolicyError("debit receipt chainId mismatch") }
        guard receipt.expiresAtMs > now, receipt.createdAtMs <= now else {
            throw OfflineBearerPolicyError("debit receipt is not currently valid")
        }
        guard now - receipt.createdAtMs <= policy.maxTokenAgeMs else {
            throw OfflineBearerPolicyError("debit receipt is too old")
        }
        guard receipt.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame else {
            throw OfflineBearerPolicyError("debit receipt policy hash mismatch")
        }
        guard receipt.recipientCertificate.accountId == accountId else {
            throw OfflineBearerPolicyError("debit receipt recipient account mismatch")
        }
        guard !policy.revokedTransferIds.contains(receipt.transferId) else {
            throw OfflineBearerPolicyError("Offline Bearer transfer is revoked")
        }
        try enforceCertificatePolicy(receipt.senderCertificate, policy: policy, now: now)
        try enforceCertificatePolicy(receipt.recipientCertificate, policy: policy, now: now)
        try requireAmountAtMost(receipt.amount, policy.maxTransactionAmount, "amount exceeds offline transaction policy")
        try signatureVerifier.verifyDebitReceipt(receipt)
    }

    private func requireHardwareUsable(policy: OfflineBearerPolicyBundleV2) throws {
        let capabilities = try secureElement.capabilities()
        guard capabilities.hardwareBacked, capabilities.statefulPurse else {
            throw OfflineBearerPolicyError("Offline Bearer requires a hardware-backed stateful purse")
        }
        guard let attestationKeyId = capabilities.attestationKeyId,
              !attestationKeyId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineBearerPolicyError("Offline Bearer requires a non-extractable hardware attestation key")
        }
        guard capabilities.rollbackResistantState else {
            throw OfflineBearerPolicyError("Offline Bearer requires rollback-resistant purse state")
        }
        guard !capabilities.attestationEvidence.isEmpty else {
            throw OfflineBearerPolicyError("Offline Bearer requires secure-element attestation evidence")
        }
        try requireSupportedSignatureAlgorithm(capabilities.signatureAlgorithm)
        try requireSupportedPublicKeyEncoding(capabilities.publicKeyEncoding)
        guard policy.allowedHardwareClasses.contains(capabilities.hardwareClass) else {
            throw OfflineBearerPolicyError("hardware class is not allowed by current Offline Bearer policy")
        }
    }

    private func enforceCertificatePolicy(_ certificate: OfflineBearerCertificateV2,
                                          policy: OfflineBearerPolicyBundleV2,
                                          now: UInt64) throws {
        try requirePolicyFresh(policy, now: now)
        guard certificate.issuedAtMs <= now, certificate.expiresAtMs > now else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is not currently valid")
        }
        guard now - certificate.issuedAtMs <= policy.maxCertificateAgeMs else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is too old")
        }
        guard certificate.issuerId == policy.issuerId,
              certificate.policyHashHex.caseInsensitiveCompare(policy.policyHashHex) == .orderedSame,
              policy.allowedHardwareClasses.contains(certificate.hardwareClass)
        else {
            throw OfflineBearerPolicyError("Offline Bearer certificate does not match policy")
        }
        try requireSupportedSignatureAlgorithm(certificate.signatureAlgorithm)
        try requireSupportedPublicKeyEncoding(certificate.publicKeyEncoding)
        guard !policy.blacklistedAccountIds.contains(certificate.accountId),
              !policy.blacklistedDeviceIds.contains(certificate.deviceId),
              !policy.blacklistedKeyIds.contains(certificate.keyId),
              !policy.revokedCertificateIds.contains(certificate.certificateId),
              !policy.revokedCertificateIds.contains(certificate.keyId)
        else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is blacklisted")
        }
        try signatureVerifier.verifyCertificate(certificate, policy: policy)
    }

    private func requirePolicyFresh(_ policy: OfflineBearerPolicyBundleV2, now: UInt64) throws {
        guard policy.issuedAtMs <= now, policy.expiresAtMs > now else {
            throw OfflineBearerPolicyError("Offline Bearer policy is not currently valid")
        }
        guard now - policy.issuedAtMs <= policy.maxPolicyAgeMs else {
            throw OfflineBearerPolicyError("Offline Bearer policy is too old")
        }
    }

    private func requireCurrentCertificate() throws -> OfflineBearerCertificateV2 {
        guard let certificate = try secureElement.currentCertificate() else {
            throw OfflineBearerPolicyError("Offline Bearer purse certificate is not installed")
        }
        return certificate
    }

    private func requireCurrentState() throws -> OfflineBearerPurseStateV2 {
        guard let state = try secureElement.currentState() else {
            throw OfflineBearerPolicyError("Offline Bearer purse state is not installed")
        }
        return state
    }

    private func currentVerifiedPolicy() throws -> OfflineBearerPolicyBundleV2 {
        let policy = try policyProvider.currentPolicy()
        try signatureVerifier.verifyPolicy(policy)
        return policy
    }
}

private func maxTransactionAmount(for assetDefinitionId: String,
                                  policy: OfflineBearerPolicyBundleV2) -> String {
    policy.assetSendLimits.first { $0.assetDefinitionId == assetDefinitionId }?.maxTransactionAmount
        ?? policy.maxTransactionAmount
}

private func normalizedSet(_ values: Set<String>) -> Set<String> {
    Set(values.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty })
}

private func requireNonBlank(_ value: String, _ field: String) throws {
    guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
        throw OfflineBearerPolicyError("\(field) must not be blank")
    }
}

private func requireHexLike(_ value: String, _ field: String) throws {
    try requireNonBlank(value, field)
    guard value.count.isMultiple(of: 2),
          value.unicodeScalars.allSatisfy({ scalar in
              (48...57).contains(scalar.value)
                  || (65...70).contains(scalar.value)
                  || (97...102).contains(scalar.value)
          })
    else {
        throw OfflineBearerPolicyError("\(field) must be hex")
    }
}

private func requireSupportedSignatureAlgorithm(_ value: String) throws {
    guard value == OfflineBearerV2Crypto.ed25519 ||
        value == OfflineBearerV2Crypto.ecdsaP256SHA256
    else {
        throw OfflineBearerPolicyError("unsupported Offline Bearer signature algorithm")
    }
}

private func requireSupportedPublicKeyEncoding(_ value: String) throws {
    guard value == OfflineBearerV2Crypto.rawEd25519PublicKey ||
        value == OfflineBearerV2Crypto.x963P256PublicKey
    else {
        throw OfflineBearerPolicyError("unsupported Offline Bearer public key encoding")
    }
}

private func requirePositiveAmount(_ value: String, _ field: String) throws {
    guard try ToriiOfflineCashCodec.compareAmounts(value, "0") == .orderedDescending else {
        throw OfflineBearerPolicyError("\(field) must be positive")
    }
}

private func requireNonNegativeAmount(_ value: String, _ field: String) throws {
    guard try ToriiOfflineCashCodec.compareAmounts(value, "0") != .orderedAscending else {
        throw OfflineBearerPolicyError("\(field) must be non-negative")
    }
}

private func requireAmountAtMost(_ value: String, _ max: String, _ message: String) throws {
    guard try ToriiOfflineCashCodec.compareAmounts(value, max) != .orderedDescending else {
        throw OfflineBearerPolicyError(message)
    }
}

private func safeAdd(_ lhs: UInt64, _ rhs: UInt64) -> UInt64 {
    let (value, overflow) = lhs.addingReportingOverflow(rhs)
    return overflow ? UInt64.max : value
}
