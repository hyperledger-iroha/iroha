package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed representation of an `ExecuteTrigger` instruction. */
class ExecuteTriggerInstruction(
    val triggerId: String,
    val authorityOverride: String? = null,
) : InstructionTemplate {

    init {
        require(triggerId.isNotBlank()) { "triggerId must not be blank" }
        if (authorityOverride != null) {
            require(authorityOverride.isNotBlank()) {
                "authorityOverride must not be blank when present"
            }
        }
    }

    override val kind: InstructionKind = InstructionKind.EXECUTE_TRIGGER

    override val arguments: Map<String, String> = buildMap {
        put("action", "ExecuteTrigger")
        put("trigger", triggerId)
        if (authorityOverride != null) {
            put("authority", authorityOverride)
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ExecuteTriggerInstruction) return false
        return triggerId == other.triggerId && authorityOverride == other.authorityOverride
    }

    override fun hashCode(): Int {
        var result = triggerId.hashCode()
        result = 31 * result + (authorityOverride?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ExecuteTriggerInstruction {
            val triggerId = requireArg(arguments, "trigger")
            val authority = arguments["authority"]
            return ExecuteTriggerInstruction(triggerId, authority)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
