package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Typed builder for the `RetirePinManifest` instruction (SoraFS manifest lifecycle).
 * The recorded retirement epoch comes exclusively from the block consensus timestamp.
 */
class RetirePinManifestInstruction private constructor(
    @JvmField val digestHex: String,
    @JvmField val reason: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RetirePinManifestInstruction) return false
        return digestHex == other.digestHex
            && reason == other.reason
    }

    override fun hashCode(): Int {
        var result = digestHex.hashCode()
        result = 31 * result + (reason?.hashCode() ?: 0)
        return result
    }

    class Builder internal constructor() {
        private var digestHex: String? = null
        private var reason: String? = null

        fun setDigestHex(digestHex: String) = apply {
            this.digestHex = requireNotNull(digestHex) { "digestHex" }
        }

        fun setReason(reason: String) = apply {
            this.reason = requireNotNull(reason) { "reason" }
        }

        fun build(): RetirePinManifestInstruction {
            val dh = digestHex
            check(!dh.isNullOrBlank()) { "digestHex must be set" }
            val args = buildMap {
                put("action", ACTION)
                put("digest_hex", dh)
                if (!reason.isNullOrBlank()) {
                    put("reason", reason!!)
                }
            }
            return RetirePinManifestInstruction(dh, reason, args)
        }
    }

    companion object {
        const val ACTION: String = "RetirePinManifest"
        private val mandatoryArgumentKeys = setOf("action", "digest_hex")
        private val optionalArgumentKeys = setOf("reason")

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RetirePinManifestInstruction {
            require(arguments["action"] == ACTION) { "Instruction argument 'action' must be $ACTION" }
            require(arguments.keys.all { it in mandatoryArgumentKeys || it in optionalArgumentKeys }) {
                "RetirePinManifest arguments contain unsupported fields"
            }
            require(mandatoryArgumentKeys.all(arguments::containsKey)) {
                "RetirePinManifest arguments are missing required fields"
            }
            val digestHex = requireArg(arguments, "digest_hex")
            val reason = arguments["reason"]?.takeIf { it.isNotBlank() }
            return builder()
                .setDigestHex(digestHex)
                .apply {
                    if (reason != null) setReason(reason)
                }
                .build()
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
