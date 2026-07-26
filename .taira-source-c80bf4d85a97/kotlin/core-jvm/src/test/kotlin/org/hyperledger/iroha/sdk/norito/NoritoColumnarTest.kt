package org.hyperledger.iroha.sdk.norito

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class NoritoColumnarTest {
    @Test
    fun `optional string columnar matches Rust golden and rejects malformed payloads`() {
        val rows = listOf(
            NoritoColumnar.OptionalStringBoolRow(1L, "a", false),
            NoritoColumnar.OptionalStringBoolRow(2L, null, true),
            NoritoColumnar.OptionalStringBoolRow(3L, "bc", false),
        )

        val columnar = NoritoColumnar.encodeNcbU64OptionalStringBool(rows)
        assertEquals(
            "030000005b000000010000000000000002020500000000000000010000000300000061626302",
            columnar.toHex(),
        )
        assertEquals(rows, NoritoColumnar.decodeNcbU64OptionalStringBool(columnar))
        assertEquals(
            rows,
            NoritoColumnar.decodeRowsU64OptionalStringBoolAdaptive(
                NoritoColumnar.encodeRowsU64OptionalStringBoolAdaptive(rows),
            ),
        )
        assertEquals(rows, NoritoAoS.decodeU64OptionalStringBool(NoritoAoS.encodeU64OptionalStringBool(rows)))

        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.decodeNcbU64OptionalStringBool(columnar.withTrailing())
        }
        val badPresence = columnar.copyOf()
        badPresence[18] = (badPresence[18].toInt() or 0x08).toByte()
        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.decodeNcbU64OptionalStringBool(badPresence)
        }
        val badUtf8 = columnar.copyOf()
        badUtf8[34] = 0xFF.toByte()
        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.decodeNcbU64OptionalStringBool(badUtf8)
        }
    }

    @Test
    fun `optional u32 columnar matches Rust golden and rejects malformed AoS`() {
        val rows = listOf(
            NoritoColumnar.OptionalU32BoolRow(1L, 7L, false),
            NoritoColumnar.OptionalU32BoolRow(2L, null, true),
            NoritoColumnar.OptionalU32BoolRow(3L, 9L, false),
        )

        val columnar = NoritoColumnar.encodeNcbU64OptionalU32Bool(rows)
        assertEquals(
            "030000005c0000000100000000000000020205000000070000000900000002",
            columnar.toHex(),
        )
        assertEquals(rows, NoritoColumnar.decodeNcbU64OptionalU32Bool(columnar))
        assertEquals(
            rows,
            NoritoColumnar.decodeRowsU64OptionalU32BoolAdaptive(
                NoritoColumnar.encodeRowsU64OptionalU32BoolAdaptive(rows),
            ),
        )
        assertEquals(rows, NoritoAoS.decodeU64OptionalU32Bool(NoritoAoS.encodeU64OptionalU32Bool(rows)))

        val badAos = NoritoAoS.encodeU64OptionalU32Bool(rows)
        badAos[10] = 2
        assertFailsWith<IllegalArgumentException> {
            NoritoAoS.decodeU64OptionalU32Bool(badAos)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.OptionalU32BoolRow(1L, 0x1_0000_0000L, false)
        }
    }

    @Test
    fun `bytes bool columnar and adaptive layouts match Rust goldens`() {
        val rows = listOf(
            NoritoColumnar.BytesBoolRow(1L, byteArrayOf(0x61, 0x62, 0x63), true),
            NoritoColumnar.BytesBoolRow(2L, byteArrayOf(0x00, 0xFF.toByte()), false),
        )

        val columnar = NoritoColumnar.encodeNcbU64BytesBool(rows)
        assertEquals(
            "020000005400000001000000000000000200000000000000030000000500000061626300ff01",
            columnar.toHex(),
        )
        assertEquals(rows, NoritoColumnar.decodeNcbU64BytesBool(columnar))

        val adaptive = NoritoColumnar.encodeRowsU64BytesBoolAdaptive(rows)
        assertEquals("0002010100000000000000036162630102000000000000000200ff00", adaptive.toHex())
        assertEquals(rows, NoritoColumnar.decodeRowsU64BytesBoolAdaptive(adaptive))
        assertEquals(rows, NoritoAoS.decodeU64BytesBool(NoritoAoS.encodeU64BytesBool(rows)))

        val badFlags = columnar.copyOf()
        badFlags[badFlags.size - 1] = (badFlags.last().toInt() or 0x04).toByte()
        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.decodeNcbU64BytesBool(badFlags)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoColumnar.decodeNcbU64BytesBool(columnar.copyOf(columnar.size - 1))
        }
    }

    private fun ByteArray.withTrailing(): ByteArray = copyOf(size + 1).also { it[it.size - 1] = 0x55 }

    private fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it.toInt() and 0xFF) }
}
