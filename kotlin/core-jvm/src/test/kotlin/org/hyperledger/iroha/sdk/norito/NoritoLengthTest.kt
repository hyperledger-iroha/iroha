package org.hyperledger.iroha.sdk.norito

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class NoritoLengthTest {
    @Test
    fun `fixed lengths must fit the JVM buffer range`() {
        assertEquals(0L, fixedLength(0L).decoder().readLength(false))
        assertEquals(4L, fixedLength(4L).decoder().readLength(false))
        assertEquals(
            Int.MAX_VALUE.toLong(),
            fixedLength(Int.MAX_VALUE.toLong()).decoder().readLength(false),
        )

        assertFailsWith<IllegalArgumentException> {
            fixedLength(0x1_0000_0004L).decoder().readLength(false)
        }
        assertFailsWith<IllegalArgumentException> {
            byteArrayOf(4, 0, 0, 0, 0, 0, 0, 0x80.toByte()).decoder().readLength(false)
        }
    }

    @Test
    fun `compact lengths must fit the JVM buffer range`() {
        assertEquals(0L, byteArrayOf(0).decoder().readLength(true))
        assertEquals(4L, byteArrayOf(4).decoder().readLength(true))
        assertEquals(
            Int.MAX_VALUE.toLong(),
            byteArrayOf(
                0xFF.toByte(),
                0xFF.toByte(),
                0xFF.toByte(),
                0xFF.toByte(),
                0x07,
            ).decoder().readLength(true),
        )

        assertFailsWith<IllegalArgumentException> {
            byteArrayOf(
                0x84.toByte(),
                0x80.toByte(),
                0x80.toByte(),
                0x80.toByte(),
                0x10,
            ).decoder().readLength(true)
        }
    }

    @Test
    fun `raw byte vector rejects a high-bit length that aliases its payload size`() {
        val forged = byteArrayOf(4, 0, 0, 0, 0, 0, 0, 0x80.toByte(), 1, 2, 3, 4)

        assertFailsWith<IllegalArgumentException> {
            NoritoAdapters.rawByteVecAdapter().decode(forged.decoder())
        }

        val canonical = fixedLength(4L) + byteArrayOf(1, 2, 3, 4)
        assertContentEquals(
            byteArrayOf(1, 2, 3, 4),
            NoritoAdapters.rawByteVecAdapter().decode(canonical.decoder()),
        )
    }

    private fun ByteArray.decoder(): NoritoDecoder = NoritoDecoder(this, 0)

    private fun fixedLength(value: Long): ByteArray = ByteArray(Long.SIZE_BYTES) { index ->
        (value ushr (index * Byte.SIZE_BITS)).toByte()
    }
}
