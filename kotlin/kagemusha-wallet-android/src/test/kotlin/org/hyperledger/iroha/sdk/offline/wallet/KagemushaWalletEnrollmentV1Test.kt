package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.*
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.testing.JvmApiInventory

/** FFI projection fixture only; it has no native provider, attestation or qualified proof grant. */
class KagemushaWalletEnrollmentV1Test {
    private class Driver : KagemushaWalletEnrollmentDriverV1 {
        var reply = KagemushaWalletCallV1(18,-1,0,7,0,0,ByteArray(32) { 4 })
        var calls = 0; var closes = 0; var original = byteArrayOf()
        var closeStatus = 0
        var retainedCertificates = emptyArray<ByteArray>()
        override fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>): KagemushaWalletCallV1 {
            assertEquals(7,handle); calls++; original=first.copyOf();first.fill(0)
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
        val driver=Driver();val owner=KagemushaWalletEnrollmentV1(7,driver);val input=byteArrayOf(1,2)
        driver.reply=KagemushaWalletCallV1(27,-1,0,7,0,0,byteArrayOf(6,7))
        assertContentEquals(byteArrayOf(6,7),owner.begin(ByteArray(32){8},input,input));assertContentEquals(byteArrayOf(1,2),input)
        driver.reply=KagemushaWalletCallV1(18,-1,0,7,0,0,ByteArray(32){4})
        assertContentEquals(ByteArray(32){4},owner.acceptPermit(input))
        val beforePermit=driver.calls
        assertFailsWith<IllegalArgumentException>{owner.acceptPermit(ByteArray(2049))}
        assertFailsWith<IllegalArgumentException>{owner.begin(ByteArray(31),input,input)}
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
    @Test fun `native evidence target fields are immutable exact FFI slices`() {
        val bytes=ByteArray(161){it.toByte()};val target=KagemushaWalletEnrollmentTargetV1(bytes);bytes.fill(0)
        assertEquals(32,target.slot().size);assertEquals(65,target.paymentKey().size)
        assertEquals(97,target.challengeDigest()[0].toInt());assertEquals(129.toByte(),target.keyBindingDigest()[0])
        target.paymentKey().fill(0);assertEquals(32,target.paymentKey()[0].toInt())
    }

    @Test fun `failed enrollment close retains actual owner and fences operations until explicit cleanup`() {
        val driver = Driver()
        val owner = KagemushaWalletEnrollmentV1(7, driver)
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
        val owner = KagemushaWalletEnrollmentV1(7, driver)
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
}
