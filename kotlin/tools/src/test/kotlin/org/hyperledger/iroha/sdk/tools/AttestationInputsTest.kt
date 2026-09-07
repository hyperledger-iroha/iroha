package org.hyperledger.iroha.sdk.tools

import java.io.ByteArrayInputStream
import java.io.InputStream
import org.junit.jupiter.api.Assertions.assertArrayEquals
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test

/** Reader limits apply before accepting bytes, including malformed stream implementations. */
class AttestationInputsTest {
    @Test
    fun zeroProgressCannotSpinForever() {
        val zeroProgress = object : InputStream() {
            override fun read(): Int = 0
            override fun read(bytes: ByteArray, offset: Int, length: Int): Int = 0
        }
        val failure = assertThrows(IllegalArgumentException::class.java) {
            AttestationInputs().readBounded(zeroProgress, 10)
        }
        assertEquals("Attestation input stream made no progress", failure.message)
    }

    @Test
    fun exactPerFileLimitIsAcceptedAndNextByteIsRejected() {
        val bytes = byteArrayOf(1, 2, 3)
        assertArrayEquals(bytes, AttestationInputs().readBounded(ByteArrayInputStream(bytes), 3))
        assertThrows(IllegalArgumentException::class.java) {
            AttestationInputs().readBounded(ByteArrayInputStream(bytes), 2)
        }
    }

    @Test
    fun aggregateBudgetIncludesEarlierFiles() {
        val reader = AttestationInputs()
        val bytes = ByteArray(1024 * 1024)
        repeat(8) { reader.readBounded(ByteArrayInputStream(bytes), bytes.size) }
        assertThrows(IllegalArgumentException::class.java) {
            reader.readBounded(ByteArrayInputStream(byteArrayOf(1)), 1)
        }
    }
}
