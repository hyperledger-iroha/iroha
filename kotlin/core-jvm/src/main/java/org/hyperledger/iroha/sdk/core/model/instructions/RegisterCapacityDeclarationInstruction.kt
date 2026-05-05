package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val ACTION = "RegisterCapacityDeclaration"

/** Typed representation of the `RegisterCapacityDeclaration` instruction (SoraFS capacity registry). */
class RegisterCapacityDeclarationInstruction(
    @JvmField val providerIdHex: String,
    @JvmField val declarationBase64: String,
    @JvmField val committedCapacityGib: Long,
    @JvmField val registeredEpoch: Long,
    @JvmField val validFromEpoch: Long,
    @JvmField val validUntilEpoch: Long,
    metadata: Map<String, String> = emptyMap(),
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()
    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    val metadata: Map<String, String> get() = _metadata

    override val kind: InstructionKind = InstructionKind.REGISTER

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(providerIdHex.isNotBlank()) { "providerIdHex must not be blank" }
        require(declarationBase64.isNotBlank()) { "declarationBase64 must not be blank" }
        require(committedCapacityGib >= 0) { "committedCapacityGib must be non-negative" }
        require(registeredEpoch >= 0) { "registeredEpoch must be non-negative" }
        require(validFromEpoch >= 0) { "validFromEpoch must be non-negative" }
        require(validUntilEpoch >= 0) { "validUntilEpoch must be non-negative" }
        require(validUntilEpoch >= validFromEpoch) { "validUntilEpoch must be >= validFromEpoch" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterCapacityDeclarationInstruction) return false
        return committedCapacityGib == other.committedCapacityGib
            && registeredEpoch == other.registeredEpoch
            && validFromEpoch == other.validFromEpoch
            && validUntilEpoch == other.validUntilEpoch
            && providerIdHex == other.providerIdHex
            && declarationBase64 == other.declarationBase64
            && _metadata == other._metadata
    }

    override fun hashCode(): Int = listOf(
        providerIdHex, declarationBase64, committedCapacityGib,
        registeredEpoch, validFromEpoch, validUntilEpoch, _metadata,
    ).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        put("provider_id_hex", providerIdHex)
        put("declaration_b64", declarationBase64)
        put("committed_capacity_gib", committedCapacityGib.toString())
        put("registered_epoch", registeredEpoch.toString())
        put("valid_from_epoch", validFromEpoch.toString())
        put("valid_until_epoch", validUntilEpoch.toString())
        _metadata.forEach { (key, value) -> put("metadata.$key", value) }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterCapacityDeclarationInstruction {
            val metadata = buildMap {
                arguments.forEach { (key, value) ->
                    if (key.startsWith("metadata.")) {
                        put(key.removePrefix("metadata."), value)
                    }
                }
            }
            return RegisterCapacityDeclarationInstruction(
                providerIdHex = requireArgument(arguments, "provider_id_hex"),
                declarationBase64 = requireArgument(arguments, "declaration_b64"),
                committedCapacityGib = requireLong(arguments, "committed_capacity_gib"),
                registeredEpoch = requireLong(arguments, "registered_epoch"),
                validFromEpoch = requireLong(arguments, "valid_from_epoch"),
                validUntilEpoch = requireLong(arguments, "valid_until_epoch"),
                metadata = metadata,
                arguments = arguments,
            )
        }

        @OptIn(ExperimentalEncodingApi::class)
        @JvmStatic
        fun requireBase64(value: String, fieldName: String): String {
            val trimmed = value.trim()
            require(trimmed.isNotEmpty()) { "$fieldName must not be blank" }
            val decoded = try {
                Base64.decode(trimmed)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$fieldName must be base64", ex)
            }
            require(decoded.isNotEmpty()) { "$fieldName must decode to non-empty bytes" }
            return trimmed
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val raw = requireArgument(arguments, key)
            return raw.toLongOrNull()
                ?: throw IllegalArgumentException("Instruction argument '$key' must be a number: $raw")
        }
    }
}
