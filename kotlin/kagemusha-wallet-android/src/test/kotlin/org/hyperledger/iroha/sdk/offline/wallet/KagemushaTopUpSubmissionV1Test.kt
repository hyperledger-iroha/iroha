package org.hyperledger.iroha.sdk.offline.wallet

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class KagemushaTopUpSubmissionV1Test {
    @Test fun validatorReceivesEntireOriginalBytes() {
        val signed = byteArrayOf(1, 2, 3); val request = byteArrayOf(4, 5, 6, 7)
        var calls = 0
        val owned = TopUpSubmissionBytesV1(signed, request) { tx, intent ->
            calls++; assertArrayEquals(signed, tx); assertArrayEquals(request, intent)
        }
        assertEquals(1, calls); assertArrayEquals(signed, owned.signedTransaction())
        assertArrayEquals(request, owned.canonicalRequest())
    }
    @Test fun inputMutationDuringValidationCannotReplaceOwnedBytes() {
        val signed = byteArrayOf(1, 2, 3); val request = byteArrayOf(4, 5, 6)
        val owned = TopUpSubmissionBytesV1(signed, request) { tx, intent ->
            signed[0] = 9; request[0] = 8
            assertArrayEquals(byteArrayOf(1, 2, 3), tx); assertArrayEquals(byteArrayOf(4, 5, 6), intent)
        }
        assertArrayEquals(byteArrayOf(1, 2, 3), owned.signedTransaction())
        assertArrayEquals(byteArrayOf(4, 5, 6), owned.canonicalRequest())
    }
    @Test fun validatorCannotMutateRetainedBytes() {
        val owned = TopUpSubmissionBytesV1(byteArrayOf(1, 2), byteArrayOf(3, 4)) { tx, intent ->
            tx[0] = 9; intent[0] = 9
        }
        assertArrayEquals(byteArrayOf(1, 2), owned.signedTransaction())
        assertArrayEquals(byteArrayOf(3, 4), owned.canonicalRequest())
    }
    @Test fun returnedBytesCannotMutatePreparedSubmission() {
        val owned = TopUpSubmissionBytesV1(byteArrayOf(1, 2), byteArrayOf(3, 4)) { _, _ -> }
        owned.signedTransaction()[0] = 9; owned.canonicalRequest()[0] = 9
        assertArrayEquals(byteArrayOf(1, 2), owned.signedTransaction())
        assertArrayEquals(byteArrayOf(3, 4), owned.canonicalRequest())
    }
    @Test fun rejectedValidationCannotProducePreparedBytes() {
        val rejection = IllegalStateException("verification rejected")
        val thrown = assertThrows(IllegalStateException::class.java) {
            TopUpSubmissionBytesV1(byteArrayOf(1), byteArrayOf(2)) { _, _ -> throw rejection }
        }
        assertSame(rejection, thrown)
    }
    @Test fun invalidInputsStopBeforeNativeValidation() {
        val pairs = listOf(byteArrayOf() to byteArrayOf(1), byteArrayOf(1) to byteArrayOf(),
            ByteArray(16 * 1024 * 1024 + 1) to byteArrayOf(1), byteArrayOf(1) to ByteArray(16 * 1024 + 1))
        for ((signed, request) in pairs) {
            var called = false
            assertThrows(IllegalArgumentException::class.java) { TopUpSubmissionBytesV1(signed, request) { _, _ -> called = true } }
            assertFalse(called)
        }
    }
}
