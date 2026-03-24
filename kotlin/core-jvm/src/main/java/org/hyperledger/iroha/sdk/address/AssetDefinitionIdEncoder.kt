// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

import org.hyperledger.iroha.sdk.crypto.Blake3

private const val AID_PREFIX = "aid:"
private const val AID_BYTES_LEN = 16
private const val AID_HEX_LEN = AID_BYTES_LEN * 2

/**
 * Computes canonical `aid:` format asset definition identifiers.
 *
 * Iroha now represents `AssetDefinitionId` as a 16-byte hash derived from
 * the legacy `"name#domain"` format. The algorithm (verified against
 * `iroha_data_model/src/asset/id.rs`):
 *
 * 1. BLAKE3(`"name#domain"`) -> 32 bytes
 * 2. Take first 16 bytes
 * 3. Set UUIDv4 version bits: `bytes[6] = (bytes[6] & 0x0F) | 0x40`
 * 4. Set RFC 4122 variant bits: `bytes[8] = (bytes[8] & 0x3F) | 0x80`
 * 5. Result: `"aid:" + lowercase_hex(16_bytes)`
 */
object AssetDefinitionIdEncoder {

    /**
     * Computes the canonical `aid:` identifier from asset name and domain.
     *
     * @param name   the asset name (e.g., "pkr")
     * @param domain the domain name (e.g., "sbp")
     * @return the canonical identifier (e.g., "aid:abcdef0123456789...")
     */
    @JvmStatic
    fun encode(name: String, domain: String): String {
        val aidBytes = computeAidBytes(name, domain)
        return AID_PREFIX + bytesToHex(aidBytes)
    }

    /**
     * Computes the raw 16-byte `aid` bytes from asset name and domain.
     *
     * @param name   the asset name
     * @param domain the domain name
     * @return 16-byte array with UUIDv4 version and RFC 4122 variant bits set
     */
    @JvmStatic
    fun computeAidBytes(name: String, domain: String): ByteArray {
        val input = "$name#$domain".toByteArray(Charsets.UTF_8)
        val hash = Blake3.hash(input)

        val aidBytes = hash.copyOf(AID_BYTES_LEN)

        // UUIDv4 version bits
        aidBytes[6] = ((aidBytes[6].toInt() and 0x0F) or 0x40).toByte()
        // RFC 4122 variant bits
        aidBytes[8] = ((aidBytes[8].toInt() and 0x3F) or 0x80).toByte()

        return aidBytes
    }

    /**
     * Checks whether the given string is in `aid:` format.
     *
     * @param value the string to test
     * @return `true` if the string starts with `aid:` and has 32 hex characters
     */
    @JvmStatic
    fun isAidEncoded(value: String?): Boolean {
        if (value == null || !value.startsWith(AID_PREFIX)) return false
        val hex = value.substring(AID_PREFIX.length)
        if (hex.length != AID_HEX_LEN) return false
        return hex.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }
    }

    /**
     * Parses the 16 raw bytes from an `aid:<hex>` string.
     *
     * @param aidString the `aid:` prefixed string
     * @return 16-byte array
     * @throws IllegalArgumentException if the format is invalid
     */
    @JvmStatic
    fun parseAidBytes(aidString: String): ByteArray {
        require(isAidEncoded(aidString)) { "Invalid aid: format: $aidString" }
        val hex = aidString.substring(AID_PREFIX.length)
        val bytes = hexToBytes(hex)
        require(bytes[6].toInt() and 0xF0 == 0x40) {
            "aid: bytes lack UUIDv4 version nibble (byte 6): $aidString"
        }
        require(bytes[8].toInt() and 0xC0 == 0x80) {
            "aid: bytes lack RFC 4122 variant bits (byte 8): $aidString"
        }
        return bytes
    }

    private fun bytesToHex(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (b in bytes) {
            append("%02x".format(b.toInt() and 0xFF))
        }
    }

    private fun hexToBytes(hex: String): ByteArray {
        val bytes = ByteArray(hex.length / 2)
        for (i in bytes.indices) {
            val hi = Character.digit(hex[i * 2], 16)
            val lo = Character.digit(hex[i * 2 + 1], 16)
            bytes[i] = ((hi shl 4) or lo).toByte()
        }
        return bytes
    }
}
