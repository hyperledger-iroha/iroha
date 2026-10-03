package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.*

/** TEST ONLY data/framing. No test constructor or receipt is a protected Native origin. */
class KagemushaExistingAndroidAccountIntakeTest {
    @Test fun borrowedProtectedLoanIsDrainedAfterConsumption() {
        val seed = ByteArray(32) { 7 }
        consumeExistingAndroidAccountLoan("S_TEST_ONLY", seed) { offered -> assertSame(seed, offered); true }
        assertTrue(seed.all { it == 0.toByte() })
    }
    @Test fun refusedOrThrowingNativeConsumptionDrainsSeed() {
        for (throws in listOf(false, true)) {
            val seed = ByteArray(32) { 11 }
            try { consumeExistingAndroidAccountLoan("S_TEST_ONLY", seed) { if (throws) error("test refusal") else false }; fail("must refuse") }
            catch (_: IllegalStateException) {}
            assertTrue(seed.all { it == 0.toByte() })
        }
    }
    @Test fun malformedLoanNeverInvokesNativeAndIsDrained() {
        val seed = ByteArray(33) { 9 }; var called = false
        try { consumeExistingAndroidAccountLoan("S_TEST_ONLY", seed) { called = true; true }; fail("must refuse") }
        catch (_: IllegalArgumentException) {}
        assertFalse(called); assertTrue(seed.all { it == 0.toByte() })
    }
    @Test fun nativeReceiptsAreStrictAndNeverUpgradeEqualSAndW() {
        for (reply in arrayOf<Array<ByteArray>?>(null,
            arrayOf(byteArrayOf(2, 0), "S_TEST_ONLY".toByteArray(), "W_TEST_ONLY".toByteArray()),
            arrayOf(byteArrayOf(1, 0), "S_TEST_ONLY".toByteArray(), "S_TEST_ONLY".toByteArray()),
            arrayOf(byteArrayOf(1, 0), byteArrayOf(0xff.toByte()), "W_TEST_ONLY".toByteArray()))) {
            try { requireExistingAndroidAccountReceipt(reply); fail("must refuse") }
            catch (_: IllegalStateException) {} catch (_: IllegalArgumentException) {}
            catch (_: java.nio.charset.CharacterCodingException) {}
        }
        requireExistingAndroidAccountReceipt(arrayOf(byteArrayOf(1, 0), "S_TEST_ONLY_ﾛ".toByteArray(), "W_TEST_ONLY_ﾊ".toByteArray()))
    }
    @Test fun malformedSignatoryCannotDispatchOrRetainSecretBytes() {
        for (name in listOf("", " S_TEST_ONLY", "S_TEST_ONLY\n", "S TEST_ONLY", "S\u0000")) {
            val seed = ByteArray(32) { 13 }; var called = false
            try { consumeExistingAndroidAccountLoan(name, seed) { called = true; true }; fail("must refuse") }
            catch (_: IllegalArgumentException) {}
            assertFalse(called); assertTrue(seed.all { it == 0.toByte() })
        }
    }
}
