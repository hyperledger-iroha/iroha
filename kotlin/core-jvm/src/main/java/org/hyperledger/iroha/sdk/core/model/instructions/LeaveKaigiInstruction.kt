package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "LeaveKaigi"
private val LEAVE_KAIGI_ARGUMENTS = setOf(
    "action",
    "call.domain_id",
    "call.call_name",
    "participant",
    "commitment.commitment",
    "commitment.alias_tag",
    "nullifier.digest",
    "nullifier.issued_at_ms",
    "roster_root",
    "proof",
)

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
        require(commitmentAliasTag == null) {
            "commitment aliasTag is off-chain only and must be omitted"
        }
        require(
            commitment == null &&
                nullifierDigest == null &&
                nullifierIssuedAtMs == null &&
                rosterRoot == null &&
                proofBase64 == null
        ) {
            "LeaveKaigi privacy artifacts are reserved and must be omitted in V1"
        }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy {
        KaigiInstructionUtils.immutableArguments(canonicalArguments())
    }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = ACTION
        KaigiInstructionUtils.appendCallId(callId, args, "call")
        args["participant"] = participant
        if (commitment != null) {
            args["commitment.commitment"] = commitment
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
            KaigiInstructionUtils.requireKnownArguments(arguments, LEAVE_KAIGI_ARGUMENTS)
            KaigiInstructionUtils.requireAction(arguments, ACTION)
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val participant = KaigiInstructionUtils.require(arguments, "participant")

            val reservedPrivacyKeys = listOf(
                "commitment.commitment",
                "commitment.alias_tag",
                "nullifier.digest",
                "nullifier.issued_at_ms",
                "roster_root",
                "proof",
            )
            require(reservedPrivacyKeys.none(arguments::containsKey)) {
                "LeaveKaigi privacy artifacts are reserved and must be omitted in V1"
            }

            return LeaveKaigiInstruction(
                callId = callId,
                participant = participant,
                commitment = null,
                commitmentAliasTag = null,
                nullifierDigest = null,
                nullifierIssuedAtMs = null,
                rosterRoot = null,
                proofBase64 = null,
            )
        }
    }
}
