// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Tag
import org.junit.jupiter.api.Test

/** Real JNI linkage and intake checks. No installed wallet, proof or phone qualification. */
@Tag("host-native")
class KagemushaWalletSuffixUnloadHostNativeV1Test {
    private val request = ByteArray(32) { 1 }
    private val empty = byteArrayOf()

    @BeforeEach
    fun requireCurrentNativeBridge() {
        assertTrue(NativeSignerBridge.isNativeAvailable(), "The supplied current ABI-33 bridge must load")
        assertEquals(1, KagemushaWalletNativeV1.revision())
    }

    private fun refused(expected: Int, value: KagemushaWalletSuffixUnloadReplyV1?) {
        val reply = assertNotNull(value, "JNI constructs the dedicated suffix holder")
        assertEquals(expected, reply.status)
        assertEquals(-1, reply.reason)
        assertEquals(0, reply.platformCode)
        assertEquals(0L, reply.sequenceLow)
        assertEquals(0L, reply.sequenceHigh)
        assertEquals(0, reply.detail)
        assertTrue(reply.bytes().isEmpty(), "a refusal cannot expose completion or a review")
        assertEquals(expected, assertFailsWith<KagemushaWalletExceptionV1> { reply.checked() }.status)
    }

    private fun prepare(
        identity: ByteArray = request,
        low: Long = 1,
        high: Long = 0,
        quote: ByteArray = empty,
        certificates: ByteArray = empty,
        beneficiary: ByteArray = empty,
    ) = KagemushaWalletNativeV1.suffixUnloadPrepare(0, 1, identity, low, high, quote, certificates, beneficiary)

    @Test
    fun allEightNamedCallsReachTheRealMissingOwnerAndNeverReturnAbsenceOrCompletion() {
        // Unknown handle 0 owns no wallet or exchange. Shape-admissible inputs reach -2;
        // this says nothing about monetary validity of the dummy original bytes.
        refused(-2, KagemushaWalletNativeV1.suffixUnloadStatus(0))
        refused(-2, prepare())
        refused(-2, KagemushaWalletNativeV1.suffixUnloadResume(0, 1, request))
        refused(-2, KagemushaWalletNativeV1.executeReviewedSuffixUnload(0, 1, 1))
        refused(-2, KagemushaWalletNativeV1.suffixUnloadInstruction(0, request, 0, 0))
        refused(-2, KagemushaWalletNativeV1.suffixUnloadSignedWire(0, request, 0, 0, 0))
        refused(-2, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, request, 0, 0, byteArrayOf(1)))
        refused(-2, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, request, 0, 0, byteArrayOf(1), byteArrayOf(2)))
    }

    @Test
    fun preparePreservesUnsignedWideAmountsAndTheThreeExactOptionalChargeBounds() {
        for ((low, high) in listOf(-1L to 0L, 0L to 1L, 0L to Long.MIN_VALUE, -1L to -1L)) {
            refused(-2, prepare(low = low, high = high))
        }
        refused(-1, prepare(low = 0))
        refused(-2, prepare(quote = ByteArray(1_024), certificates = ByteArray(10_034), beneficiary = ByteArray(4_096)))
        for ((quote, certificates, beneficiary) in listOf(
            Triple(byteArrayOf(1), empty, empty),
            Triple(empty, byteArrayOf(1), empty),
            Triple(empty, empty, byteArrayOf(1)),
            Triple(byteArrayOf(1), byteArrayOf(1), empty),
            Triple(byteArrayOf(1), empty, byteArrayOf(1)),
            Triple(empty, byteArrayOf(1), byteArrayOf(1)),
            Triple(ByteArray(1_025), byteArrayOf(1), byteArrayOf(1)),
            Triple(byteArrayOf(1), ByteArray(10_035), byteArrayOf(1)),
            Triple(byteArrayOf(1), byteArrayOf(1), ByteArray(4_097)),
        )) refused(-1, prepare(quote = quote, certificates = certificates, beneficiary = beneficiary))
    }

    @Test
    fun malformedRequestIdentitiesFailEveryNamedIntakeWithoutBecomingRequestAbsent() {
        for (bad in listOf(empty, ByteArray(31), ByteArray(32), ByteArray(33))) {
            refused(-1, prepare(identity = bad))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadResume(0, 1, bad))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadInstruction(0, bad, 0, 0))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadSignedWire(0, bad, 0, 0, 0))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, bad, 0, 0, byteArrayOf(1)))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, bad, 0, 0, byteArrayOf(1), byteArrayOf(2)))
        }
    }

    @Test
    fun ordinalHighBitsAndNegativeAttemptIndexesCannotAliasValidRetainedStages() {
        for (ordinal in listOf(0L, 1L, 513L)) {
            refused(-2, KagemushaWalletNativeV1.suffixUnloadInstruction(0, request, ordinal, 0))
            refused(-2, KagemushaWalletNativeV1.suffixUnloadSignedWire(0, request, ordinal, 0, Long.MAX_VALUE))
        }
        for ((low, high) in listOf(514L to 0L, -1L to 0L, 0L to 1L, 0L to -1L)) {
            refused(-1, KagemushaWalletNativeV1.suffixUnloadInstruction(0, request, low, high))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadSignedWire(0, request, low, high, 0))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, request, low, high, byteArrayOf(1)))
            refused(-1, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, request, low, high, byteArrayOf(1), byteArrayOf(2)))
        }
        refused(-1, KagemushaWalletNativeV1.suffixUnloadSignedWire(0, request, 0, 0, -1))
        for (token in listOf(0L, -1L, Long.MIN_VALUE)) {
            refused(-1, KagemushaWalletNativeV1.executeReviewedSuffixUnload(0, 1, token))
        }
    }

    @Test
    fun suffixWireAndFinalityCapsReachNativeWithoutUsingOrdinaryMonetaryBounds() {
        assertEquals(593_952, KagemushaWalletSuffixUnloadV1.SIGNED_WIRE_MAX_BYTES)
        assertEquals(544_800, KagemushaWalletSuffixUnloadV1.INSTRUCTION_MAX_BYTES)
        val wire = ByteArray(KagemushaWalletSuffixUnloadV1.SIGNED_WIRE_MAX_BYTES)
        val finality = ByteArray(1_327_104)
        refused(-2, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, request, 513, 0, wire))
        refused(-2, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, request, 513, 0, wire, finality))
        refused(-1, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, request, 0, 0, empty))
        refused(-1, KagemushaWalletNativeV1.suffixUnloadAttemptOutcome(0, request, 0, 0, ByteArray(wire.size + 1)))
        refused(-1, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, request, 0, 0, ByteArray(wire.size + 1), byteArrayOf(1)))
        refused(-1, KagemushaWalletNativeV1.suffixUnloadIngestFinality(0, 1, request, 0, 0, byteArrayOf(1), ByteArray(finality.size + 1)))
    }
}
