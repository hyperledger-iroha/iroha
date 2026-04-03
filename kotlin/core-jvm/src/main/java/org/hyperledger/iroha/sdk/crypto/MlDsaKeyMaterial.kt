package org.hyperledger.iroha.sdk.crypto

import java.io.Serializable
import java.security.KeyPair
import java.security.PrivateKey
import java.security.PublicKey
import java.security.SecureRandom

internal const val ML_DSA_SEED_LENGTH_BYTES = 32

/** Raw ML-DSA public key wrapper used by the JVM/Android SDK software provider. */
class MlDsaPublicKey(encoded: ByteArray) : PublicKey, Serializable {
    private val encodedBytes = encoded.copyOf()

    override fun getAlgorithm(): String = SigningAlgorithm.ML_DSA.providerName

    override fun getFormat(): String = "RAW"

    override fun getEncoded(): ByteArray = encodedBytes.copyOf()
}

/** Raw ML-DSA private key wrapper used by the JVM/Android SDK software provider. */
class MlDsaPrivateKey(
    encoded: ByteArray,
    publicKey: ByteArray? = null,
) : PrivateKey, Serializable {
    private val encodedBytes = encoded.copyOf()
    private val cachedPublicKey = publicKey?.copyOf()

    override fun getAlgorithm(): String = SigningAlgorithm.ML_DSA.providerName

    override fun getFormat(): String = "RAW"

    override fun getEncoded(): ByteArray = encodedBytes.copyOf()

    fun publicKey(): MlDsaPublicKey =
        MlDsaPublicKey(
            cachedPublicKey?.copyOf()
                ?: NativeSignerBridge.publicKeyFromPrivate(
                    SigningAlgorithm.ML_DSA,
                    encodedBytes,
                )
        )
}

internal object MlDsaKeyMaterial {
    fun generate(secureRandom: SecureRandom): KeyPair {
        val seed = ByteArray(ML_DSA_SEED_LENGTH_BYTES)
        secureRandom.nextBytes(seed)
        return fromSeed(seed)
    }

    fun fromSeed(seed: ByteArray): KeyPair {
        val (privateKey, publicKey) = NativeSignerBridge.keypairFromSeed(SigningAlgorithm.ML_DSA, seed)
        return fromRaw(privateKey, publicKey)
    }

    fun fromRaw(privateKey: ByteArray, publicKey: ByteArray): KeyPair {
        val expected = NativeSignerBridge.publicKeyFromPrivate(SigningAlgorithm.ML_DSA, privateKey)
        require(expected.contentEquals(publicKey)) { "ML-DSA public key does not match private key" }
        return KeyPair(MlDsaPublicKey(publicKey), MlDsaPrivateKey(privateKey, publicKey))
    }

    fun validate(keyPair: KeyPair?): KeyValidation {
        if (keyPair?.public !is MlDsaPublicKey) {
            return KeyValidation.invalid(0, 0, "", "mldsa_public_key_missing")
        }
        if (keyPair.private !is MlDsaPrivateKey) {
            val length = keyPair.public.encoded?.size ?: 0
            return KeyValidation.invalid(length, length, prefixHex(keyPair.public.encoded), "mldsa_private_key_missing")
        }
        val encodedPublic = keyPair.public.encoded
        val expected = try {
            NativeSignerBridge.publicKeyFromPrivate(
                SigningAlgorithm.ML_DSA,
                keyPair.private.encoded ?: ByteArray(0),
            )
        } catch (_: IllegalArgumentException) {
            return KeyValidation.invalid(
                encodedPublic?.size ?: 0,
                encodedPublic?.size ?: 0,
                prefixHex(encodedPublic),
                "mldsa_private_key_empty",
            )
        }
        if (!expected.contentEquals(encodedPublic)) {
            return KeyValidation.invalid(
                encodedPublic.size,
                expected.size,
                prefixHex(encodedPublic),
                "mldsa_public_key_mismatch",
            )
        }
        return KeyValidation.valid(
            encodedPublic.size,
            expected.size,
            prefixHex(encodedPublic),
        )
    }

    private fun prefixHex(bytes: ByteArray?): String {
        if (bytes == null || bytes.isEmpty()) return ""
        val limit = minOf(bytes.size, 12)
        return (0 until limit).joinToString("") { "%02x".format(bytes[it]) }
    }
}

internal class KeyValidation private constructor(
    val valid: Boolean,
    val length: Int,
    val expectedLength: Int,
    val prefixHex: String,
    val reason: String,
) {
    fun detail(): String =
        "reason=$reason, key_len=$length, expected_len=$expectedLength, prefix=${prefixHex.ifEmpty { "unknown" }}"

    companion object {
        fun valid(length: Int, expectedLength: Int, prefixHex: String): KeyValidation =
            KeyValidation(true, length, expectedLength, prefixHex, "ok")

        fun invalid(length: Int, expectedLength: Int, prefixHex: String, reason: String): KeyValidation =
            KeyValidation(false, length, expectedLength, prefixHex, reason)
    }
}
