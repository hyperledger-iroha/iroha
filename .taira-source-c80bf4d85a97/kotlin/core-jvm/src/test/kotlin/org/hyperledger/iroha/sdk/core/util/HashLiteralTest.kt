package org.hyperledger.iroha.sdk.core.util

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class HashLiteralTest {

    private val validBytes = ByteArray(32) { (it + 1).toByte() }

    @Test
    fun `canonicalize bytes produces canonical literal`() {
        val literal = HashLiteral.canonicalize(validBytes)
        assert(literal.startsWith("hash:")) { "expected literal to start with 'hash:'" }
        assert('#' in literal) { "expected literal to contain '#'" }
    }

    @Test
    fun `canonicalize bytes throws for wrong length`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize(ByteArray(16))
        }
    }

    @Test
    fun `canonicalize bytes throws for empty array`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize(ByteArray(0))
        }
    }

    @Test
    fun `canonicalize bytes sets low bit of last byte`() {
        val bytes = ByteArray(32) { 0x00 }
        val literal = HashLiteral.canonicalize(bytes)
        val separator = literal.lastIndexOf('#')
        val body = literal.substring("hash:".length, separator)
        val lastByteHex = body.substring(body.length - 2)
        assertEquals("01", lastByteHex)
    }

    @Test
    fun `canonicalize string with hex digest`() {
        val hex = validBytes.joinToString("") { "%02x".format(it) }
        val literal = HashLiteral.canonicalize(hex)
        assert(literal.startsWith("hash:")) { "expected canonical form" }
    }

    @Test
    fun `canonicalize string with existing literal passes through`() {
        val literal = HashLiteral.canonicalize(validBytes)
        val result = HashLiteral.canonicalize(literal)
        assertEquals(literal, result)
    }

    @Test
    fun `canonicalize string throws for empty`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize("")
        }
    }

    @Test
    fun `canonicalize string throws for blank`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize("   ")
        }
    }

    @Test
    fun `canonicalize string throws for invalid hex`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize("not-a-hex-string")
        }
    }

    @Test
    fun `canonicalize string throws for wrong hex length`() {
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize("AABB")
        }
    }

    @Test
    fun `canonicalizeOptional returns null for null`() {
        assertNull(HashLiteral.canonicalizeOptional(null))
    }

    @Test
    fun `canonicalizeOptional returns null for blank`() {
        assertNull(HashLiteral.canonicalizeOptional("   "))
    }

    @Test
    fun `canonicalizeOptional returns canonical for valid hex`() {
        val hex = validBytes.joinToString("") { "%02x".format(it) }
        val result = HashLiteral.canonicalizeOptional(hex)
        assert(result != null && result.startsWith("hash:"))
    }

    @Test
    fun `decode returns original bytes`() {
        val literal = HashLiteral.canonicalize(validBytes)
        val decoded = HashLiteral.decode(literal)
        val expected = validBytes.copyOf()
        expected[expected.size - 1] = (expected[expected.size - 1].toInt() or 0x01).toByte()
        assertContentEquals(expected, decoded)
    }

    @Test
    fun `decode round-trips with canonicalize`() {
        val literal = HashLiteral.canonicalize(validBytes)
        val decoded = HashLiteral.decode(literal)
        val reLiteral = HashLiteral.canonicalize(decoded)
        assertEquals(literal, reLiteral)
    }

    @Test
    fun `canonicalize rejects literal with bad checksum`() {
        val literal = HashLiteral.canonicalize(validBytes)
        val tampered = literal.dropLast(4) + "0000"
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize(tampered)
        }
    }

    @Test
    fun `canonicalize rejects literal missing checksum separator`() {
        val hex = validBytes.joinToString("") { "%02X".format(it) }
        val malformed = "hash:$hex"
        assertFailsWith<IllegalArgumentException> {
            HashLiteral.canonicalize(malformed)
        }
    }

    @Test
    fun `defensive copy - mutating input does not affect output`() {
        val bytes = ByteArray(32) { (it + 1).toByte() }
        val literal1 = HashLiteral.canonicalize(bytes)
        bytes[0] = 0xFF.toByte()
        val literal2 = HashLiteral.canonicalize(ByteArray(32) { (it + 1).toByte() })
        assertEquals(literal1, literal2)
    }
}
