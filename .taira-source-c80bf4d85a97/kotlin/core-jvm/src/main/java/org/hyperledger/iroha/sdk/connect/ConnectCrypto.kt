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
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address

/** Cryptographic helpers for the wallet-role Connect session. */
object ConnectCrypto {

    private const val KEY_LENGTH = 32
    private const val NONCE_LENGTH = 12
    private const val AEAD_TAG_BITS = 128
    private val X25519_HKDF_SALT = "iroha:x25519:hkdf:v1".toByteArray(StandardCharsets.UTF_8)
    private val X25519_HKDF_INFO = "iroha:x25519:session-key".toByteArray(StandardCharsets.UTF_8)
    private val RELAY_AUTH_DOMAIN = "iroha-connect|relay-auth|v1".toByteArray(StandardCharsets.UTF_8)

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
        try {
            agreement.calculateAgreement(peer, shared, 0)
        } catch (ex: RuntimeException) {
            Arrays.fill(shared, 0.toByte())
            throw ConnectProtocolException(
                "x25519 agreement failed (invalid public key or all-zero shared secret)",
                ex,
            )
        }
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

        val nonce = nonceFromSequence(sequence)
        val aad = buildAad(sessionId, direction, sequence)
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

        val nonce = nonceFromSequence(sequence)
        val aad = buildAad(sessionId, direction, sequence)
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
        relayAuthHash: ByteArray? = null,
    ): ByteArray {
        requireLength(sessionId, KEY_LENGTH, "sessionId")
        requireLength(appPublicKey, KEY_LENGTH, "appPublicKey")
        requireLength(walletPublicKey, KEY_LENGTH, "walletPublicKey")
        val normalizedAccountId = try {
            requireCanonicalI105Address(accountId ?: "", "accountId")
        } catch (ex: IllegalArgumentException) {
            throw ConnectProtocolException(
                ex.message ?: "accountId must use a canonical I105 encoded account literal",
                ex,
            )
        }

        val prefix = "iroha-connect|approve|v1".toByteArray(StandardCharsets.UTF_8)
        val accountBytes = normalizedAccountId.toByteArray(StandardCharsets.UTF_8)
        val fields = mutableListOf(
            "domain" to prefix,
            "sid" to sessionId,
            "app_pk" to appPublicKey,
            "wallet_pk" to walletPublicKey,
            "account_id" to accountBytes,
        )
        if (permissionsHash != null) fields += "permissions" to permissionsHash
        if (proofHash != null) fields += "proof" to proofHash
        if (relayAuthHash != null) fields += "relay_auth" to relayAuthHash

        var size = 0
        for ((tag, value) in fields) {
            size += 2 + tag.toByteArray(StandardCharsets.UTF_8).size + 8 + value.size
        }
        val buffer = ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN)
        for ((tag, value) in fields) {
            val tagBytes = tag.toByteArray(StandardCharsets.UTF_8)
            buffer.putShort(tagBytes.size.toShort())
            buffer.put(tagBytes)
            buffer.putLong(value.size.toLong())
            buffer.put(value)
        }
        return buffer.array()
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun relayAuthHash(sessionId: ByteArray, relayToken: String): ByteArray {
        requireLength(sessionId, KEY_LENGTH, "sessionId")
        if (relayToken.isBlank()) {
            throw ConnectProtocolException("relayToken must not be empty")
        }
        val tokenBytes = relayToken.toByteArray(StandardCharsets.UTF_8)
        val digest = SHA256Digest()
        digest.update(RELAY_AUTH_DOMAIN, 0, RELAY_AUTH_DOMAIN.size)
        digest.update(sessionId, 0, sessionId.size)
        digest.update(tokenBytes, 0, tokenBytes.size)
        val out = ByteArray(KEY_LENGTH)
        digest.doFinal(out, 0)
        return out
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun nonceFromSequence(sequence: Long): ByteArray {
        requireNonNegativeSequence(sequence)
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
    private fun requireNonNegativeSequence(sequence: Long) {
        if (sequence < 0L) {
            throw ConnectProtocolException("sequence must be non-negative")
        }
    }

    @Throws(ConnectProtocolException::class)
    private fun requireLength(value: ByteArray?, expected: Int, name: String) {
        if (value == null || value.size != expected) {
            val actual = value?.size ?: 0
            throw ConnectProtocolException("$name must contain $expected bytes (got $actual)")
        }
    }
}
