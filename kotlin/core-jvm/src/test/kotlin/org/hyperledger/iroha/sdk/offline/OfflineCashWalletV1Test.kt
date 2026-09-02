// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import kotlin.math.min
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class OfflineCashWalletV1Test {
    @Test
    fun `wallet rejects every partial hardware contract before recovery`() {
        OfflineCashHardwareCapabilityV1.values().forEach { missing ->
            val capabilities = OfflineCashHardwareCapabilityV1.values().toMutableSet().apply { remove(missing) }
            val partial = OfflineCashHardwareQualificationV1(
                1,
                OfflineCashV1TestSupport.profile,
                OfflineCashV1TestSupport.credential,
                OfflineCashV1TestSupport.qualification.releaseId(),
                capabilities,
            )
            val provider = BatchProvider(0, partial)
            assertFailsWith<IllegalArgumentException>("accepted provider missing $missing") {
                OfflineCashWalletV1.open(provider)
            }
            assertEquals(0, provider.recoveryCalls)
        }
    }

    @Test
    fun `stable snapshot drain folds more than sixteen credits in fixed batches`() {
        val provider = BatchProvider(33)
        val wallet = OfflineCashWalletV1.open(provider)

        assertEquals(BigInteger.valueOf(33), wallet.drainPendingCredits())
        assertEquals(listOf(16, 16, 1, 0), provider.foldResults)
        assertTrue(provider.maximumArguments.all { it == 16 })
        assertEquals(1, provider.watermarkCalls)
        assertEquals(BigInteger.valueOf(3), wallet.journalRevision())
        assertEquals(BigInteger.valueOf(3), wallet.aggregateState().sequence)
    }

    @Test
    fun `rotation synchronously drains the complete stable snapshot without relocking`() {
        val provider = BatchProvider(17)
        val wallet = OfflineCashWalletV1.open(provider)

        val rotated = wallet.rotateHardwareEpoch()

        assertEquals(listOf(16, 1, 0), provider.foldResults)
        assertEquals(0, provider.remainingCredits)
        assertTrue(provider.rotationObservedDrainedInbox)
        assertEquals(BigInteger.valueOf(3), rotated.sequence)
        assertEquals(BigInteger.valueOf(3), wallet.journalRevision())
    }

    @Test
    fun `drain rejects a non progressing native fold result`() {
        val provider = BatchProvider(1, preserveStateCommitment = true)
        val wallet = OfflineCashWalletV1.open(provider)
        assertFailsWith<IllegalArgumentException> { wallet.drainPendingCredits() }
    }

    private class BatchProvider(
        pending: Int,
        private var activeQualification: OfflineCashHardwareQualificationV1 = OfflineCashV1TestSupport.qualification,
        private val preserveStateCommitment: Boolean = false,
    ) : OfflineCashHardwareProviderV1 {
        var remainingCredits = pending
        var recoveryCalls = 0
        var watermarkCalls = 0
        var rotationObservedDrainedInbox = false
        val foldResults = mutableListOf<Int>()
        val maximumArguments = mutableListOf<Int>()
        private var revision = BigInteger.ZERO
        private var sequence = 0L
        private var stateTag = 0x51

        override fun qualification(): OfflineCashHardwareQualificationV1 = activeQualification

        override fun recover(): OfflineCashHardwareRecoveryV1 {
            recoveryCalls += 1
            return OfflineCashHardwareRecoveryV1(
                stateBytes(),
                revision,
                BigInteger.valueOf(remainingCredits.toLong()),
                BigInteger.ZERO,
            )
        }

        override fun bootstrapState(): ByteArray = stateBytes()

        override fun createPaymentRequest(
            recipientAccount: ByteArray,
            requestMode: ByteArray,
            validityWindowMillis: Long,
        ): ByteArray = unsupported()

        override fun createAcceptanceIntentAuthorization(
            canonicalRequest: ByteArray,
            exactAmount: BigInteger,
        ): ByteArray = unsupported()

        override fun issueAcceptanceTicket(
            canonicalRequest: ByteArray,
            canonicalAuthorization: ByteArray,
        ): ByteArray = unsupported()

        override fun stagePayment(
            canonicalRequest: ByteArray,
            canonicalPayment: ByteArray,
        ): OfflineCashHardwarePaymentStageV1 = unsupported()

        override fun stageMintCredit(
            canonicalAuthorization: ByteArray,
            canonicalMintCredit: ByteArray,
        ): OfflineCashHardwareMintStageV1 = unsupported()

        override fun pendingCreditWatermark(): BigInteger {
            watermarkCalls += 1
            return BigInteger.valueOf((remainingCredits + 100).toLong())
        }

        override fun journalRevision(): BigInteger = revision

        override fun foldPendingCreditBatch(
            inboxSequenceInclusive: BigInteger,
            maximumCredits: Int,
        ): OfflineCashHardwareFoldBatchV1 {
            assertTrue(inboxSequenceInclusive.signum() >= 0)
            maximumArguments += maximumCredits
            val folded = min(remainingCredits, maximumCredits)
            foldResults += folded
            if (folded == 0) return OfflineCashHardwareFoldBatchV1(0, null)
            remainingCredits -= folded
            revision += BigInteger.ONE
            sequence += 1
            if (!preserveStateCommitment) stateTag += 1
            return OfflineCashHardwareFoldBatchV1(folded, stateBytes())
        }

        override fun commitPayment(
            canonicalRequest: ByteArray,
            canonicalAuthorization: ByteArray,
            canonicalTicket: ByteArray,
        ): OfflineCashHardwareTerminalResultV1 = unsupported()

        override fun recoverPayment(creditId: ByteArray): ByteArray? = null

        override fun recordAcknowledgement(creditId: ByteArray, canonicalAcknowledgement: ByteArray) = Unit

        override fun commitRedemption(
            amount: BigInteger,
            beneficiaryAccount: ByteArray,
        ): OfflineCashHardwareTerminalResultV1 = unsupported()

        override fun recoverRedemption(redemptionId: ByteArray): ByteArray? = null

        override fun rotateHardwareEpoch(): ByteArray {
            rotationObservedDrainedInbox = remainingCredits == 0
            revision += BigInteger.ONE
            sequence += 1
            stateTag += 1
            return stateBytes()
        }

        private fun stateBytes(): ByteArray = OfflineCashNoritoV1.encodeAggregateStateShape(
            OfflineCashV1TestSupport.state(sequence, stateTag),
        )

        private fun <T> unsupported(): T = throw UnsupportedOperationException("not used")
    }
}
