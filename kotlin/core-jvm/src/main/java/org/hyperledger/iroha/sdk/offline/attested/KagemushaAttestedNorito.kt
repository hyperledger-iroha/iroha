// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * Canonical Norito framing for the attested-app suite.
 *
 * Mirrors the Rust `norito` derive layout with the default `COMPACT_LEN` flag: every struct
 * field is prefixed by its compact varint byte length, `[u8; N]` fields carry their raw bytes,
 * sequences carry a fixed little-endian u64 count followed by length-prefixed elements, options
 * carry a one-byte tag, and enums carry a little-endian u32 discriminant. No suite type needs an
 * alignment above eight, so the 40-byte header is never followed by padding.
 */
internal object KagemushaAttestedNorito {
    /** Explicit schema-name prefix shared by every suite type. */
    const val MODEL: String = "iroha_data_model::kagemusha::kagemusha_attested_v1::"

    /** Encode one canonical frame: header, then payload. */
    fun frame(schema: String, payload: ByteArray): ByteArray {
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return header + payload
    }

    /** Return the schema hash of a frame without validating the rest of it. */
    fun schemaHashOf(bytes: ByteArray): ByteArray? =
        if (bytes.size >= NoritoHeader.HEADER_LENGTH &&
            bytes.copyOfRange(0, 4).contentEquals(NoritoHeader.MAGIC)
        ) {
            bytes.copyOfRange(6, 22)
        } else {
            null
        }

    /**
     * Strictly unframe a canonical value: exact schema, no compression, `COMPACT_LEN` only, no
     * padding, exact payload length and a matching checksum.
     */
    fun unframe(bytes: ByteArray, schema: String, maximumBytes: Int): ByteArray {
        require(bytes.size in (NoritoHeader.HEADER_LENGTH + 1)..maximumBytes) {
            "KAGEMUSHA attested value is empty or exceeds $maximumBytes bytes"
        }
        val decoded = try {
            NoritoHeader.decode(bytes, SchemaHash.hash16(schema))
        } catch (failure: RuntimeException) {
            throw IllegalArgumentException("KAGEMUSHA attested value has an invalid Norito header", failure)
        }
        val header = decoded.header
        require(
            header.compression == NoritoHeader.COMPRESSION_NONE &&
                header.flags == NoritoHeader.COMPACT_LEN &&
                header.payloadLength == bytes.size - NoritoHeader.HEADER_LENGTH &&
                decoded.payload.size == header.payloadLength &&
                header.encode().contentEquals(bytes.copyOfRange(0, NoritoHeader.HEADER_LENGTH)),
        ) { "KAGEMUSHA attested value must use canonical compact Norito framing" }
        header.validateChecksum(decoded.payload)
        return decoded.payload
    }

    /** Decode a payload with [read], requiring every byte to be consumed. */
    fun <T> readPayload(payload: ByteArray, read: (NoritoIn) -> T): T {
        val input = NoritoIn(payload, 0, payload.size)
        val value = read(input)
        input.requireExhausted()
        return value
    }

    /** Encode a payload with [write]. */
    fun payload(write: (NoritoOut) -> Unit): ByteArray = NoritoOut().also(write).toByteArray()
}

/** Field-level writer that mirrors the Rust derive layout. */
internal class NoritoOut {
    private val buffer = ByteArrayOutputStream()

    fun toByteArray(): ByteArray = buffer.toByteArray()

    fun raw(bytes: ByteArray) {
        buffer.write(bytes, 0, bytes.size)
    }

    fun varint(value: Long) {
        require(value >= 0) { "Norito length must be non-negative" }
        var remaining = value
        while (true) {
            val bits = (remaining and 0x7f).toInt()
            remaining = remaining ushr 7
            if (remaining == 0L) {
                buffer.write(bits)
                return
            }
            buffer.write(bits or 0x80)
        }
    }

    fun le(value: Long, width: Int) {
        for (index in 0 until width) buffer.write((value ushr (8 * index)).toInt() and 0xff)
    }

    /** `u8` struct field. */
    fun u8(value: Int) {
        require(value in 0..0xff) { "u8 field is out of range" }
        varint(1)
        le(value.toLong(), 1)
    }

    /** `u16` struct field. */
    fun u16(value: Int) {
        require(value in 0..0xffff) { "u16 field is out of range" }
        varint(2)
        le(value.toLong(), 2)
    }

    /** `u32` struct field. */
    fun u32(value: Long) {
        require(value in 0..0xffff_ffffL) { "u32 field is out of range" }
        varint(4)
        le(value, 4)
    }

    /** `u64` struct field; the JVM value is interpreted as unsigned and must be non-negative. */
    fun u64(value: Long) {
        require(value >= 0) { "u64 field is outside the supported range" }
        varint(8)
        le(value, 8)
    }

    /** `bool` struct field. */
    fun bool(value: Boolean) {
        varint(1)
        buffer.write(if (value) 1 else 0)
    }

    /** `[u8; N]` struct field. */
    fun array(bytes: ByteArray, width: Int) {
        require(bytes.size == width) { "fixed byte field must be exactly $width bytes" }
        varint(width.toLong())
        raw(bytes)
    }

    /** `String` struct field. */
    fun string(value: String) {
        val utf8 = value.toByteArray(StandardCharsets.UTF_8)
        nested {
            it.varint(utf8.size.toLong())
            it.raw(utf8)
        }
    }

    /** Nested struct or enum field. */
    fun nested(write: (NoritoOut) -> Unit) {
        val child = NoritoOut().also(write).toByteArray()
        varint(child.size.toLong())
        raw(child)
    }

    /** Raw, already encoded field body. */
    fun opaque(fieldBody: ByteArray) {
        varint(fieldBody.size.toLong())
        raw(fieldBody)
    }

    /** `Vec<T>` struct field for a non-byte element type. */
    fun <T> vec(items: List<T>, writeElement: (NoritoOut, T) -> Unit) {
        nested { body ->
            body.le(items.size.toLong(), 8)
            items.forEach { item -> body.nested { element -> writeElement(element, item) } }
        }
    }

    /** `Option<T>` struct field. */
    fun <T : Any> option(value: T?, writeInner: (NoritoOut, T) -> Unit) {
        nested { body ->
            if (value == null) {
                body.raw(byteArrayOf(0))
            } else {
                body.raw(byteArrayOf(1))
                body.nested { inner -> writeInner(inner, value) }
            }
        }
    }

    /** Bare `[u8; N]` value (outside a struct field): every byte is a length-prefixed `u8`. */
    fun bareByteArray(bytes: ByteArray) {
        bytes.forEach { byte ->
            varint(1)
            buffer.write(byte.toInt() and 0xff)
        }
    }
}

/** Field-level strict reader that mirrors [NoritoOut]. */
internal class NoritoIn(
    private val bytes: ByteArray,
    private var position: Int,
    private val end: Int,
) {
    fun remaining(): Int = end - position

    fun requireExhausted() {
        require(position == end) { "KAGEMUSHA attested value has trailing bytes" }
    }

    fun raw(count: Int): ByteArray {
        require(count in 0..remaining()) { "KAGEMUSHA attested value is truncated" }
        val out = bytes.copyOfRange(position, position + count)
        position += count
        return out
    }

    fun varint(): Long {
        var result = 0L
        var shift = 0
        while (true) {
            require(position < end) { "KAGEMUSHA attested value has a truncated varint" }
            val byte = bytes[position++].toInt() and 0xff
            val chunk = byte and 0x7f
            require(shift < 63 || chunk <= 1) { "Norito varint exceeds 64 bits" }
            result = result or (chunk.toLong() shl shift)
            if (byte and 0x80 == 0) {
                require(shift == 0 || chunk != 0) { "Norito varint is not canonical" }
                require(result >= 0) { "Norito varint exceeds the supported range" }
                return result
            }
            shift += 7
            require(shift < 64) { "Norito varint exceeds 64 bits" }
        }
    }

    fun le(width: Int): Long {
        val raw = raw(width)
        var value = 0L
        for (index in 0 until width) value = value or ((raw[index].toLong() and 0xff) shl (8 * index))
        return value
    }

    private fun fieldLength(): Int {
        val length = varint()
        require(length <= remaining().toLong()) { "KAGEMUSHA attested field is truncated" }
        return length.toInt()
    }

    private fun exactField(width: Int): ByteArray {
        val length = fieldLength()
        require(length == width) { "KAGEMUSHA attested field has an unexpected width" }
        return raw(width)
    }

    fun u8(): Int = (exactField(1)[0].toInt() and 0xff)

    fun u16(): Int {
        val raw = exactField(2)
        return (raw[0].toInt() and 0xff) or ((raw[1].toInt() and 0xff) shl 8)
    }

    fun u32(): Long {
        val raw = exactField(4)
        var value = 0L
        for (index in 0 until 4) value = value or ((raw[index].toLong() and 0xff) shl (8 * index))
        return value
    }

    fun u64(): Long {
        val raw = exactField(8)
        var value = 0L
        for (index in 0 until 8) value = value or ((raw[index].toLong() and 0xff) shl (8 * index))
        require(value >= 0) { "u64 field exceeds the supported range" }
        return value
    }

    fun bool(): Boolean = when (exactField(1)[0].toInt()) {
        0 -> false
        1 -> true
        else -> throw IllegalArgumentException("KAGEMUSHA attested bool is not canonical")
    }

    fun array(width: Int): ByteArray = exactField(width)

    fun string(maximumBytes: Int): String = nested { body ->
        val length = body.varint()
        require(length <= maximumBytes.toLong() && length == body.remaining().toLong()) {
            "KAGEMUSHA attested string is malformed or too long"
        }
        val utf8 = body.raw(length.toInt())
        try {
            StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(utf8))
                .toString()
        } catch (failure: CharacterCodingException) {
            throw IllegalArgumentException("KAGEMUSHA attested string is not UTF-8", failure)
        }
    }

    fun <T> nested(read: (NoritoIn) -> T): T {
        val length = fieldLength()
        val child = NoritoIn(bytes, position, position + length)
        val value = read(child)
        child.requireExhausted()
        position += length
        return value
    }

    /** Return a field body without interpreting it. */
    fun opaque(maximumBytes: Int): ByteArray {
        val length = fieldLength()
        require(length <= maximumBytes) { "KAGEMUSHA attested field exceeds $maximumBytes bytes" }
        return raw(length)
    }

    fun <T> vec(maximumCount: Int, readElement: (NoritoIn) -> T): List<T> = nested { body ->
        val count = body.le(8)
        require(count in 0..maximumCount.toLong()) { "KAGEMUSHA attested sequence exceeds $maximumCount items" }
        List(count.toInt()) { body.nested(readElement) }
    }

    fun <T> option(readInner: (NoritoIn) -> T): T? = nested { body ->
        when (body.raw(1)[0].toInt()) {
            0 -> null
            1 -> body.nested(readInner)
            else -> throw IllegalArgumentException("KAGEMUSHA attested option tag is not canonical")
        }
    }

    fun bareByteArray(width: Int): ByteArray = ByteArray(width) {
        require(varint() == 1L) { "KAGEMUSHA attested byte element is malformed" }
        raw(1)[0]
    }
}
