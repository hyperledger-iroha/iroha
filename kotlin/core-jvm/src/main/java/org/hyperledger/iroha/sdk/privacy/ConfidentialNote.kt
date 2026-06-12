package org.hyperledger.iroha.sdk.privacy

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.core.model.instructions.ConfidentialEncryptedPayload
import org.hyperledger.iroha.sdk.crypto.Blake3

private val U128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

/** Confidential-v2 note opening material used by commitment and nullifier derivation. */
class ConfidentialNoteOpening(
    rho: ByteArray,
    spendKey: ByteArray,
    ownerTag: ByteArray,
    asset: String,
    chainId: String,
    amount: String,
) {
    private val rhoBytes = fixedBytes(rho, 32, "rho")
    private val spendKeyBytes = copyNonEmpty(spendKey, "spendKey")
    private val ownerTagBytes = fixedScalar(ownerTag, "ownerTag")

    @JvmField
    val asset: String = canonicalText(asset, "asset")

    @JvmField
    val chainId: String = canonicalText(chainId, "chainId")

    @JvmField
    val amount: String = canonicalU128(amount, "amount")

    val rho: ByteArray get() = rhoBytes.copyOf()
    val spendKey: ByteArray get() = spendKeyBytes.copyOf()
    val ownerTag: ByteArray get() = ownerTagBytes.copyOf()

    fun rhoBytes(): ByteArray = rho
    fun spendKeyBytes(): ByteArray = spendKey
    fun ownerTagBytes(): ByteArray = ownerTag

    companion object {
        @JvmStatic
        fun fromSpendKey(
            rho: ByteArray,
            spendKey: ByteArray,
            asset: String,
            chainId: String,
            amount: String,
        ): ConfidentialNoteOpening =
            ConfidentialNoteOpening(
                rho,
                spendKey,
                ConfidentialOwnerTag.deriveFromSpendKey(spendKey),
                asset,
                chainId,
                amount,
            )
    }
}

/** Owner-tag derivation matching `derive_confidential_owner_tag_v2` in Rust. */
object ConfidentialOwnerTag {
    @JvmStatic
    fun defaultDiversifier(): ByteArray = scalarToLittleEndian(BigInteger.ONE)

    @JvmStatic
    fun deriveDiversifier(seed: ByteArray): ByteArray =
        scalarToLittleEndian(hashToScalar("iroha.confidential.v2.diversifier", listOf(seed.copyOf())))

    @JvmStatic
    fun deriveFromSpendKey(spendKey: ByteArray): ByteArray =
        deriveFromSpendKeyWithDiversifier(spendKey, defaultDiversifier())

    @JvmStatic
    fun deriveFromSpendKeyWithDiversifier(spendKey: ByteArray, diversifier: ByteArray): ByteArray {
        val spendScalar = hashToScalar("iroha.confidential.v2.spend_scalar", listOf(copyNonEmpty(spendKey, "spendKey")))
        val diversifierScalar = littleEndianScalar(diversifier, "diversifier")
        return scalarToLittleEndian(poseidonPair(spendScalar, diversifierScalar))
    }
}

/** Commitment derivation matching `derive_confidential_note_v2` in Rust. */
object ConfidentialNoteCommitment {
    @JvmStatic
    fun deriveFromOpening(opening: ConfidentialNoteOpening): ByteArray {
        val amount = scalarFromU128(opening.amount)
        val rho = hashToScalar("iroha.confidential.v2.note_rho", listOf(opening.rho))
        val ownerTag = littleEndianScalar(opening.ownerTag, "ownerTag")
        val assetTag = littleEndianScalar(ConfidentialNoteTags.deriveAssetTag(opening.asset), "assetTag")
        return scalarToLittleEndian(
            poseidonPair(
                amount,
                poseidonPair(rho, poseidonPair(ownerTag, assetTag)),
            ),
        )
    }
}

/** Nullifier derivation matching `derive_confidential_nullifier_v2` in Rust. */
object ConfidentialNoteNullifier {
    @JvmStatic
    fun deriveFromOpening(opening: ConfidentialNoteOpening): ByteArray {
        val spendScalar = hashToScalar("iroha.confidential.v2.spend_scalar", listOf(opening.spendKey))
        val rho = hashToScalar("iroha.confidential.v2.note_rho", listOf(opening.rho))
        val assetTag = littleEndianScalar(ConfidentialNoteTags.deriveAssetTag(opening.asset), "assetTag")
        val chainTag = littleEndianScalar(ConfidentialNoteTags.deriveChainTag(opening.chainId), "chainTag")
        return scalarToLittleEndian(
            poseidonPair(
                spendScalar,
                poseidonPair(rho, poseidonPair(assetTag, chainTag)),
            ),
        )
    }
}

/** Asset and chain tags used by the confidential-v2 note derivation. */
object ConfidentialNoteTags {
    @JvmStatic
    fun deriveAssetTag(asset: String): ByteArray =
        scalarToLittleEndian(
            hashToScalar(
                "iroha.confidential.v2.asset_tag",
                listOf(canonicalText(asset, "asset").toByteArray(StandardCharsets.UTF_8)),
            ),
        )

    @JvmStatic
    fun deriveChainTag(chainId: String): ByteArray =
        scalarToLittleEndian(
            hashToScalar(
                "iroha.confidential.v2.chain_tag",
                listOf(canonicalText(chainId, "chainId").toByteArray(StandardCharsets.UTF_8)),
            ),
        )
}

/** Fail-closed entry point reserved for the note plaintext contract once it is defined. */
object ConfidentialNoteDecryption {
    @JvmStatic
    fun decryptNote(
        encryptedPayload: ConfidentialEncryptedPayload,
        recipientPrivateKey: ByteArray,
    ): ConfidentialNoteOpening {
        require(recipientPrivateKey.size == 32) { "recipientPrivateKey must be 32 bytes" }
        require(encryptedPayload.version == ConfidentialEncryptedPayload.VERSION_V1) {
            "encryptedPayload version must be ${ConfidentialEncryptedPayload.VERSION_V1}"
        }
        throw UnsupportedOperationException(
            "confidential note plaintext layout is not defined by the node or bridge yet",
        )
    }
}

private val PASTA_MODULUS =
    BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16)
private val TWO = BigInteger.valueOf(2)
private val THREE = BigInteger.valueOf(3)
private val SEVEN = BigInteger.valueOf(7)
private val THIRTEEN = BigInteger.valueOf(13)

private fun poseidonPair(lhs: BigInteger, rhs: BigInteger): BigInteger {
    val left = lhs.add(SEVEN).mod(PASTA_MODULUS)
    val right = rhs.add(THIRTEEN).mod(PASTA_MODULUS)
    return TWO.multiply(pow5(left)).add(THREE.multiply(pow5(right))).mod(PASTA_MODULUS)
}

private fun pow5(value: BigInteger): BigInteger {
    val square = value.multiply(value).mod(PASTA_MODULUS)
    val fourth = square.multiply(square).mod(PASTA_MODULUS)
    return fourth.multiply(value).mod(PASTA_MODULUS)
}

private fun hashToScalar(label: String, parts: List<ByteArray>): BigInteger {
    val labelBytes = label.toByteArray(StandardCharsets.UTF_8)
    var counter = 0L
    while (true) {
        val buffer = ByteArrayOutputStream()
        buffer.write(labelBytes)
        buffer.write(leU64(counter))
        for (part in parts) {
            buffer.write(leU64(part.size.toLong()))
            buffer.write(part)
        }
        val candidate = scalarFromLittleEndianOrNull(Blake3.hash(buffer.toByteArray()))
        if (candidate != null) return candidate
        counter += 1
    }
}

private fun leU64(value: Long): ByteArray {
    val out = ByteArray(8)
    for (i in out.indices) {
        out[i] = (value ushr (8 * i)).toByte()
    }
    return out
}

private fun scalarFromU128(amount: String): BigInteger = BigInteger(canonicalU128(amount, "amount"))

private fun fixedScalar(value: ByteArray, name: String): ByteArray {
    val bytes = fixedBytes(value, 32, name)
    littleEndianScalar(bytes, name)
    return bytes
}

private fun littleEndianScalar(bytes: ByteArray, field: String): BigInteger =
    scalarFromLittleEndianOrNull(fixedBytes(bytes, 32, field))
        ?: throw IllegalArgumentException("$field must be a canonical Pasta scalar")

private fun scalarFromLittleEndianOrNull(bytes: ByteArray): BigInteger? {
    if (bytes.size != 32) return null
    val bigEndian = bytes.copyOf().also { it.reverse() }
    val value = BigInteger(1, bigEndian)
    return if (value < PASTA_MODULUS) value else null
}

private fun scalarToLittleEndian(value: BigInteger): ByteArray {
    val bigEndian = value.mod(PASTA_MODULUS).toByteArray().dropWhile { it == 0.toByte() }.toByteArray()
    require(bigEndian.size <= 32) { "scalar encoding overflow" }
    val out = ByteArray(32)
    for (i in bigEndian.indices) {
        out[i] = bigEndian[bigEndian.size - 1 - i]
    }
    return out
}

private fun canonicalU128(value: String, name: String): String {
    val text = canonicalText(value, name)
    require(text.all { it in '0'..'9' }) { "$name must be an unsigned decimal integer" }
    require(text == "0" || !text.startsWith("0")) { "$name must be canonical decimal without leading zeroes" }
    val parsed = BigInteger(text)
    require(parsed.signum() >= 0 && parsed <= U128_MAX) { "$name must fit in u128" }
    return text
}

private fun canonicalText(value: String, name: String): String {
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$name must not be blank" }
    require(trimmed == value) { "$name must not contain surrounding whitespace" }
    require(trimmed.indexOf('\u0000') < 0) { "$name must not contain NUL" }
    return trimmed
}

private fun fixedBytes(value: ByteArray, expected: Int, name: String): ByteArray {
    require(value.size == expected) { "$name must be $expected bytes" }
    return value.copyOf()
}

private fun copyNonEmpty(value: ByteArray, name: String): ByteArray {
    require(value.isNotEmpty()) { "$name must not be empty" }
    return value.copyOf()
}
