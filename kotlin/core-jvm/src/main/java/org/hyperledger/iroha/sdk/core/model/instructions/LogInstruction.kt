package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed representation of a `Log` instruction. */
class LogInstruction(
    @JvmField val level: String = "INFO",
    @JvmField val message: String,
) : InstructionTemplate {

    init {
        require(message.isNotBlank()) { "message must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.LOG

    override val arguments: Map<String, String> by lazy {
        linkedMapOf(
            "action" to "Log",
            "level" to level,
            "message" to message,
        )
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is LogInstruction) return false
        return level == other.level && message == other.message
    }

    override fun hashCode(): Int {
        var result = level.hashCode()
        result = 31 * result + message.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): LogInstruction {
            val level = requireArg(arguments, "level")
            val message = requireArg(arguments, "message")
            return LogInstruction(level = level, message = message)
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
