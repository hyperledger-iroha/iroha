package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "EndKaigi"

/** Typed representation of an `EndKaigi` instruction. */
class EndKaigiInstruction(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val endedAtMs: Long? = null,
) : InstructionTemplate {

    init {
        if (endedAtMs != null) {
            require(endedAtMs >= 0) { "endedAtMs must be non-negative" }
        }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        KaigiInstructionUtils.appendCallId(callId, args, "call")
        if (endedAtMs != null) {
            args["ended_at_ms"] = java.lang.Long.toUnsignedString(endedAtMs)
        }
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is EndKaigiInstruction) return false
        return callId == other.callId && endedAtMs == other.endedAtMs
    }

    override fun hashCode(): Int {
        var result = callId.hashCode()
        result = 31 * result + (endedAtMs?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): EndKaigiInstruction {
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val ended = KaigiInstructionUtils.parseOptionalUnsignedLong(
                arguments["ended_at_ms"],
                "ended_at_ms",
            )
            return EndKaigiInstruction(callId, ended)
        }
    }
}
