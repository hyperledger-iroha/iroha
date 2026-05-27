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
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "50")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )
        try third.installLoadedPurse(
            certificate: Self.certificate(accountId: "carol", purseId: "third-purse", publicKey: thirdElement.publicKey),
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
        XCTAssertEqual(try recipient.exportSettlementBatch().debitReceipts.count, 2)
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

    func testSecureEnclaveAdapterFailsClosedWithoutRollbackResistantStore() throws {
        let element = try SecureEnclaveOfflineBearerSecureElement(
            keyTag: "org.hyperledger.iroha.tests.offline-bearer.no-store",
            purseStore: nil,
            attestationEvidenceProvider: { Data([1]) },
            createKeyIfNeeded: false
        )

        let capabilities = try element.capabilities()
        XCTAssertFalse(capabilities.hardwareBacked)
        XCTAssertFalse(capabilities.statefulPurse)
        XCTAssertFalse(capabilities.rollbackResistantState)
        XCTAssertEqual(capabilities.signatureAlgorithm, OfflineBearerV2Crypto.ecdsaP256SHA256)
        XCTAssertEqual(capabilities.publicKeyEncoding, OfflineBearerV2Crypto.x963P256PublicKey)
        XCTAssertThrowsError(try element.pruneSettled(transferIds: []))
    }

    func testHardwareWithoutAttestationKeyDisablesOfflineValue() throws {
        let clock = TestClock()
        let element = TestStatefulSecureElement(purseId: "weak-purse", attestationKeyId: nil)
        let wallet = try wallet(accountId: "alice", secureElement: element, policy: Self.policy(), clock: clock)

        XCTAssertThrowsError(try wallet.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "weak-purse", publicKey: element.publicKey),
            state: Self.state(accountId: "alice", purseId: "weak-purse", balance: "1")
        ))
    }

    func testPolicyRejectsOldCertificatesAndBlacklistedAccounts() throws {
        let clock = TestClock()
        let oldCertificatePolicy = try Self.policy(maxCertificateAgeMs: 1_000)
        let oldElement = TestStatefulSecureElement(purseId: "old-purse")
        let oldWallet = try wallet(accountId: "alice", secureElement: oldElement, policy: oldCertificatePolicy, clock: clock)

        XCTAssertThrowsError(try oldWallet.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "old-purse", publicKey: oldElement.publicKey, issuedAtMs: Self.now - 10_000),
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
            certificate: Self.certificate(accountId: "bob", purseId: "bob-purse", publicKey: blacklistedElement.publicKey),
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
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "5")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
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
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "2")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "2")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let receipt = try sender.pay(request)

        XCTAssertThrowsError(try recipient.accept(receipt))
        XCTAssertEqual(try XCTUnwrap(recipient.currentState()).balance, "2")
    }

    func testTamperedDebitReceiptSignatureIsRejectedBeforeCredit() throws {
        let clock = TestClock()
        let policy = try Self.policy()
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        try sender.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "5")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let receipt = try sender.pay(request)
        let tampered = try OfflineBearerDebitReceiptV2(
            transferId: receipt.transferId,
            chainId: receipt.chainId,
            paymentRequestId: receipt.paymentRequestId,
            senderCertificate: receipt.senderCertificate,
            recipientCertificate: receipt.recipientCertificate,
            assetDefinitionId: receipt.assetDefinitionId,
            amount: "2",
            senderPreBalance: receipt.senderPreBalance,
            senderPostBalance: receipt.senderPostBalance,
            senderSequence: receipt.senderSequence,
            createdAtMs: receipt.createdAtMs,
            expiresAtMs: receipt.expiresAtMs,
            policyHashHex: receipt.policyHashHex,
            receiveChallengeSignature: receipt.receiveChallengeSignature,
            debitSignature: receipt.debitSignature
        )

        XCTAssertThrowsError(try recipient.accept(tampered))
        XCTAssertEqual(try XCTUnwrap(recipient.currentState()).balance, "0")
    }

    func testSettlementBatchVerifierAcceptsExportsAndRejectsInvalidBalanceTransitions() throws {
        let clock = TestClock()
        let policy = try Self.policy()
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        let verifier = OfflineBearerSignatureVerifier(trustedIssuerPublicKeys: [Self.issuerKeypair.publicKey])
        try sender.installLoadedPurse(
            certificate: Self.certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "5")
        )
        try recipient.installLoadedPurse(
            certificate: Self.certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let debit = try sender.pay(request)
        _ = try recipient.accept(debit)

        try OfflineBearerSettlementBatchVerifier.verify(
            try sender.exportSettlementBatch(),
            policy: policy,
            signatureVerifier: verifier,
            now: clock.now
        )
        try OfflineBearerSettlementBatchVerifier.verify(
            try recipient.exportSettlementBatch(),
            policy: policy,
            signatureVerifier: verifier,
            now: clock.now
        )

        let tamperedDebit = try OfflineBearerDebitReceiptV2(
            transferId: debit.transferId,
            chainId: debit.chainId,
            paymentRequestId: debit.paymentRequestId,
            senderCertificate: debit.senderCertificate,
            recipientCertificate: debit.recipientCertificate,
            assetDefinitionId: debit.assetDefinitionId,
            amount: debit.amount,
            senderPreBalance: debit.senderPreBalance,
            senderPostBalance: "5",
            senderSequence: debit.senderSequence,
            createdAtMs: debit.createdAtMs,
            expiresAtMs: debit.expiresAtMs,
            policyHashHex: debit.policyHashHex,
            receiveChallengeSignature: debit.receiveChallengeSignature,
            debitSignature: debit.debitSignature
        )
        let tamperedBatch = try OfflineBearerSettlementBatchV2(
            chainId: Self.chain,
            purseId: "sender-purse",
            debitReceipts: [tamperedDebit],
            creditReceipts: []
        )

        XCTAssertThrowsError(try OfflineBearerSettlementBatchVerifier.verify(
            tamperedBatch,
            policy: policy,
            signatureVerifier: verifier,
            now: clock.now
        ))
    }

    func testBearerNoritoAndTextCodecsRoundTripCanonicalPayloadsAndRejectOfflineNotePrefixes() throws {
        let clock = TestClock()
        let policy = try Self.policy()
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try wallet(accountId: "alice", secureElement: senderElement, policy: policy, clock: clock)
        let recipient = try wallet(accountId: "bob", secureElement: recipientElement, policy: policy, clock: clock)
        let senderCertificate = try Self.certificate(
            accountId: "alice",
            purseId: "sender-purse",
            publicKey: senderElement.publicKey
        )
        let recipientCertificate = try Self.certificate(
            accountId: "bob",
            purseId: "recipient-purse",
            publicKey: recipientElement.publicKey
        )
        try sender.installLoadedPurse(
            certificate: senderCertificate,
            state: Self.state(accountId: "alice", purseId: "sender-purse", balance: "5")
        )
        try recipient.installLoadedPurse(
            certificate: recipientCertificate,
            state: Self.state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )

        let request = try recipient.prepareReceive(assetDefinitionId: Self.asset, amount: "1")
        let debit = try sender.pay(request)
        let credit = try recipient.accept(debit)
        let settlement = try recipient.exportSettlementBatch()

        XCTAssertEqual(
            policy,
            try OfflineBearerV2TextCodec.decodePolicyBundleNorito(
                OfflineBearerV2TextCodec.encodePolicyBundleNorito(policy)
            )
        )
        XCTAssertEqual(
            senderCertificate,
            try OfflineBearerV2TextCodec.decodeCertificateNorito(
                OfflineBearerV2TextCodec.encodeCertificateNorito(senderCertificate)
            )
        )
        XCTAssertEqual(
            request,
            try OfflineBearerV2TextCodec.decodeReceiveRequestNorito(
                OfflineBearerV2TextCodec.encodeReceiveRequestNorito(request)
            )
        )
        XCTAssertEqual(
            debit,
            try OfflineBearerV2TextCodec.decodeDebitReceiptNorito(
                OfflineBearerV2TextCodec.encodeDebitReceiptNorito(debit)
            )
        )
        XCTAssertEqual(
            credit,
            try OfflineBearerV2TextCodec.decodeCreditReceiptNorito(
                OfflineBearerV2TextCodec.encodeCreditReceiptNorito(credit)
            )
        )
        XCTAssertEqual(
            settlement,
            try OfflineBearerV2TextCodec.decodeSettlementBatchNorito(
                OfflineBearerV2TextCodec.encodeSettlementBatchNorito(settlement)
            )
        )

        let requestText = try OfflineBearerV2TextCodec.encodeReceiveRequestText(request)
        let paymentText = try OfflineBearerV2TextCodec.encodePaymentText(debit)
        let ackText = try OfflineBearerV2TextCodec.encodeAckText(credit)
        XCTAssertTrue(requestText.hasPrefix(OfflineBearerV2TextCodec.receiveRequestTextPrefix))
        XCTAssertTrue(paymentText.hasPrefix(OfflineBearerV2TextCodec.paymentTextPrefix))
        XCTAssertTrue(ackText.hasPrefix(OfflineBearerV2TextCodec.ackTextPrefix))
        XCTAssertEqual(.receiveRequest, OfflineBearerV2TextCodec.payloadKind(requestText))
        XCTAssertEqual(.payment, OfflineBearerV2TextCodec.payloadKind(paymentText))
        XCTAssertEqual(.ack, OfflineBearerV2TextCodec.payloadKind(ackText))
        XCTAssertEqual(request, try OfflineBearerV2TextCodec.decodeReceiveRequestText(requestText))
        XCTAssertEqual(debit, try OfflineBearerV2TextCodec.decodePaymentText(paymentText))
        XCTAssertEqual(credit, try OfflineBearerV2TextCodec.decodeAckText(ackText))

        XCTAssertNil(OfflineBearerV2TextCodec.payloadKind("wallet-offline-receive:AAAA"))
        XCTAssertNil(OfflineBearerV2TextCodec.payloadKind("wallet-offline-payment:AAAA"))
        XCTAssertNil(OfflineBearerV2TextCodec.payloadKind("wallet-offline-ack:AAAA"))
        XCTAssertThrowsError(try OfflineBearerV2TextCodec.decodeReceiveRequestText("wallet-offline-receive:AAAA"))
        XCTAssertThrowsError(try OfflineBearerV2TextCodec.decodePaymentText("wallet-offline-payment:AAAA"))
        XCTAssertThrowsError(try OfflineBearerV2TextCodec.decodeAckText("wallet-offline-ack:AAAA"))
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
            signatureVerifier: OfflineBearerSignatureVerifier(trustedIssuerPublicKeys: [Self.issuerKeypair.publicKey]),
            idGenerator: TestIdGenerator(accountId: accountId),
            clock: { clock.now }
        )
    }

    private static func policy(maxCertificateAgeMs: UInt64 = 24 * 60 * 60 * 1_000,
                               maxTokenAgeMs: UInt64 = 5 * 60 * 1_000,
                               maxOfflineBalance: String = "100",
                               blacklistedAccountIds: Set<String> = []) throws -> OfflineBearerPolicyBundleV2 {
        let unsigned = try OfflineBearerPolicyBundleV2(
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
            issuerSignature: Data([1])
        )
        return try OfflineBearerPolicyBundleV2(
            policyId: unsigned.policyId,
            policyHashHex: unsigned.policyHashHex,
            issuerId: unsigned.issuerId,
            issuedAtMs: unsigned.issuedAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            maxCertificateAgeMs: unsigned.maxCertificateAgeMs,
            maxPolicyAgeMs: unsigned.maxPolicyAgeMs,
            maxTokenAgeMs: unsigned.maxTokenAgeMs,
            maxOfflineBalance: unsigned.maxOfflineBalance,
            maxTransactionAmount: unsigned.maxTransactionAmount,
            allowedHardwareClasses: unsigned.allowedHardwareClasses,
            blacklistedAccountIds: unsigned.blacklistedAccountIds,
            blacklistedDeviceIds: unsigned.blacklistedDeviceIds,
            blacklistedKeyIds: unsigned.blacklistedKeyIds,
            issuerSignature: issuerKeypair.sign(OfflineBearerV2Payloads.policyUnsignedPayload(unsigned)),
            policyEpoch: unsigned.policyEpoch,
            policySource: unsigned.policySource,
            revokedCertificateIds: unsigned.revokedCertificateIds,
            revokedTransferIds: unsigned.revokedTransferIds,
            assetSendLimits: unsigned.assetSendLimits
        )
    }

    private static func certificate(accountId: String,
                                    purseId: String,
                                    publicKey: Data,
                                    issuedAtMs: UInt64 = now - 1_000) throws -> OfflineBearerCertificateV2 {
        let unsigned = try OfflineBearerCertificateV2(
            certificateId: "cert-\(purseId)",
            chainId: chain,
            issuerId: issuer,
            purseId: purseId,
            accountId: accountId,
            assetDefinitionId: asset,
            deviceId: "device-\(purseId)",
            keyId: "key-\(purseId)",
            hardwareClass: hardwareClass,
            publicKey: publicKey,
            issuedAtMs: issuedAtMs,
            expiresAtMs: now + 60 * 60 * 1_000,
            policyId: "policy-1",
            policyHashHex: policyHash,
            issuerSignature: Data([1])
        )
        return try OfflineBearerCertificateV2(
            certificateId: unsigned.certificateId,
            chainId: unsigned.chainId,
            issuerId: unsigned.issuerId,
            purseId: unsigned.purseId,
            accountId: unsigned.accountId,
            assetDefinitionId: unsigned.assetDefinitionId,
            deviceId: unsigned.deviceId,
            keyId: unsigned.keyId,
            hardwareClass: unsigned.hardwareClass,
            publicKey: unsigned.publicKey,
            issuedAtMs: unsigned.issuedAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            policyId: unsigned.policyId,
            policyHashHex: unsigned.policyHashHex,
            issuerSignature: issuerKeypair.sign(OfflineBearerV2Payloads.certificateUnsignedPayload(unsigned))
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
    fileprivate static let issuerKeypair = try! Keypair(privateKeyBytes: Data(SHA256.hash(data: Data("issuer".utf8))))

    fileprivate static func signature(_ value: String) -> Data {
        Data(SHA256.hash(data: Data(value.utf8)))
    }
}

struct OfflineBearerTextPayloadFixture {
    let receiveRequest: String
    let payment: String
    let ack: String
}

extension OfflineBearerWalletTests {
    static func bearerTextPayloadFixture() throws -> OfflineBearerTextPayloadFixture {
        let policy = try policy()
        let clock = TestClock()
        let senderElement = TestStatefulSecureElement(purseId: "sender-purse")
        let recipientElement = TestStatefulSecureElement(purseId: "recipient-purse")
        let sender = try OfflineBearerWallet(
            chainId: chain,
            accountId: "alice",
            secureElement: senderElement,
            policyProvider: StaticOfflineBearerPolicyProvider(policy: policy),
            signatureVerifier: OfflineBearerSignatureVerifier(trustedIssuerPublicKeys: [issuerKeypair.publicKey]),
            idGenerator: TestIdGenerator(accountId: "alice"),
            clock: { clock.now }
        )
        let recipient = try OfflineBearerWallet(
            chainId: chain,
            accountId: "bob",
            secureElement: recipientElement,
            policyProvider: StaticOfflineBearerPolicyProvider(policy: policy),
            signatureVerifier: OfflineBearerSignatureVerifier(trustedIssuerPublicKeys: [issuerKeypair.publicKey]),
            idGenerator: TestIdGenerator(accountId: "bob"),
            clock: { clock.now }
        )
        try sender.installLoadedPurse(
            certificate: certificate(accountId: "alice", purseId: "sender-purse", publicKey: senderElement.publicKey),
            state: state(accountId: "alice", purseId: "sender-purse", balance: "50")
        )
        try recipient.installLoadedPurse(
            certificate: certificate(accountId: "bob", purseId: "recipient-purse", publicKey: recipientElement.publicKey),
            state: state(accountId: "bob", purseId: "recipient-purse", balance: "0")
        )
        let request = try recipient.prepareReceive(assetDefinitionId: asset, amount: "2")
        let debit = try sender.pay(request)
        let credit = try recipient.accept(debit)
        return try OfflineBearerTextPayloadFixture(
            receiveRequest: OfflineBearerV2TextCodec.encodeReceiveRequestText(request),
            payment: OfflineBearerV2TextCodec.encodePaymentText(debit),
            ack: OfflineBearerV2TextCodec.encodeAckText(credit)
        )
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
    private let keypair: Keypair
    private var certificate: OfflineBearerCertificateV2?
    private var state: OfflineBearerPurseStateV2?
    private var debits: [OfflineBearerDebitReceiptV2] = []
    private var credits: [OfflineBearerCreditReceiptV2] = []
    var publicKey: Data { keypair.publicKey }

    init(purseId: String) {
        self.purseId = purseId
        self.attestationKeyId = "attestation-\(purseId)"
        self.keypair = try! Keypair(privateKeyBytes: Data(SHA256.hash(data: Data("purse:\(purseId)".utf8))))
    }

    init(purseId: String, attestationKeyId: String?) {
        self.purseId = purseId
        self.attestationKeyId = attestationKeyId
        self.keypair = try! Keypair(privateKeyBytes: Data(SHA256.hash(data: Data("purse:\(purseId)".utf8))))
    }

    func capabilities() throws -> OfflineBearerSecureElementCapabilities {
        try OfflineBearerSecureElementCapabilities(
            hardwareBacked: true,
            statefulPurse: true,
            hardwareClass: OfflineBearerWalletTests.hardwareClass,
            attestationKeyId: attestationKeyId,
            rollbackResistantState: true,
            attestationEvidence: Data([1])
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
        let unsigned = try OfflineBearerReceiveRequestV2(
            chainId: current.chainId,
            paymentRequestId: paymentRequestId,
            recipientCertificate: certificate,
            assetDefinitionId: current.assetDefinitionId,
            amount: amount,
            createdAtMs: createdAtMs,
            expiresAtMs: expiresAtMs,
            policyHashHex: policyHashHex,
            challengeSignature: Data([1])
        )
        return try OfflineBearerReceiveRequestV2(
            chainId: unsigned.chainId,
            paymentRequestId: unsigned.paymentRequestId,
            recipientCertificate: unsigned.recipientCertificate,
            assetDefinitionId: unsigned.assetDefinitionId,
            amount: unsigned.amount,
            createdAtMs: unsigned.createdAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            policyHashHex: unsigned.policyHashHex,
            challengeSignature: keypair.sign(OfflineBearerV2Payloads.receiveRequestUnsignedPayload(unsigned))
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
        let unsigned = try OfflineBearerDebitReceiptV2(
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
            debitSignature: Data([1])
        )
        let receipt = try OfflineBearerDebitReceiptV2(
            transferId: unsigned.transferId,
            chainId: unsigned.chainId,
            paymentRequestId: unsigned.paymentRequestId,
            senderCertificate: unsigned.senderCertificate,
            recipientCertificate: unsigned.recipientCertificate,
            assetDefinitionId: unsigned.assetDefinitionId,
            amount: unsigned.amount,
            senderPreBalance: unsigned.senderPreBalance,
            senderPostBalance: unsigned.senderPostBalance,
            senderSequence: unsigned.senderSequence,
            createdAtMs: unsigned.createdAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            policyHashHex: unsigned.policyHashHex,
            receiveChallengeSignature: unsigned.receiveChallengeSignature,
            debitSignature: keypair.sign(OfflineBearerV2Payloads.debitReceiptUnsignedPayload(unsigned))
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
        let unsigned = try OfflineBearerCreditReceiptV2(
            transferId: receipt.transferId,
            chainId: receipt.chainId,
            recipientCertificate: certificate,
            amount: receipt.amount,
            recipientPreBalance: current.balance,
            recipientPostBalance: postBalance,
            recipientSequence: nextSequence,
            acceptedAtMs: acceptedAtMs,
            creditSignature: Data([1])
        )
        let credit = try OfflineBearerCreditReceiptV2(
            transferId: unsigned.transferId,
            chainId: unsigned.chainId,
            recipientCertificate: unsigned.recipientCertificate,
            amount: unsigned.amount,
            recipientPreBalance: unsigned.recipientPreBalance,
            recipientPostBalance: unsigned.recipientPostBalance,
            recipientSequence: unsigned.recipientSequence,
            acceptedAtMs: unsigned.acceptedAtMs,
            creditSignature: keypair.sign(OfflineBearerV2Payloads.creditReceiptUnsignedPayload(unsigned))
        )
        if !debits.contains(where: { $0.transferId == receipt.transferId }) {
            debits.append(receipt)
        }
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
