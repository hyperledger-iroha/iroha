package org.hyperledger.iroha.sdk.norito

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class NoritoHeaderTest {
    @Test
    fun `header accepts only the default and compact length flag bytes`() {
        val payload = byteArrayOf(1)
        val checksum = CRC64.compute(payload)

        for (flags in 0..0xFF) {
            val framed = frameWithUncheckedFlags(payload, checksum, flags)
            if (flags == 0 || flags == NoritoHeader.COMPACT_LEN) {
                val header =
                    NoritoHeader(ByteArray(16), payload.size, checksum, flags, NoritoHeader.COMPRESSION_NONE)
                assertEquals(flags, header.flags)
                assertEquals(flags, NoritoHeader.decode(framed, null).header.flags)
            } else {
                assertFailsWith<IllegalArgumentException>("flags 0x${"%02x".format(flags)}") {
                    NoritoHeader(ByteArray(16), payload.size, checksum, flags, NoritoHeader.COMPRESSION_NONE)
                }
                assertFailsWith<IllegalArgumentException>("flags 0x${"%02x".format(flags)}") {
                    NoritoHeader.decode(framed, null)
                }
            }
        }
    }

    private fun frameWithUncheckedFlags(payload: ByteArray, checksum: Long, flags: Int): ByteArray {
        val header = NoritoHeader(
            ByteArray(16),
            payload.size,
            checksum,
            0,
            NoritoHeader.COMPRESSION_NONE,
        )
        val encoded = header.encode()
        encoded[NoritoHeader.HEADER_LENGTH - 1] = (flags and 0xFF).toByte()
        return encoded + payload
    }
}
