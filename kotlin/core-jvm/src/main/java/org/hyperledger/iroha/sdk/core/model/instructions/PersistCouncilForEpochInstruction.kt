package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "PersistCouncilForEpoch"

/** Typed representation of a `PersistCouncilForEpoch` instruction. */
class PersistCouncilForEpochInstruction(
    @JvmField val epoch: Long,
    members: List<String>,
    alternates: List<String> = emptyList(),
) : InstructionTemplate {

    @JvmField val members: List<String> = members.toList()
    @JvmField val alternates: List<String> = alternates.toList()

    init {
        require(epoch >= 0) { "epoch must be non-negative" }
        require(this.members.isNotEmpty()) { "members must contain at least one account id" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        args["epoch"] = epoch.toString()
        args["members"] = members.joinToString(",")
        args["alternates"] = alternates.joinToString(",")
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PersistCouncilForEpochInstruction) return false
        return epoch == other.epoch
            && members == other.members
            && alternates == other.alternates
    }

    override fun hashCode(): Int {
        var result = epoch.hashCode()
        result = 31 * result + members.hashCode()
        result = 31 * result + alternates.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): PersistCouncilForEpochInstruction {
            val epoch = parseLong(requireArg(arguments, "epoch"), "epoch")

            val members = parseCsvList(arguments["members"])
            val alternates = parseCsvList(arguments["alternates"])

            return PersistCouncilForEpochInstruction(
                epoch = epoch,
                members = members,
                alternates = alternates,
            )
        }

        private fun parseCsvList(csv: String?): List<String> {
            if (csv.isNullOrBlank()) return emptyList()
            return csv.split(",").map { it.trim() }.filter { it.isNotEmpty() }
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }

        private fun parseLong(value: String, field: String): Long {
            try {
                val parsed = value.toLong()
                if (parsed < 0) throw IllegalArgumentException("$field must be non-negative")
                return parsed
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$field must be numeric: $value", ex)
            }
        }
    }
}
