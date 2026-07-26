package org.hyperledger.iroha.sdk.norito

import kotlin.test.Test
import kotlin.test.assertFailsWith

class NoritoHeaderTest {
    @Test
    fun `decode rejects reserved layout flags`() {
        val payload = byteArrayOf(1)
        val checksum = CRC64.compute(payload)
        val cases = intArrayOf(
            NoritoHeader.VARINT_OFFSETS,
            NoritoHeader.COMPACT_SEQ_LEN,
            NoritoHeader.VARINT_OFFSETS or NoritoHeader.COMPACT_SEQ_LEN,
        )

        for (flags in cases) {
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader(ByteArray(16), payload.size, checksum, flags, NoritoHeader.COMPRESSION_NONE)
            }
            val framed = frameWithUncheckedFlags(payload, checksum, flags)
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader.decode(framed, null)
            }
        }
    }

    @Test
    fun `decode rejects field bitset without required flags`() {
        val payload = byteArrayOf(1)
        val checksum = CRC64.compute(payload)
        val cases = intArrayOf(
            NoritoHeader.FIELD_BITSET,
            NoritoHeader.FIELD_BITSET or NoritoHeader.COMPACT_LEN,
            NoritoHeader.FIELD_BITSET or NoritoHeader.PACKED_STRUCT,
        )

        for (flags in cases) {
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader(ByteArray(16), payload.size, checksum, flags, NoritoHeader.COMPRESSION_NONE)
            }
            val framed = frameWithUncheckedFlags(payload, checksum, flags)
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader.decode(framed, null)
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
