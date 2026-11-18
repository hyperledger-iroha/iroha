package org.hyperledger.iroha.sdk.offline.wallet

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.*
import org.junit.jupiter.api.Test

class KagemushaSpartanWalletV1Test {
    @Test fun `completed non Send has no Payment but head and completion remain required`() {
        fun reply(status: Int, data: ByteArray = byteArrayOf()) =
            KagemushaSpartanReplyV1(status, -1, 0, 0, 0, 0, data).checked()
        val output = KagemushaSpartanRetainedOutputV1.from(reply(1), KagemushaSpartanOutputV1.PAYMENT)
        assertContentEquals(byteArrayOf(), assertIs<KagemushaSpartanRetainedOutputV1.Released>(output).bytes())
        for (part in listOf(KagemushaSpartanOutputV1.HEAD, KagemushaSpartanOutputV1.COMPLETION)) {
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanRetainedOutputV1.from(reply(1), part) }
        }
        assertFailsWith<KagemushaWalletExceptionV1> {
            KagemushaSpartanRetainedOutputV1.from(reply(2, byteArrayOf(1)), KagemushaSpartanOutputV1.PAYMENT)
        }
    }
    @Test fun `Unload preview binds both accounts and conserves the face debit`() {
        val vector = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, "fixtures/kagemusha/spartan_unload_review_v3.bin") }.first { it.isFile }.readBytes()
        val review = KagemushaSpartanUnloadProjectionV3(vector)
        assertEquals(KagemushaWalletUInt128V1(4, 0), review.faceDebit)
        assertEquals(KagemushaWalletUInt128V1(1, 0), review.onlineCharge)
        assertEquals(KagemushaWalletUInt128V1(3, 0), review.netToAccount)
        val account = review.credentialAccountBytes()
        val beneficiary = assertNotNull(review.beneficiaryAccountBytes())
        assertFalse(account.contentEquals(beneficiary))
        assertContentEquals(account, vector.copyOfRange(421, 421 + account.size))
        assertContentEquals(beneficiary, vector.copyOfRange(423 + account.size, vector.size))
        review.credentialAccountBytes().fill(0)
        assertContentEquals(account, review.credentialAccountBytes())
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(vector) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeAnchorProjectionV2(vector) }

        val free = vector.copyOf(423 + account.size)
        free.fill(0, 421 + account.size, free.size)
        for (range in listOf(227 until 243, 259 until 291, 323 until 355)) free.fill(0, range.first, range.last + 1)
        free[243] = 4
        assertNull(KagemushaSpartanUnloadProjectionV3(free).beneficiaryAccountBytes())
        val borrowed = vector.copyOf()
        borrowed.fill(0, 211, 227); borrowed[219] = 1
        borrowed.fill(-1, 243, 251); borrowed.fill(0, 251, 259)
        assertEquals(KagemushaWalletUInt128V1(-1L, 0), KagemushaSpartanUnloadProjectionV3(borrowed).netToAccount)
        val allCharged = vector.copyOf()
        allCharged[227] = 4; allCharged[243] = 0
        assertEquals(KagemushaWalletUInt128V1(0, 0), KagemushaSpartanUnloadProjectionV3(allCharged).netToAccount)

        for (i in listOf(0, 1, 2, 211, 243, 420)) {
            val invalid = vector.copyOf()
            invalid[i] = (invalid[i].toInt() xor 0x80).toByte()
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
        for (range in listOf(163 until 195, 211 until 227, 227 until 243, 259 until 291, 323 until 355, 419 until 421)) {
            val invalid = vector.copyOf()
            invalid.fill(0, range.first, range.last + 1)
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
        for (invalid in listOf(vector.copyOf(vector.size - 1), vector + byteArrayOf(0), ByteArray(16_385))) {
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanUnloadProjectionV3(invalid) }
        }
    }

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
    @Test fun `signed time anchor binds unsigned floor and selected heads`() {
        val bytes = ByteArray(243)
        bytes[0] = 2; bytes[2] = 7
        for ((index, offset) in listOf(3, 35, 67, 99, 131, 179, 211).withIndex()) {
            bytes.fill((index + 1).toByte(), offset, offset + 32)
        }
        ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).apply {
            putLong(163, Long.MIN_VALUE); putLong(171, Long.MIN_VALUE + 1)
        }
        val anchor = KagemushaSpartanTimeAnchorProjectionV2(bytes)
        assertEquals(1uL shl 63, anchor.issuerTime)
        assertEquals((1uL shl 63) + 1uL, anchor.successorFloor)
        val fields = listOf(anchor.schemeId(), anchor.walletId(), anchor.operationId(), anchor.capsuleDigest(),
            anchor.anchorDigest(), anchor.beforeHead(), anchor.afterHead())
        fields.forEachIndexed { index, field -> assertContentEquals(ByteArray(32) { (index + 1).toByte() }, field) }
        anchor.anchorDigest().fill(0)
        assertContentEquals(ByteArray(32) { 5 }, anchor.anchorDigest())
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanReviewProjectionV1(bytes) }
        val malformed = listOf(bytes.copyOf(242), bytes.copyOf(244), projection(),
            bytes.copyOf().also { it[0] = 1 }, bytes.copyOf().also { it[1] = 1 },
            bytes.copyOf().also { it[2] = 6 }, bytes.copyOf().also { it[178] = 0 })
        malformed.forEach { assertFailsWith<KagemushaWalletExceptionV1> { KagemushaSpartanTimeAnchorProjectionV2(it) } }
        bytes[171] = 0
        assertEquals(anchor.issuerTime, KagemushaSpartanTimeAnchorProjectionV2(bytes).successorFloor)
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
    @Test fun `execute validates native sequence and status specific reason`() {
        fun execution(status: Int, low: Long = 0, high: Long = 0, detail: Int = 0): KagemushaSpartanExecutionV1 {
            val bytes = if (status == 1) byteArrayOf(1) else byteArrayOf()
            return KagemushaSpartanExecutionV1(KagemushaSpartanReplyV1(status, -1, 0, low, high, detail, bytes))
        }
        for (status in 1..5) {
            assertFailsWith<KagemushaWalletExceptionV1> { execution(status, low = 1) }
            assertFailsWith<KagemushaWalletExceptionV1> { execution(status, high = 1) }
            if (status != 3) assertFailsWith<KagemushaWalletExceptionV1> { execution(status, detail = 1) }
        }
        for (reason in 0..2) assertEquals(reason, execution(3, detail = reason).detail)
        assertFailsWith<KagemushaWalletExceptionV1> { execution(3, detail = 3) }
        assertFailsWith<KagemushaWalletExceptionV1> { execution(3, detail = -1) }
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
