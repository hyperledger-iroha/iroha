package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for the `RetirePinManifest` instruction (SoraFS manifest lifecycle). */
class RetirePinManifestInstruction private constructor(
    @JvmField val digestHex: String,
    @JvmField val retiredEpoch: Long,
    @JvmField val reason: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RetirePinManifestInstruction) return false
        return retiredEpoch == other.retiredEpoch
            && digestHex == other.digestHex
            && reason == other.reason
    }

    override fun hashCode(): Int {
        var result = digestHex.hashCode()
        result = 31 * result + retiredEpoch.hashCode()
        result = 31 * result + (reason?.hashCode() ?: 0)
        return result
    }

    class Builder internal constructor() {
        private var digestHex: String? = null
        private var retiredEpoch: Long? = null
        private var reason: String? = null

        fun setDigestHex(digestHex: String) = apply {
            this.digestHex = requireNotNull(digestHex) { "digestHex" }
        }

        fun setRetiredEpoch(retiredEpoch: Long) = apply {
            require(retiredEpoch >= 0) { "retiredEpoch must be non-negative" }
            this.retiredEpoch = retiredEpoch
        }

        fun setReason(reason: String) = apply {
            this.reason = requireNotNull(reason) { "reason" }
        }

        fun build(): RetirePinManifestInstruction {
            val dh = digestHex
            check(!dh.isNullOrBlank()) { "digestHex must be set" }
            val re = checkNotNull(retiredEpoch) { "retiredEpoch must be set" }
            val args = buildMap {
                put("action", ACTION)
                put("digest_hex", dh)
                put("retired_epoch", re.toString())
                if (!reason.isNullOrBlank()) {
                    put("reason", reason!!)
                }
            }
            return RetirePinManifestInstruction(dh, re, reason, args)
        }
    }

    companion object {
        const val ACTION: String = "RetirePinManifest"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RetirePinManifestInstruction {
            val digestHex = requireArg(arguments, "digest_hex")
            val retiredEpoch = requireLong(arguments, "retired_epoch")
            val reason = arguments["reason"]?.takeIf { it.isNotBlank() }
            return RetirePinManifestInstruction(digestHex, retiredEpoch, reason, LinkedHashMap(arguments))
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = requireArg(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a number: $value", ex,
                )
            }
        }
    }
}
