package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Typed builder for the `SetParameter` instruction.
 *
 * The payload stores the Norito-encoded parameter as canonical JSON. Callers can surface the raw
 * JSON string via [parameterJson] and hand it to higher level decoders when richer
 * inspection is required.
 */
class SetParameterInstruction private constructor(
    /** Returns the canonical JSON representation of the parameter that will be set. */
    val parameterJson: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.SET_PARAMETER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetParameterInstruction) return false
        return parameterJson == other.parameterJson
    }

    override fun hashCode(): Int = parameterJson.hashCode()

    class Builder internal constructor() {
        private var parameterJson: String? = null

        /**
         * Sets the canonical JSON representation of the parameter to apply.
         *
         * The JSON must be a single-key object whose key matches one of the supported parameter
         * families (for example `Sumeragi`, `Block`, or `Custom`). The value mirrors
         * the Norito payload accepted by Iroha.
         */
        fun setParameterJson(parameterJson: String) = apply {
            this.parameterJson = requireNotNull(parameterJson) { "parameterJson" }
        }

        fun build(): SetParameterInstruction {
            val json = parameterJson
            check(!json.isNullOrBlank()) { "parameterJson must be provided" }
            return SetParameterInstruction(json, canonicalArguments(json))
        }

        private fun canonicalArguments(json: String): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("parameter", json)
            }
    }

    companion object {
        private const val ACTION = "SetParameter"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetParameterInstruction {
            val json = require(arguments, "parameter")
            return SetParameterInstruction(json, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
