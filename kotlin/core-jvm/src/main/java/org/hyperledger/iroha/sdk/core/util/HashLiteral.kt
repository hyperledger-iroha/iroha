package org.hyperledger.iroha.sdk.core.util

private const val TAG = "hash"
private const val HASH_BYTES = 32
private const val BODY_HEX_LENGTH = 64
private const val CHECKSUM_HEX_LENGTH = 4

/** Canonicalises and validates Norito hash literals (`hash:...#....`). */
object HashLiteral {

    /** Returns the canonical literal for the provided 32-byte hash. */
    @JvmStatic
    fun canonicalize(bytes: ByteArray): String {
        require(bytes.size == HASH_BYTES) { "hash literal requires exactly 32 bytes" }
        val normalized = bytes.copyOf()
        normalized[normalized.size - 1] = (normalized[normalized.size - 1].toInt() or 0x01).toByte()
        val body = normalized.joinToString("") { "%02X".format(it) }
        val checksum = crc16(TAG, body)
        return literalFor(body, checksum)
    }

    /** Canonicalises either a hex digest or an existing literal. */
    @JvmStatic
    fun canonicalize(value: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "hash literal must not be empty" }
        if (trimmed.startsWith("$TAG:", ignoreCase = true)) {
            validate(trimmed)
            return trimmed
        }
        require(trimmed.matches(Regex("^[0-9A-Fa-f]{$BODY_HEX_LENGTH}$"))) {
            "hash literal must be a 64-character hexadecimal string or canonical hash literal"
        }
        return canonicalize(hexToBytes(trimmed))
    }

    @JvmStatic
    fun canonicalizeOptional(value: String?): String? {
        if (value.isNullOrBlank()) return null
        return canonicalize(value)
    }

    /** Returns the decoded bytes after validating the literal. */
    @JvmStatic
    fun decode(literal: String): ByteArray {
        val canonical = canonicalize(literal)
        val separator = canonical.lastIndexOf('#')
        val body = canonical.substring(TAG.length + 1, separator)
        return hexToBytes(body)
    }
}

private fun validate(literal: String) {
    val trimmed = literal.trim()
    require(trimmed.startsWith("$TAG:", ignoreCase = true)) {
        "hash literal must start with 'hash:'"
    }
    val separator = trimmed.lastIndexOf('#')
    require(separator >= 0) { "hash literal must include checksum suffix" }
    val body = trimmed.substring(TAG.length + 1, separator)
    val checksum = trimmed.substring(separator + 1)
    require(body.matches(Regex("^[0-9A-Fa-f]{$BODY_HEX_LENGTH}$"))) {
        "hash literal must contain 64 hexadecimal digits"
    }
    require(checksum.matches(Regex("^[0-9A-Fa-f]{$CHECKSUM_HEX_LENGTH}$"))) {
        "hash literal checksum must contain four hexadecimal digits"
    }
    val expected = crc16(TAG, body.uppercase())
    val provided = checksum.toInt(16)
    require(expected == provided) {
        "hash literal checksum mismatch; expected %04X".format(expected)
    }
}

private fun literalFor(body: String, checksum: Int): String =
    "$TAG:$body#${"%0${CHECKSUM_HEX_LENGTH}X".format(checksum and 0xFFFF)}"

private fun hexToBytes(hex: String): ByteArray {
    val bytes = ByteArray(HASH_BYTES)
    for (index in bytes.indices) {
        val offset = index * 2
        bytes[index] = hex.substring(offset, offset + 2).toInt(16).toByte()
    }
    return bytes
}

private fun crc16(tag: String, body: String): Int {
    var crc = 0xFFFF
    for (value in tag.toByteArray(Charsets.UTF_8)) {
        crc = updateCrc(crc, value)
    }
    crc = updateCrc(crc, ':'.code.toByte())
    for (value in body.toByteArray(Charsets.UTF_8)) {
        crc = updateCrc(crc, value)
    }
    return crc and 0xFFFF
}

private fun updateCrc(crc: Int, value: Byte): Int {
    var c = crc xor ((value.toInt() and 0xFF) shl 8)
    for (bit in 0 until 8) {
        c = if ((c and 0x8000) != 0) {
            ((c shl 1) xor 0x1021) and 0xFFFF
        } else {
            (c shl 1) and 0xFFFF
        }
    }
    return c
}
