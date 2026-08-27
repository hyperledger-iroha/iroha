package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RecordKaigiUsage"
private val RECORD_KAIGI_USAGE_ARGUMENTS = setOf(
    "action",
    "call.domain_id",
    "call.call_name",
    "duration_ms",
    "billed_gas",
    "usage_commitment",
    "proof",
)

/** Typed representation of `RecordKaigiUsage` instructions. */
class RecordKaigiUsageInstruction(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val durationMs: Long,
    @JvmField val billedGas: Long = 0,
    usageCommitment: String? = null,
    @JvmField val proofBase64: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    @JvmField val usageCommitment: String? =
        KaigiInstructionUtils.canonicalizeOptionalHash(usageCommitment)

    private val _arguments: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(canonicalArguments())

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(durationMs != 0L) { "durationMs must be greater than zero" }
        if (proofBase64 != null) {
            KaigiInstructionUtils.requireBase64(proofBase64, "proof")
        }
        require(arguments == null || arguments == _arguments) {
            "arguments must match the canonical RecordKaigiUsage representation"
        }
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
            KaigiInstructionUtils.requireKnownArguments(arguments, RECORD_KAIGI_USAGE_ARGUMENTS)
            KaigiInstructionUtils.requireAction(arguments, ACTION)
            return RecordKaigiUsageInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                durationMs = KaigiInstructionUtils.parseUnsignedLong(
                    KaigiInstructionUtils.require(arguments, "duration_ms"), "duration_ms",
                ),
                billedGas = KaigiInstructionUtils.parseUnsignedLong(
                    arguments.getOrDefault("billed_gas", "0"), "billed_gas",
                ),
                usageCommitment = arguments["usage_commitment"]
                    ?.let(KaigiInstructionUtils::canonicalizeHash),
                proofBase64 = arguments["proof"],
            )
        }
    }
}
