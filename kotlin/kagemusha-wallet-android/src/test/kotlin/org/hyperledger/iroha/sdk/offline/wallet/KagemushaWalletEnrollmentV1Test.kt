package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.*
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.testing.JvmApiInventory

/** FFI projection fixture only; it has no native provider, attestation or qualified proof grant. */
class KagemushaWalletEnrollmentV1Test {
    @Test fun `session originals preserve exact BPNG empty proof and CBSI proof without granting authority`() {
        val token = byteArrayOf(1, 2)
        val root = byteArrayOf(3, 4)
        val bpng = KagemushaWalletEnrollmentSessionOriginalsV1(token, byteArrayOf(), root)
        token.fill(0); root.fill(0)
        assertContentEquals(byteArrayOf(1, 2), bpng.frames()[0])
        assertTrue(bpng.frames()[1].isEmpty())
        assertContentEquals(byteArrayOf(3, 4), bpng.frames()[2])
        bpng.frames()[0].fill(0)
        assertContentEquals(byteArrayOf(1, 2), bpng.frames()[0])
        val cbsi = KagemushaWalletEnrollmentSessionOriginalsV1(byteArrayOf(1), byteArrayOf(2), byteArrayOf(3))
        assertContentEquals(byteArrayOf(2), cbsi.frames()[1])
        for (bad in listOf(
            listOf(byteArrayOf(), byteArrayOf(), byteArrayOf(1)),
            listOf(byteArrayOf(1), byteArrayOf(), byteArrayOf()),
            listOf(ByteArray(16_385), byteArrayOf(), byteArrayOf(1)),
            listOf(byteArrayOf(1), ByteArray(4097), byteArrayOf(1)),
            listOf(byteArrayOf(1), byteArrayOf(), ByteArray(16_385))
        )) assertFailsWith<IllegalArgumentException> { KagemushaWalletEnrollmentSessionOriginalsV1(bad[0], bad[1], bad[2]) }
        assertEquals("KagemushaWalletEnrollmentSessionOriginalsV1(originals=[REDACTED])", bpng.toString())
    }
    private class Driver : KagemushaWalletEnrollmentDriverV1 {
        var reply = KagemushaWalletCallV1(18,-1,0,7,0,0,ByteArray(32) { 4 })
        var calls = 0; var closes = 0; var lastSelector = -1; var original = byteArrayOf()
        var closeStatus = 0
        var retainedCertificates = emptyArray<ByteArray>()
        override fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>): KagemushaWalletCallV1 {
            assertEquals(7,handle); lastSelector=selector; calls++; original=first.copyOf();first.fill(0)
            retainedCertificates = certificates.map { it.copyOf() }.toTypedArray()
            certificates.forEach { it.fill(0) }; return reply
        }
        override fun close(handle: Long): Int {assertEquals(7,handle);closes++;return closeStatus}
    }
    @Test fun `enrollment projections never become payment completion and retain exact originals`() {
        for ((status,size) in listOf(18 to 32,19 to 161,20 to 0,21 to 0,22 to 0,23 to 32,24 to 131072,25 to 262144,26 to 0,27 to 16384,28 to 1024)) {
            val bytes=ByteArray(size) { 5 };val reply=KagemushaWalletCallV1(status,-1,0,7,0,0,bytes)
            assertFailsWith<KagemushaWalletExceptionV1>{reply.completion()}
            bytes.fill(0);if(size>0)assertEquals(5,reply.bytes()[0].toInt())
            assertFailsWith<KagemushaWalletExceptionV1>{KagemushaWalletCallV1(status,-1,0,0,0,0,reply.bytes())}
        }
        assertFailsWith<KagemushaWalletExceptionV1>{KagemushaWalletCallV1(1,-1,0,0,0,0,ByteArray(10001))}
        assertFailsWith<KagemushaWalletExceptionV1>{KagemushaWalletCallV1(25,-1,0,7,0,0,ByteArray(262145))}
    }
    @Test fun `wrapper copies inputs requires exact projection and closes once`() {
        val driver=Driver();val owner=KagemushaWalletEnrollmentV1(7,driver,byteArrayOf(1,2),Any(),2);val input=byteArrayOf(1,2)
        driver.reply=KagemushaWalletCallV1(27,-1,0,7,0,0,byteArrayOf(6,7))
        assertContentEquals(byteArrayOf(6,7),owner.begin(ByteArray(32){8},input));assertContentEquals(byteArrayOf(1,2),input)
        driver.reply=KagemushaWalletCallV1(18,-1,0,7,0,0,ByteArray(32){4})
        assertContentEquals(ByteArray(32){4},owner.acceptPermit(input))
        val beforePermit=driver.calls
        assertFailsWith<IllegalArgumentException>{owner.acceptPermit(ByteArray(2049))}
        assertFailsWith<IllegalArgumentException>{owner.begin(ByteArray(31),input)}
        assertEquals(beforePermit,driver.calls)
        driver.reply=KagemushaWalletCallV1(20,-1,0,7,0,0,byteArrayOf())
        assertSame(KagemushaWalletEnrollmentProgressV1.Pending,owner.progress())
        assertFailsWith<KagemushaWalletExceptionV1>{owner.retainRequest(ByteArray(64))}
        driver.reply=KagemushaWalletCallV1(24,-1,0,7,0,0,byteArrayOf(9,8,7))
        assertContentEquals(byteArrayOf(9,8,7),assertIs<KagemushaWalletEnrollmentRequestV1.Retained>(owner.prepareRequest(listOf(byteArrayOf(1),byteArrayOf(2)),byteArrayOf(3))).bytes())
        val before=driver.calls;assertFailsWith<IllegalArgumentException>{owner.acceptCredential(ByteArray(262145))};assertEquals(before,driver.calls)
        driver.reply=KagemushaWalletCallV1(28,-1,0,7,0,0,byteArrayOf(7,8))
        assertContentEquals(byteArrayOf(7,8),owner.abandon())
        assertContentEquals(byteArrayOf(7,8),owner.abandon())
        owner.close();owner.close();assertEquals(1,driver.closes);assertFailsWith<KagemushaWalletExceptionV1>{owner.progress()}
    }
    @Test fun `retained request distinguishes absent original from durable E5`() {
        val driver = Driver()
        val owner = KagemushaWalletEnrollmentV1(7, driver, byteArrayOf(1,2), Any(), 2)
        driver.reply = KagemushaWalletCallV1(20,-1,0,7,0,0,byteArrayOf())
        assertNull(owner.retainedRequest())
        driver.reply = KagemushaWalletCallV1(24,-1,0,7,0,0,byteArrayOf(4,5,6))
        assertContentEquals(byteArrayOf(4,5,6), owner.retainedRequest())
        owner.close()
    }
    @Test fun `native evidence target fields are immutable exact FFI slices`() {
        val bytes=ByteArray(161){it.toByte()};val target=KagemushaWalletEnrollmentTargetV1(bytes);bytes.fill(0)
        assertEquals(32,target.slot().size);assertEquals(65,target.paymentKey().size)
        assertEquals(97,target.challengeDigest()[0].toInt());assertEquals(129.toByte(),target.keyBindingDigest()[0])
        target.paymentKey().fill(0);assertEquals(32,target.paymentKey()[0].toInt())
    }

    @Test fun `failed enrollment close retains actual owner and fences operations until explicit cleanup`() {
        val driver = Driver()
        val owner = KagemushaWalletEnrollmentV1(7, driver, byteArrayOf(1,2), Any(), 2)
        driver.closeStatus = -5
        val failure = assertFailsWith<KagemushaWalletExceptionV1> { owner.close() }
        assertFalse(owner.cleanupReleased())
        assertSame(failure, assertFailsWith<KagemushaWalletExceptionV1> { owner.progress() })
        assertSame(failure, assertFailsWith<KagemushaWalletExceptionV1> { owner.close() })
        assertEquals(1, driver.closes)
        driver.closeStatus = 0
        owner.retryCleanup()
        assertTrue(owner.cleanupReleased())
        assertEquals(2, driver.closes)
        owner.close()
        assertEquals(2, driver.closes)
        assertFailsWith<KagemushaWalletExceptionV1> { owner.progress() }
        KagemushaWalletInstalledRuntimeV1.requireNoUnreleasedAdmissions()
    }
    @Test fun `canonical request retains maximum DER originals and rejects bounds before dispatch`() {
        val driver = Driver()
        driver.reply = KagemushaWalletCallV1(23, -1, 0, 7, 0, 0, ByteArray(32) { 1 })
        val owner = KagemushaWalletEnrollmentV1(7, driver, byteArrayOf(1,2), Any(), 2)
        val maximum = List(8) { certificate -> ByteArray(16384) { index -> ((index + certificate) % 251).toByte() } }
        assertIs<KagemushaWalletEnrollmentRequestV1.AccountChallenge>(owner.prepareRequest(maximum, byteArrayOf(1)))
        maximum.zip(driver.retainedCertificates).forEach { (expected, actual) -> assertContentEquals(expected, actual) }
        val before = driver.calls
        for (bad in listOf(List(9) { byteArrayOf(1) }, listOf(byteArrayOf()), listOf(ByteArray(16385)))) {
            assertFailsWith<IllegalArgumentException> { owner.prepareRequest(bad, byteArrayOf(1)) }
        }
        assertEquals(before, driver.calls)
        owner.close()
    }
    @Test fun `canonical JNI enrollment carries only native selector and original frames`() {
        val method = JvmApiInventory.read(KagemushaWalletNativeV1::class.java).methods.single { it.name == "enrollment" && it.isNative }
        assertTrue(method.isStatic)
        assertEquals("(JI[B[B[B[[B)Lorg/hyperledger/iroha/sdk/offline/wallet/KagemushaWalletCallV1;", method.descriptor)
    }
    @Test fun `private chain callback deep copies every bounded DER original`() {
        val one = byteArrayOf(1)
        val certificate = byteArrayOf(7)
        val chain = arrayOf(certificate, certificate)
        val response = KagemushaWalletNativeReplyV1(0, chain = chain)
        certificate.fill(0); response.certificatesDer().forEach { it.fill(0) }
        response.certificatesDer().forEach { assertContentEquals(byteArrayOf(7), it) }
        assertTrue(response.bytes().isEmpty())
        for (make in listOf<() -> KagemushaWalletNativeReplyV1>(
            { KagemushaWalletNativeReplyV1(0, chain = arrayOf(one)) },
            { KagemushaWalletNativeReplyV1(0, chain = Array(9) { one }) },
            { KagemushaWalletNativeReplyV1(0, chain = arrayOf(one, ByteArray(16385))) },
            { KagemushaWalletNativeReplyV1(0, bytes = one, chain = chain) },
            { KagemushaWalletNativeReplyV1(1, chain = chain) },
        )) assertFailsWith<IllegalArgumentException> { make() }
    }
    @Test fun `retained E5 and E6 distinguish original absence refusal and wrong result kind`() {
        val driver = Driver()
        val owner = KagemushaWalletEnrollmentV1(7, driver, byteArrayOf(1,2), Any(), 2)
        for ((selector, present, read) in listOf(
            Triple(16, 24, { owner.retainedRequest() }),
            Triple(18, 25, { owner.retainedResult() })
        )) {
            driver.reply = KagemushaWalletCallV1(20,-1,0,7,0,0,byteArrayOf())
            assertNull(read()); assertEquals(selector, driver.lastSelector)
            val original = byteArrayOf(4,5,6)
            driver.reply = KagemushaWalletCallV1(present,-1,0,7,0,0,original)
            val received = assertNotNull(read()); assertContentEquals(original, received)
            received.fill(0); assertContentEquals(original, read())
            driver.reply = KagemushaWalletCallV1(-5,17,23,0,0,0,byteArrayOf())
            val error = assertFailsWith<KagemushaWalletExceptionV1> { read() }
            assertEquals(-5,error.status); assertEquals(17,error.reason); assertEquals(23,error.platformCode)
            driver.reply = KagemushaWalletCallV1(if(present == 24)25 else 24,-1,0,7,0,0,original)
            assertFailsWith<KagemushaWalletExceptionV1> { read() }
            driver.reply = KagemushaWalletCallV1(present,-1,0,8,0,0,original)
            assertFailsWith<KagemushaWalletExceptionV1> { read() }
        }
        owner.close()
    }

}
