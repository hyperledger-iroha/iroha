package org.hyperledger.iroha.sdk.core.model.instructions

private const val BURN_TRIGGER_ACTION = "BurnTriggerRepetitions"

/** Typed representation of `BurnTriggerRepetitions` instructions. */
class BurnTriggerRepetitionsInstruction private constructor(
    val triggerId: String,
    val repetitions: Int,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.BURN

    override val arguments: Map<String, String> get() = _arguments

    constructor(triggerId: String, repetitions: Int) : this(
        triggerId = triggerId.also {
            require(it.isNotBlank()) { "triggerId must not be blank" }
        },
        repetitions = repetitions.also {
            require(it >= 0) { "repetitions must be non-negative" }
        },
        _arguments = linkedMapOf(
            "action" to BURN_TRIGGER_ACTION,
            "trigger" to triggerId,
            "repetitions" to Integer.toUnsignedString(repetitions),
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BurnTriggerRepetitionsInstruction) return false
        return repetitions == other.repetitions && triggerId == other.triggerId
    }

    override fun hashCode(): Int {
        var result = triggerId.hashCode()
        result = 31 * result + repetitions
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): BurnTriggerRepetitionsInstruction {
            return BurnTriggerRepetitionsInstruction(
                triggerId = require(arguments, "trigger"),
                repetitions = parseUnsignedInt(require(arguments, "repetitions")),
                _arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
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
