package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "PersistCouncilForEpoch"

/** Typed representation of a `PersistCouncilForEpoch` instruction. */
class PersistCouncilForEpochInstruction(
    @JvmField val epoch: Long,
    members: List<String>,
    alternates: List<String> = emptyList(),
    @JvmField val verified: Int = 0,
    @JvmField val candidatesCount: Int,
    @JvmField val derivedBy: GovernanceInstructionUtils.CouncilDerivationKind,
) : InstructionTemplate {

    @JvmField val members: List<String> = members.toList()
    @JvmField val alternates: List<String> = alternates.toList()

    init {
        require(epoch >= 0) { "epoch must be non-negative" }
        require(this.members.isNotEmpty()) { "members must contain at least one account id" }
        require(verified >= 0) { "verified must be non-negative" }
        require(candidatesCount >= 0) { "candidatesCount must be non-negative" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        args["epoch"] = epoch.toString()
        args["members"] = members.joinToString(",")
        args["alternates"] = alternates.joinToString(",")
        args["verified"] = verified.toString()
        args["candidates_count"] = candidatesCount.toString()
        args["derived_by"] = derivedBy.wireValue
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PersistCouncilForEpochInstruction) return false
        return epoch == other.epoch
            && candidatesCount == other.candidatesCount
            && verified == other.verified
            && members == other.members
            && alternates == other.alternates
            && derivedBy == other.derivedBy
    }

    override fun hashCode(): Int {
        var result = epoch.hashCode()
        result = 31 * result + members.hashCode()
        result = 31 * result + alternates.hashCode()
        result = 31 * result + verified
        result = 31 * result + candidatesCount
        result = 31 * result + derivedBy.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): PersistCouncilForEpochInstruction {
            val epoch = parseLong(requireArg(arguments, "epoch"), "epoch")
            val verified = parseInt(arguments.getOrDefault("verified", "0"), "verified")
            val candidatesCount = parseInt(requireArg(arguments, "candidates_count"), "candidates_count")
            val derivedBy = GovernanceInstructionUtils.CouncilDerivationKind.parse(
                requireArg(arguments, "derived_by"),
            )

            val members = parseCsvList(arguments["members"])
            val alternates = parseCsvList(arguments["alternates"])

            return PersistCouncilForEpochInstruction(
                epoch = epoch,
                members = members,
                alternates = alternates,
                verified = verified,
                candidatesCount = candidatesCount,
                derivedBy = derivedBy,
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

        private fun parseInt(value: String, field: String): Int {
            try {
                val parsed = value.toInt()
                if (parsed < 0) throw IllegalArgumentException("$field must be non-negative")
                return parsed
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$field must be numeric: $value", ex)
            }
        }
    }
}
