package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "LeaveKaigi"

/** Typed representation of a `LeaveKaigi` instruction. */
class LeaveKaigiInstruction(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val participant: String,
    @JvmField val commitment: String? = null,
    @JvmField val commitmentAliasTag: String? = null,
    @JvmField val nullifierDigest: String? = null,
    @JvmField val nullifierIssuedAtMs: Long? = null,
    @JvmField val rosterRoot: String? = null,
    @JvmField val proofBase64: String? = null,
) : InstructionTemplate {

    init {
        require(participant.isNotBlank()) { "participant must not be blank" }
        if (nullifierIssuedAtMs != null) {
            require(nullifierIssuedAtMs >= 0) { "nullifier issuedAtMs must be non-negative" }
        }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        KaigiInstructionUtils.appendCallId(callId, args, "call")
        args["participant"] = participant
        if (commitment != null) {
            args["commitment.commitment"] = commitment
            if (commitmentAliasTag != null) {
                args["commitment.alias_tag"] = commitmentAliasTag
            }
        }
        if (nullifierDigest != null) {
            args["nullifier.digest"] = nullifierDigest
            if (nullifierIssuedAtMs != null) {
                args["nullifier.issued_at_ms"] = java.lang.Long.toUnsignedString(nullifierIssuedAtMs)
            }
        }
        if (rosterRoot != null) {
            args["roster_root"] = rosterRoot
        }
        if (proofBase64 != null) {
            args["proof"] = proofBase64
        }
        return args
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is LeaveKaigiInstruction) return false
        return callId == other.callId
            && participant == other.participant
            && commitment == other.commitment
            && commitmentAliasTag == other.commitmentAliasTag
            && nullifierDigest == other.nullifierDigest
            && nullifierIssuedAtMs == other.nullifierIssuedAtMs
            && rosterRoot == other.rosterRoot
            && proofBase64 == other.proofBase64
    }

    override fun hashCode(): Int {
        var result = callId.hashCode()
        result = 31 * result + participant.hashCode()
        result = 31 * result + (commitment?.hashCode() ?: 0)
        result = 31 * result + (commitmentAliasTag?.hashCode() ?: 0)
        result = 31 * result + (nullifierDigest?.hashCode() ?: 0)
        result = 31 * result + (nullifierIssuedAtMs?.hashCode() ?: 0)
        result = 31 * result + (rosterRoot?.hashCode() ?: 0)
        result = 31 * result + (proofBase64?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): LeaveKaigiInstruction {
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val participant = KaigiInstructionUtils.require(arguments, "participant")

            val commitmentValue = arguments["commitment.commitment"]
            val commitmentAliasTag = if (commitmentValue != null) arguments["commitment.alias_tag"] else null

            val nullifierDigest = arguments["nullifier.digest"]
            val nullifierIssuedAtMs = if (nullifierDigest != null) {
                KaigiInstructionUtils.parseOptionalUnsignedLong(
                    arguments["nullifier.issued_at_ms"],
                    "nullifier.issued_at_ms",
                )
            } else {
                null
            }

            val rosterRoot = arguments["roster_root"]
            val proof = arguments["proof"]

            return LeaveKaigiInstruction(
                callId = callId,
                participant = participant,
                commitment = commitmentValue,
                commitmentAliasTag = commitmentAliasTag,
                nullifierDigest = nullifierDigest,
                nullifierIssuedAtMs = nullifierIssuedAtMs,
                rosterRoot = rosterRoot,
                proofBase64 = proof,
            )
        }
    }
}
