package org.hyperledger.iroha.sdk.offline.wallet

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.*
import org.junit.jupiter.api.Test

class KagemushaSpartanWalletV1Test {
    @Test fun `admission refuses another credential profile before Native`() {
        for (count in listOf(0, 532, 595, 597)) assertFails {
            KagemushaSpartanAccountOriginalsV1(ByteArray(count), ByteArray(172), byteArrayOf(1), byteArrayOf(2))
        }
        for (count in listOf(0, 171, 173)) assertFails {
            KagemushaSpartanAccountOriginalsV1(ByteArray(596), ByteArray(count), byteArrayOf(1), byteArrayOf(2))
        }
        val credential = ByteArray(596) { 7 }
        val originals = KagemushaSpartanAccountOriginalsV1(credential, ByteArray(172), byteArrayOf(1), byteArrayOf(2))
        credential.fill(0); originals.frames()[0].fill(0)
        assertContentEquals(ByteArray(596) { 7 }, originals.frames()[0])
    }
    private fun projection(): ByteArray = ByteArray(291).also {
        it[0] = 1; it[2] = 3
        for ((index, offset) in listOf(3, 35, 67, 99, 163, 195, 227, 259).withIndex()) it.fill((index + 1).toByte(), offset, offset + 32)
        ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).apply {
            putLong(131, -1L); putLong(139, Long.MIN_VALUE); putLong(147, 7); putLong(155, 9)
        }
    }
    @Test fun `review preserves unsigned amounts and every bound identity`() {
        val source = projection()
        val review = KagemushaSpartanReviewProjectionV1(source)
        source.fill(0)
        assertEquals(KagemushaSpartanOperationV1.SEND, review.operation)
        assertEquals(KagemushaWalletUInt128V1(-1L, Long.MIN_VALUE), review.amount)
        assertEquals(KagemushaWalletUInt128V1(7, 9), review.fee)
        val identities = listOf(review.schemeId(), review.walletId(), review.operationId(), review.capsuleDigest(),
            review.receiverWalletId(), review.receiverAccountId(), review.beforeHead(), review.afterHead())
        identities.forEachIndexed { index, bytes -> assertContentEquals(ByteArray(32) { (index + 1).toByte() }, bytes) }
        identities[0].fill(0)
        assertContentEquals(ByteArray(32) { 1 }, review.schemeId())
    }
    @Test fun `review rejects unsupported version kind truncation and trailing data`() {
        val bad = listOf(projection().copyOf(290), projection().copyOf(292),
            projection().also { it[0] = 2 }, projection().also { it[1] = 1 },
            projection().also { it[2] = 0 }, projection().also { it[2] = 5 })
        bad.forEach { assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(it) } }
    }
    @Test fun `reply domains reject unexpected tokens output shapes and false complete`() {
        fun reply(status: Int, token: Long = 0, data: ByteArray = byteArrayOf()) =
            KagemushaSpartanReplyV1(status, -1, 0, token, 0, 0, data).checked()
        assertFails { reply(82, 0, projection()).requireShape(82, 291, token = true) }
        assertFails { reply(83, 1, ByteArray(514)).requireShape(83, 514) }
        assertFails { reply(81, 1, ByteArray(31)).requireShape(81, 32, token = true) }
        assertFails { KagemushaSpartanExecutionV1(reply(1)) }
        assertFails { KagemushaSpartanExecutionV1(reply(2, data = byteArrayOf(1))) }
        assertFails { KagemushaSpartanExecutionV1(reply(84)) }
        assertEquals(KagemushaSpartanExecutionV1.Status.PENDING, KagemushaSpartanExecutionV1(reply(2)).status)
        val original = byteArrayOf(1, 2, 3)
        val completed = KagemushaSpartanExecutionV1(reply(1, data = original))
        original.fill(0)
        completed.completion()!!.fill(0)
        assertContentEquals(byteArrayOf(1, 2, 3), completed.completion())
    }
    @Test fun `verified payment owns its digest and exact 482 byte request`() {
        val bytes = ByteArray(514) { (it % 251).toByte() }
        val result = KagemushaSpartanVerifiedPaymentV1(bytes)
        val digest = bytes.copyOfRange(0, 32); val request = bytes.copyOfRange(32, 514)
        bytes.fill(0); result.digest().fill(0); result.request().fill(0)
        assertContentEquals(digest, result.digest()); assertContentEquals(request, result.request())
        assertFails { KagemushaSpartanVerifiedPaymentV1(ByteArray(513)) }
    }
}
