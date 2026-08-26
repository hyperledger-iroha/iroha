package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed representation of a `JoinKaigi` instruction. */
class JoinKaigiInstruction(
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
        require(nullifierIssuedAtMs == null || nullifierIssuedAtMs == 0L) {
            "nullifier issuedAtMs is off-chain only and must be zero when provided"
        }
        require(nullifierIssuedAtMs == null || nullifierDigest != null) {
            "nullifier issuedAtMs requires nullifier digest"
        }
        if (proofBase64 != null) {
            KaigiInstructionUtils.requireBase64(proofBase64, "proof")
        }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> by lazy { canonicalArguments() }

    private fun canonicalArguments(): Map<String, String> {
        val args = linkedMapOf<String, String>()
        args["action"] = "JoinKaigi"
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
        if (other !is JoinKaigiInstruction) return false
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
        fun fromArguments(arguments: Map<String, String>): JoinKaigiInstruction {
            KaigiInstructionUtils.requireAction(arguments, "JoinKaigi")
            val callId = KaigiInstructionUtils.parseCallId(arguments, "call")
            val participant = KaigiInstructionUtils.require(arguments, "participant")

            val commitmentValue = arguments["commitment.commitment"]
            require(arguments["commitment.alias_tag"] == null) {
                "commitment aliasTag is off-chain only and must be omitted"
            }

            val nullifier = arguments["nullifier.digest"]
            val parsedNullifierIssuedAt = KaigiInstructionUtils.parseOptionalUnsignedLong(
                arguments["nullifier.issued_at_ms"],
                "nullifier.issued_at_ms",
            )
            require(parsedNullifierIssuedAt == null || parsedNullifierIssuedAt == 0L) {
                "nullifier issuedAtMs is off-chain only and must be zero when provided"
            }
            require(parsedNullifierIssuedAt == null || nullifier != null) {
                "nullifier issuedAtMs requires nullifier digest"
            }
            val nullifierIssuedAt = parsedNullifierIssuedAt.takeIf { nullifier != null }

            val proof = arguments["proof"]

            return JoinKaigiInstruction(
                callId = callId,
                participant = participant,
                commitment = commitmentValue,
                commitmentAliasTag = null,
                nullifierDigest = nullifier,
                nullifierIssuedAtMs = nullifierIssuedAt,
                rosterRoot = arguments["roster_root"],
                proofBase64 = proof,
            )
        }
    }
}
