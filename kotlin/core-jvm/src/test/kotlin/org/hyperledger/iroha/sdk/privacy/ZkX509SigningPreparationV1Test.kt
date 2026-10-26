package org.hyperledger.iroha.sdk.privacy

import java.security.MessageDigest
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Wrapper bounds, ownership and unavailable-native behavior require no test-only production API. */
class ZkX509SigningPreparationV1Test {
    @Test fun unavailableAndMissingSymbolsFailWithoutProducingSigningBytes() {
        var calls = 0
        val unavailable = assertThrows(ZkX509SigningPreparationException::class.java) {
            ZkX509SigningPreparationV1.prepareWithNative(byteArrayOf(1), { false }) { calls++; null }
        }
        assertEquals(-101, unavailable.code)
        assertEquals(0, calls)
        val missing = assertThrows(ZkX509SigningPreparationException::class.java) {
            ZkX509SigningPreparationV1.prepareWithNative(byteArrayOf(1), { true }) {
                throw UnsatisfiedLinkError("missing X509 symbol")
            }
        }
        assertEquals(-101, missing.code)
    }

    @Test fun localBoundsRejectBeforeCallingNativeAndNativeErrorsRemainTyped() {
        for ((input, code) in listOf(ByteArray(0) to -2, ByteArray(ZkX509SigningPreparationV1.MAX_STATEMENT_BYTES + 1) to -3)) {
            val error = assertThrows(ZkX509SigningPreparationException::class.java) {
                ZkX509SigningPreparationV1.prepareWithNative(input, { true }) { error("must not call native") }
            }
            assertEquals(code, error.code)
        }
        val failure = ZkX509SigningPreparationException(-5)
        assertSame(failure, assertThrows(ZkX509SigningPreparationException::class.java) {
            ZkX509SigningPreparationV1.prepareWithNative(byteArrayOf(1), { true }) { throw failure }
        })
    }

    @Test fun resultCopiesAndValidatesBothNativeArrays() {
        val original = byteArrayOf(4, 5, 6)
        val message = byteArrayOf(7, 8, 9)
        val digest = MessageDigest.getInstance("SHA-256").digest(message)
        val expectedMessage = message.clone()
        val expectedDigest = digest.clone()
        val result = ZkX509SigningPreparationV1.prepareWithNative(original, { true }) {
            it.fill(0)
            arrayOf(message, digest)
        }
        assertArrayEquals(byteArrayOf(4, 5, 6), original)
        message.fill(0); digest.fill(0)
        result.message.fill(0); result.digest.fill(0)
        assertArrayEquals(expectedMessage, result.message)
        assertArrayEquals(expectedDigest, result.digest)
        for (outputs in listOf<Array<ByteArray?>?>(
            null, emptyArray(), arrayOf(null, expectedDigest), arrayOf(expectedMessage, null),
            arrayOf(ByteArray(0), expectedDigest), arrayOf(expectedMessage, ByteArray(31)),
            arrayOf(expectedMessage, ByteArray(32)), arrayOf(expectedMessage, expectedDigest, expectedDigest),
            arrayOf(ByteArray(ZkX509SigningPreparationV1.MAX_STATEMENT_BYTES + 1), expectedDigest),
        )) {
            val failure = assertThrows(ZkX509SigningPreparationException::class.java) {
                ZkX509SigningPreparationV1.prepareWithNative(original, { true }) { outputs }
            }
            assertEquals(-102, failure.code)
        }
    }
}
