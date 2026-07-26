package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Base64

private const val APPROVE_PIN_ACTION = "ApprovePinManifest"

/**
 * Typed representation of the `ApprovePinManifest` instruction (SoraFS manifest lifecycle).
 */
class ApprovePinManifestInstruction private constructor(
    val digestHex: String,
    val approvedEpoch: Long,
    val councilEnvelopeBase64: String?,
    val councilEnvelopeDigestHex: String?,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(
        digestHex: String,
        approvedEpoch: Long,
        councilEnvelopeBase64: String? = null,
        councilEnvelopeDigestHex: String? = null,
    ) : this(
        digestHex = digestHex.also {
            require(it.isNotBlank()) { "digestHex must not be blank" }
        },
        approvedEpoch = approvedEpoch.also {
            require(it >= 0) { "approvedEpoch must be non-negative" }
        },
        councilEnvelopeBase64 = councilEnvelopeBase64?.let { requireBase64(it, "councilEnvelopeBase64") },
        councilEnvelopeDigestHex = councilEnvelopeDigestHex,
        _arguments = buildCanonicalArguments(digestHex, approvedEpoch, councilEnvelopeBase64, councilEnvelopeDigestHex),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ApprovePinManifestInstruction) return false
        return approvedEpoch == other.approvedEpoch
            && digestHex == other.digestHex
            && councilEnvelopeBase64 == other.councilEnvelopeBase64
            && councilEnvelopeDigestHex == other.councilEnvelopeDigestHex
    }

    override fun hashCode(): Int {
        var result = digestHex.hashCode()
        result = 31 * result + approvedEpoch.hashCode()
        result = 31 * result + (councilEnvelopeBase64?.hashCode() ?: 0)
        result = 31 * result + (councilEnvelopeDigestHex?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ApprovePinManifestInstruction {
            val digestHex = require(arguments, "digest_hex")
            val approvedEpoch = requireLong(arguments, "approved_epoch")
            val envelope = arguments["council_envelope_base64"]?.takeIf { it.isNotBlank() }
            val envelopeDigest = arguments["council_envelope_digest_hex"]?.takeIf { it.isNotBlank() }
            return ApprovePinManifestInstruction(
                digestHex = digestHex,
                approvedEpoch = approvedEpoch,
                councilEnvelopeBase64 = envelope,
                councilEnvelopeDigestHex = envelopeDigest,
                _arguments = LinkedHashMap(arguments),
            )
        }

        @JvmStatic
        fun fromEnvelopeBytes(
            digestHex: String,
            approvedEpoch: Long,
            councilEnvelopeBytes: ByteArray,
            councilEnvelopeDigestHex: String? = null,
        ): ApprovePinManifestInstruction = ApprovePinManifestInstruction(
            digestHex = digestHex,
            approvedEpoch = approvedEpoch,
            councilEnvelopeBase64 = Base64.getEncoder().encodeToString(councilEnvelopeBytes),
            councilEnvelopeDigestHex = councilEnvelopeDigestHex,
        )

        private fun buildCanonicalArguments(
            digestHex: String,
            approvedEpoch: Long,
            councilEnvelopeBase64: String?,
            councilEnvelopeDigestHex: String?,
        ): Map<String, String> {
            val args = linkedMapOf(
                "action" to APPROVE_PIN_ACTION,
                "digest_hex" to digestHex,
                "approved_epoch" to approvedEpoch.toString(),
            )
            if (!councilEnvelopeBase64.isNullOrBlank()) {
                args["council_envelope_base64"] = councilEnvelopeBase64
            }
            if (!councilEnvelopeDigestHex.isNullOrBlank()) {
                args["council_envelope_digest_hex"] = councilEnvelopeDigestHex
            }
            return args
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = require(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a number: $value", ex,
                )
            }
        }

        private fun requireBase64(value: String, fieldName: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$fieldName must not be blank" }
            val decoded: ByteArray
            try {
                decoded = Base64.getDecoder().decode(trimmed)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$fieldName must be base64", ex)
            }
            require(decoded.isNotEmpty()) { "$fieldName must decode to non-empty bytes" }
            return trimmed
        }
    }
}
