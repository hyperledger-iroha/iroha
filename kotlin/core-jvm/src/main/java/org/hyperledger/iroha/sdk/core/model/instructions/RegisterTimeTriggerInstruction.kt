package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.core.model.InstructionBox

/** Typed builder for `RegisterTimeTrigger` instructions. */
class RegisterTimeTriggerInstruction private constructor(
    @JvmField val triggerId: String,
    @JvmField val authority: String,
    @JvmField val startMs: Long,
    @JvmField val periodMs: Long?,
    @JvmField val repeats: Int?,
    @JvmField val instructions: List<InstructionBox>,
    @JvmField val metadata: Map<String, String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterTimeTriggerInstruction) return false
        return startMs == other.startMs
            && triggerId == other.triggerId
            && authority == other.authority
            && periodMs == other.periodMs
            && repeats == other.repeats
            && instructions == other.instructions
            && metadata == other.metadata
    }

    override fun hashCode(): Int {
        var result = triggerId.hashCode()
        result = 31 * result + authority.hashCode()
        result = 31 * result + startMs.hashCode()
        result = 31 * result + (periodMs?.hashCode() ?: 0)
        result = 31 * result + (repeats?.hashCode() ?: 0)
        result = 31 * result + instructions.hashCode()
        result = 31 * result + metadata.hashCode()
        return result
    }

    class Builder internal constructor() {
        private var triggerId: String? = null
        private var authority: String? = null
        private var startMs: Long? = null
        private var periodMs: Long? = null
        private var repeats: Int? = null
        private val instructions: MutableList<InstructionBox> = mutableListOf()
        private val metadata: MutableMap<String, String> = linkedMapOf()

        fun setTriggerId(triggerId: String) = apply {
            require(triggerId.isNotBlank()) { "triggerId must not be blank" }
            this.triggerId = triggerId
        }

        fun setAuthority(authority: String) = apply {
            require(authority.isNotBlank()) { "authority must not be blank" }
            this.authority = authority
        }

        fun setStartMs(startMs: Long) = apply {
            require(startMs > 0) { "startMs must be greater than zero" }
            this.startMs = startMs
        }

        fun setPeriodMs(periodMs: Long?) = apply {
            if (periodMs != null) {
                require(periodMs > 0) { "periodMs must be greater than zero when provided" }
            }
            this.periodMs = periodMs
        }

        fun setRepeats(repeats: Int?) = apply {
            if (repeats != null) {
                require(repeats > 0) { "repeats must be greater than zero when provided" }
            }
            this.repeats = repeats
        }

        fun addInstruction(instruction: InstructionBox) = apply {
            instructions.add(requireNotNull(instruction) { "instruction" })
        }

        fun setInstructions(newInstructions: List<InstructionBox>?) = apply {
            instructions.clear()
            newInstructions?.forEach { addInstruction(it) }
        }

        fun putMetadata(key: String, value: String) = apply {
            metadata[requireNotNull(key) { "metadata key" }] = requireNotNull(value) { "metadata value" }
        }

        fun setMetadata(entries: Map<String, String>?) = apply {
            metadata.clear()
            entries?.forEach { (k, v) -> putMetadata(k, v) }
        }

        fun build(): RegisterTimeTriggerInstruction {
            val tid = checkNotNull(triggerId) { "triggerId must be set" }
            val auth = checkNotNull(authority) { "authority must be set" }
            val sm = checkNotNull(startMs) { "startMs must be set" }
            check(instructions.isNotEmpty()) { "at least one instruction must be provided" }
            val args = buildCanonicalArguments(tid, auth, sm, periodMs, repeats, instructions, metadata)
            return RegisterTimeTriggerInstruction(
                tid, auth, sm, periodMs, repeats, instructions.toList(), metadata.toMap(), args,
            )
        }

        private fun buildCanonicalArguments(
            triggerId: String,
            authority: String,
            startMs: Long,
            periodMs: Long?,
            repeats: Int?,
            instructions: List<InstructionBox>,
            metadata: Map<String, String>,
        ): Map<String, String> = buildMap {
            put("action", ACTION)
            put("trigger", triggerId)
            put("authority", authority)
            put("start_ms", java.lang.Long.toUnsignedString(startMs))
            if (periodMs != null) {
                put("period_ms", java.lang.Long.toUnsignedString(periodMs))
            }
            put(
                "repeats",
                if (repeats == null) REPEATS_INDEFINITE
                else Integer.toUnsignedString(repeats),
            )
            TriggerInstructionUtils.appendInstructions(instructions, this)
            TriggerInstructionUtils.appendMetadata(metadata, this)
        }
    }

    companion object {
        const val ACTION: String = "RegisterTimeTrigger"
        @JvmField val REPEATS_INDEFINITE: String = "indefinite"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterTimeTriggerInstruction {
            val triggerId = requireArg(arguments, "trigger")
            val authority = requireArg(arguments, "authority")
            val startMs = parsePositiveLong(requireArg(arguments, "start_ms"), "start_ms")
            val instructions = TriggerInstructionUtils.parseInstructions(arguments)
            val metadata = TriggerInstructionUtils.extractMetadata(arguments)
            val periodMs = arguments["period_ms"]?.let { parsePositiveLong(it, "period_ms") }
            val repeats = TriggerInstructionUtils.parseRepeats(arguments["repeats"])
            return RegisterTimeTriggerInstruction(
                triggerId = triggerId,
                authority = authority,
                startMs = startMs,
                periodMs = periodMs,
                repeats = repeats,
                instructions = instructions,
                metadata = metadata,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun parsePositiveLong(value: String, field: String): Long = try {
            val parsed = java.lang.Long.parseUnsignedLong(value)
            if (parsed <= 0) throw IllegalArgumentException("$field must be greater than zero")
            parsed
        } catch (ex: NumberFormatException) {
            throw IllegalArgumentException("$field must be an unsigned integer", ex)
        }
    }
}
