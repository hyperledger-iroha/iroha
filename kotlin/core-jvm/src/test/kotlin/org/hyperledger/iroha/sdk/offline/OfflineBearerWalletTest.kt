package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicLong
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer

class OfflineBearerWalletTest {

    @Test
    fun statefulBearerPurseSupportsPartialSpendAndRespendingWithoutTrailGrowth() {
        val clock = AtomicLong(NOW)
        val policy = policy()
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val thirdElement = TestStatefulSecureElement("third-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        val third = wallet("carol", thirdElement, policy, clock)

        sender.installLoadedPurse(certificate("alice", "sender-purse", senderElement.publicKey()), state("alice", "sender-purse", "50"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse", recipientElement.publicKey()), state("bob", "recipient-purse", "0"))
        third.installLoadedPurse(certificate("carol", "third-purse", thirdElement.publicKey()), state("carol", "third-purse", "0"))

        val requestTwoRupees = recipient.prepareReceive(ASSET, "2")
        val debitTwoRupees = sender.pay(requestTwoRupees)
        val creditTwoRupees = recipient.accept(debitTwoRupees)

        assertEquals("50", debitTwoRupees.senderPreBalance)
        assertEquals("48", debitTwoRupees.senderPostBalance)
        assertEquals("0", creditTwoRupees.recipientPreBalance)
        assertEquals("2", creditTwoRupees.recipientPostBalance)
        assertEquals("48", sender.currentState()?.balance)
        assertEquals("2", recipient.currentState()?.balance)

        clock.addAndGet(1_000)
        val requestOneRupee = third.prepareReceive(ASSET, "1")
        val debitOneRupee = recipient.pay(requestOneRupee)
        val creditOneRupee = third.accept(debitOneRupee)

        assertEquals("2", debitOneRupee.senderPreBalance)
        assertEquals("1", debitOneRupee.senderPostBalance)
        assertEquals("0", creditOneRupee.recipientPreBalance)
        assertEquals("1", creditOneRupee.recipientPostBalance)
        assertEquals("1", recipient.currentState()?.balance)
        assertEquals("1", third.currentState()?.balance)
        assertEquals(1, sender.exportSettlementBatch().debitReceipts().size)
        assertEquals(2, recipient.exportSettlementBatch().debitReceipts().size)
        assertEquals(1, recipient.exportSettlementBatch().creditReceipts().size)
    }

    @Test
    fun unsupportedHardwareDisablesOfflineValue() {
        val wallet = OfflineBearerWallet(
            chainId = CHAIN,
            accountId = "alice",
            secureElement = UnsupportedOfflineBearerSecureElement(),
            policyProvider = StaticOfflineBearerPolicyProvider(policy()),
        )

        assertFailsWith<OfflineBearerPolicyException> {
            wallet.prepareReceive(ASSET, "1")
        }
    }

    @Test
    fun hardwareWithoutAttestationKeyDisablesOfflineValue() {
        val clock = AtomicLong(NOW)
        val element = TestStatefulSecureElement("weak-purse", attestationKeyId = null)
        val wallet = wallet("alice", element, policy(), clock)

        assertFailsWith<OfflineBearerPolicyException> {
            wallet.installLoadedPurse(
                certificate("alice", "weak-purse", element.publicKey()),
                state("alice", "weak-purse", "1"),
            )
        }
    }

    @Test
    fun policyRejectsOldCertificatesAndBlacklistedAccounts() {
        val clock = AtomicLong(NOW)
        val oldCertificatePolicy = policy(maxCertificateAgeMs = 1_000)
        val oldElement = TestStatefulSecureElement("old-purse")
        val oldWallet = wallet("alice", oldElement, oldCertificatePolicy, clock)

        assertFailsWith<OfflineBearerPolicyException> {
            oldWallet.installLoadedPurse(
                certificate("alice", "old-purse", oldElement.publicKey(), issuedAtMs = NOW - 10_000),
                state("alice", "old-purse", "1"),
            )
        }

        val blacklistPolicy = policy(blacklistedAccountIds = listOf("bob"))
        val blacklistedElement = TestStatefulSecureElement("bob-purse")
        val blacklistedWallet = wallet("bob", blacklistedElement, blacklistPolicy, clock)

        assertFailsWith<OfflineBearerPolicyException> {
            blacklistedWallet.installLoadedPurse(
                certificate("bob", "bob-purse", blacklistedElement.publicKey()),
                state("bob", "bob-purse", "1"),
            )
        }
    }

    @Test
    fun expiredReceiveRequestIsRejectedBeforeDebit() {
        val clock = AtomicLong(NOW)
        val policy = policy(maxTokenAgeMs = 1_000)
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        sender.installLoadedPurse(certificate("alice", "sender-purse", senderElement.publicKey()), state("alice", "sender-purse", "5"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse", recipientElement.publicKey()), state("bob", "recipient-purse", "0"))

        val request = recipient.prepareReceive(ASSET, "1", ttlMs = 1_000)
        clock.addAndGet(1_001)

        assertFailsWith<IllegalArgumentException> {
            sender.pay(request)
        }
        assertEquals("5", sender.currentState()?.balance)
    }

    @Test
    fun incomingCreditCannotExceedPolicyMaxBalance() {
        val clock = AtomicLong(NOW)
        val policy = policy(maxOfflineBalance = "2")
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        sender.installLoadedPurse(certificate("alice", "sender-purse", senderElement.publicKey()), state("alice", "sender-purse", "2"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse", recipientElement.publicKey()), state("bob", "recipient-purse", "2"))

        val request = recipient.prepareReceive(ASSET, "1")
        val receipt = sender.pay(request)

        assertFailsWith<OfflineBearerPolicyException> {
            recipient.accept(receipt)
        }
        assertEquals("2", recipient.currentState()?.balance)
    }

    @Test
    fun tamperedDebitReceiptSignatureIsRejectedBeforeCredit() {
        val clock = AtomicLong(NOW)
        val policy = policy()
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        sender.installLoadedPurse(certificate("alice", "sender-purse", senderElement.publicKey()), state("alice", "sender-purse", "5"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse", recipientElement.publicKey()), state("bob", "recipient-purse", "0"))

        val request = recipient.prepareReceive(ASSET, "1")
        val receipt = sender.pay(request)
        val tampered = OfflineBearerDebitReceiptV2(
            version = receipt.version,
            transferId = receipt.transferId,
            chainId = receipt.chainId,
            paymentRequestId = receipt.paymentRequestId,
            senderCertificate = receipt.senderCertificate,
            recipientCertificate = receipt.recipientCertificate,
            assetDefinitionId = receipt.assetDefinitionId,
            amount = "2",
            senderPreBalance = receipt.senderPreBalance,
            senderPostBalance = receipt.senderPostBalance,
            senderSequence = receipt.senderSequence,
            createdAtMs = receipt.createdAtMs,
            expiresAtMs = receipt.expiresAtMs,
            policyHashHex = receipt.policyHashHex,
            receiveChallengeSignature = receipt.receiveChallengeSignature(),
            debitSignature = receipt.debitSignature(),
        )

        assertFailsWith<OfflineBearerPolicyException> {
            recipient.accept(tampered)
        }
        assertEquals("0", recipient.currentState()?.balance)
    }

    @Test
    fun settlementBatchVerifierAcceptsExportsAndRejectsInvalidBalanceTransitions() {
        val clock = AtomicLong(NOW)
        val policy = policy()
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        val verifier = OfflineBearerSignatureVerifier(listOf(ISSUER_KEYPAIR.publicKey()))
        sender.installLoadedPurse(certificate("alice", "sender-purse", senderElement.publicKey()), state("alice", "sender-purse", "5"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse", recipientElement.publicKey()), state("bob", "recipient-purse", "0"))

        val request = recipient.prepareReceive(ASSET, "1")
        val debit = sender.pay(request)
        recipient.accept(debit)

        OfflineBearerSettlementBatchVerifier.verify(sender.exportSettlementBatch(), policy, verifier, clock.get())
        OfflineBearerSettlementBatchVerifier.verify(recipient.exportSettlementBatch(), policy, verifier, clock.get())

        val tamperedDebit = OfflineBearerDebitReceiptV2(
            version = debit.version,
            transferId = debit.transferId,
            chainId = debit.chainId,
            paymentRequestId = debit.paymentRequestId,
            senderCertificate = debit.senderCertificate,
            recipientCertificate = debit.recipientCertificate,
            assetDefinitionId = debit.assetDefinitionId,
            amount = debit.amount,
            senderPreBalance = debit.senderPreBalance,
            senderPostBalance = "5",
            senderSequence = debit.senderSequence,
            createdAtMs = debit.createdAtMs,
            expiresAtMs = debit.expiresAtMs,
            policyHashHex = debit.policyHashHex,
            receiveChallengeSignature = debit.receiveChallengeSignature(),
            debitSignature = debit.debitSignature(),
        )
        val tamperedBatch = OfflineBearerSettlementBatchV2(
            version = OfflineBearerSettlementBatchV2.VERSION,
            chainId = CHAIN,
            purseId = "sender-purse",
            debitReceipts = listOf(tamperedDebit),
            creditReceipts = emptyList(),
        )

        assertFailsWith<IllegalArgumentException> {
            OfflineBearerSettlementBatchVerifier.verify(tamperedBatch, policy, verifier, clock.get())
        }
    }

    @Test
    fun bearerNoritoAndTextCodecsRoundTripCanonicalPayloadsAndRejectOfflineNotePrefixes() {
        val clock = AtomicLong(NOW)
        val policy = policy()
        val senderElement = TestStatefulSecureElement("sender-purse")
        val recipientElement = TestStatefulSecureElement("recipient-purse")
        val sender = wallet("alice", senderElement, policy, clock)
        val recipient = wallet("bob", recipientElement, policy, clock)
        val senderCertificate = certificate("alice", "sender-purse", senderElement.publicKey())
        val recipientCertificate = certificate("bob", "recipient-purse", recipientElement.publicKey())
        sender.installLoadedPurse(senderCertificate, state("alice", "sender-purse", "5"))
        recipient.installLoadedPurse(recipientCertificate, state("bob", "recipient-purse", "0"))

        val request = recipient.prepareReceive(ASSET, "1")
        val debit = sender.pay(request)
        val credit = recipient.accept(debit)
        val settlement = recipient.exportSettlementBatch()

        assertPolicyEquals(
            policy,
            OfflineBearerV2TextCodec.decodePolicyBundleNorito(
                OfflineBearerV2TextCodec.encodePolicyBundleNorito(policy),
            ),
        )
        assertCertificateEquals(
            senderCertificate,
            OfflineBearerV2TextCodec.decodeCertificateNorito(
                OfflineBearerV2TextCodec.encodeCertificateNorito(senderCertificate),
            ),
        )
        assertReceiveRequestEquals(
            request,
            OfflineBearerV2TextCodec.decodeReceiveRequestNorito(
                OfflineBearerV2TextCodec.encodeReceiveRequestNorito(request),
            ),
        )
        assertDebitReceiptEquals(
            debit,
            OfflineBearerV2TextCodec.decodeDebitReceiptNorito(
                OfflineBearerV2TextCodec.encodeDebitReceiptNorito(debit),
            ),
        )
        assertCreditReceiptEquals(
            credit,
            OfflineBearerV2TextCodec.decodeCreditReceiptNorito(
                OfflineBearerV2TextCodec.encodeCreditReceiptNorito(credit),
            ),
        )
        assertSettlementBatchEquals(
            settlement,
            OfflineBearerV2TextCodec.decodeSettlementBatchNorito(
                OfflineBearerV2TextCodec.encodeSettlementBatchNorito(settlement),
            ),
        )

        val requestText = OfflineBearerV2TextCodec.encodeReceiveRequestText(request)
        val paymentText = OfflineBearerV2TextCodec.encodePaymentText(debit)
        val ackText = OfflineBearerV2TextCodec.encodeAckText(credit)
        assertTrue(requestText.startsWith(OfflineBearerV2TextCodec.RECEIVE_REQUEST_TEXT_PREFIX))
        assertTrue(paymentText.startsWith(OfflineBearerV2TextCodec.PAYMENT_TEXT_PREFIX))
        assertTrue(ackText.startsWith(OfflineBearerV2TextCodec.ACK_TEXT_PREFIX))
        assertEquals(OfflineBearerV2TextCodec.PayloadKind.RECEIVE_REQUEST, OfflineBearerV2TextCodec.payloadKind(requestText))
        assertEquals(OfflineBearerV2TextCodec.PayloadKind.PAYMENT, OfflineBearerV2TextCodec.payloadKind(paymentText))
        assertEquals(OfflineBearerV2TextCodec.PayloadKind.ACK, OfflineBearerV2TextCodec.payloadKind(ackText))
        assertReceiveRequestEquals(request, OfflineBearerV2TextCodec.decodeReceiveRequestText(requestText))
        assertDebitReceiptEquals(debit, OfflineBearerV2TextCodec.decodePaymentText(paymentText))
        assertCreditReceiptEquals(credit, OfflineBearerV2TextCodec.decodeAckText(ackText))

        assertNull(OfflineBearerV2TextCodec.payloadKind("wallet-offline-receive:AAAA"))
        assertNull(OfflineBearerV2TextCodec.payloadKind("wallet-offline-payment:AAAA"))
        assertNull(OfflineBearerV2TextCodec.payloadKind("wallet-offline-ack:AAAA"))
        assertFailsWith<IllegalArgumentException> {
            OfflineBearerV2TextCodec.decodeReceiveRequestText("wallet-offline-receive:AAAA")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineBearerV2TextCodec.decodePaymentText("wallet-offline-payment:AAAA")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineBearerV2TextCodec.decodeAckText("wallet-offline-ack:AAAA")
        }
    }

    private fun wallet(
        accountId: String,
        secureElement: TestStatefulSecureElement,
        policy: OfflineBearerPolicyBundleV2,
        clock: AtomicLong,
    ): OfflineBearerWallet =
        OfflineBearerWallet(
            chainId = CHAIN,
            accountId = accountId,
            secureElement = secureElement,
            policyProvider = StaticOfflineBearerPolicyProvider(policy),
            idGenerator = object : OfflineNoteIdGenerator {
                private var next = 0

                override fun nextId(prefix: String): String {
                    next += 1
                    return "$prefix-$accountId-$next"
                }
            },
            clock = java.util.function.LongSupplier { clock.get() },
            signatureVerifier = OfflineBearerSignatureVerifier(listOf(ISSUER_KEYPAIR.publicKey())),
        )

    private fun assertPolicyEquals(expected: OfflineBearerPolicyBundleV2, actual: OfflineBearerPolicyBundleV2) {
        assertEquals(expected.policyId, actual.policyId)
        assertEquals(expected.policyHashHex, actual.policyHashHex)
        assertEquals(expected.issuerId, actual.issuerId)
        assertEquals(expected.issuedAtMs, actual.issuedAtMs)
        assertEquals(expected.expiresAtMs, actual.expiresAtMs)
        assertEquals(expected.maxCertificateAgeMs, actual.maxCertificateAgeMs)
        assertEquals(expected.maxPolicyAgeMs, actual.maxPolicyAgeMs)
        assertEquals(expected.maxTokenAgeMs, actual.maxTokenAgeMs)
        assertEquals(expected.maxOfflineBalance, actual.maxOfflineBalance)
        assertEquals(expected.maxTransactionAmount, actual.maxTransactionAmount)
        assertEquals(expected.allowedHardwareClasses, actual.allowedHardwareClasses)
        assertEquals(expected.blacklistedAccountIds, actual.blacklistedAccountIds)
        assertEquals(expected.blacklistedDeviceIds, actual.blacklistedDeviceIds)
        assertEquals(expected.blacklistedKeyIds, actual.blacklistedKeyIds)
        assertEquals(expected.signatureAlgorithm, actual.signatureAlgorithm)
        assertBytesEqual(expected.issuerSignature(), actual.issuerSignature())
        assertEquals(expected.policyEpoch, actual.policyEpoch)
        assertEquals(expected.policySource, actual.policySource)
        assertEquals(expected.revokedCertificateIds, actual.revokedCertificateIds)
        assertEquals(expected.revokedTransferIds, actual.revokedTransferIds)
        assertEquals(expected.assetSendLimits.size, actual.assetSendLimits.size)
    }

    private fun assertCertificateEquals(
        expected: OfflineBearerCertificateV2,
        actual: OfflineBearerCertificateV2,
    ) {
        assertEquals(expected.certificateId, actual.certificateId)
        assertEquals(expected.chainId, actual.chainId)
        assertEquals(expected.issuerId, actual.issuerId)
        assertEquals(expected.purseId, actual.purseId)
        assertEquals(expected.accountId, actual.accountId)
        assertEquals(expected.assetDefinitionId, actual.assetDefinitionId)
        assertEquals(expected.deviceId, actual.deviceId)
        assertEquals(expected.keyId, actual.keyId)
        assertEquals(expected.hardwareClass, actual.hardwareClass)
        assertEquals(expected.signatureAlgorithm, actual.signatureAlgorithm)
        assertEquals(expected.publicKeyEncoding, actual.publicKeyEncoding)
        assertBytesEqual(expected.publicKey(), actual.publicKey())
        assertEquals(expected.issuedAtMs, actual.issuedAtMs)
        assertEquals(expected.expiresAtMs, actual.expiresAtMs)
        assertEquals(expected.policyId, actual.policyId)
        assertEquals(expected.policyHashHex, actual.policyHashHex)
        assertBytesEqual(expected.issuerSignature(), actual.issuerSignature())
    }

    private fun assertReceiveRequestEquals(
        expected: OfflineBearerReceiveRequestV2,
        actual: OfflineBearerReceiveRequestV2,
    ) {
        assertEquals(expected.version, actual.version)
        assertEquals(expected.chainId, actual.chainId)
        assertEquals(expected.paymentRequestId, actual.paymentRequestId)
        assertCertificateEquals(expected.recipientCertificate, actual.recipientCertificate)
        assertEquals(expected.assetDefinitionId, actual.assetDefinitionId)
        assertEquals(expected.amount, actual.amount)
        assertEquals(expected.createdAtMs, actual.createdAtMs)
        assertEquals(expected.expiresAtMs, actual.expiresAtMs)
        assertEquals(expected.policyHashHex, actual.policyHashHex)
        assertEquals(expected.signatureAlgorithm, actual.signatureAlgorithm)
        assertBytesEqual(expected.challengeSignature(), actual.challengeSignature())
    }

    private fun assertDebitReceiptEquals(
        expected: OfflineBearerDebitReceiptV2,
        actual: OfflineBearerDebitReceiptV2,
    ) {
        assertEquals(expected.version, actual.version)
        assertEquals(expected.transferId, actual.transferId)
        assertEquals(expected.chainId, actual.chainId)
        assertEquals(expected.paymentRequestId, actual.paymentRequestId)
        assertCertificateEquals(expected.senderCertificate, actual.senderCertificate)
        assertCertificateEquals(expected.recipientCertificate, actual.recipientCertificate)
        assertEquals(expected.assetDefinitionId, actual.assetDefinitionId)
        assertEquals(expected.amount, actual.amount)
        assertEquals(expected.senderPreBalance, actual.senderPreBalance)
        assertEquals(expected.senderPostBalance, actual.senderPostBalance)
        assertEquals(expected.senderSequence, actual.senderSequence)
        assertEquals(expected.createdAtMs, actual.createdAtMs)
        assertEquals(expected.expiresAtMs, actual.expiresAtMs)
        assertEquals(expected.policyHashHex, actual.policyHashHex)
        assertBytesEqual(expected.receiveChallengeSignature(), actual.receiveChallengeSignature())
        assertEquals(expected.signatureAlgorithm, actual.signatureAlgorithm)
        assertBytesEqual(expected.debitSignature(), actual.debitSignature())
    }

    private fun assertCreditReceiptEquals(
        expected: OfflineBearerCreditReceiptV2,
        actual: OfflineBearerCreditReceiptV2,
    ) {
        assertEquals(expected.version, actual.version)
        assertEquals(expected.transferId, actual.transferId)
        assertEquals(expected.chainId, actual.chainId)
        assertCertificateEquals(expected.recipientCertificate, actual.recipientCertificate)
        assertEquals(expected.amount, actual.amount)
        assertEquals(expected.recipientPreBalance, actual.recipientPreBalance)
        assertEquals(expected.recipientPostBalance, actual.recipientPostBalance)
        assertEquals(expected.recipientSequence, actual.recipientSequence)
        assertEquals(expected.acceptedAtMs, actual.acceptedAtMs)
        assertEquals(expected.signatureAlgorithm, actual.signatureAlgorithm)
        assertBytesEqual(expected.creditSignature(), actual.creditSignature())
    }

    private fun assertSettlementBatchEquals(
        expected: OfflineBearerSettlementBatchV2,
        actual: OfflineBearerSettlementBatchV2,
    ) {
        assertEquals(expected.version, actual.version)
        assertEquals(expected.chainId, actual.chainId)
        assertEquals(expected.purseId, actual.purseId)
        assertEquals(expected.debitReceipts().size, actual.debitReceipts().size)
        assertEquals(expected.creditReceipts().size, actual.creditReceipts().size)
        expected.debitReceipts().zip(actual.debitReceipts()).forEach { (expectedReceipt, actualReceipt) ->
            assertDebitReceiptEquals(expectedReceipt, actualReceipt)
        }
        expected.creditReceipts().zip(actual.creditReceipts()).forEach { (expectedReceipt, actualReceipt) ->
            assertCreditReceiptEquals(expectedReceipt, actualReceipt)
        }
    }

    private fun assertBytesEqual(expected: ByteArray, actual: ByteArray) {
        assertTrue(expected.contentEquals(actual), "byte arrays differ")
    }

    private fun policy(
        maxCertificateAgeMs: Long = 24L * 60L * 60L * 1000L,
        maxTokenAgeMs: Long = 5L * 60L * 1000L,
        maxOfflineBalance: String = "100",
        blacklistedAccountIds: Collection<String> = emptyList(),
    ): OfflineBearerPolicyBundleV2 {
        val unsigned = OfflineBearerPolicyBundleV2(
            policyId = "policy-1",
            policyHashHex = POLICY_HASH,
            issuerId = ISSUER,
            issuedAtMs = NOW - 1_000,
            expiresAtMs = NOW + 60L * 60L * 1000L,
            maxCertificateAgeMs = maxCertificateAgeMs,
            maxPolicyAgeMs = 12L * 60L * 60L * 1000L,
            maxTokenAgeMs = maxTokenAgeMs,
            maxOfflineBalance = maxOfflineBalance,
            maxTransactionAmount = "10",
            allowedHardwareClasses = listOf(HARDWARE_CLASS),
            blacklistedAccountIds = blacklistedAccountIds,
            issuerSignature = byteArrayOf(1),
        )
        return OfflineBearerPolicyBundleV2(
            policyId = unsigned.policyId,
            policyHashHex = unsigned.policyHashHex,
            issuerId = unsigned.issuerId,
            issuedAtMs = unsigned.issuedAtMs,
            expiresAtMs = unsigned.expiresAtMs,
            maxCertificateAgeMs = unsigned.maxCertificateAgeMs,
            maxPolicyAgeMs = unsigned.maxPolicyAgeMs,
            maxTokenAgeMs = unsigned.maxTokenAgeMs,
            maxOfflineBalance = unsigned.maxOfflineBalance,
            maxTransactionAmount = unsigned.maxTransactionAmount,
            allowedHardwareClasses = unsigned.allowedHardwareClasses,
            blacklistedAccountIds = unsigned.blacklistedAccountIds,
            blacklistedDeviceIds = unsigned.blacklistedDeviceIds,
            blacklistedKeyIds = unsigned.blacklistedKeyIds,
            issuerSignature = ISSUER_KEYPAIR.sign(OfflineBearerV2Payloads.policyUnsignedPayload(unsigned)),
            policyEpoch = unsigned.policyEpoch,
            policySource = unsigned.policySource,
            revokedCertificateIds = unsigned.revokedCertificateIds,
            revokedTransferIds = unsigned.revokedTransferIds,
            assetSendLimits = unsigned.assetSendLimits,
        )
    }

    private fun certificate(
        accountId: String,
        purseId: String,
        publicKey: ByteArray,
        issuedAtMs: Long = NOW - 1_000,
    ): OfflineBearerCertificateV2 {
        val unsigned = OfflineBearerCertificateV2(
            certificateId = "cert-$purseId",
            chainId = CHAIN,
            issuerId = ISSUER,
            purseId = purseId,
            accountId = accountId,
            assetDefinitionId = ASSET,
            deviceId = "device-$purseId",
            keyId = "key-$purseId",
            hardwareClass = HARDWARE_CLASS,
            publicKey = publicKey,
            issuedAtMs = issuedAtMs,
            expiresAtMs = NOW + 60L * 60L * 1000L,
            policyId = "policy-1",
            policyHashHex = POLICY_HASH,
            issuerSignature = byteArrayOf(1),
        )
        return OfflineBearerCertificateV2(
            certificateId = unsigned.certificateId,
            chainId = unsigned.chainId,
            issuerId = unsigned.issuerId,
            purseId = unsigned.purseId,
            accountId = unsigned.accountId,
            assetDefinitionId = unsigned.assetDefinitionId,
            deviceId = unsigned.deviceId,
            keyId = unsigned.keyId,
            hardwareClass = unsigned.hardwareClass,
            publicKey = unsigned.publicKey(),
            issuedAtMs = unsigned.issuedAtMs,
            expiresAtMs = unsigned.expiresAtMs,
            policyId = unsigned.policyId,
            policyHashHex = unsigned.policyHashHex,
            issuerSignature = ISSUER_KEYPAIR.sign(OfflineBearerV2Payloads.certificateUnsignedPayload(unsigned)),
        )
    }

    private fun state(accountId: String, purseId: String, balance: String): OfflineBearerPurseStateV2 =
        OfflineBearerPurseStateV2(
            chainId = CHAIN,
            accountId = accountId,
            assetDefinitionId = ASSET,
            purseId = purseId,
            balance = balance,
            sequence = 0,
            policyHashHex = POLICY_HASH,
            updatedAtMs = NOW,
        )

    private class TestStatefulSecureElement(
        private val purseId: String,
        private val attestationKeyId: String? = "attestation-$purseId",
    ) : OfflineBearerSecureElement {
        private val keypair = TestEd25519Keypair("purse:$purseId")
        private var certificate: OfflineBearerCertificateV2? = null
        private var state: OfflineBearerPurseStateV2? = null
        private val debits = ArrayList<OfflineBearerDebitReceiptV2>()
        private val credits = ArrayList<OfflineBearerCreditReceiptV2>()

        fun publicKey(): ByteArray = keypair.publicKey()

        override fun capabilities(): OfflineBearerSecureElementCapabilities =
            OfflineBearerSecureElementCapabilities(
                hardwareBacked = true,
                statefulPurse = true,
                hardwareClass = HARDWARE_CLASS,
                attestationKeyId = attestationKeyId,
                rollbackResistantState = true,
                attestationEvidence = byteArrayOf(1),
            )

        override fun currentCertificate(): OfflineBearerCertificateV2? = certificate

        override fun currentState(): OfflineBearerPurseStateV2? = state

        override fun installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) {
            this.certificate = certificate
            this.state = state
        }

        override fun createReceiveRequest(
            paymentRequestId: String,
            amount: String,
            createdAtMs: Long,
            expiresAtMs: Long,
            policyHashHex: String,
        ): OfflineBearerReceiveRequestV2 {
            val cert = requireNotNull(certificate)
            val current = requireNotNull(state)
            val unsigned = OfflineBearerReceiveRequestV2(
                version = OfflineBearerReceiveRequestV2.VERSION,
                chainId = current.chainId,
                paymentRequestId = paymentRequestId,
                recipientCertificate = cert,
                assetDefinitionId = current.assetDefinitionId,
                amount = amount,
                createdAtMs = createdAtMs,
                expiresAtMs = expiresAtMs,
                policyHashHex = policyHashHex,
                challengeSignature = byteArrayOf(1),
            )
            return OfflineBearerReceiveRequestV2(
                version = unsigned.version,
                chainId = unsigned.chainId,
                paymentRequestId = unsigned.paymentRequestId,
                recipientCertificate = unsigned.recipientCertificate,
                assetDefinitionId = unsigned.assetDefinitionId,
                amount = unsigned.amount,
                createdAtMs = unsigned.createdAtMs,
                expiresAtMs = unsigned.expiresAtMs,
                policyHashHex = unsigned.policyHashHex,
                challengeSignature = keypair.sign(OfflineBearerV2Payloads.receiveRequestUnsignedPayload(unsigned)),
            )
        }

        override fun debit(
            request: OfflineBearerReceiveRequestV2,
            transferId: String,
            createdAtMs: Long,
            expiresAtMs: Long,
        ): OfflineBearerDebitReceiptV2 {
            val cert = requireNotNull(certificate)
            val current = requireNotNull(state)
            val pre = decimal(current.balance)
            val amount = decimal(request.amount)
            require(pre.compareTo(amount) >= 0) { "insufficient Offline Bearer balance" }
            val post = pre.subtract(amount).canonical()
            val nextSequence = current.sequence + 1
            state = OfflineBearerPurseStateV2(
                chainId = current.chainId,
                accountId = current.accountId,
                assetDefinitionId = current.assetDefinitionId,
                purseId = current.purseId,
                balance = post,
                sequence = nextSequence,
                policyHashHex = current.policyHashHex,
                updatedAtMs = createdAtMs,
            )
            val unsigned = OfflineBearerDebitReceiptV2(
                version = OfflineBearerDebitReceiptV2.VERSION,
                transferId = transferId,
                chainId = request.chainId,
                paymentRequestId = request.paymentRequestId,
                senderCertificate = cert,
                recipientCertificate = request.recipientCertificate,
                assetDefinitionId = request.assetDefinitionId,
                amount = request.amount,
                senderPreBalance = current.balance,
                senderPostBalance = post,
                senderSequence = nextSequence,
                createdAtMs = createdAtMs,
                expiresAtMs = expiresAtMs,
                policyHashHex = request.policyHashHex,
                receiveChallengeSignature = request.challengeSignature(),
                debitSignature = byteArrayOf(1),
            )
            val receipt = OfflineBearerDebitReceiptV2(
                version = unsigned.version,
                transferId = unsigned.transferId,
                chainId = unsigned.chainId,
                paymentRequestId = unsigned.paymentRequestId,
                senderCertificate = unsigned.senderCertificate,
                recipientCertificate = unsigned.recipientCertificate,
                assetDefinitionId = unsigned.assetDefinitionId,
                amount = unsigned.amount,
                senderPreBalance = unsigned.senderPreBalance,
                senderPostBalance = unsigned.senderPostBalance,
                senderSequence = unsigned.senderSequence,
                createdAtMs = unsigned.createdAtMs,
                expiresAtMs = unsigned.expiresAtMs,
                policyHashHex = unsigned.policyHashHex,
                receiveChallengeSignature = unsigned.receiveChallengeSignature(),
                debitSignature = keypair.sign(OfflineBearerV2Payloads.debitReceiptUnsignedPayload(unsigned)),
            )
            debits.add(receipt)
            return receipt
        }

        override fun credit(receipt: OfflineBearerDebitReceiptV2, acceptedAtMs: Long): OfflineBearerCreditReceiptV2 {
            val cert = requireNotNull(certificate)
            val current = requireNotNull(state)
            val post = decimal(current.balance).add(decimal(receipt.amount)).canonical()
            val nextSequence = current.sequence + 1
            state = OfflineBearerPurseStateV2(
                chainId = current.chainId,
                accountId = current.accountId,
                assetDefinitionId = current.assetDefinitionId,
                purseId = current.purseId,
                balance = post,
                sequence = nextSequence,
                policyHashHex = current.policyHashHex,
                updatedAtMs = acceptedAtMs,
            )
            val unsigned = OfflineBearerCreditReceiptV2(
                version = OfflineBearerCreditReceiptV2.VERSION,
                transferId = receipt.transferId,
                chainId = receipt.chainId,
                recipientCertificate = cert,
                amount = receipt.amount,
                recipientPreBalance = current.balance,
                recipientPostBalance = post,
                recipientSequence = nextSequence,
                acceptedAtMs = acceptedAtMs,
                creditSignature = byteArrayOf(1),
            )
            val credit = OfflineBearerCreditReceiptV2(
                version = unsigned.version,
                transferId = unsigned.transferId,
                chainId = unsigned.chainId,
                recipientCertificate = unsigned.recipientCertificate,
                amount = unsigned.amount,
                recipientPreBalance = unsigned.recipientPreBalance,
                recipientPostBalance = unsigned.recipientPostBalance,
                recipientSequence = unsigned.recipientSequence,
                acceptedAtMs = unsigned.acceptedAtMs,
                creditSignature = keypair.sign(OfflineBearerV2Payloads.creditReceiptUnsignedPayload(unsigned)),
            )
            if (debits.none { it.transferId == receipt.transferId }) {
                debits.add(receipt)
            }
            credits.add(credit)
            return credit
        }

        override fun exportSettlementBatch(maxReceipts: Int): OfflineBearerSettlementBatchV2 {
            val current = requireNotNull(state)
            return OfflineBearerSettlementBatchV2(
                version = OfflineBearerSettlementBatchV2.VERSION,
                chainId = current.chainId,
                purseId = current.purseId,
                debitReceipts = debits.take(maxReceipts),
                creditReceipts = credits.take(maxReceipts),
            )
        }

        override fun pruneSettled(transferIds: Collection<String>) {
            val ids = transferIds.toSet()
            debits.removeAll { ids.contains(it.transferId) }
            credits.removeAll { ids.contains(it.transferId) }
        }
    }

    companion object {
        private const val CHAIN = "test-chain"
        private const val ASSET = "rupee#india"
        private const val ISSUER = "offline-issuer"
        private const val HARDWARE_CLASS = "test-stateful-secure-element"
        private const val POLICY_HASH = "00112233445566778899aabbccddeeff"
        private const val NOW = 1_700_000_000_000L
        private val ISSUER_KEYPAIR = TestEd25519Keypair("issuer")

        private fun signature(value: String): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(value.toByteArray(Charsets.UTF_8))

        private fun decimal(value: String): BigDecimal =
            BigDecimal(OfflineCashCodec.canonicalAmountString(value))

        private fun BigDecimal.canonical(): String {
            var normalized = stripTrailingZeros()
            if (normalized.scale() < 0) {
                normalized = normalized.setScale(0)
            }
            return normalized.toPlainString()
        }
    }
}

private class TestEd25519Keypair(seedText: String) {
    private val privateKey = Ed25519PrivateKeyParameters(signature(seedText), 0)

    fun publicKey(): ByteArray = privateKey.generatePublicKey().encoded

    fun sign(payload: ByteArray): ByteArray {
        val signer = Ed25519Signer()
        signer.init(true, privateKey)
        signer.update(payload, 0, payload.size)
        return signer.generateSignature()
    }

    private fun signature(value: String): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(value.toByteArray(Charsets.UTF_8))
}
