package org.hyperledger.iroha.sdk.core.model.instructions

private const val REPORT_KAIGI_RELAY_HEALTH_ACTION = "ReportKaigiRelayHealth"
private const val MAX_RELAY_HEALTH_NOTES_CHARS = 512
private val REPORT_KAIGI_RELAY_HEALTH_ARGUMENTS = setOf(
    "action",
    "call.domain_id",
    "call.call_name",
    "relay_id",
    "status",
    "reported_at_ms",
    "notes",
)

/** Typed representation of `ReportKaigiRelayHealth` instructions. */
class ReportKaigiRelayHealthInstruction(
    @JvmField val callId: KaigiInstructionUtils.CallId,
    @JvmField val relayId: String,
    @JvmField val status: Status,
    @JvmField val reportedAtMs: Long,
    @JvmField val notes: String? = null,
    arguments: Map<String, String>? = null,
) : InstructionTemplate {

    /** Relay health variants accepted by the Rust data model. */
    enum class Status(@JvmField val wireName: String) {
        HEALTHY("Healthy"),
        DEGRADED("Degraded"),
        UNAVAILABLE("Unavailable"),
        ;

        companion object {
            @JvmStatic
            fun fromWireName(value: String): Status =
                entries.firstOrNull { it.wireName == value }
                    ?: throw IllegalArgumentException("Unknown Kaigi relay health status: $value")
        }
    }

    private val _arguments: Map<String, String> =
        KaigiInstructionUtils.immutableArguments(canonicalArguments())

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    init {
        require(relayId.isNotBlank()) { "relayId must not be blank" }
        validateNotes(notes)
        require(arguments == null || arguments == _arguments) {
            "arguments must match the canonical ReportKaigiRelayHealth representation"
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ReportKaigiRelayHealthInstruction) return false
        return callId == other.callId
            && relayId == other.relayId
            && status == other.status
            && reportedAtMs == other.reportedAtMs
            && notes == other.notes
    }

    override fun hashCode(): Int =
        listOf(callId, relayId, status, reportedAtMs, notes).hashCode()

    private fun canonicalArguments(): Map<String, String> = linkedMapOf<String, String>().apply {
        put("action", REPORT_KAIGI_RELAY_HEALTH_ACTION)
        KaigiInstructionUtils.appendCallId(callId, this, "call")
        put("relay_id", relayId)
        put("status", status.wireName)
        put("reported_at_ms", reportedAtMs.toULong().toString())
        if (notes != null) put("notes", notes)
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ReportKaigiRelayHealthInstruction {
            KaigiInstructionUtils.requireKnownArguments(arguments, REPORT_KAIGI_RELAY_HEALTH_ARGUMENTS)
            KaigiInstructionUtils.requireAction(arguments, REPORT_KAIGI_RELAY_HEALTH_ACTION)
            return ReportKaigiRelayHealthInstruction(
                callId = KaigiInstructionUtils.parseCallId(arguments, "call"),
                relayId = KaigiInstructionUtils.require(arguments, "relay_id"),
                status = Status.fromWireName(KaigiInstructionUtils.require(arguments, "status")),
                reportedAtMs = KaigiInstructionUtils.parseUnsignedLong(
                    KaigiInstructionUtils.require(arguments, "reported_at_ms"),
                    "reported_at_ms",
                ),
                notes = arguments["notes"],
            )
        }

        private fun validateNotes(notes: String?) {
            if (notes != null) {
                requireWellFormedUtf16(notes)
                val characterCount = notes.codePointCount(0, notes.length)
                require(characterCount <= MAX_RELAY_HEALTH_NOTES_CHARS) {
                    "relay health notes must not exceed 512 characters"
                }
            }
        }

        private fun requireWellFormedUtf16(value: String) {
            var index = 0
            while (index < value.length) {
                val current = value[index]
                when {
                    Character.isHighSurrogate(current) -> {
                        require(
                            index + 1 < value.length &&
                                Character.isLowSurrogate(value[index + 1]),
                        ) {
                            "relay health notes must not contain unpaired UTF-16 surrogates"
                        }
                        index += 2
                    }

                    Character.isLowSurrogate(current) -> {
                        throw IllegalArgumentException(
                            "relay health notes must not contain unpaired UTF-16 surrogates",
                        )
                    }

                    else -> index++
                }
            }
        }
    }
}
