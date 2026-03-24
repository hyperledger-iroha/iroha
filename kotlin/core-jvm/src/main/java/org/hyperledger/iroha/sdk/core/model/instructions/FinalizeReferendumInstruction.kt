package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "FinalizeReferendum"

/** Typed representation of a `FinalizeReferendum` instruction. */
class FinalizeReferendumInstruction(
    val referendumId: String,
    proposalIdHex: String,
) : InstructionTemplate {

    val proposalIdHex: String = GovernanceInstructionUtils.requireHex(proposalIdHex, "proposalIdHex", 32)

    init {
        require(referendumId.isNotBlank()) { "referendumId must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> = buildMap {
        put("action", ACTION)
        put("referendum_id", referendumId)
        put("proposal_id_hex", this@FinalizeReferendumInstruction.proposalIdHex)
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is FinalizeReferendumInstruction) return false
        return referendumId == other.referendumId && this.proposalIdHex == other.proposalIdHex
    }

    override fun hashCode(): Int {
        var result = referendumId.hashCode()
        result = 31 * result + proposalIdHex.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): FinalizeReferendumInstruction {
            return FinalizeReferendumInstruction(
                referendumId = requireArg(arguments, "referendum_id"),
                proposalIdHex = requireArg(arguments, "proposal_id_hex"),
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
