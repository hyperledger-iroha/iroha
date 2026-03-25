// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

import java.math.BigInteger
import java.util.Arrays
import org.hyperledger.iroha.sdk.crypto.Blake3

private const val ADDRESS_VERSION = 1
private const val UUID_BYTES_LEN = 16
private const val CHECKSUM_LEN = 4
private const val ADDRESS_PAYLOAD_LEN = 1 + UUID_BYTES_LEN + CHECKSUM_LEN
private val BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".toCharArray()
private val BASE58_INDEX = IntArray(128) { -1 }.also { index ->
    for (i in BASE58_ALPHABET.indices) {
        index[BASE58_ALPHABET[i].code] = i
    }
}
private val BASE_58: BigInteger = BigInteger.valueOf(58L)

/**
 * Computes and validates canonical unprefixed Base58 asset-definition addresses.
 *
 * Iroha represents `AssetDefinitionId` as canonical UUIDv4 bytes wrapped in a versioned
 * Base58 address with a BLAKE3 checksum:
 *
 * 1. BLAKE3(`"name#domain"`) -> 32 bytes
 * 2. Take the first 16 bytes
 * 3. Set UUIDv4 version bits
 * 4. Set RFC 4122 variant bits
 * 5. Wrap as `version_byte + 16 uuid bytes + 4-byte BLAKE3 checksum`
 * 6. Encode the 21-byte payload as unprefixed Base58
 */
object AssetDefinitionIdEncoder {

    /**
     * Computes the canonical asset-definition address from asset name and domain.
     */
    @JvmStatic
    fun encode(name: String, domain: String): String =
        encodeFromBytes(computeDefinitionBytes(name, domain))

    /**
     * Wraps canonical UUIDv4 bytes as a canonical unprefixed Base58 address.
     */
    @JvmStatic
    fun encodeFromBytes(definitionBytes: ByteArray): String {
        require(definitionBytes.size == UUID_BYTES_LEN) {
            "Asset definition bytes must be exactly 16 bytes: ${definitionBytes.size}"
        }
        require((definitionBytes[6].toInt() and 0xF0) == 0x40) {
            "Asset definition bytes must encode UUIDv4 version bits"
        }
        require((definitionBytes[8].toInt() and 0xC0) == 0x80) {
            "Asset definition bytes must encode RFC 4122 variant bits"
        }
        return encodeBase58(addressPayload(definitionBytes))
    }

    /**
     * Computes the raw 16-byte UUIDv4 payload from asset name and domain.
     */
    @JvmStatic
    fun computeDefinitionBytes(name: String, domain: String): ByteArray {
        val input = "$name#$domain".toByteArray(Charsets.UTF_8)
        val hash = Blake3.hash(input)
        val definitionBytes = hash.copyOf(UUID_BYTES_LEN)
        definitionBytes[6] = ((definitionBytes[6].toInt() and 0x0F) or 0x40).toByte()
        definitionBytes[8] = ((definitionBytes[8].toInt() and 0x3F) or 0x80).toByte()
        return definitionBytes
    }

    /**
     * Checks whether the given value is a canonical unprefixed Base58 asset-definition address.
     */
    @JvmStatic
    fun isCanonicalAddress(value: String?): Boolean {
        if (value == null) {
            return false
        }
        val trimmed = value.trim()
        if (trimmed != value || trimmed.isEmpty()) {
            return false
        }
        return try {
            encodeFromBytes(parseAddressBytes(trimmed)) == trimmed
        } catch (_: IllegalArgumentException) {
            false
        }
    }

    /**
     * Parses canonical UUIDv4 bytes from an unprefixed Base58 asset-definition address.
     */
    @JvmStatic
    fun parseAddressBytes(address: String): ByteArray {
        val trimmed = address.trim()
        require(trimmed == address && trimmed.isNotEmpty()) {
            "Asset definition id must use canonical unprefixed Base58 form"
        }
        val payload = decodeBase58(trimmed)
        require(payload.size == ADDRESS_PAYLOAD_LEN) {
            "Asset definition id must decode to exactly 21 bytes"
        }
        require((payload[0].toInt() and 0xFF) == ADDRESS_VERSION) {
            "Asset definition id version is not supported"
        }

        val definitionBytes = payload.copyOfRange(1, 1 + UUID_BYTES_LEN)
        val expectedChecksum = checksum(payload.copyOfRange(0, 1 + UUID_BYTES_LEN))
        val actualChecksum = payload.copyOfRange(1 + UUID_BYTES_LEN, ADDRESS_PAYLOAD_LEN)
        require(Arrays.equals(expectedChecksum, actualChecksum)) {
            "Asset definition id checksum is invalid"
        }
        require((definitionBytes[6].toInt() and 0xF0) == 0x40) {
            "Asset definition bytes lack UUIDv4 version nibble (byte 6): $address"
        }
        require((definitionBytes[8].toInt() and 0xC0) == 0x80) {
            "Asset definition bytes lack RFC 4122 variant bits (byte 8): $address"
        }
        return definitionBytes
    }

    private fun addressPayload(definitionBytes: ByteArray): ByteArray {
        val payload = ByteArray(ADDRESS_PAYLOAD_LEN)
        payload[0] = ADDRESS_VERSION.toByte()
        definitionBytes.copyInto(payload, destinationOffset = 1)
        val checksum = checksum(payload.copyOfRange(0, 1 + UUID_BYTES_LEN))
        checksum.copyInto(payload, destinationOffset = 1 + UUID_BYTES_LEN)
        return payload
    }

    private fun checksum(payload: ByteArray): ByteArray =
        Blake3.hash(payload).copyOf(CHECKSUM_LEN)

    private fun encodeBase58(input: ByteArray): String {
        if (input.isEmpty()) {
            return BASE58_ALPHABET[0].toString()
        }
        var value = BigInteger(1, input)
        val builder = StringBuilder()
        while (value > BigInteger.ZERO) {
            val divRem = value.divideAndRemainder(BASE_58)
            builder.append(BASE58_ALPHABET[divRem[1].toInt()])
            value = divRem[0]
        }
        val zeroSymbol = BASE58_ALPHABET[0]
        for (byte in input) {
            if (byte.toInt() != 0) {
                break
            }
            builder.append(zeroSymbol)
        }
        if (builder.isEmpty()) {
            builder.append(zeroSymbol)
        }
        return builder.reverse().toString()
    }

    private fun decodeBase58(encoded: String): ByteArray {
        require(encoded.isNotEmpty()) {
            "Asset definition id must use canonical unprefixed Base58 form"
        }
        var value = BigInteger.ZERO
        for (symbol in encoded) {
            require(symbol.code < BASE58_INDEX.size && BASE58_INDEX[symbol.code] >= 0) {
                "Asset definition id must be valid Base58"
            }
            value = value.multiply(BASE_58).add(BigInteger.valueOf(BASE58_INDEX[symbol.code].toLong()))
        }
        var decoded = value.toByteArray()
        if (decoded.isNotEmpty() && decoded[0].toInt() == 0) {
            decoded = decoded.copyOfRange(1, decoded.size)
        }
        var leadingZeros = 0
        while (leadingZeros < encoded.length && encoded[leadingZeros] == BASE58_ALPHABET[0]) {
            leadingZeros += 1
        }
        if (leadingZeros == 0) {
            return decoded
        }
        val withZeros = ByteArray(leadingZeros + decoded.size)
        decoded.copyInto(withZeros, destinationOffset = leadingZeros)
        return withZeros
    }
}
