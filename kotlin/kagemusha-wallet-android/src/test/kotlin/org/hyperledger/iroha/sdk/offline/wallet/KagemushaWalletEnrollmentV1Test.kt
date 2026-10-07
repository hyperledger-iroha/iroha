package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.*
import org.junit.jupiter.api.Test

/** FFI projection fixture only; it has no native provider, attestation or qualified proof grant. */
class KagemushaWalletEnrollmentV1Test {
    private class Driver : KagemushaWalletEnrollmentDriverV1 {
        var reply = KagemushaWalletCallV1(18,-1,0,7,0,0,ByteArray(32) { 4 })
        var calls = 0; var closes = 0; var original = byteArrayOf()
        override fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>): KagemushaWalletCallV1 {
            assertEquals(7,handle); calls++; original=first.copyOf();first.fill(0);return reply
        }
        override fun close(handle: Long): Int {assertEquals(7,handle);closes++;return 0}
    }
    @Test fun `enrollment projections never become payment completion and retain exact originals`() {
        for ((status,size) in listOf(18 to 32,19 to 161,20 to 0,21 to 0,22 to 0,23 to 32,24 to 131072,25 to 262144,26 to 0)) {
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
        assertContentEquals(ByteArray(32){4},owner.begin(input,input,input));assertContentEquals(byteArrayOf(1,2),input)
        driver.reply=KagemushaWalletCallV1(20,-1,0,7,0,0,byteArrayOf())
        assertSame(KagemushaWalletEnrollmentProgressV1.Pending,owner.progress())
        assertFailsWith<KagemushaWalletExceptionV1>{owner.retainRequest(ByteArray(64))}
        driver.reply=KagemushaWalletCallV1(24,-1,0,7,0,0,byteArrayOf(9,8,7))
        assertContentEquals(byteArrayOf(9,8,7),assertIs<KagemushaWalletEnrollmentRequestV1.Retained>(owner.prepareRequest(listOf(byteArrayOf(1),byteArrayOf(2)),byteArrayOf(3))).bytes())
        val before=driver.calls;assertFailsWith<IllegalArgumentException>{owner.acceptCredential(ByteArray(262145))};assertEquals(before,driver.calls)
        owner.close();owner.close();assertEquals(1,driver.closes);assertFailsWith<KagemushaWalletExceptionV1>{owner.progress()}
    }
    @Test fun `native evidence target fields are immutable exact FFI slices`() {
        val bytes=ByteArray(161){it.toByte()};val target=KagemushaWalletEnrollmentTargetV1(bytes);bytes.fill(0)
        assertEquals(32,target.slot().size);assertEquals(65,target.paymentKey().size)
        assertEquals(97,target.challengeDigest()[0].toInt());assertEquals(129.toByte(),target.keyBindingDigest()[0])
        target.paymentKey().fill(0);assertEquals(32,target.paymentKey()[0].toInt())
    }
}
