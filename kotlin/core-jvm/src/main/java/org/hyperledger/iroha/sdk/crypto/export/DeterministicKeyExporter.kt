package org.hyperledger.iroha.sdk.crypto.export

import java.nio.ByteBuffer
import java.nio.CharBuffer
import java.nio.charset.CharacterCodingException
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.PublicKey
import java.security.SecureRandom
import java.security.spec.PKCS8EncodedKeySpec
import java.security.spec.X509EncodedKeySpec
import java.util.ArrayDeque
import java.util.HexFormat
import java.util.LinkedHashSet
import javax.crypto.Cipher
import javax.crypto.Mac
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.PBEKeySpec
import javax.crypto.spec.SecretKeySpec

private const val HMAC_ALGORITHM = "HmacSHA256"
private const val DIGEST_ALGORITHM = "SHA-256"
private const val KEY_ALGORITHM = "Ed25519"
private const val PBKDF2_ALGORITHM = "PBKDF2WithHmacSHA256"
private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
private const val GCM_TAG_BITS = 128
private const val ED25519_SPKI_SIZE = 44
private const val DEFAULT_PBKDF2_ITERATIONS = 350_000
private const val DEFAULT_ARGON2_MEMORY_KIB = 64 * 1024
private const val DEFAULT_ARGON2_ITERATIONS = 3
private const val DEFAULT_ARGON2_PARALLELISM = 2
private const val SALT_LENGTH_BYTES = KeyExportBundle.EXPECTED_SALT_LENGTH_BYTES
private const val NONCE_LENGTH_BYTES = KeyExportBundle.EXPECTED_NONCE_LENGTH_BYTES
private const val AES_KEY_LENGTH_BYTES = 32
private const val MIN_PASSPHRASE_LENGTH = 12
private const val MAX_TRACKED_EXPORTS = 1024

private val ED25519_SPKI_PREFIX = byteArrayOf(
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
)
private val ED25519_OID = byteArrayOf(0x2b, 0x65, 0x70)

/**
 * Exports and recovers software-generated Ed25519 keys using salted HKDF + AES-GCM.
 *
 * Each export uses a fresh random salt and nonce bound to the alias, derives a key with a
 * memory-hard KDF (Argon2id preferred, PBKDF2 fallback), and records the KDF kind/work factor in
 * the bundle. The exported bundle contains the public key, nonce, salt, and ciphertext so the
 * receiver can validate and recover the key pair deterministically across JVMs.
 */
object DeterministicKeyExporter {

    @JvmField
    internal val KDF_KIND_PBKDF2_HMAC_SHA256 = 1
    @JvmField
    internal val KDF_KIND_ARGON2ID = 2

    private val SECURE_RANDOM = SecureRandom()
    private val recentExportFingerprints = ArrayDeque<String>()
    private val recentExportIndex = LinkedHashSet<String>()

    /**
     * Exports the `privateKey` deterministically under `passphrase` and `alias`.
     *
     * The passphrase is consumed as UTF-8 bytes and cleared after derivation.
     */
    @JvmStatic
    @Throws(KeyExportException::class)
    fun exportKeyPair(
        privateKey: PrivateKey,
        publicKey: PublicKey,
        alias: String,
        passphrase: CharArray,
    ): KeyExportBundle {
        ensureEd25519KeyPair(privateKey, publicKey)
        ensurePassphraseStrength(passphrase)

        val privateKeyBytes = privateKey.encoded
        val publicKeyBytes = publicKey.encoded
        val salt = ByteArray(SALT_LENGTH_BYTES)
        fillRandomNonZero(salt)
        val nonce = ByteArray(NONCE_LENGTH_BYTES)
        fillRandomNonZero(nonce)
        guardSaltNonceReuse(salt, nonce)
        val kdf = derivePreferredKey(alias, passphrase, salt)
        try {
            val cipher = Cipher.getInstance(AES_TRANSFORMATION)
            val spec = GCMParameterSpec(GCM_TAG_BITS, nonce)
            cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(kdf.key, "AES"), spec)
            cipher.updateAAD(alias.toByteArray(Charsets.UTF_8))
            val ciphertext = cipher.doFinal(privateKeyBytes)
            return KeyExportBundle(
                alias,
                publicKeyBytes,
                nonce,
                ciphertext,
                salt,
                kdf.kind,
                kdf.workFactor,
                KeyExportBundle.VERSION_V3,
            )
        } catch (ex: GeneralSecurityException) {
            throw KeyExportException("Failed to encrypt private key", ex)
        } finally {
            privateKeyBytes.fill(0)
            kdf.zero()
        }
    }

    /** Imports a key pair from the supplied bundle using `passphrase`. */
    @JvmStatic
    @Throws(KeyExportException::class)
    fun importKeyPair(bundle: KeyExportBundle, passphrase: CharArray): KeyPairData {
        if (bundle.version != KeyExportBundle.VERSION_V3) {
            throw KeyExportException("Unsupported key export version: ${bundle.version}")
        }
        ensurePassphraseStrength(passphrase)
        if (isAllZero(bundle.nonce) || isAllZero(bundle.salt)) {
            throw KeyExportException("Salt and nonce must be non-zero for deterministic import")
        }
        val aesKey = deriveKeyForBundle(
            bundle.alias, passphrase, bundle.salt, bundle.kdfKind, bundle.kdfWorkFactor,
        )
        try {
            val cipher = Cipher.getInstance(AES_TRANSFORMATION)
            val spec = GCMParameterSpec(GCM_TAG_BITS, bundle.nonce)
            cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(aesKey, "AES"), spec)
            cipher.updateAAD(bundle.alias.toByteArray(Charsets.UTF_8))
            val privateKeyBytes = cipher.doFinal(bundle.ciphertext)
            try {
                val factory = KeyFactory.getInstance(KEY_ALGORITHM)
                val privKey = factory.generatePrivate(PKCS8EncodedKeySpec(privateKeyBytes))
                val pubKey = factory.generatePublic(X509EncodedKeySpec(bundle.publicKey))
                return KeyPairData(privKey, pubKey)
            } catch (ex: GeneralSecurityException) {
                throw KeyExportException("Failed to reconstruct Ed25519 key pair", ex)
            } finally {
                privateKeyBytes.fill(0)
            }
        } catch (ex: GeneralSecurityException) {
            throw KeyExportException("Failed to decrypt private key", ex)
        } finally {
            aesKey.fill(0)
        }
    }

    private fun ensureEd25519KeyPair(privateKey: PrivateKey, publicKey: PublicKey) {
        if (!isEd25519PrivateKey(privateKey) || !isEd25519PublicKey(publicKey)) {
            throw KeyExportException("Deterministic export currently supports Ed25519 keys only")
        }
    }

    private fun isEd25519PrivateKey(privateKey: PrivateKey): Boolean {
        val encoded = privateKey.encoded ?: return false
        if (encoded.isEmpty()) return false
        return hasEd25519Pkcs8(encoded)
    }

    private fun isEd25519PublicKey(publicKey: PublicKey): Boolean {
        val encoded = publicKey.encoded ?: return false
        if (encoded.isEmpty()) return false
        if (encoded.size == ED25519_SPKI_SIZE) {
            var matches = true
            for (i in ED25519_SPKI_PREFIX.indices) {
                if (encoded[i] != ED25519_SPKI_PREFIX[i]) {
                    matches = false
                    break
                }
            }
            if (matches) return true
        }
        return hasEd25519Spki(encoded)
    }

    private fun hasEd25519Pkcs8(encoded: ByteArray): Boolean {
        if (encoded.size < 16 || encoded[0] != 0x30.toByte()) return false
        var offset = 1
        val lengthBytes = IntArray(1)
        val totalLength = readDerLength(encoded, offset, lengthBytes)
        if (totalLength < 0) return false
        offset += lengthBytes[0]
        if (offset + totalLength > encoded.size) return false
        if (offset >= encoded.size || encoded[offset++] != 0x02.toByte()) return false
        val versionLength = readDerLength(encoded, offset, lengthBytes)
        if (versionLength < 0) return false
        offset += lengthBytes[0]
        if (offset + versionLength > encoded.size) return false
        offset += versionLength
        return readAlgorithmOid(encoded, offset, lengthBytes)
    }

    private fun hasEd25519Spki(encoded: ByteArray): Boolean {
        if (encoded.size < 12 || encoded[0] != 0x30.toByte()) return false
        var offset = 1
        val lengthBytes = IntArray(1)
        val totalLength = readDerLength(encoded, offset, lengthBytes)
        if (totalLength < 0) return false
        offset += lengthBytes[0]
        if (offset + totalLength > encoded.size) return false
        return readAlgorithmOid(encoded, offset, lengthBytes)
    }

    private fun readAlgorithmOid(
        encoded: ByteArray,
        startOffset: Int,
        lengthBytes: IntArray,
    ): Boolean {
        var offset = startOffset
        if (offset >= encoded.size || encoded[offset++] != 0x30.toByte()) return false
        val algLength = readDerLength(encoded, offset, lengthBytes)
        if (algLength < 0) return false
        offset += lengthBytes[0]
        if (offset + algLength > encoded.size) return false
        if (offset >= encoded.size || encoded[offset++] != 0x06.toByte()) return false
        val oidLength = readDerLength(encoded, offset, lengthBytes)
        if (oidLength != ED25519_OID.size) return false
        offset += lengthBytes[0]
        if (offset + oidLength > encoded.size) return false
        for (i in 0 until oidLength) {
            if (encoded[offset + i] != ED25519_OID[i]) return false
        }
        return true
    }

    private fun readDerLength(encoded: ByteArray, offset: Int, lengthBytes: IntArray): Int {
        if (offset >= encoded.size) return -1
        val first = encoded[offset].toInt() and 0xFF
        if (first and 0x80 == 0) {
            lengthBytes[0] = 1
            return first
        }
        val count = first and 0x7F
        if (count == 0 || count > 4 || offset + count >= encoded.size) return -1
        var length = 0
        for (i in 0 until count) {
            length = (length shl 8) or (encoded[offset + 1 + i].toInt() and 0xFF)
        }
        lengthBytes[0] = 1 + count
        return length
    }

    private fun deriveKeyForBundle(
        alias: String,
        passphrase: CharArray,
        salt: ByteArray,
        kdfKind: Int,
        workFactor: Int,
    ): ByteArray {
        if (salt.size != SALT_LENGTH_BYTES) {
            throw KeyExportException("Salt must be provided for key import")
        }
        if (workFactor <= 0) {
            throw KeyExportException("Invalid KDF work factor: $workFactor")
        }
        return when (kdfKind) {
            KDF_KIND_PBKDF2_HMAC_SHA256 -> derivePbkdf2Key(alias, passphrase, salt, workFactor)
            KDF_KIND_ARGON2ID -> deriveArgon2Key(alias, passphrase, salt, workFactor)
            else -> throw KeyExportException("Unsupported KDF kind: $kdfKind")
        }
    }

    private fun derivePreferredKey(
        alias: String,
        passphrase: CharArray,
        salt: ByteArray,
    ): KdfResult {
        if (argon2Available()) {
            try {
                val argonKey = deriveArgon2Key(alias, passphrase, salt, DEFAULT_ARGON2_ITERATIONS)
                return KdfResult(argonKey, KDF_KIND_ARGON2ID, DEFAULT_ARGON2_ITERATIONS)
            } catch (_: KeyExportException) {
                // fall through to PBKDF2 fallback
            } catch (_: RuntimeException) {
                // fall through to PBKDF2 fallback
            } catch (_: LinkageError) {
                // fall through to PBKDF2 fallback
            }
        }
        val pbkdfKey = derivePbkdf2Key(alias, passphrase, salt, DEFAULT_PBKDF2_ITERATIONS)
        return KdfResult(pbkdfKey, KDF_KIND_PBKDF2_HMAC_SHA256, DEFAULT_PBKDF2_ITERATIONS)
    }

    private fun deriveArgon2Key(
        alias: String,
        passphrase: CharArray,
        salt: ByteArray,
        iterations: Int,
    ): ByteArray {
        if (!argon2Available()) {
            throw KeyExportException("Argon2id derivation unavailable")
        }
        val aliasBytes = alias.toByteArray(Charsets.UTF_8)
        val kdfSalt = ByteBuffer.allocate(salt.size + aliasBytes.size)
            .put(salt)
            .put(aliasBytes)
            .array()
        val passphraseBytes = encodeUtf8(passphrase)
        val kdfOutput = ByteArray(AES_KEY_LENGTH_BYTES)
        try {
            val paramsClass = Class.forName("org.bouncycastle.crypto.params.Argon2Parameters")
            val builderClass =
                Class.forName("org.bouncycastle.crypto.params.Argon2Parameters\$Builder")
            val argon2id = paramsClass.getField("ARGON2_id").getInt(null)
            val builder = builderClass.getConstructor(Int::class.javaPrimitiveType)
                .newInstance(argon2id)
            builderClass.getMethod("withSalt", ByteArray::class.java).invoke(builder, kdfSalt)
            builderClass.getMethod("withParallelism", Int::class.javaPrimitiveType)
                .invoke(builder, DEFAULT_ARGON2_PARALLELISM)
            builderClass.getMethod("withIterations", Int::class.javaPrimitiveType)
                .invoke(builder, iterations)
            builderClass.getMethod("withMemoryAsKB", Int::class.javaPrimitiveType)
                .invoke(builder, DEFAULT_ARGON2_MEMORY_KIB)
            val params = builderClass.getMethod("build").invoke(builder)
            val generatorClass =
                Class.forName("org.bouncycastle.crypto.generators.Argon2BytesGenerator")
            val generator = generatorClass.getConstructor().newInstance()
            generatorClass.getMethod("init", paramsClass).invoke(generator, params)
            generatorClass.getMethod("generateBytes", ByteArray::class.java, ByteArray::class.java)
                .invoke(generator, passphraseBytes, kdfOutput)
            val derived = hkdf(
                kdfOutput,
                sha256(hkdfSaltDomain(), aliasBytes),
                hkdfInfoDomain().toByteArray(Charsets.UTF_8),
                AES_KEY_LENGTH_BYTES,
            )
            kdfOutput.fill(0)
            return derived
        } catch (ex: ReflectiveOperationException) {
            throw KeyExportException("Argon2id derivation unavailable", ex)
        } finally {
            kdfSalt.fill(0)
            passphraseBytes.fill(0)
            kdfOutput.fill(0)
        }
    }

    private fun derivePbkdf2Key(
        alias: String,
        passphrase: CharArray,
        salt: ByteArray,
        iterations: Int,
    ): ByteArray {
        val aliasBytes = alias.toByteArray(Charsets.UTF_8)
        val kdfSalt = ByteBuffer.allocate(salt.size + aliasBytes.size)
            .put(salt)
            .put(aliasBytes)
            .array()
        var spec: PBEKeySpec? = null
        var kdfOutput: ByteArray? = null
        try {
            spec = PBEKeySpec(passphrase, kdfSalt, iterations, AES_KEY_LENGTH_BYTES * 8)
            val factory = SecretKeyFactory.getInstance(PBKDF2_ALGORITHM)
            kdfOutput = factory.generateSecret(spec).encoded
            val derived = hkdf(
                kdfOutput,
                sha256(hkdfSaltDomain(), aliasBytes),
                hkdfInfoDomain().toByteArray(Charsets.UTF_8),
                AES_KEY_LENGTH_BYTES,
            )
            kdfOutput.fill(0)
            return derived
        } catch (ex: GeneralSecurityException) {
            throw KeyExportException("PBKDF2 derivation failed", ex)
        } finally {
            spec?.clearPassword()
            kdfOutput?.fill(0)
            kdfSalt.fill(0)
        }
    }

    private fun hkdf(ikm: ByteArray, salt: ByteArray?, info: ByteArray, length: Int): ByteArray {
        try {
            val mac = Mac.getInstance(HMAC_ALGORITHM)
            val saltValue = salt?.copyOf() ?: ByteArray(mac.macLength)
            mac.init(SecretKeySpec(saltValue, HMAC_ALGORITHM))
            val prk = mac.doFinal(ikm)
            val result = ByteArray(length)
            var previous = ByteArray(0)
            var offset = 0
            var blockIndex = 1
            while (offset < length) {
                mac.init(SecretKeySpec(prk, HMAC_ALGORITHM))
                mac.update(previous)
                mac.update(info)
                mac.update(blockIndex.toByte())
                previous = mac.doFinal()
                val copy = minOf(previous.size, length - offset)
                System.arraycopy(previous, 0, result, offset, copy)
                offset += copy
                blockIndex++
            }
            prk.fill(0)
            previous.fill(0)
            return result
        } catch (ex: GeneralSecurityException) {
            throw KeyExportException("HKDF derivation failed", ex)
        }
    }

    private fun encodeUtf8(chars: CharArray): ByteArray {
        val charBuffer = CharBuffer.wrap(chars)
        val encoder = Charsets.UTF_8.newEncoder()
        val byteBuffer: ByteBuffer
        try {
            byteBuffer = encoder.encode(charBuffer)
        } catch (ex: CharacterCodingException) {
            throw IllegalArgumentException("Failed to encode passphrase as UTF-8", ex)
        }
        val out = ByteArray(byteBuffer.remaining())
        byteBuffer.get(out)
        if (byteBuffer.hasArray()) {
            byteBuffer.array().fill(0)
        }
        return out
    }

    private fun sha256(domain: String, data: ByteArray): ByteArray {
        try {
            val digest = MessageDigest.getInstance(DIGEST_ALGORITHM)
            digest.update(domain.toByteArray(Charsets.UTF_8))
            digest.update(data)
            return digest.digest()
        } catch (ex: GeneralSecurityException) {
            throw KeyExportException("SHA-256 digest failed", ex)
        }
    }

    private fun argon2Available(): Boolean {
        return try {
            Class.forName("org.bouncycastle.crypto.generators.Argon2BytesGenerator")
            true
        } catch (_: ClassNotFoundException) {
            false
        } catch (_: LinkageError) {
            false
        }
    }

    private fun hkdfSaltDomain(): String = "iroha-android-software-export-v3-salt"

    private fun hkdfInfoDomain(): String = "iroha-android-software-export-v3-info"

    private fun guardSaltNonceReuse(salt: ByteArray, nonce: ByteArray) {
        val fingerprintMaterial = ByteBuffer.allocate(salt.size + nonce.size)
            .put(salt)
            .put(nonce)
            .array()
        val fingerprint = HexFormat.of().formatHex(
            sha256("iroha-android-software-export-v3-fingerprint", fingerprintMaterial)
        )
        fingerprintMaterial.fill(0)
        synchronized(recentExportIndex) {
            if (recentExportIndex.contains(fingerprint)) {
                throw KeyExportException("Salt/nonce pair must not be reused across exports")
            }
            recentExportIndex.add(fingerprint)
            recentExportFingerprints.addLast(fingerprint)
            while (recentExportIndex.size > MAX_TRACKED_EXPORTS) {
                val evicted = recentExportFingerprints.removeFirst()
                recentExportIndex.remove(evicted)
            }
        }
    }

    private fun fillRandomNonZero(target: ByteArray) {
        do {
            SECURE_RANDOM.nextBytes(target)
        } while (isAllZero(target))
    }

    private fun isAllZero(data: ByteArray): Boolean {
        for (b in data) {
            if (b != 0.toByte()) return false
        }
        return true
    }

    private fun ensurePassphraseStrength(passphrase: CharArray) {
        if (passphrase.size < MIN_PASSPHRASE_LENGTH) {
            throw KeyExportException(
                "Passphrase must be at least $MIN_PASSPHRASE_LENGTH characters long"
            )
        }
    }

    private class KdfResult(val key: ByteArray, val kind: Int, val workFactor: Int) {
        fun zero() {
            key.fill(0)
        }
    }
}
