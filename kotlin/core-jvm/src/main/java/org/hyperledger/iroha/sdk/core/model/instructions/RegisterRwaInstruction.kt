package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterRwa"

/**
 * Typed representation of the `RegisterRwa` instruction.
 *
 * The payload stores the canonical JSON representation of `NewRwa`. Callers can surface the raw
 * JSON string via [rwaJson] and hand it to higher level decoders when richer inspection is
 * required.
 */
class RegisterRwaInstruction private constructor(
    /** Returns the canonical JSON representation of the `NewRwa` payload. */
    @JvmField val rwaJson: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind = InstructionKind.CUSTOM

    constructor(rwaJson: String) : this(
        rwaJson = validated(rwaJson, "rwaJson"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "rwa" to rwaJson,
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterRwaInstruction) return false
        return rwaJson == other.rwaJson
    }

    override fun hashCode(): Int = rwaJson.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterRwaInstruction {
            val rwaJson = require(arguments, "rwa")
            return RegisterRwaInstruction(
                rwaJson = rwaJson,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun validated(value: String, name: String): String {
            require(value.isNotBlank()) { "$name must not be blank" }
            return value
        }
    }
}
