package org.hyperledger.iroha.sdk.telemetry

import java.nio.charset.StandardCharsets
import java.util.Optional
import org.hyperledger.iroha.sdk.crypto.Blake2b

private const val DEFAULT_ALGORITHM = "blake2b-256"

/**
 * Redaction policy shared between Android telemetry emitters and the CLI tooling that validates
 * parity with the Rust baseline.
 */
class Redaction(
    @JvmField val enabled: Boolean,
    salt: ByteArray,
    @JvmField val saltVersion: String,
    @JvmField val algorithm: String = DEFAULT_ALGORITHM,
    rotationId: String = "",
) {
    private val _salt: ByteArray = salt.copyOf()

    @JvmField val rotationId: String =
        if (rotationId.isEmpty()) saltVersion else rotationId

    init {
        if (enabled) {
            check(_salt.isNotEmpty()) { "Telemetry redaction requires a non-empty salt" }
            check(saltVersion.isNotEmpty()) { "Telemetry redaction requires a salt version" }
            check(this.rotationId.isNotEmpty()) { "Telemetry redaction requires a rotation id" }
            check(DEFAULT_ALGORITHM.equals(algorithm, ignoreCase = true)) {
                "Unsupported telemetry hash algorithm: $algorithm"
            }
        }
    }

    /** Returns a copy of the salt bytes. */
    fun salt(): ByteArray = _salt.copyOf()

    /**
     * Hashes `authority` using the configured salt and algorithm.
     *
     * @return Empty when disabled or when `authority` is blank.
     */
    fun hashAuthority(authority: String?): Optional<String> {
        if (!enabled) return Optional.empty()
        if (authority == null) return Optional.empty()
        val normalised = authority.trim()
        if (normalised.isEmpty()) return Optional.empty()
        val authorityBytes = normalised.lowercase().toByteArray(StandardCharsets.UTF_8)
        return Optional.of(bytesToHex(hashWithSalt(authorityBytes)))
    }

    /**
     * Hashes `identifier` using the configured salt and algorithm. Unlike `hashAuthority`
     * the input casing is preserved to allow stack trace/crash identifiers
     * that already ship in canonical form.
     *
     * @return Empty when disabled or when `identifier` is blank.
     */
    fun hashIdentifier(identifier: String?): Optional<String> {
        if (!enabled) return Optional.empty()
        if (identifier == null) return Optional.empty()
        val normalised = identifier.trim()
        if (normalised.isEmpty()) return Optional.empty()
        val identifierBytes = normalised.toByteArray(StandardCharsets.UTF_8)
        return Optional.of(bytesToHex(hashWithSalt(identifierBytes)))
    }

    private fun hashWithSalt(payload: ByteArray): ByteArray {
        val input = ByteArray(_salt.size + payload.size)
        System.arraycopy(_salt, 0, input, 0, _salt.size)
        System.arraycopy(payload, 0, input, _salt.size, payload.size)
        return hash(input)
    }

    private fun hash(input: ByteArray): ByteArray {
        check(DEFAULT_ALGORITHM.equals(algorithm, ignoreCase = true)) {
            "Unsupported telemetry hash algorithm: $algorithm"
        }
        return Blake2b.digest256(input)
    }

    class Builder {
        private var salt: ByteArray = ByteArray(0)
        private var saltVersion: String = ""
        private var algorithm: String = DEFAULT_ALGORITHM
        private var rotationId: String = ""

        fun setSalt(salt: ByteArray): Builder { this.salt = salt.copyOf(); return this }
        fun setSaltHex(hex: String): Builder { this.salt = decodeHex(hex); return this }
        fun setSaltVersion(saltVersion: String): Builder { this.saltVersion = saltVersion; return this }
        fun setRotationId(rotationId: String): Builder { this.rotationId = rotationId; return this }
        fun setAlgorithm(algorithm: String): Builder { this.algorithm = algorithm; return this }
        fun build(): Redaction = Redaction(enabled = true, salt = salt, saltVersion = saltVersion, algorithm = algorithm, rotationId = rotationId)
    }

    companion object {
        /** Returns a disabled redaction policy. */
        @JvmStatic
        fun disabled(): Redaction =
            Redaction(
                enabled = false,
                salt = ByteArray(0),
                saltVersion = "none",
                algorithm = DEFAULT_ALGORITHM,
                rotationId = "none",
            )

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun decodeHex(hex: String): ByteArray {
            val trimmed = hex.trim()
            require(trimmed.length and 1 != 1) {
                "Hex salt must contain an even number of characters"
            }
            val out = ByteArray(trimmed.length / 2)
            for (i in trimmed.indices step 2) {
                val hi = Character.digit(trimmed[i], 16)
                val lo = Character.digit(trimmed[i + 1], 16)
                require(hi >= 0 && lo >= 0) { "Salt contains non-hexadecimal characters" }
                out[i / 2] = ((hi shl 4) + lo).toByte()
            }
            return out
        }

        private fun bytesToHex(data: ByteArray): String {
            val builder = StringBuilder(data.size * 2)
            for (value in data) {
                builder.append(String.format(java.util.Locale.ROOT, "%02x", value))
            }
            return builder.toString()
        }
    }
}
