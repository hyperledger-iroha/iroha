package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RecordKaigiUsage"

/** Typed representation of `RecordKaigiUsage` instructions. */
class RecordKaigiUsageInstruction(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val durationMs: Long,
    @JvmField val billedGas: Long = 0,
    @JvmField val usageCommitment: String? = null,
    @JvmField val proofBase64: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    private val _arguments: Map<String, String> = arguments?.toMap() ?: canonicalArguments()

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(durationMs > 0) { "durationMs must be greater than zero" }
        require(billedGas >= 0) { "billedGas must be non-negative" }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RecordKaigiUsageInstruction) return false
        return callId.domainId == other.callId.domainId
            && callId.callName == other.callId.callName
            && durationMs == other.durationMs
            && billedGas == other.billedGas
            && usageCommitment == other.usageCommitment
            && proofBase64 == other.proofBase64
    }

    override fun hashCode(): Int =
        listOf(callId.domainId, callId.callName, durationMs, billedGas, usageCommitment, proofBase64).hashCode()

    private fun canonicalArguments(): Map<String, String> = buildMap {
        put("action", ACTION)
        KaigiInstructionUtils.appendCallId(callId, this, "call")
        put("duration_ms", durationMs.toULong().toString())
        put("billed_gas", billedGas.toULong().toString())
        if (usageCommitment != null) put("usage_commitment", usageCommitment)
        if (proofBase64 != null) put("proof", proofBase64)
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RecordKaigiUsageInstruction {
            return RecordKaigiUsageInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                durationMs = KaigiInstructionUtils.parsePositiveInt(
                    KaigiInstructionUtils.require(arguments, "duration_ms"), "duration_ms",
                ).toLong(),
                billedGas = KaigiInstructionUtils.parseUnsignedLong(
                    arguments.getOrDefault("billed_gas", "0"), "billed_gas",
                ),
                usageCommitment = arguments["usage_commitment"],
                proofBase64 = arguments["proof"],
                arguments = arguments,
            )
        }
    }
}
