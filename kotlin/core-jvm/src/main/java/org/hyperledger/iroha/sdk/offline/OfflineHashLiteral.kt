package org.hyperledger.iroha.sdk.offline

private const val BODY_LENGTH = 64
private const val CHECKSUM_LENGTH = 4

object OfflineHashLiteral {

    fun normalize(value: String, context: String): String {
        val hex = parseHex(value, context)
        return format(hex)
    }

    fun parseHex(value: String?, context: String): String {
        require(!value.isNullOrBlank()) { "$context must be a non-empty hash literal or hex" }
        val trimmed = value.trim()
        if (trimmed.lowercase().startsWith("hash:")) {
            return parseHashLiteral(trimmed, context)
        }
        return normalizeHex(trimmed, context)
    }

    private fun parseHashLiteral(literal: String, context: String): String {
        val separator = literal.lastIndexOf('#')
        require(separator >= 0) { "$context missing hash literal checksum" }
        val body = literal.substring(5, separator)
        val checksum = literal.substring(separator + 1)
        require(body.length == BODY_LENGTH && isHex(body)) { "$context hash literal has invalid body" }
        require(checksum.length == CHECKSUM_LENGTH && isHex(checksum)) { "$context hash literal has invalid checksum" }
        val expected = crc16("hash", body.uppercase())
        require(expected.equals(checksum, ignoreCase = true)) {
            "$context hash literal checksum mismatch (expected $expected)"
        }
        return body.lowercase()
    }

    private fun normalizeHex(value: String, context: String): String {
        val hex = if (value.startsWith("0x") || value.startsWith("0X")) value.substring(2) else value
        require(hex.length == BODY_LENGTH && isHex(hex)) { "$context must be a 32-byte hex string" }
        return hex.lowercase()
    }

    private fun isHex(value: String): Boolean {
        for (c in value) {
            val isDigit = c in '0'..'9'
            val isLower = c in 'a'..'f'
            val isUpper = c in 'A'..'F'
            if (!(isDigit || isLower || isUpper)) return false
        }
        return true
    }

    private fun format(hex: String): String {
        val upper = hex.uppercase()
        val checksum = crc16("hash", upper)
        return "hash:$upper#$checksum"
    }

    private fun crc16(tag: String, body: String): String {
        var crc = 0xffff
        crc = updateCrc(crc, tag)
        crc = updateCrc(crc, ":")
        crc = updateCrc(crc, body)
        return String.format("%04X", crc and 0xffff)
    }

    private fun updateCrc(crc: Int, text: String): Int {
        var next = crc
        for (c in text) {
            next = updateCrcByte(next, c.code.toByte())
        }
        return next
    }

    private fun updateCrcByte(crc: Int, value: Byte): Int {
        var current = crc xor ((value.toInt() and 0xff) shl 8)
        for (i in 0 until 8) {
            current = if ((current and 0x8000) != 0) {
                (current shl 1) xor 0x1021
            } else {
                current shl 1
            }
        }
        return current and 0xffff
    }
}
