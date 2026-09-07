package org.hyperledger.iroha.sdk.core.model.instructions

/** Canonical V1 `EndKaigi` wire instruction; private authorization is verified against ledger state. */
class EndKaigiInstruction @JvmOverloads constructor(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val endedAtMs: Long? = null,
    @JvmField val commitment: KaigiAuthorizationScalarV1? = null,
    @JvmField val nullifierDigest: KaigiAuthorizationScalarV1? = null,
    rosterRoot: String? = null,
    @JvmField val proofBase64: String? = null,
) : KaigiWireInstructionV1 {
    @JvmField val rosterRoot: String? = rosterRoot?.let(KaigiInstructionUtils::canonicalRosterRoot)

    init {
        KaigiInstructionUtils.requireAuthorizationArtifacts(commitment, nullifierDigest, this.rosterRoot, proofBase64)
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> by lazy {
        KaigiInstructionUtils.immutableArguments(buildMap {
        put("action", "EndKaigi")
        KaigiInstructionUtils.appendCallId(callId, this, "call")
        endedAtMs?.let { put("ended_at_ms", java.lang.Long.toUnsignedString(it)) }
        commitment?.let { put("commitment.commitment", it.toHex()) }
        nullifierDigest?.let { put("nullifier.digest", it.toHex()) }
        this@EndKaigiInstruction.rosterRoot?.let { put("roster_root", it) }
        proofBase64?.let { put("proof", it) }
        })
    }

    override fun equals(other: Any?): Boolean = other is EndKaigiInstruction && arguments == other.arguments
    override fun hashCode(): Int = arguments.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): EndKaigiInstruction {
            KaigiInstructionUtils.requireKnownArguments(arguments, setOf(
                "action", "call.domain_id", "call.call_name", "ended_at_ms", "commitment.commitment", "nullifier.digest", "roster_root", "proof",
            ))
            KaigiInstructionUtils.requireAction(arguments, "EndKaigi")
            return EndKaigiInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                endedAtMs = KaigiInstructionUtils.parseOptionalUnsignedLong(arguments["ended_at_ms"], "ended_at_ms"),
                commitment = arguments["commitment.commitment"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                nullifierDigest = arguments["nullifier.digest"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                rosterRoot = arguments["roster_root"],
                proofBase64 = arguments["proof"],
            )
        }
    }
}
