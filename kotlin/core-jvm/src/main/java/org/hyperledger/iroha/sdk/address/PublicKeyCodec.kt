package org.hyperledger.iroha.sdk.address

import org.hyperledger.iroha.sdk.norito.Varint

class PublicKeyPayload(
    @JvmField val curveId: Int,
    keyBytes: ByteArray,
) {
    private val _keyBytes: ByteArray = keyBytes.copyOf()

    val keyBytes: ByteArray get() = _keyBytes.copyOf()
}

/**
 * Decodes a multihash public key literal into its curve id and payload bytes.
 * Returns `null` when the literal is not a valid multihash key.
 */
fun decodePublicKeyLiteral(literal: String?): PublicKeyPayload? {
    if (literal.isNullOrBlank()) return null
    var trimmed = literal.trim()
    val colonIndex = trimmed.indexOf(':')
    if (colonIndex > 0) {
        trimmed = trimmed.substring(colonIndex + 1)
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) return null
    if ((trimmed.length and 1) == 1) return null
    if (!trimmed.matches(Regex("(?i)[0-9a-f]+"))) return null
    val bytes = hexToBytes(trimmed)
    val code = Varint.decode(bytes, 0)
    val len = Varint.decode(bytes, code.nextOffset())
    if (len.value() > Int.MAX_VALUE) return null
    val payloadOffset = len.nextOffset()
    val payloadLength = len.value().toInt()
    if (payloadOffset + payloadLength != bytes.size) return null
    val curveId = curveIdForMultihashCode(code.value())
    if (curveId < 0) return null
    val keyBytes = bytes.copyOfRange(payloadOffset, payloadOffset + payloadLength)
    return PublicKeyPayload(curveId, keyBytes)
}

/** Encodes the multihash public key literal from the given curve id and key bytes. */
fun encodePublicKeyMultihash(curveId: Int, keyBytes: ByteArray): String {
    val codeVarint = Varint.encode(multihashCodeForCurveId(curveId))
    val lenVarint = Varint.encode(keyBytes.size.toLong())
    val builder = StringBuilder((codeVarint.size + lenVarint.size + keyBytes.size) * 2)
    appendHexLower(builder, codeVarint)
    appendHexLower(builder, lenVarint)
    appendHexUpper(builder, keyBytes)
    return builder.toString()
}

/** Returns the canonical algorithm label for the curve id, or `null` when unknown. */
fun algorithmForCurveId(curveId: Int): String? = when (curveId) {
    0x01 -> "ed25519"
    0x02 -> "ml-dsa"
    0x0A -> "gost256a"
    0x0B -> "gost256b"
    0x0C -> "gost256c"
    0x0D -> "gost512a"
    0x0E -> "gost512b"
    0x0F -> "sm2"
    else -> null
}

private fun curveIdForMultihashCode(code: Long): Int = when (code) {
    0xedL -> 0x01
    0xeeL -> 0x02
    0x1200L -> 0x0A
    0x1201L -> 0x0B
    0x1202L -> 0x0C
    0x1203L -> 0x0D
    0x1204L -> 0x0E
    0x1306L -> 0x0F
    else -> -1
}

private fun multihashCodeForCurveId(curveId: Int): Long = when (curveId) {
    0x01 -> 0xedL
    0x02 -> 0xeeL
    0x0A -> 0x1200L
    0x0B -> 0x1201L
    0x0C -> 0x1202L
    0x0D -> 0x1203L
    0x0E -> 0x1204L
    0x0F -> 0x1306L
    else -> throw IllegalArgumentException("Unsupported curve id: $curveId")
}

private fun hexToBytes(hex: String): ByteArray {
    val out = ByteArray(hex.length / 2)
    for (i in out.indices) {
        val high = Character.digit(hex[i * 2], 16)
        val low = Character.digit(hex[i * 2 + 1], 16)
        require(high >= 0 && low >= 0) { "Invalid hex literal" }
        out[i] = ((high shl 4) + low).toByte()
    }
    return out
}

private fun appendHexLower(builder: StringBuilder, bytes: ByteArray) {
    for (b in bytes) {
        builder.append(String.format("%02x", b.toInt() and 0xFF))
    }
}

private fun appendHexUpper(builder: StringBuilder, bytes: ByteArray) {
    for (b in bytes) {
        builder.append(String.format("%02X", b.toInt() and 0xFF))
    }
}
