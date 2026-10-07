// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.testing.JvmApiInventory
import org.junit.jupiter.api.Test

/** Managed original transport tests; no generated key or Native admission is simulated. */
class KagemushaWalletEnrollmentV1Test {
    private val slot = ByteArray(32) { 1 }
    private val key = ByteArray(65) { 2 }.also { it[0] = 4 }
    private val one = byteArrayOf(1)
    private fun originals() = KagemushaWalletEnrollmentOriginalsV1(one, one, one)

    @Test fun `E1 original carrier enforces all bounds and owns exact copies`() {
        val bytes = byteArrayOf(0, -1, 7)
        val input = KagemushaWalletEnrollmentOriginalsV1(bytes, bytes, bytes)
        bytes.fill(3); input.frames().forEach { it.fill(4) }
        input.frames().forEach { assertContentEquals(byteArrayOf(0, -1, 7), it) }
        for (role in 0..2) {
            for (length in listOf(0, listOf(1024,1024,4096)[role] + 1)) {
                val data = MutableList(3) { one }; data[role] = ByteArray(length)
                assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentOriginalsV1(data[0],data[1],data[2]) }
            }
        }
    }
    @Test fun `enrollment selector rejects unused inputs and preserves request bytes`() {
        for (selector in 0..3) {
            val input = KagemushaWalletEnrollmentInputV1(selector, originals(),
                if (selector == 0) byteArrayOf() else slot,
                if (selector >= 2) one else byteArrayOf(), if (selector == 3) one else byteArrayOf())
            assertEquals(6, input.frames().size)
            input.frames().forEach { it.fill(9) }
            assertContentEquals(one, input.frames()[1])
        }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(0,originals(),slot) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(1,originals(),ByteArray(32)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(2,originals(),slot,ByteArray(524289)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(2,originals(),slot,one,one) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(3,originals(),slot,one) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentInputV1(4,originals(),slot) }
    }
    @Test fun `progress separates pending abandoned and enrolled without monetary completion`() {
        val marker = byteArrayOf(8)
        val reply = KagemushaWalletEnrollmentReplyV1(0,-1,0,slot,key,marker)
        val progress = reply.progress()
        marker.fill(0); progress.slot().fill(0); progress.paymentKey().fill(0); progress.markerOriginal().fill(0)
        assertEquals(KagemushaWalletEnrollmentStateV1.ENROLLED, progress.state)
        assertContentEquals(slot,progress.slot()); assertContentEquals(key,progress.paymentKey())
        assertContentEquals(byteArrayOf(8),progress.markerOriginal())
        for ((status,state) in listOf(1 to KagemushaWalletEnrollmentStateV1.PENDING,2 to KagemushaWalletEnrollmentStateV1.SLOT_ABANDONED)) {
            val pending = KagemushaWalletEnrollmentReplyV1(status,-1,0,slot,byteArrayOf(),byteArrayOf()).progress()
            assertEquals(state,pending.state); assertTrue(pending.paymentKey().isEmpty()); assertTrue(pending.markerOriginal().isEmpty())
        }
    }
    @Test fun `request and credential responses are bound to their exact slot and operation`() {
        val fullRequest = ByteArray(524288) { 7 }
        val fullInput = KagemushaWalletEnrollmentInputV1(2, originals(), slot, fullRequest)
        val fullReply = KagemushaWalletEnrollmentReplyV1(3,-1,0,slot,byteArrayOf(),fullRequest)
        fullRequest.fill(8)
        assertContentEquals(fullInput.frames()[4],fullReply.original(3,slot))
        for (status in listOf(3,4)) {
            val reply = KagemushaWalletEnrollmentReplyV1(status,-1,0,slot,byteArrayOf(),one)
            assertContentEquals(one,reply.original(status,slot))
            assertFailsWith<KagemushaWalletExceptionV1> { reply.original(status,ByteArray(32) { 2 }) }
            assertFailsWith<KagemushaWalletExceptionV1> { reply.original(if (status == 3) 4 else 3,slot) }
            assertFailsWith<KagemushaWalletExceptionV1> { reply.progress() }
        }
        val error = assertFailsWith<KagemushaWalletExceptionV1> {
            KagemushaWalletEnrollmentReplyV1(-5,4,42,byteArrayOf(),byteArrayOf(),byteArrayOf()).checked()
        }
        assertEquals(-5,error.status); assertEquals(4,error.reason); assertEquals(42,error.platformCode)
    }
    @Test fun `malformed native result cannot select enrollment progress`() {
        for (make in listOf<() -> KagemushaWalletEnrollmentReplyV1>(
            { KagemushaWalletEnrollmentReplyV1(0,-1,0,slot,byteArrayOf(),one) },
            { KagemushaWalletEnrollmentReplyV1(0,-1,0,slot,key,ByteArray(1025)) },
            { KagemushaWalletEnrollmentReplyV1(1,-1,0,slot,key,byteArrayOf()) },
            { KagemushaWalletEnrollmentReplyV1(3,-1,0,slot,byteArrayOf(),ByteArray(524289)) },
            { KagemushaWalletEnrollmentReplyV1(4,-1,0,slot,byteArrayOf(),ByteArray(1025)) },
            { KagemushaWalletEnrollmentReplyV1(-5,2,0,slot,byteArrayOf(),byteArrayOf()) },
            { KagemushaWalletEnrollmentReplyV1(5,-1,0,slot,byteArrayOf(),byteArrayOf()) },
        )) assertEquals(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT,
            assertFailsWith<KagemushaWalletExceptionV1> { make() }.status)
    }
    @Test fun `compiled JNI only accepts runtime selector and six bounded original arrays`() {
        val methods = JvmApiInventory.read(KagemushaWalletEnrollmentNativeV1::class.java).methods.filter { it.isNative }
        assertEquals(1,methods.size)
        assertEquals("enroll",methods.single().name); assertTrue(methods.single().isStatic)
        assertEquals("(JI[B[B[B[B[B[B)Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletEnrollmentReplyV1;",methods.single().descriptor)
    }
}
