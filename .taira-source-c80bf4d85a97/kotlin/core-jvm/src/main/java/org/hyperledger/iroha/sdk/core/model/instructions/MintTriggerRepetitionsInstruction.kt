package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "MintTriggerRepetitions"

/** Typed representation of a `MintTriggerRepetitions` instruction. */
class MintTriggerRepetitionsInstruction(
    @JvmField val triggerId: String,
    @JvmField val repetitions: Int,
) : InstructionTemplate {

    init {
        require(triggerId.isNotBlank()) { "triggerId must not be blank" }
        require(repetitions >= 0) { "repetitions must be non-negative" }
    }

    override val kind: InstructionKind = InstructionKind.MINT

    override val arguments: Map<String, String> by lazy {
        linkedMapOf(
            "action" to ACTION,
            "trigger" to triggerId,
            "repetitions" to Integer.toUnsignedString(repetitions),
        )
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MintTriggerRepetitionsInstruction) return false
        return triggerId == other.triggerId && repetitions == other.repetitions
    }

    override fun hashCode(): Int {
        var result = triggerId.hashCode()
        result = 31 * result + repetitions
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MintTriggerRepetitionsInstruction {
            val triggerId = requireArg(arguments, "trigger")
            val repetitions = parseUnsignedInt(requireArg(arguments, "repetitions"))
            return MintTriggerRepetitionsInstruction(triggerId = triggerId, repetitions = repetitions)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }

        private fun parseUnsignedInt(value: String): Int {
            try {
                val parsed = Integer.parseUnsignedInt(value)
                if (parsed < 0) throw NumberFormatException("negative")
                return parsed
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("repetitions must be an unsigned integer", ex)
            }
        }
    }
}
