@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.crypto.export

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.nio.ByteBuffer
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private val MAGIC = "IRKEY".toByteArray(Charsets.UTF_8)

/**
 * Encapsulates the deterministic representation of an exported key.
 *
 * The bundle layout is stable and versioned so future tooling (Rust bindings, Android clients) can
 * decode the same payload deterministically:
 *
 * ```
 * magic[5] = "IRKEY"
 * version[1] = 0x03
 * alias_len[2] (big-endian)
 * alias_bytes
 * public_key_len[2]
 * public_key_bytes
 * nonce_len[1]
 * nonce_bytes
 * ciphertext_len[2]
 * ciphertext_bytes (GCM ciphertext + tag)
 * salt_len[1] (v2+)
 * salt_bytes (v2+)
 * kdf_kind[1] (v2+)
 * kdf_work_factor[4] (big-endian, v2+)
 * ```
 */
class KeyExportBundle internal constructor(
    val alias: String,
    publicKey: ByteArray,
    nonce: ByteArray,
    ciphertext: ByteArray,
    salt: ByteArray?,
    val kdfKind: Int,
    val kdfWorkFactor: Int,
    val version: Byte,
) {
    private val _publicKey: ByteArray = publicKey.copyOf()
    private val _nonce: ByteArray = nonce.copyOf()
    private val _ciphertext: ByteArray = ciphertext.copyOf()
    private val _salt: ByteArray = salt?.copyOf() ?: ByteArray(0)

    val publicKey: ByteArray get() = _publicKey.copyOf()
    val nonce: ByteArray get() = _nonce.copyOf()
    val ciphertext: ByteArray get() = _ciphertext.copyOf()
    val salt: ByteArray get() = _salt.copyOf()

    /** Serializes the bundle to a base64 string with the canonical layout. */
    fun encodeBase64(): String = Base64.encode(encode())

    /** Serializes the bundle to its canonical byte representation. */
    fun encode(): ByteArray {
        val aliasBytes = alias.toByteArray(Charsets.UTF_8)
        require(aliasBytes.size <= 0xFFFF) { "alias is too long" }
        require(version == VERSION_V3) { "unsupported key export version: $version" }
        require(_publicKey.size <= 0xFFFF) { "publicKey is too long" }
        require(_ciphertext.size <= 0xFFFF) { "ciphertext is too long" }
        require(_nonce.size == EXPECTED_NONCE_LENGTH_BYTES) {
            "nonce must be $EXPECTED_NONCE_LENGTH_BYTES bytes, found ${_nonce.size}"
        }
        require(_salt.size == EXPECTED_SALT_LENGTH_BYTES) {
            "salt must be $EXPECTED_SALT_LENGTH_BYTES bytes, found ${_salt.size}"
        }
        require(kdfWorkFactor >= 0) { "kdfWorkFactor must be non-negative" }

        val output = ByteArrayOutputStream()
        try {
            output.write(MAGIC)
            output.write(version.toInt())
            output.write(shortBytes(aliasBytes.size))
            output.write(aliasBytes)
            output.write(shortBytes(_publicKey.size))
            output.write(_publicKey)
            output.write(_nonce.size)
            output.write(_nonce)
            output.write(shortBytes(_ciphertext.size))
            output.write(_ciphertext)
            output.write(_salt.size)
            output.write(_salt)
            output.write(kdfKind and 0xFF)
            output.write(intBytes(kdfWorkFactor))
        } catch (ex: IOException) {
            throw IllegalStateException("Unexpected I/O error writing bundle", ex)
        }
        return output.toByteArray()
    }

    companion object {
        const val VERSION_V3: Byte = 3
        const val EXPECTED_NONCE_LENGTH_BYTES: Int = 12
        const val EXPECTED_SALT_LENGTH_BYTES: Int = 16

        /** Decodes a bundle from the canonical base64 representation. */
        @JvmStatic
        @Throws(KeyExportException::class)
        fun decodeBase64(encoded: String): KeyExportBundle {
            try {
                val raw = Base64.decode(encoded)
                return decode(raw)
            } catch (ex: IllegalArgumentException) {
                throw KeyExportException("Key export bundle is not valid base64", ex)
            }
        }

        /** Decodes a bundle from the canonical byte representation. */
        @JvmStatic
        @Throws(KeyExportException::class)
        fun decode(encoded: ByteArray): KeyExportBundle {
            try {
                ByteArrayInputStream(encoded).use { input ->
                    val magic = readExactly(input,MAGIC.size)
                    if (!MAGIC.contentEquals(magic)) {
                        throw KeyExportException("Key export bundle magic mismatch")
                    }
                    val version = input.read()
                    if (version != VERSION_V3.toInt()) {
                        throw KeyExportException(
                            "Unsupported key export version: ${if (version < 0) "EOF" else version}"
                        )
                    }
                    val aliasLength = readShort(input)
                    val aliasBytes = readExactly(input,aliasLength)
                    if (aliasBytes.size != aliasLength) {
                        throw KeyExportException("Unexpected end of stream while reading alias")
                    }
                    val pubKeyLength = readShort(input)
                    val pubKey = readExactly(input,pubKeyLength)
                    if (pubKey.size != pubKeyLength) {
                        throw KeyExportException("Unexpected end of stream while reading public key")
                    }
                    val nonceLength = input.read()
                    if (nonceLength <= 0) {
                        throw KeyExportException("Nonce must be present in export bundle")
                    }
                    if (nonceLength != EXPECTED_NONCE_LENGTH_BYTES) {
                        throw KeyExportException(
                            "Nonce length mismatch: expected $EXPECTED_NONCE_LENGTH_BYTES bytes, found $nonceLength"
                        )
                    }
                    val nonce = readExactly(input,nonceLength)
                    if (nonce.size != nonceLength) {
                        throw KeyExportException("Unexpected end of stream while reading nonce")
                    }
                    val cipherLength = readShort(input)
                    val cipher = readExactly(input,cipherLength)
                    if (cipher.size != cipherLength) {
                        throw KeyExportException("Unexpected end of stream while reading ciphertext")
                    }
                    val saltLength = input.read()
                    if (saltLength < 0) {
                        throw KeyExportException("Unexpected end of stream while reading salt length")
                    }
                    val salt = readExactly(input,saltLength)
                    if (salt.size != saltLength) {
                        throw KeyExportException("Unexpected end of stream while reading salt")
                    }
                    if (saltLength != EXPECTED_SALT_LENGTH_BYTES) {
                        throw KeyExportException(
                            "Salt length mismatch: expected $EXPECTED_SALT_LENGTH_BYTES bytes, found $saltLength"
                        )
                    }
                    val kdfKind = input.read()
                    if (kdfKind < 0) {
                        throw KeyExportException("Unexpected end of stream while reading kdf kind")
                    }
                    val workFactorBytes = readExactly(input,4)
                    if (workFactorBytes.size != 4) {
                        throw KeyExportException("Unexpected end of stream while reading kdf work factor")
                    }
                    val kdfWorkFactor = ByteBuffer.wrap(workFactorBytes).getInt()
                    if (input.read() != -1) {
                        throw KeyExportException("Trailing data found in key export bundle")
                    }
                    return KeyExportBundle(
                        String(aliasBytes, Charsets.UTF_8),
                        pubKey,
                        nonce,
                        cipher,
                        salt,
                        kdfKind,
                        kdfWorkFactor,
                        version.toByte(),
                    )
                }
            } catch (ex: IOException) {
                throw KeyExportException("Failed to decode key export bundle", ex)
            }
        }

        private fun shortBytes(value: Int): ByteArray =
            ByteBuffer.allocate(2).putShort(value.toShort()).array()

        private fun intBytes(value: Int): ByteArray =
            ByteBuffer.allocate(4).putInt(value).array()

        private fun readExactly(input: InputStream, length: Int): ByteArray {
            val buffer = ByteArray(length)
            var offset = 0
            while (offset < length) {
                val read = input.read(buffer, offset, length - offset)
                if (read < 0) break
                offset += read
            }
            return if (offset == length) buffer else buffer.copyOf(offset)
        }

        private fun readShort(input: ByteArrayInputStream): Int {
            val buffer = readExactly(input,2)
            if (buffer.size != 2) {
                throw IOException("Unexpected end of stream while reading length")
            }
            return ByteBuffer.wrap(buffer).getShort().toInt() and 0xFFFF
        }
    }
}
