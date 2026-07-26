package org.hyperledger.iroha.sdk.crypto

import java.io.Serializable
import java.security.KeyPair
import java.security.PrivateKey
import java.security.PublicKey
import java.security.SecureRandom

internal const val NATIVE_SIGNING_SEED_LENGTH_BYTES = 32

/** Raw public key wrapper for native-backed Iroha signing algorithms. */
class NativeSigningPublicKey(
    val signingAlgorithm: SigningAlgorithm,
    encoded: ByteArray,
) : PublicKey, Serializable {
    private val encodedBytes = encoded.copyOf()

    init {
        require(NativeSigningKeyMaterial.supports(signingAlgorithm)) {
            "algorithm must be a native-backed non-ML-DSA signing algorithm"
        }
        require(encodedBytes.isNotEmpty()) { "encoded must not be empty" }
    }

    override fun getAlgorithm(): String = signingAlgorithm.providerName

    override fun getFormat(): String = "RAW"

    override fun getEncoded(): ByteArray = encodedBytes.copyOf()
}

/** Raw private key wrapper for native-backed Iroha signing algorithms. */
class NativeSigningPrivateKey(
    val signingAlgorithm: SigningAlgorithm,
    encoded: ByteArray,
    publicKey: ByteArray? = null,
) : PrivateKey, Serializable {
    private val encodedBytes = encoded.copyOf()
    private val cachedPublicKey = publicKey?.copyOf()

    init {
        require(NativeSigningKeyMaterial.supports(signingAlgorithm)) {
            "algorithm must be a native-backed non-ML-DSA signing algorithm"
        }
        require(encodedBytes.isNotEmpty()) { "encoded must not be empty" }
    }

    override fun getAlgorithm(): String = signingAlgorithm.providerName

    override fun getFormat(): String = "RAW"

    override fun getEncoded(): ByteArray = encodedBytes.copyOf()

    fun publicKey(): NativeSigningPublicKey =
        NativeSigningPublicKey(
            signingAlgorithm,
            cachedPublicKey?.copyOf()
                ?: NativeSignerBridge.publicKeyFromPrivate(
                    signingAlgorithm,
                    encodedBytes,
                )
        )
}

internal object NativeSigningKeyMaterial {
    fun supports(algorithm: SigningAlgorithm): Boolean =
        algorithm.isNativeBacked() && algorithm != SigningAlgorithm.ML_DSA

    fun generate(algorithm: SigningAlgorithm, secureRandom: SecureRandom): KeyPair {
        val seed = ByteArray(NATIVE_SIGNING_SEED_LENGTH_BYTES)
        secureRandom.nextBytes(seed)
        try {
            return fromSeed(algorithm, seed)
        } finally {
            seed.fill(0)
        }
    }

    fun fromSeed(algorithm: SigningAlgorithm, seed: ByteArray): KeyPair {
        require(supports(algorithm)) { "Unsupported native signing algorithm: $algorithm" }
        val (privateKey, publicKey) = NativeSignerBridge.keypairFromSeed(algorithm, seed)
        return fromRaw(algorithm, privateKey, publicKey)
    }

    fun fromRaw(algorithm: SigningAlgorithm, privateKey: ByteArray, publicKey: ByteArray): KeyPair {
        require(supports(algorithm)) { "Unsupported native signing algorithm: $algorithm" }
        val expected = NativeSignerBridge.publicKeyFromPrivate(algorithm, privateKey)
        require(expected.contentEquals(publicKey)) {
            "${algorithm.providerName} public key does not match private key"
        }
        return KeyPair(
            NativeSigningPublicKey(algorithm, publicKey),
            NativeSigningPrivateKey(algorithm, privateKey, publicKey),
        )
    }

    fun validate(algorithm: SigningAlgorithm, keyPair: KeyPair?): KeyValidation {
        if (!supports(algorithm)) {
            return KeyValidation.invalid(0, 0, "", "native_algorithm_unsupported")
        }
        val publicKey = keyPair?.public as? NativeSigningPublicKey
        if (publicKey == null || publicKey.signingAlgorithm != algorithm) {
            return KeyValidation.invalid(0, 0, "", "${algorithm.wireName}_public_key_missing")
        }
        val privateKey = keyPair.private as? NativeSigningPrivateKey
        if (privateKey == null || privateKey.signingAlgorithm != algorithm) {
            val encoded = publicKey.encoded
            return KeyValidation.invalid(
                encoded.size,
                encoded.size,
                prefixHex(encoded),
                "${algorithm.wireName}_private_key_missing",
            )
        }
        val encodedPublic = publicKey.encoded
        val expected = try {
            NativeSignerBridge.publicKeyFromPrivate(
                algorithm,
                privateKey.encoded,
            )
        } catch (_: RuntimeException) {
            return KeyValidation.invalid(
                encodedPublic.size,
                encodedPublic.size,
                prefixHex(encodedPublic),
                "${algorithm.wireName}_public_key_derivation_failed",
            )
        }
        if (!expected.contentEquals(encodedPublic)) {
            return KeyValidation.invalid(
                encodedPublic.size,
                expected.size,
                prefixHex(encodedPublic),
                "${algorithm.wireName}_public_key_mismatch",
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
