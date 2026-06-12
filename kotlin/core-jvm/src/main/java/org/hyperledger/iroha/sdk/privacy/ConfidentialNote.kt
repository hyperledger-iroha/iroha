package org.hyperledger.iroha.sdk.privacy

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import java.util.Arrays
import org.bouncycastle.crypto.InvalidCipherTextException
import org.bouncycastle.crypto.agreement.X25519Agreement
import org.bouncycastle.crypto.digests.SHA256Digest
import org.bouncycastle.crypto.generators.HKDFBytesGenerator
import org.bouncycastle.crypto.modes.ChaCha20Poly1305
import org.bouncycastle.crypto.params.AEADParameters
import org.bouncycastle.crypto.params.HKDFParameters
import org.bouncycastle.crypto.params.KeyParameter
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.bouncycastle.crypto.params.X25519PublicKeyParameters
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

/** Encrypts confidential-v2 note openings into `ConfidentialEncryptedPayload` envelopes. */
object ConfidentialNoteEncryption {
    @JvmStatic
    fun publicKeyFromPrivateKey(privateKey: ByteArray): ByteArray {
        val privateBytes = fixedNonZeroBytes(privateKey, 32, "privateKey")
        return try {
            val publicKey = X25519PrivateKeyParameters(privateBytes, 0).generatePublicKey()
            ByteArray(32).also { publicKey.encode(it, 0) }
        } finally {
            Arrays.fill(privateBytes, 0.toByte())
        }
    }

    @JvmStatic
    fun encryptNote(
        opening: ConfidentialNoteOpening,
        recipientPublicKey: ByteArray,
    ): ConfidentialEncryptedPayload {
        val random = SecureRandom()
        val ephemeralPrivateKey = ByteArray(32)
        val nonce = ByteArray(24)
        random.nextBytes(ephemeralPrivateKey)
        random.nextBytes(nonce)
        return try {
            encryptNote(opening, recipientPublicKey, ephemeralPrivateKey, nonce)
        } finally {
            Arrays.fill(ephemeralPrivateKey, 0.toByte())
        }
    }

    @JvmStatic
    fun encryptNote(
        opening: ConfidentialNoteOpening,
        recipientPublicKey: ByteArray,
        ephemeralPrivateKey: ByteArray,
        nonce: ByteArray,
    ): ConfidentialEncryptedPayload {
        val recipientPublic = fixedBytes(recipientPublicKey, 32, "recipientPublicKey")
        val ephemeralPrivate = fixedNonZeroBytes(ephemeralPrivateKey, 32, "ephemeralPrivateKey")
        val nonceBytes = fixedBytes(nonce, 24, "nonce")
        val ephemeralPublic = publicKeyFromPrivateKey(ephemeralPrivate)
        var key: ByteArray? = null
        var plaintext: ByteArray? = null
        return try {
            val derivedKey = derivePayloadKey(
                ephemeralPrivate,
                recipientPublic,
                ephemeralPublic,
                recipientPublic,
            )
            key = derivedKey
            val plaintextBytes = encodePlaintext(opening)
            plaintext = plaintextBytes
            val ciphertext = runXChaCha20Poly1305(
                encrypt = true,
                key = derivedKey,
                nonce = nonceBytes,
                aad = payloadAad(ephemeralPublic, recipientPublic),
                input = plaintextBytes,
            )
            ConfidentialEncryptedPayload(
                ephemeralPublicKey = ephemeralPublic,
                nonce = nonceBytes,
                ciphertext = ciphertext,
            )
        } finally {
            key?.let { Arrays.fill(it, 0.toByte()) }
            plaintext?.let { Arrays.fill(it, 0.toByte()) }
            Arrays.fill(ephemeralPrivate, 0.toByte())
        }
    }
}

/** Decrypts confidential-v2 note payload envelopes into validated note openings. */
object ConfidentialNoteDecryption {
    @JvmStatic
    fun decryptNote(
        encryptedPayload: ConfidentialEncryptedPayload,
        recipientPrivateKey: ByteArray,
        spendKey: ByteArray,
    ): ConfidentialNoteOpening =
        decryptNote(encryptedPayload, recipientPrivateKey, spendKey, null)

    @JvmStatic
    fun decryptNote(
        encryptedPayload: ConfidentialEncryptedPayload,
        recipientPrivateKey: ByteArray,
        spendKey: ByteArray,
        expectedChainId: String?,
    ): ConfidentialNoteOpening {
        val recipientPrivate = fixedNonZeroBytes(recipientPrivateKey, 32, "recipientPrivateKey")
        val recipientPublic = ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivate)
        require(encryptedPayload.version == ConfidentialEncryptedPayload.VERSION_V1) {
            "encryptedPayload version must be ${ConfidentialEncryptedPayload.VERSION_V1}"
        }
        val key = derivePayloadKey(
            localPrivateKey = recipientPrivate,
            peerPublicKey = encryptedPayload.ephemeralPublicKey,
            ephemeralPublicKey = encryptedPayload.ephemeralPublicKey,
            recipientPublicKey = recipientPublic,
        )
        var plaintext: ByteArray? = null
        return try {
            val plaintextBytes = runXChaCha20Poly1305(
                encrypt = false,
                key = key,
                nonce = encryptedPayload.nonce,
                aad = payloadAad(encryptedPayload.ephemeralPublicKey, recipientPublic),
                input = encryptedPayload.ciphertext,
            )
            plaintext = plaintextBytes
            val decoded = decodePlaintext(plaintextBytes)
            if (expectedChainId != null) {
                require(decoded.chainId == canonicalText(expectedChainId, "expectedChainId")) {
                    "confidential note chainId does not match expectedChainId"
                }
            }
            ConfidentialNoteOpening(
                decoded.rho,
                spendKey,
                decoded.ownerTag,
                decoded.asset,
                decoded.chainId,
                decoded.amount,
            )
        } finally {
            Arrays.fill(key, 0.toByte())
            plaintext?.let { Arrays.fill(it, 0.toByte()) }
            Arrays.fill(recipientPrivate, 0.toByte())
        }
    }
}

private data class DecodedPlaintext(
    val rho: ByteArray,
    val ownerTag: ByteArray,
    val asset: String,
    val chainId: String,
    val amount: String,
)

private const val NOTE_PLAINTEXT_VERSION_V1 = 1
private const val NOTE_TEXT_MAX_BYTES = 4096
private const val AEAD_TAG_BITS = 128
private val NOTE_KDF_SALT = "iroha:confidential-note:v1:x25519".toByteArray(StandardCharsets.UTF_8)
private val NOTE_KDF_INFO_PREFIX =
    "iroha:confidential-note:v1:xchacha20poly1305".toByteArray(StandardCharsets.UTF_8)
private val NOTE_AAD_PREFIX = "iroha:confidential-note:v1".toByteArray(StandardCharsets.UTF_8)

private fun encodePlaintext(opening: ConfidentialNoteOpening): ByteArray {
    val assetBytes = opening.asset.toByteArray(StandardCharsets.UTF_8)
    val chainIdBytes = opening.chainId.toByteArray(StandardCharsets.UTF_8)
    val amountBytes = opening.amount.toByteArray(StandardCharsets.US_ASCII)
    require(assetBytes.size <= NOTE_TEXT_MAX_BYTES) { "asset is too large" }
    require(chainIdBytes.size <= NOTE_TEXT_MAX_BYTES) { "chainId is too large" }
    require(amountBytes.size <= NOTE_TEXT_MAX_BYTES) { "amount is too large" }
    val out = ByteArrayOutputStream()
    out.write(NOTE_PLAINTEXT_VERSION_V1)
    out.write(opening.rho)
    out.write(opening.ownerTag)
    writeVarint(assetBytes.size, out)
    out.write(assetBytes)
    writeVarint(chainIdBytes.size, out)
    out.write(chainIdBytes)
    writeVarint(amountBytes.size, out)
    out.write(amountBytes)
    return out.toByteArray()
}

private fun decodePlaintext(bytes: ByteArray): DecodedPlaintext {
    require(bytes.isNotEmpty()) { "confidential note plaintext must not be empty" }
    require(bytes[0].toInt() and 0xff == NOTE_PLAINTEXT_VERSION_V1) {
        "unsupported confidential note plaintext version"
    }
    var offset = 1
    require(bytes.size >= offset + 64) { "confidential note plaintext is truncated" }
    val rho = bytes.copyOfRange(offset, offset + 32)
    offset += 32
    val ownerTag = fixedScalar(bytes.copyOfRange(offset, offset + 32), "ownerTag")
    offset += 32
    val (assetLen, assetLenBytes) = readVarint(bytes, offset)
    offset += assetLenBytes
    require(assetLen in 1..NOTE_TEXT_MAX_BYTES) { "asset length is invalid" }
    require(bytes.size >= offset + assetLen) { "asset is truncated" }
    val asset = canonicalText(decodeUtf8(bytes, offset, assetLen, "asset"), "asset")
    offset += assetLen
    val (chainLen, chainLenBytes) = readVarint(bytes, offset)
    offset += chainLenBytes
    require(chainLen in 1..NOTE_TEXT_MAX_BYTES) { "chainId length is invalid" }
    require(bytes.size >= offset + chainLen) { "chainId is truncated" }
    val chainId = canonicalText(decodeUtf8(bytes, offset, chainLen, "chainId"), "chainId")
    offset += chainLen
    val (amountLen, amountLenBytes) = readVarint(bytes, offset)
    offset += amountLenBytes
    require(amountLen in 1..NOTE_TEXT_MAX_BYTES) { "amount length is invalid" }
    require(bytes.size >= offset + amountLen) { "amount is truncated" }
    val amount = canonicalU128(String(bytes, offset, amountLen, StandardCharsets.US_ASCII), "amount")
    offset += amountLen
    require(offset == bytes.size) { "confidential note plaintext has trailing bytes" }
    return DecodedPlaintext(rho, ownerTag, asset, chainId, amount)
}

private fun derivePayloadKey(
    localPrivateKey: ByteArray,
    peerPublicKey: ByteArray,
    ephemeralPublicKey: ByteArray,
    recipientPublicKey: ByteArray,
): ByteArray {
    val localPrivate = fixedNonZeroBytes(localPrivateKey, 32, "localPrivateKey")
    val local = X25519PrivateKeyParameters(localPrivate, 0)
    val peer = X25519PublicKeyParameters(fixedBytes(peerPublicKey, 32, "peerPublicKey"), 0)
    val agreement = X25519Agreement()
    agreement.init(local)
    val shared = ByteArray(32)
    try {
        try {
            agreement.calculateAgreement(peer, shared, 0)
        } catch (ex: IllegalStateException) {
            throw IllegalArgumentException("peerPublicKey must not be low-order", ex)
        }
        require(!shared.all { it.toInt() == 0 }) { "X25519 shared secret is all zero" }
        val hkdf = HKDFBytesGenerator(SHA256Digest())
        hkdf.init(
            HKDFParameters(
                shared,
                NOTE_KDF_SALT,
                payloadKdfInfo(ephemeralPublicKey, recipientPublicKey),
            ),
        )
        return ByteArray(32).also { hkdf.generateBytes(it, 0, it.size) }
    } finally {
        Arrays.fill(shared, 0.toByte())
        Arrays.fill(localPrivate, 0.toByte())
    }
}

private fun payloadKdfInfo(ephemeralPublicKey: ByteArray, recipientPublicKey: ByteArray): ByteArray {
    val out = ByteArrayOutputStream()
    out.write(NOTE_KDF_INFO_PREFIX)
    out.write(fixedBytes(ephemeralPublicKey, 32, "ephemeralPublicKey"))
    out.write(fixedBytes(recipientPublicKey, 32, "recipientPublicKey"))
    return out.toByteArray()
}

private fun payloadAad(ephemeralPublicKey: ByteArray, recipientPublicKey: ByteArray): ByteArray {
    val out = ByteArrayOutputStream()
    out.write(NOTE_AAD_PREFIX)
    out.write(NOTE_PLAINTEXT_VERSION_V1)
    out.write(fixedBytes(ephemeralPublicKey, 32, "ephemeralPublicKey"))
    out.write(fixedBytes(recipientPublicKey, 32, "recipientPublicKey"))
    return out.toByteArray()
}

private fun runXChaCha20Poly1305(
    encrypt: Boolean,
    key: ByteArray,
    nonce: ByteArray,
    aad: ByteArray,
    input: ByteArray,
): ByteArray {
    val subkey = hChaCha20(
        fixedBytes(key, 32, "key"),
        fixedBytes(nonce, 24, "nonce").copyOfRange(0, 16),
    )
    val ietfNonce = ByteArray(12)
    System.arraycopy(nonce, 16, ietfNonce, 4, 8)
    return try {
        val cipher = ChaCha20Poly1305()
        cipher.init(encrypt, AEADParameters(KeyParameter(subkey), AEAD_TAG_BITS, ietfNonce, aad))
        val out = ByteArray(cipher.getOutputSize(input.size))
        var written = cipher.processBytes(input, 0, input.size, out, 0)
        written += cipher.doFinal(out, written)
        out.copyOf(written)
    } catch (ex: InvalidCipherTextException) {
        throw SecurityException("confidential note payload authentication failed", ex)
    } catch (ex: RuntimeException) {
        throw IllegalArgumentException("confidential note payload cryptography failed", ex)
    } finally {
        Arrays.fill(subkey, 0.toByte())
    }
}

private fun hChaCha20(key: ByteArray, nonce16: ByteArray): ByteArray {
    require(key.size == 32) { "key must be 32 bytes" }
    require(nonce16.size == 16) { "nonce16 must be 16 bytes" }
    val state = IntArray(16)
    state[0] = 0x61707865
    state[1] = 0x3320646e
    state[2] = 0x79622d32
    state[3] = 0x6b206574
    for (i in 0 until 8) state[4 + i] = leI32(key, i * 4)
    for (i in 0 until 4) state[12 + i] = leI32(nonce16, i * 4)
    repeat(10) {
        quarterRound(state, 0, 4, 8, 12)
        quarterRound(state, 1, 5, 9, 13)
        quarterRound(state, 2, 6, 10, 14)
        quarterRound(state, 3, 7, 11, 15)
        quarterRound(state, 0, 5, 10, 15)
        quarterRound(state, 1, 6, 11, 12)
        quarterRound(state, 2, 7, 8, 13)
        quarterRound(state, 3, 4, 9, 14)
    }
    val out = ByteArray(32)
    intToLe(state[0], out, 0)
    intToLe(state[1], out, 4)
    intToLe(state[2], out, 8)
    intToLe(state[3], out, 12)
    intToLe(state[12], out, 16)
    intToLe(state[13], out, 20)
    intToLe(state[14], out, 24)
    intToLe(state[15], out, 28)
    return out
}

private fun quarterRound(state: IntArray, a: Int, b: Int, c: Int, d: Int) {
    state[a] += state[b]
    state[d] = Integer.rotateLeft(state[d] xor state[a], 16)
    state[c] += state[d]
    state[b] = Integer.rotateLeft(state[b] xor state[c], 12)
    state[a] += state[b]
    state[d] = Integer.rotateLeft(state[d] xor state[a], 8)
    state[c] += state[d]
    state[b] = Integer.rotateLeft(state[b] xor state[c], 7)
}

private fun leI32(bytes: ByteArray, offset: Int): Int =
    (bytes[offset].toInt() and 0xff) or
        ((bytes[offset + 1].toInt() and 0xff) shl 8) or
        ((bytes[offset + 2].toInt() and 0xff) shl 16) or
        ((bytes[offset + 3].toInt() and 0xff) shl 24)

private fun intToLe(value: Int, out: ByteArray, offset: Int) {
    out[offset] = value.toByte()
    out[offset + 1] = (value ushr 8).toByte()
    out[offset + 2] = (value ushr 16).toByte()
    out[offset + 3] = (value ushr 24).toByte()
}

private fun writeVarint(value: Int, out: ByteArrayOutputStream) {
    var remaining = value
    while (true) {
        var byte = remaining and 0x7f
        remaining = remaining ushr 7
        if (remaining != 0) byte = byte or 0x80
        out.write(byte)
        if (remaining == 0) return
    }
}

private fun readVarint(bytes: ByteArray, offset: Int): Pair<Int, Int> {
    var value = 0
    var shift = 0
    var cursor = offset
    while (cursor < bytes.size && shift < 28) {
        val byte = bytes[cursor].toInt() and 0xff
        value = value or ((byte and 0x7f) shl shift)
        cursor += 1
        if (byte and 0x80 == 0) {
            val encodedBytes = cursor - offset
            require(encodedBytes == 1 || value >= (1 shl (7 * (encodedBytes - 1)))) {
                "non-canonical confidential note plaintext length"
            }
            return value to encodedBytes
        }
        shift += 7
    }
    throw IllegalArgumentException("invalid confidential note plaintext length")
}

private fun decodeUtf8(bytes: ByteArray, offset: Int, len: Int, name: String): String =
    try {
        StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(bytes, offset, len))
            .toString()
    } catch (ex: CharacterCodingException) {
        throw IllegalArgumentException("$name must be valid UTF-8", ex)
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

private fun fixedNonZeroBytes(value: ByteArray, expected: Int, name: String): ByteArray {
    val bytes = fixedBytes(value, expected, name)
    require(bytes.any { it.toInt() != 0 }) { "$name must not be all zero" }
    return bytes
}

private fun copyNonEmpty(value: ByteArray, name: String): ByteArray {
    require(value.isNotEmpty()) { "$name must not be empty" }
    return value.copyOf()
}
