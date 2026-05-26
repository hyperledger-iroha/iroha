import Foundation

/// Capabilities reported by a hardware-backed Offline Bearer purse provider.
public struct OfflineBearerSecureElementCapabilities: Equatable, Sendable {
    public let hardwareBacked: Bool
    public let statefulPurse: Bool
    public let hardwareClass: String
    public let attestationKeyId: String?

    public init(hardwareBacked: Bool,
                statefulPurse: Bool,
                hardwareClass: String,
                attestationKeyId: String? = nil) throws {
        guard !hardwareClass.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineBearerPolicyError("hardwareClass must not be blank")
        }
        self.hardwareBacked = hardwareBacked
        self.statefulPurse = statefulPurse
        self.hardwareClass = hardwareClass
        self.attestationKeyId = attestationKeyId
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
        self.version = version
        self.transferId = transferId
        self.chainId = chainId
        self.recipientCertificate = recipientCertificate
        self.amount = canonicalAmount
        self.recipientPreBalance = canonicalPre
        self.recipientPostBalance = canonicalPost
        self.recipientSequence = recipientSequence
        self.acceptedAtMs = acceptedAtMs
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

public final class OfflineBearerWallet {
    private let chainId: String
    private let accountId: String
    private let secureElement: OfflineBearerSecureElement
    private let policyProvider: OfflineBearerPolicyProvider
    private let idGenerator: OfflineNoteIdGenerator
    private let clock: () -> UInt64

    public init(chainId: String,
                accountId: String,
                secureElement: OfflineBearerSecureElement,
                policyProvider: OfflineBearerPolicyProvider,
                idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
                clock: @escaping () -> UInt64 = { UInt64(Date().timeIntervalSince1970 * 1000) }) throws {
        try requireNonBlank(chainId, "chainId")
        try requireNonBlank(accountId, "accountId")
        self.chainId = chainId
        self.accountId = accountId
        self.secureElement = secureElement
        self.policyProvider = policyProvider
        self.idGenerator = idGenerator
        self.clock = clock
    }

    public func currentState() throws -> OfflineBearerPurseStateV2? {
        try secureElement.currentState()
    }

    public func installLoadedPurse(certificate: OfflineBearerCertificateV2,
                                   state: OfflineBearerPurseStateV2) throws {
        let policy = try policyProvider.currentPolicy()
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
        let policy = try policyProvider.currentPolicy()
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
        return try secureElement.createReceiveRequest(
            paymentRequestId: idGenerator.nextId(prefix: "offline-bearer-request"),
            amount: canonicalAmount,
            createdAtMs: now,
            expiresAtMs: safeAdd(now, min(ttlMs ?? policy.maxTokenAgeMs, policy.maxTokenAgeMs)),
            policyHashHex: policy.policyHashHex
        )
    }

    public func pay(_ request: OfflineBearerReceiveRequestV2,
                    ttlMs: UInt64? = nil) throws -> OfflineBearerDebitReceiptV2 {
        let policy = try policyProvider.currentPolicy()
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
        return try secureElement.debit(
            request: request,
            transferId: idGenerator.nextId(prefix: "offline-bearer-transfer"),
            createdAtMs: now,
            expiresAtMs: safeAdd(now, min(ttlMs ?? policy.maxTokenAgeMs, policy.maxTokenAgeMs))
        )
    }

    public func accept(_ receipt: OfflineBearerDebitReceiptV2) throws -> OfflineBearerCreditReceiptV2 {
        let policy = try policyProvider.currentPolicy()
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
        return try secureElement.credit(receipt: receipt, acceptedAtMs: now)
    }

    public func exportSettlementBatch(maxReceipts: Int = 256) throws -> OfflineBearerSettlementBatchV2 {
        try requireHardwareUsable(policy: try policyProvider.currentPolicy())
        guard maxReceipts > 0 else { throw OfflineBearerPolicyError("maxReceipts must be positive") }
        return try secureElement.exportSettlementBatch(maxReceipts: maxReceipts)
    }

    public func pruneSettled(transferIds: Set<String>) throws {
        try requireHardwareUsable(policy: try policyProvider.currentPolicy())
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
        guard !policy.blacklistedAccountIds.contains(certificate.accountId),
              !policy.blacklistedDeviceIds.contains(certificate.deviceId),
              !policy.blacklistedKeyIds.contains(certificate.keyId),
              !policy.revokedCertificateIds.contains(certificate.certificateId),
              !policy.revokedCertificateIds.contains(certificate.keyId)
        else {
            throw OfflineBearerPolicyError("Offline Bearer certificate is blacklisted")
        }
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
