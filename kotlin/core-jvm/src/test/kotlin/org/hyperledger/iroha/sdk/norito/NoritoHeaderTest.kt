package org.hyperledger.iroha.sdk.norito

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class NoritoHeaderTest {
    @Test
    fun `compressed frame keeps raw length checksum and layout`() {
        // Same standard Zstd raw block and CRC64 check vector as Python's
        // reference test. Native helper symbol names are not wire fields.
        val compressed = hex("28b52ffd2009490000313233343536373839")
        val fixedHeader = hex(
            "4e52543000000000000000000000000000000000000001" +
                "0900000000000000fa3919dfbbc95d9900",
        )
        for (flags in listOf(0, NoritoHeader.COMPACT_LEN)) {
            val expectedHeader = fixedHeader.copyOf()
            expectedHeader[expectedHeader.lastIndex] = flags.toByte()
            val decoded = NoritoHeader.decode(expectedHeader + compressed, null)
            assertContentEquals(expectedHeader, decoded.header.encode())
            assertEquals(NoritoHeader.COMPRESSION_ZSTD, decoded.header.compression)
            assertEquals(9, decoded.header.payloadLength)
            assertEquals(flags, decoded.header.flags)
            assertContentEquals(compressed, decoded.payload)
            val payload = NoritoCompression.decompressZstd(decoded.payload, decoded.header.payloadLength)
            assertContentEquals("123456789".toByteArray(Charsets.US_ASCII), payload)
            decoded.header.validateChecksum(payload)
            assertFailsWith<IllegalArgumentException> { decoded.header.validateChecksum(compressed) }
            assertFailsWith<IllegalArgumentException> {
                decoded.header.validateChecksum("123456780".toByteArray(Charsets.US_ASCII))
            }
        }
    }

    private fun hex(value: String): ByteArray =
        value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()

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
