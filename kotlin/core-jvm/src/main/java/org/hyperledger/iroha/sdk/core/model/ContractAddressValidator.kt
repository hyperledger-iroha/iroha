package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission
import org.hyperledger.iroha.sdk.crypto.IrohaHash

private const val BECH32M_CHECKSUM = 0x2BC830A3
private const val CONTRACT_ADDRESS_VERSION_V1 = 1
private const val CONTRACT_ADDRESS_PAYLOAD_BYTES_V1 = 29
private const val CONTRACT_ADDRESS_HRP = "irohac"
private const val CONTRACT_SUBJECT_COUNTER_MAX = 0xFFFF_FFFFL
private const val CHECKSUM_WORDS = 6
private const val MAX_BECH32_LENGTH = 90
private const val MAX_HRP_LENGTH = 83
private const val BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
private val CONTRACT_SUBJECT_HASH_TO_POINT_TAG_V1 =
    "iroha:contract-subject:hash-to-point:v1:".toByteArray(Charsets.UTF_8)
private val BECH32_GENERATORS = intArrayOf(
    0x3B6A57B2,
    0x26508E6D,
    0x1EA119FA,
    0x3D4233DD,
    0x2A1462B3,
)

/**
 * Validate the canonical contract-address subset that Core can decode as ABI V1.
 *
 * This public primitive is intended for clients embedding a typed
 * `ContractAddress` inside larger authenticated Norito payloads.
 */
fun requireCanonicalV1ContractAddress(value: String): String {
    require(value.isNotEmpty() && value == value.trim()) {
        "contractAddress must be an exact non-empty string"
    }
    require(value.length <= MAX_BECH32_LENGTH) {
        "contractAddress must not exceed $MAX_BECH32_LENGTH characters"
    }
    require(value.all { it.code in 33..126 } && value.none { it in 'A'..'Z' }) {
        "contractAddress must be canonical lowercase Bech32m"
    }

    val separator = value.lastIndexOf('1')
    require(separator in 1..MAX_HRP_LENGTH && value.length - separator - 1 >= CHECKSUM_WORDS) {
        "contractAddress must contain a valid Bech32m human-readable prefix"
    }
    val hrp = value.substring(0, separator)
    require(hrp == CONTRACT_ADDRESS_HRP) {
        "contractAddress must use the canonical $CONTRACT_ADDRESS_HRP prefix"
    }
    val data = IntArray(value.length - separator - 1) { index ->
        BECH32_CHARSET.indexOf(value[separator + 1 + index]).also { digit ->
            require(digit >= 0) { "contractAddress contains an invalid Bech32m character" }
        }
    }
    require(bech32Polymod(hrp, data) == BECH32M_CHECKSUM) {
        "contractAddress has an invalid Bech32m checksum"
    }

    val payload = decodeBase32(data.copyOf(data.size - CHECKSUM_WORDS))
    require(payload.size == CONTRACT_ADDRESS_PAYLOAD_BYTES_V1) {
        "contractAddress must contain a $CONTRACT_ADDRESS_PAYLOAD_BYTES_V1-byte V1 payload"
    }
    require((payload[0].toInt() and 0xFF) == CONTRACT_ADDRESS_VERSION_V1) {
        "contractAddress uses an unsupported payload version"
    }
    return value
}

/**
 * Derive the canonical non-signable account subject for an ABI V1 contract address.
 *
 * The domain separator, big-endian retry counter, marked Blake2b-256 hash, and
 * prime-order Ed25519 admission rule are consensus-visible. This helper is the
 * client-side parity surface for Rust `ContractAddress::subject_id()`.
 */
@JvmOverloads
fun contractSubjectAccountIdV1(
    contractAddress: String,
    networkDiscriminant: Int = AccountAddress.DEFAULT_I105_DISCRIMINANT,
): String {
    val canonicalAddress = requireCanonicalV1ContractAddress(contractAddress)
    val addressBytes = canonicalAddress.toByteArray(Charsets.UTF_8)
    var counter = 0L
    while (counter <= CONTRACT_SUBJECT_COUNTER_MAX) {
        val counterBytes = byteArrayOf(
            ((counter ushr 24) and 0xFF).toByte(),
            ((counter ushr 16) and 0xFF).toByte(),
            ((counter ushr 8) and 0xFF).toByte(),
            (counter and 0xFF).toByte(),
        )
        val candidate = IrohaHash.prehash(
            CONTRACT_SUBJECT_HASH_TO_POINT_TAG_V1 + addressBytes + counterBytes,
        )
        if (Ed25519PublicKeyAdmission.isValid(candidate)) {
            return AccountAddress
                .fromAccount(candidate, "ed25519")
                .toI105(networkDiscriminant)
        }
        counter += 1
    }
    throw IllegalArgumentException("contract subject hash-to-point retry counter exhausted")
}

private fun bech32Polymod(hrp: String, data: IntArray): Int {
    var checksum = 1
    hrp.forEach { checksum = polymodStep(checksum, it.code ushr 5) }
    checksum = polymodStep(checksum, 0)
    hrp.forEach { checksum = polymodStep(checksum, it.code and 0x1F) }
    data.forEach { checksum = polymodStep(checksum, it) }
    return checksum
}

private fun polymodStep(checksum: Int, value: Int): Int {
    val top = checksum ushr 25
    var next = (checksum and 0x01FF_FFFF) shl 5 xor value
    BECH32_GENERATORS.forEachIndexed { index, generator ->
        if ((top ushr index) and 1 != 0) next = next xor generator
    }
    return next
}

private fun decodeBase32(words: IntArray): ByteArray {
    val decoded = ByteArray(words.size * 5 / 8)
    var accumulator = 0
    var bits = 0
    var outputIndex = 0
    words.forEach { word ->
        accumulator = ((accumulator shl 5) or word) and 0x0FFF
        bits += 5
        if (bits >= 8) {
            bits -= 8
            decoded[outputIndex++] = (accumulator ushr bits and 0xFF).toByte()
        }
    }
    require(bits < 5 && (accumulator shl (8 - bits) and 0xFF) == 0) {
        "contractAddress has non-canonical Bech32m padding"
    }
    return if (outputIndex == decoded.size) decoded else decoded.copyOf(outputIndex)
}
