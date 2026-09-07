package org.hyperledger.iroha.sdk.core.model.instructions

/** Canonical V1 `LeaveKaigi` wire instruction; private authorization is verified against ledger state. */
class LeaveKaigiInstruction @JvmOverloads constructor(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val participant: String,
    @JvmField val commitment: KaigiAuthorizationScalarV1? = null,
    @JvmField val nullifierDigest: KaigiAuthorizationScalarV1? = null,
    rosterRoot: String? = null,
    @JvmField val proofBase64: String? = null,
) : KaigiWireInstructionV1 {
    @JvmField val rosterRoot: String? = rosterRoot?.let(KaigiInstructionUtils::canonicalRosterRoot)

    init {
        require(participant.isNotBlank()) { "participant must not be blank" }
        KaigiInstructionUtils.requireAuthorizationArtifacts(commitment, nullifierDigest, this.rosterRoot, proofBase64)
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> by lazy {
        KaigiInstructionUtils.immutableArguments(buildMap {
        put("action", "LeaveKaigi")
        KaigiInstructionUtils.appendCallId(callId, this, "call")
        put("participant", participant)
        commitment?.let { put("commitment.commitment", it.toHex()) }
        nullifierDigest?.let { put("nullifier.digest", it.toHex()) }
        this@LeaveKaigiInstruction.rosterRoot?.let { put("roster_root", it) }
        proofBase64?.let { put("proof", it) }
        })
    }

    override fun equals(other: Any?): Boolean = other is LeaveKaigiInstruction && arguments == other.arguments
    override fun hashCode(): Int = arguments.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): LeaveKaigiInstruction {
            KaigiInstructionUtils.requireKnownArguments(arguments, setOf(
                "action", "call.domain_id", "call.call_name", "participant", "commitment.commitment", "nullifier.digest", "roster_root", "proof",
            ))
            KaigiInstructionUtils.requireAction(arguments, "LeaveKaigi")
            return LeaveKaigiInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                participant = KaigiInstructionUtils.require(arguments, "participant"),
                commitment = arguments["commitment.commitment"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                nullifierDigest = arguments["nullifier.digest"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                rosterRoot = arguments["roster_root"],
                proofBase64 = arguments["proof"],
            )
        }
    }
}
