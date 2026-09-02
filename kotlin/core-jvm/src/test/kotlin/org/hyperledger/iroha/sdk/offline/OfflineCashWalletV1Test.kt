// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
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
            val provider = FoldProvider(0, partial)
            assertFailsWith<IllegalArgumentException>("accepted provider missing $missing") {
                OfflineCashWalletV1.open(provider)
            }
            assertEquals(0, provider.recoveryCalls)
        }
    }

    @Test
    fun `stable snapshot drain folds every pending credit`() {
        val provider = FoldProvider(33)
        val wallet = OfflineCashWalletV1.open(provider)

        assertEquals(BigInteger.valueOf(33), wallet.drainPendingCredits())
        assertEquals(33, provider.foldResults.count { it })
        assertEquals(false, provider.foldResults.last())
        assertEquals(1, provider.watermarkCalls)
        assertEquals(BigInteger.valueOf(33), wallet.journalRevision())
        assertEquals(BigInteger.valueOf(33), wallet.aggregateState().sequence)
    }

    @Test
    fun `rotation synchronously drains the complete stable snapshot without relocking`() {
        val provider = FoldProvider(17)
        val wallet = OfflineCashWalletV1.open(provider)

        val rotated = wallet.rotateHardwareEpoch()

        assertEquals(17, provider.foldResults.count { it })
        assertEquals(false, provider.foldResults.last())
        assertEquals(0, provider.remainingCredits)
        assertTrue(provider.rotationObservedDrainedInbox)
        assertEquals(BigInteger.valueOf(18), rotated.sequence)
        assertEquals(BigInteger.valueOf(18), wallet.journalRevision())
    }

    @Test
    fun `drain rejects a non progressing native fold result`() {
        val provider = FoldProvider(1, preserveStateCommitment = true)
        val wallet = OfflineCashWalletV1.open(provider)
        assertFailsWith<IllegalArgumentException> { wallet.drainPendingCredits() }
    }

    private class FoldProvider(
        pending: Int,
        private var activeQualification: OfflineCashHardwareQualificationV1 = OfflineCashV1TestSupport.qualification,
        private val preserveStateCommitment: Boolean = false,
    ) : OfflineCashHardwareProviderV1 {
        var remainingCredits = pending
        var recoveryCalls = 0
        var watermarkCalls = 0
        var rotationObservedDrainedInbox = false
        val foldResults = mutableListOf<Boolean>()
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
            amount: BigInteger,
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

        override fun foldPendingCredit(inboxSequenceInclusive: BigInteger): ByteArray? {
            assertTrue(inboxSequenceInclusive.signum() >= 0)
            val folded = remainingCredits > 0
            foldResults += folded
            if (!folded) return null
            remainingCredits -= 1
            revision += BigInteger.ONE
            sequence += 1
            if (!preserveStateCommitment) stateTag += 1
            return stateBytes()
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
