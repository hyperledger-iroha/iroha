@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.model.instructions.InstructionKind
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val ARG_WIRE_NAME = "wire_name"
private const val ARG_PAYLOAD_BASE64 = "payload_base64"

/**
 * Typed representation of an instruction scheduled for execution within a transaction.
 *
 * The container wraps strongly typed instruction payloads where available (for example, register
 * domain/account/asset definition builders) and can wrap opaque key/value payloads for local use.
 * Transaction encoding requires wire-framed instruction payloads so Norito parity is preserved
 * whenever raw instruction bytes are available.
 */
class InstructionBox private constructor(val payload: InstructionPayload) {

    /** Instruction display name (matches `InstructionType` tag). */
    val name: String
        get() = when (payload) {
            is WirePayload -> payload.wireName
            else -> kind.displayName
        }

    val kind: InstructionKind get() = payload.kind

    /**
     * Returns Norito-ready arguments describing the instruction. The returned map is immutable and
     * safe to cache by callers.
     */
    val arguments: Map<String, String> get() = payload.arguments

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is InstructionBox) return false
        return kind == other.kind && arguments == other.arguments
    }

    override fun hashCode(): Int {
        var result = kind.hashCode()
        result = 31 * result + arguments.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun of(payload: InstructionPayload): InstructionBox = InstructionBox(payload)

        /**
         * Builds an `InstructionBox` from a canonical wire identifier and its Norito-framed payload.
         *
         * The payload must include the Norito header with a valid checksum.
         */
        @JvmStatic
        fun fromWirePayload(wireName: String, payloadBytes: ByteArray): InstructionBox =
            InstructionBox(WireInstructionPayload(wireName, payloadBytes))

        /**
         * Builds an `InstructionBox` from Norito decoded components.
         *
         * The arguments must include `wire_name` and `payload_base64` fields carrying a
         * Norito-framed instruction payload.
         */
        @JvmStatic
        fun fromNorito(arguments: Map<String, String>): InstructionBox =
            InstructionBox(resolvePayload(arguments))

        private fun resolvePayload(arguments: Map<String, String>): InstructionPayload {
            val wireName = arguments[ARG_WIRE_NAME]
            val payloadBase64 = arguments[ARG_PAYLOAD_BASE64]
            require(wireName != null || payloadBase64 != null) {
                "instruction arguments must include wire_name and payload_base64 fields"
            }
            require(wireName != null && payloadBase64 != null) {
                "wire payload requires both wire_name and payload_base64 fields"
            }
            val payloadBytes = try {
                Base64.decode(payloadBase64)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("wire payload base64 is invalid", ex)
            }
            return WireInstructionPayload(wireName, payloadBytes)
        }
    }

    private class WireInstructionPayload(
        override val wireName: String,
        payloadBytes: ByteArray,
    ) : WirePayload {

        init {
            require(wireName.isNotBlank()) { "wireName must not be blank" }
            require(payloadBytes.isNotEmpty()) { "payloadBytes must not be empty" }
        }

        private val _payloadBytes: ByteArray = payloadBytes.copyOf()

        override val kind: InstructionKind = wireKindForName(wireName)

        override val arguments: Map<String, String> = mapOf(
            ARG_WIRE_NAME to wireName,
            ARG_PAYLOAD_BASE64 to Base64.encode(_payloadBytes),
        )

        override val payloadBytes: ByteArray get() = _payloadBytes.copyOf()
    }
}

private fun wireKindForName(wireName: String): InstructionKind {
    val normalized = wireName.lowercase()
    return when {
        normalized.startsWith("iroha.register") -> InstructionKind.REGISTER
        normalized.startsWith("iroha.unregister") -> InstructionKind.UNREGISTER
        normalized.startsWith("iroha.transfer") -> InstructionKind.TRANSFER
        normalized.startsWith("iroha.mint") -> InstructionKind.MINT
        normalized.startsWith("iroha.burn") -> InstructionKind.BURN
        normalized.startsWith("iroha.grant") -> InstructionKind.GRANT
        normalized.startsWith("iroha.revoke") -> InstructionKind.REVOKE
        normalized.startsWith("iroha.set_key_value") -> InstructionKind.SET_KEY_VALUE
        normalized.startsWith("iroha.remove_key_value") -> InstructionKind.REMOVE_KEY_VALUE
        normalized.startsWith("iroha.set_parameter") -> InstructionKind.SET_PARAMETER
        normalized.startsWith("iroha.execute_trigger") -> InstructionKind.EXECUTE_TRIGGER
        normalized.startsWith("iroha.log") -> InstructionKind.LOG
        normalized.startsWith("iroha.runtime_upgrade") || normalized.startsWith("iroha.upgrade") -> InstructionKind.UPGRADE
        else -> InstructionKind.CUSTOM
    }
}
