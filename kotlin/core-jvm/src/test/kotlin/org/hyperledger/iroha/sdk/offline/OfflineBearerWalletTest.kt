package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicLong
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

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

        sender.installLoadedPurse(certificate("alice", "sender-purse"), state("alice", "sender-purse", "50"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "0"))
        third.installLoadedPurse(certificate("carol", "third-purse"), state("carol", "third-purse", "0"))

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
        assertEquals(1, recipient.exportSettlementBatch().debitReceipts().size)
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
                certificate("alice", "weak-purse"),
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
                certificate("alice", "old-purse", issuedAtMs = NOW - 10_000),
                state("alice", "old-purse", "1"),
            )
        }

        val blacklistPolicy = policy(blacklistedAccountIds = listOf("bob"))
        val blacklistedElement = TestStatefulSecureElement("bob-purse")
        val blacklistedWallet = wallet("bob", blacklistedElement, blacklistPolicy, clock)

        assertFailsWith<OfflineBearerPolicyException> {
            blacklistedWallet.installLoadedPurse(
                certificate("bob", "bob-purse"),
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
        sender.installLoadedPurse(certificate("alice", "sender-purse"), state("alice", "sender-purse", "5"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "0"))

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
        sender.installLoadedPurse(certificate("alice", "sender-purse"), state("alice", "sender-purse", "2"))
        recipient.installLoadedPurse(certificate("bob", "recipient-purse"), state("bob", "recipient-purse", "2"))

        val request = recipient.prepareReceive(ASSET, "1")
        val receipt = sender.pay(request)

        assertFailsWith<OfflineBearerPolicyException> {
            recipient.accept(receipt)
        }
        assertEquals("2", recipient.currentState()?.balance)
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
        )

    private fun policy(
        maxCertificateAgeMs: Long = 24L * 60L * 60L * 1000L,
        maxTokenAgeMs: Long = 5L * 60L * 1000L,
        maxOfflineBalance: String = "100",
        blacklistedAccountIds: Collection<String> = emptyList(),
    ): OfflineBearerPolicyBundleV2 =
        OfflineBearerPolicyBundleV2(
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
            issuerSignature = byteArrayOf(9),
        )

    private fun certificate(
        accountId: String,
        purseId: String,
        issuedAtMs: Long = NOW - 1_000,
    ): OfflineBearerCertificateV2 =
        OfflineBearerCertificateV2(
            certificateId = "cert-$purseId",
            chainId = CHAIN,
            issuerId = ISSUER,
            purseId = purseId,
            accountId = accountId,
            assetDefinitionId = ASSET,
            deviceId = "device-$purseId",
            keyId = "key-$purseId",
            hardwareClass = HARDWARE_CLASS,
            publicKey = signature("pub:$purseId"),
            issuedAtMs = issuedAtMs,
            expiresAtMs = NOW + 60L * 60L * 1000L,
            policyId = "policy-1",
            policyHashHex = POLICY_HASH,
            issuerSignature = byteArrayOf(1, 2, 3),
        )

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
        private var certificate: OfflineBearerCertificateV2? = null
        private var state: OfflineBearerPurseStateV2? = null
        private val debits = ArrayList<OfflineBearerDebitReceiptV2>()
        private val credits = ArrayList<OfflineBearerCreditReceiptV2>()

        override fun capabilities(): OfflineBearerSecureElementCapabilities =
            OfflineBearerSecureElementCapabilities(
                hardwareBacked = true,
                statefulPurse = true,
                hardwareClass = HARDWARE_CLASS,
                attestationKeyId = attestationKeyId,
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
            return OfflineBearerReceiveRequestV2(
                version = OfflineBearerReceiveRequestV2.VERSION,
                chainId = current.chainId,
                paymentRequestId = paymentRequestId,
                recipientCertificate = cert,
                assetDefinitionId = current.assetDefinitionId,
                amount = amount,
                createdAtMs = createdAtMs,
                expiresAtMs = expiresAtMs,
                policyHashHex = policyHashHex,
                challengeSignature = signature("receive:$paymentRequestId:$amount:${current.sequence}"),
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
            return OfflineBearerDebitReceiptV2(
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
                debitSignature = signature("debit:$transferId:${current.balance}:$post:$nextSequence"),
            ).also { debits.add(it) }
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
            return OfflineBearerCreditReceiptV2(
                version = OfflineBearerCreditReceiptV2.VERSION,
                transferId = receipt.transferId,
                chainId = receipt.chainId,
                recipientCertificate = cert,
                amount = receipt.amount,
                recipientPreBalance = current.balance,
                recipientPostBalance = post,
                recipientSequence = nextSequence,
                acceptedAtMs = acceptedAtMs,
                creditSignature = signature("credit:${receipt.transferId}:${current.balance}:$post:$nextSequence"),
            ).also { credits.add(it) }
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
