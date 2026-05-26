import CryptoKit
import XCTest
@testable import IrohaSwift

final class OfflineBearerWalletTests: XCTestCase {
    func testStatefulBearerPurseSupportsPartialSpendAndRespendingWithoutTrailGrowth() throws {
        let clock = TestClock()
        let policy = try Self.policy()
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let thirdElement = TestStatefulSecureElement(purseId: "third-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        let third = try wallet(accountId: "carol", secureElement: thirdElement, policy: policy, clock: clock)

        try sender.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse"),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "50")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse"),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )
        try third.installLoadedPurse(
            certificate: Self.certificate(accountId: "carol", purseId: "third-purse"),
            state: Self.state(accountId: "carol", purseId: "third-purse", balance: "0")
        )

        let requestTwoRupees = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "2")
        let debitTwoRupees = try sender.pay(requestTwoRupees)
        let creditTwoRupees = try recipient.accept(debitTwoRupees)

        XCTAssertEqual(debitTwoRupees.senderPreBalance, "50")
        XCTAssertEqual(debitTwoRupees.senderPostBalance, "48")
        XCTAssertEqual(creditTwoRupees.recipientPreBalance, "0")
        XCTAssertEqual(creditTwoRupees.recipientPostBalance, "2")
        XCTAssertEqual(try XCTUnwrap(sender.currentState()).balance, "48")
        XCTAssertEqual(try XCTUnwrap(recipient.currentState()).balance, "2")

        clock.now += 1_000
        let requestOneRupee = try third.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let debitOneRupee = try recipient.pay(requestOneRupee)
        let creditOneRupee = try third.accept(debitOneRupee)

        XCTAssertEqual(debitOneRupee.senderPreBalance, "2")
        XCTAssertEqual(debitOneRupee.senderPostBalance, "1")
        XCTAssertEqual(creditOneRupee.recipientPreBalance, "0")
        XCTAssertEqual(creditOneRupee.recipientPostBalance, "1")
        XCTAssertEqual(try XCTUnwrap(recipient.currentState()).balance, "1")
        XCTAssertEqual(try XCTUnwrap(third.currentState()).balance, "1")
        XCTAssertEqual(try sender.exportSettlementBatch().debitReceipts.count, 1)
        XCTAssertEqual(try recipient.exportSettlementBatch().debitReceipts.count, 1)
        XCTAssertEqual(try recipient.exportSettlementBatch().creditReceipts.count, 1)
    }

    func testUnsupportedHardwareDisablesOfflineValue() throws {
        let wallet = try OfflineBearerWallet(
            chainId: Self.chain,
            accountId: "alice",
            secureElement: UnsupportedOfflineBearerSecureElement(),
            policyProvider: StaticOfflineBearerPolicyProvider(policy: Self.policy())
        )

        XCTAssertThrowsError(try wallet.prepareReceive(assetDefinitionId: Self.asset, amount: "1"))
    }

    func testHardwareWithoutAttestationKeyDisablesOfflineValue() throws {
        let clock = TestClock()
        let element = TestStatefulSecureElement(purseId: "weak-purse", attestationKeyId: nil)
        let wallet = try wallet(accountId: "alice", secureElement: element, policy: Self.policy(), clock: clock)

        XCTAssertThrowsError(try wallet.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "weak-purse"),
            state: Self.state(accountId: "alice", purseId: "weak-purse", balance: "1")
        ))
    }

    func testPolicyRejectsOldCertificatesAndBlacklistedAccounts() throws {
        let clock = TestClock()
        let oldCertificatePolicy = try Self.policy(maxCertificateAgeMs: 1_000)
        let oldElement = TestStatefulSecureElement(purseId: "old-purse")
        let oldWallet = try wallet(accountId: "alice", secureElement: oldElement, policy: oldCertificatePolicy, clock: clock)

        XCTAssertThrowsError(try oldWallet.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "old-purse", issuedAtMs: Self.now - 10_000),
            state: Self.state(accountId: "alice", purseId: "old-purse", balance: "1")
        ))

        let blacklistPolicy = try Self.policy(blacklistedAccountIds: ["bob"])
        let blacklistedElement = TestStatefulSecureElement(purseId: "bob-purse")
        let blacklistedWallet = try wallet(
            accountId: "bob",
            secureElement: blacklistedElement,
            policy: blacklistPolicy,
            clock: clock
        )

        XCTAssertThrowsError(try blacklistedWallet.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "bob-purse"),
            state: Self.state(accountId: "bob", purseId: "bob-purse", balance: "1")
        ))
    }

    func testExpiredReceiveRequestIsRejectedBeforeDebit() throws {
        let clock = TestClock()
        let policy = try Self.policy(maxTokenAgeMs: 1_000)
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        try sender.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse"),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "5")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse"),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1", ttlMs: 1_000)
        clock.now += 1_001

        XCTAssertThrowsError(try sender.pay(request))
        XCTAssertEqual(try XCTUnwrap(sender.currentState()).balance, "5")
    }

    func testIncomingCreditCannotExceedPolicyMaxBalance() throws {
        let clock = TestClock()
        let policy = try Self.policy(maxOfflineBalance: "2")
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        try sender.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse"),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "2")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse"),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "2")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let receipt = try sender.pay(request)

        XCTAssertThrowsError(try recipient.accept(receipt))
        XCTAssertEqual(try XCTUnwrap(recipient.currentState()).balance, "2")
    }

    private func wallet(accountId: String,
                        secureElement: TestStatefulSecureElement,
                        policy: OfflineBearerPolicyBundleV2,
                        clock: TestClock) throws -> OfflineBearerWallet {
        try OfflineBearerWallet(
            chainId: Self.chain,
            accountId: accountId,
            secureElement: secureElement,
            policyProvider: StaticOfflineBearerPolicyProvider(policy: policy),
            idGenerator: TestIdGenerator(accountId: accountId),
            clock: { clock.now }
        )
    }

    private static func policy(maxCertificateAgeMs: UInt64 = 24 * 60 * 60 * 1_000,
                               maxTokenAgeMs: UInt64 = 5 * 60 * 1_000,
                               maxOfflineBalance: String = "100",
                               blacklistedAccountIds: Set<String> = []) throws -> OfflineBearerPolicyBundleV2 {
        try OfflineBearerPolicyBundleV2(
            policyId: "policy-1",
            policyHashHex: policyHash,
            issuerId: issuer,
            issuedAtMs: now - 1_000,
            expiresAtMs: now + 60 * 60 * 1_000,
            maxCertificateAgeMs: maxCertificateAgeMs,
            maxPolicyAgeMs: 12 * 60 * 60 * 1_000,
            maxTokenAgeMs: maxTokenAgeMs,
            maxOfflineBalance: maxOfflineBalance,
            maxTransactionAmount: "10",
            allowedHardwareClasses: [hardwareClass],
            blacklistedAccountIds: blacklistedAccountIds,
            issuerSignature: Data([9])
        )
    }

    private static func certificate(accountId: String,
                                    purseId: String,
                                    issuedAtMs: UInt64 = now - 1_000) throws -> OfflineBearerCertificateV2 {
        try OfflineBearerCertificateV2(
            certificateId: "cert-\(purseId)",
            chainId: chain,
            issuerId: issuer,
            purseId: purseId,
            accountId: accountId,
            assetDefinitionId: asset,
            deviceId: "device-\(purseId)",
            keyId: "key-\(purseId)",
            hardwareClass: hardwareClass,
            publicKey: signature("pub:\(purseId)"),
            issuedAtMs: issuedAtMs,
            expiresAtMs: now + 60 * 60 * 1_000,
            policyId: "policy-1",
            policyHashHex: policyHash,
            issuerSignature: Data([1, 2, 3])
        )
    }

    private static func state(accountId: String, purseId: String, balance: String) throws -> OfflineBearerPurseStateV2 {
        try OfflineBearerPurseStateV2(
            chainId: chain,
            accountId: accountId,
            assetDefinitionId: asset,
            purseId: purseId,
            balance: balance,
            sequence: 0,
            policyHashHex: policyHash,
            updatedAtMs: now
        )
    }

    fileprivate static let chain = "test-chain"
    fileprivate static let asset = "rupee#india"
    fileprivate static let issuer = "offline-issuer"
    fileprivate static let hardwareClass = "test-stateful-secure-element"
    fileprivate static let policyHash = "00112233445566778899aabbccddeeff"
    fileprivate static let now: UInt64 = 1_700_000_000_000

    fileprivate static func signature(_ value: String) -> Data {
        Data(SHA256.hash(data: Data(value.utf8)))
    }
}

private final class TestClock {
    var now = OfflineBearerWalletTests.now
}

private final class TestIdGenerator: OfflineNoteIdGenerator {
    private let accountId: String
    private var next = 0

    init(accountId: String) {
        self.accountId = accountId
    }

    func nextId(prefix: String) -> String {
        next += 1
        return "\(prefix)-\(accountId)-\(next)"
    }
}

private final class TestStatefulSecureElement: OfflineBearerSecureElement {
    private let purseId: String
    private let attestationKeyId: String?
    private var certificate: OfflineBearerCertificateV2?
    private var state: OfflineBearerPurseStateV2?
    private var debits: [OfflineBearerDebitReceiptV2] = []
    private var credits: [OfflineBearerCreditReceiptV2] = []

    init(purseId: String) {
        self.purseId = purseId
        self.attestationKeyId = "attestation-\(purseId)"
    }

    init(purseId: String, attestationKeyId: String?) {
        self.purseId = purseId
        self.attestationKeyId = attestationKeyId
    }

    func capabilities() throws -> OfflineBearerSecureElementCapabilities {
        try OfflineBearerSecureElementCapabilities(
            hardwareBacked: true,
            statefulPurse: true,
            hardwareClass: OfflineBearerWalletTests.hardwareClass,
            attestationKeyId: attestationKeyId
        )
    }

    func currentCertificate() throws -> OfflineBearerCertificateV2? {
        certificate
    }

    func currentState() throws -> OfflineBearerPurseStateV2? {
        state
    }

    func installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) throws {
        self.certificate = certificate
        self.state = state
    }

    func createReceiveRequest(paymentRequestId: String,
                              amount: String,
                              createdAtMs: UInt64,
                              expiresAtMs: UInt64,
                              policyHashHex: String) throws -> OfflineBearerReceiveRequestV2 {
        guard let certificate, let current = state else {
            throw OfflineBearerPolicyError("purse is not installed")
        }
        return try OfflineBearerReceiveRequestV2(
            chainId: current.chainId,
            paymentRequestId: paymentRequestId,
            recipientCertificate: certificate,
            assetDefinitionId: current.assetDefinitionId,
            amount: amount,
            createdAtMs: createdAtMs,
            expiresAtMs: expiresAtMs,
            policyHashHex: policyHashHex,
            challengeSignature: OfflineBearerWalletTests.signature(
                "receive:\(paymentRequestId):\(amount):\(current.sequence)"
            )
        )
    }

    func debit(request: OfflineBearerReceiveRequestV2,
               transferId: String,
               createdAtMs: UInt64,
               expiresAtMs: UInt64) throws -> OfflineBearerDebitReceiptV2 {
        guard let certificate, let current = state else {
            throw OfflineBearerPolicyError("purse is not installed")
        }
        let postBalance = try ToriiOfflineCashCodec.subtractAmounts(current.balance, request.amount)
        let nextSequence = current.sequence + 1
        state = try OfflineBearerPurseStateV2(
            chainId: current.chainId,
            accountId: current.accountId,
            assetDefinitionId: current.assetDefinitionId,
            purseId: current.purseId,
            balance: postBalance,
            sequence: nextSequence,
            policyHashHex: current.policyHashHex,
            updatedAtMs: createdAtMs
        )
        let receipt = try OfflineBearerDebitReceiptV2(
            transferId: transferId,
            chainId: request.chainId,
            paymentRequestId: request.paymentRequestId,
            senderCertificate: certificate,
            recipientCertificate: request.recipientCertificate,
            assetDefinitionId: request.assetDefinitionId,
            amount: request.amount,
            senderPreBalance: current.balance,
            senderPostBalance: postBalance,
            senderSequence: nextSequence,
            createdAtMs: createdAtMs,
            expiresAtMs: expiresAtMs,
            policyHashHex: request.policyHashHex,
            receiveChallengeSignature: request.challengeSignature,
            debitSignature: OfflineBearerWalletTests.signature(
                "debit:\(transferId):\(current.balance):\(postBalance):\(nextSequence)"
            )
        )
        debits.append(receipt)
        return receipt
    }

    func credit(receipt: OfflineBearerDebitReceiptV2, acceptedAtMs: UInt64) throws -> OfflineBearerCreditReceiptV2 {
        guard let certificate, let current = state else {
            throw OfflineBearerPolicyError("purse is not installed")
        }
        let postBalance = try ToriiOfflineCashCodec.addAmounts(current.balance, receipt.amount)
        let nextSequence = current.sequence + 1
        state = try OfflineBearerPurseStateV2(
            chainId: current.chainId,
            accountId: current.accountId,
            assetDefinitionId: current.assetDefinitionId,
            purseId: current.purseId,
            balance: postBalance,
            sequence: nextSequence,
            policyHashHex: current.policyHashHex,
            updatedAtMs: acceptedAtMs
        )
        let credit = try OfflineBearerCreditReceiptV2(
            transferId: receipt.transferId,
            chainId: receipt.chainId,
            recipientCertificate: certificate,
            amount: receipt.amount,
            recipientPreBalance: current.balance,
            recipientPostBalance: postBalance,
            recipientSequence: nextSequence,
            acceptedAtMs: acceptedAtMs,
            creditSignature: OfflineBearerWalletTests.signature(
                "credit:\(receipt.transferId):\(current.balance):\(postBalance):\(nextSequence)"
            )
        )
        credits.append(credit)
        return credit
    }

    func exportSettlementBatch(maxReceipts: Int) throws -> OfflineBearerSettlementBatchV2 {
        guard let current = state else {
            throw OfflineBearerPolicyError("purse is not installed")
        }
        return try OfflineBearerSettlementBatchV2(
            chainId: current.chainId,
            purseId: current.purseId,
            debitReceipts: Array(debits.prefix(maxReceipts)),
            creditReceipts: Array(credits.prefix(maxReceipts))
        )
    }

    func pruneSettled(transferIds: Set<String>) throws {
        debits.removeAll { transferIds.contains($0.transferId) }
        credits.removeAll { transferIds.contains($0.transferId) }
    }
}
