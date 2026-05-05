@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val ACTION = "ProposeRuntimeUpgrade"
private const val MANIFEST_BYTES_BASE64 = "manifest_bytes_base64"

/**
 * Typed representation of a `ProposeRuntimeUpgrade` instruction.
 *
 * Encodes canonical runtime-upgrade manifest bytes using base64 so Android callers can round-trip
 * the same payloads Norito expects on the wire.
 */
class ProposeRuntimeUpgradeInstruction(
    manifestBytes: ByteArray,
) : InstructionTemplate {

    private val _manifestBytes: ByteArray = manifestBytes.copyOf()

    init {
        require(_manifestBytes.isNotEmpty()) { "manifestBytes must not be empty" }
    }

    val manifestBytes: ByteArray get() = _manifestBytes.copyOf()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy {
        linkedMapOf(
            "action" to ACTION,
            MANIFEST_BYTES_BASE64 to Base64.encode(_manifestBytes),
        )
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ProposeRuntimeUpgradeInstruction) return false
        return _manifestBytes.contentEquals(other._manifestBytes)
    }

    override fun hashCode(): Int = _manifestBytes.contentHashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ProposeRuntimeUpgradeInstruction {
            val encoded = requireArg(arguments, MANIFEST_BYTES_BASE64)
            val decoded = try {
                Base64.decode(encoded)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$MANIFEST_BYTES_BASE64 must be base64 encoded", ex)
            }
            return ProposeRuntimeUpgradeInstruction(manifestBytes = decoded)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }
    }
}
