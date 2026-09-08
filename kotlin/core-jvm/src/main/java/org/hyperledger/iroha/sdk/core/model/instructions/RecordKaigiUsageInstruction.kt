package org.hyperledger.iroha.sdk.core.model.instructions

/** Canonical V1 `RecordKaigiUsage` wire instruction; private authorization is verified against ledger state. */
class RecordKaigiUsageInstruction @JvmOverloads constructor(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val durationMs: Long,
    @JvmField val billedGas: Long = 0,
    @JvmField val usageCommitment: KaigiAuthorizationScalarV1? = null,
    @JvmField val proofBase64: String? = null,
) : KaigiWireInstructionV1 {

    init {
        require(durationMs != 0L) { "durationMs must be greater than zero" }
        KaigiInstructionUtils.requireUsageArtifacts(usageCommitment, proofBase64)
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM
    override val arguments: Map<String, String> by lazy {
        KaigiInstructionUtils.immutableArguments(buildMap {
        put("action", "RecordKaigiUsage")
        KaigiInstructionUtils.appendCallId(callId, this, "call")
        put("duration_ms", java.lang.Long.toUnsignedString(durationMs))
        put("billed_gas", java.lang.Long.toUnsignedString(billedGas))
        usageCommitment?.let { put("usage_commitment", it.toHex()) }
        proofBase64?.let { put("proof", it) }
        })
    }

    override fun equals(other: Any?): Boolean = other is RecordKaigiUsageInstruction && arguments == other.arguments
    override fun hashCode(): Int = arguments.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RecordKaigiUsageInstruction {
            KaigiInstructionUtils.requireKnownArguments(arguments, setOf(
                "action", "call.domain_id", "call.call_name", "duration_ms", "billed_gas", "usage_commitment", "proof",
            ))
            KaigiInstructionUtils.requireAction(arguments, "RecordKaigiUsage")
            return RecordKaigiUsageInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                durationMs = KaigiInstructionUtils.parseUnsignedLong(KaigiInstructionUtils.require(arguments, "duration_ms"), "duration_ms"),
                billedGas = KaigiInstructionUtils.parseUnsignedLong(arguments.getOrDefault("billed_gas", "0"), "billed_gas"),
                usageCommitment = arguments["usage_commitment"]?.let(KaigiAuthorizationScalarV1::fromArgument),
                proofBase64 = arguments["proof"],
            )
        }
    }
}
