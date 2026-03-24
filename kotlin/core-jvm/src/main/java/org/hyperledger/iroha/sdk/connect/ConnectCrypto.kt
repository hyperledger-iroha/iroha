package org.hyperledger.iroha.sdk.connect

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import java.util.Arrays
import org.bouncycastle.crypto.InvalidCipherTextException
import org.bouncycastle.crypto.agreement.X25519Agreement
import org.bouncycastle.crypto.digests.Blake2bDigest
import org.bouncycastle.crypto.digests.SHA256Digest
import org.bouncycastle.crypto.generators.HKDFBytesGenerator
import org.bouncycastle.crypto.modes.ChaCha20Poly1305
import org.bouncycastle.crypto.params.AEADParameters
import org.bouncycastle.crypto.params.HKDFParameters
import org.bouncycastle.crypto.params.KeyParameter
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.bouncycastle.crypto.params.X25519PublicKeyParameters

/** Cryptographic helpers for the wallet-role Connect session. */
object ConnectCrypto {

    private const val KEY_LENGTH = 32
    private const val NONCE_LENGTH = 12
    private const val AEAD_TAG_BITS = 128
    private val X25519_HKDF_SALT = "iroha:x25519:hkdf:v1".toByteArray(StandardCharsets.UTF_8)
    private val X25519_HKDF_INFO = "iroha:x25519:session-key".toByteArray(StandardCharsets.UTF_8)

    class KeyPair internal constructor(publicKey: ByteArray, privateKey: ByteArray) {
        private val _publicKey: ByteArray = publicKey.copyOf()
        private val _privateKey: ByteArray = privateKey.copyOf()

        fun publicKey(): ByteArray = _publicKey.clone()
        fun privateKey(): ByteArray = _privateKey.clone()
    }

    class DirectionKeys internal constructor(appToWallet: ByteArray, walletToApp: ByteArray) {
        private val _appToWallet: ByteArray = appToWallet.copyOf()
        private val _walletToApp: ByteArray = walletToApp.copyOf()

        fun appToWallet(): ByteArray = _appToWallet.clone()
        fun walletToApp(): ByteArray = _walletToApp.clone()

        fun keyForDirection(direction: ConnectDirection): ByteArray =
            if (direction == ConnectDirection.APP_TO_WALLET) appToWallet() else walletToApp()
    }

    @JvmStatic
    fun generateKeyPair(): KeyPair {
        val privateKey = X25519PrivateKeyParameters(SecureRandom())
        val publicKey = privateKey.generatePublicKey()
        val privateBytes = ByteArray(KEY_LENGTH)
        val publicBytes = ByteArray(KEY_LENGTH)
        privateKey.encode(privateBytes, 0)
        publicKey.encode(publicBytes, 0)
        return KeyPair(publicBytes, privateBytes)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun deriveDirectionKeys(
        localPrivateKey: ByteArray,
        peerPublicKey: ByteArray,
        sessionId: ByteArray,
    ): DirectionKeys {
        requireLength(localPrivateKey, KEY_LENGTH, "localPrivateKey")
        requireLength(peerPublicKey, KEY_LENGTH, "peerPublicKey")
        requireLength(sessionId, KEY_LENGTH, "sessionId")

        val local = X25519PrivateKeyParameters(localPrivateKey, 0)
        val peer = X25519PublicKeyParameters(peerPublicKey, 0)
        val agreement = X25519Agreement()
        agreement.init(local)
        val shared = ByteArray(KEY_LENGTH)
        agreement.calculateAgreement(peer, shared, 0)
        if (isAllZero(shared)) {
            Arrays.fill(shared, 0.toByte())
            throw ConnectProtocolException("x25519 shared secret is all-zero (invalid public key)")
        }

        val sessionKey = hkdfExpand(shared, X25519_HKDF_SALT, X25519_HKDF_INFO)
        val salt = blake2b32(
            "iroha-connect|salt|".toByteArray(StandardCharsets.UTF_8),
            sessionId,
        )
        val appKey = hkdfExpand(
            sessionKey, salt,
            "iroha-connect|k_app".toByteArray(StandardCharsets.UTF_8),
        )
        val walletKey = hkdfExpand(
            sessionKey, salt,
            "iroha-connect|k_wallet".toByteArray(StandardCharsets.UTF_8),
        )
        Arrays.fill(sessionKey, 0.toByte())
        Arrays.fill(shared, 0.toByte())
        return DirectionKeys(appKey, walletKey)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encryptEnvelope(
        envelope: ByteArray,
        key: ByteArray,
        sessionId: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
    ): ByteArray {
        requireLength(key, KEY_LENGTH, "key")
        requireLength(sessionId, KEY_LENGTH, "sessionId")

        val aad = buildAad(sessionId, direction, sequence)
        val nonce = nonceFromSequence(sequence)
        return runAead(true, key, nonce, aad, envelope)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun decryptCiphertext(
        ciphertext: ByteArray,
        key: ByteArray,
        sessionId: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
    ): ByteArray {
        requireLength(key, KEY_LENGTH, "key")
        requireLength(sessionId, KEY_LENGTH, "sessionId")

        val aad = buildAad(sessionId, direction, sequence)
        val nonce = nonceFromSequence(sequence)
        return runAead(false, key, nonce, aad, ciphertext)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun buildApprovePreimage(
        sessionId: ByteArray,
        appPublicKey: ByteArray,
        walletPublicKey: ByteArray,
        accountId: String?,
        permissionsHash: ByteArray?,
        proofHash: ByteArray?,
    ): ByteArray {
        requireLength(sessionId, KEY_LENGTH, "sessionId")
        requireLength(appPublicKey, KEY_LENGTH, "appPublicKey")
        requireLength(walletPublicKey, KEY_LENGTH, "walletPublicKey")
        if (accountId.isNullOrBlank()) {
            throw ConnectProtocolException("accountId must not be empty")
        }

        val prefix = "iroha-connect|approve|".toByteArray(StandardCharsets.UTF_8)
        val accountBytes = accountId.toByteArray(StandardCharsets.UTF_8)
        var size = prefix.size + sessionId.size + appPublicKey.size +
            walletPublicKey.size + accountBytes.size
        if (permissionsHash != null) size += permissionsHash.size
        if (proofHash != null) size += proofHash.size

        val buffer = ByteBuffer.allocate(size)
        buffer.put(prefix)
        buffer.put(sessionId)
        buffer.put(appPublicKey)
        buffer.put(walletPublicKey)
        buffer.put(accountBytes)
        if (permissionsHash != null) buffer.put(permissionsHash)
        if (proofHash != null) buffer.put(proofHash)
        return buffer.array()
    }

    @JvmStatic
    fun nonceFromSequence(sequence: Long): ByteArray {
        val nonce = ByteArray(NONCE_LENGTH)
        val buffer = ByteBuffer.wrap(nonce).order(ByteOrder.LITTLE_ENDIAN)
        buffer.putInt(0)
        buffer.putLong(sequence)
        return nonce
    }

    private fun buildAad(
        sessionId: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
    ): ByteArray {
        val prefix = "connect:v1".toByteArray(StandardCharsets.UTF_8)
        val buffer = ByteBuffer
            .allocate(prefix.size + KEY_LENGTH + 1 + Long.SIZE_BYTES + 1)
            .order(ByteOrder.LITTLE_ENDIAN)
        buffer.put(prefix)
        buffer.put(sessionId)
        buffer.put(if (direction == ConnectDirection.APP_TO_WALLET) 0.toByte() else 1.toByte())
        buffer.putLong(sequence)
        buffer.put(1.toByte())
        return buffer.array()
    }

    @Throws(ConnectProtocolException::class)
    private fun runAead(
        encrypt: Boolean,
        key: ByteArray,
        nonce: ByteArray,
        aad: ByteArray,
        input: ByteArray,
    ): ByteArray {
        try {
            val cipher = ChaCha20Poly1305()
            val params = AEADParameters(KeyParameter(key), AEAD_TAG_BITS, nonce, aad)
            cipher.init(encrypt, params)
            val out = ByteArray(cipher.getOutputSize(input.size))
            var written = cipher.processBytes(input, 0, input.size, out, 0)
            written += cipher.doFinal(out, written)
            return out.copyOf(written)
        } catch (ex: InvalidCipherTextException) {
            throw ConnectProtocolException(
                if (encrypt) "Connect encryption failed" else "Connect decryption failed", ex,
            )
        } catch (ex: RuntimeException) {
            throw ConnectProtocolException("Connect AEAD failure", ex)
        }
    }

    @Throws(ConnectProtocolException::class)
    private fun hkdfExpand(ikm: ByteArray, salt: ByteArray, info: ByteArray): ByteArray {
        try {
            val hkdf = HKDFBytesGenerator(SHA256Digest())
            hkdf.init(HKDFParameters(ikm, salt, info))
            val out = ByteArray(KEY_LENGTH)
            hkdf.generateBytes(out, 0, out.size)
            return out
        } catch (ex: RuntimeException) {
            throw ConnectProtocolException("Connect HKDF expansion failed", ex)
        }
    }

    private fun blake2b32(vararg segments: ByteArray): ByteArray {
        val digest = Blake2bDigest(256)
        for (segment in segments) {
            digest.update(segment, 0, segment.size)
        }
        val out = ByteArray(KEY_LENGTH)
        digest.doFinal(out, 0)
        return out
    }

    private fun isAllZero(value: ByteArray): Boolean {
        for (b in value) {
            if (b.toInt() != 0) return false
        }
        return true
    }

    @Throws(ConnectProtocolException::class)
    private fun requireLength(value: ByteArray?, expected: Int, name: String) {
        if (value == null || value.size != expected) {
            val actual = value?.size ?: 0
            throw ConnectProtocolException("$name must contain $expected bytes (got $actual)")
        }
    }
}
