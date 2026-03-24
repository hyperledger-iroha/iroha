package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for the `RevokeSpaceDirectoryManifest` instruction. */
class RevokeSpaceDirectoryManifestInstruction private constructor(
    val uaid: String,
    val dataspace: Long,
    val revokedEpoch: Long,
    val reason: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RevokeSpaceDirectoryManifestInstruction) return false
        return dataspace == other.dataspace
            && revokedEpoch == other.revokedEpoch
            && uaid == other.uaid
            && reason == other.reason
    }

    override fun hashCode(): Int = listOf(uaid, dataspace, revokedEpoch, reason).hashCode()

    class Builder internal constructor() {
        private var uaid: String? = null
        private var dataspace: Long? = null
        private var revokedEpoch: Long? = null
        private var reason: String? = null

        fun setUaid(uaid: String) = apply {
            this.uaid = requireNotNull(uaid) { "uaid" }
        }

        fun setDataspace(dataspace: Long) = apply {
            this.dataspace = dataspace
        }

        fun setRevokedEpoch(revokedEpoch: Long) = apply {
            this.revokedEpoch = revokedEpoch
        }

        fun setReason(reason: String?) = apply {
            this.reason = reason
        }

        fun build(): RevokeSpaceDirectoryManifestInstruction {
            val u = uaid
            if (u.isNullOrBlank()) throw IllegalStateException("uaid must be provided")
            val ds = checkNotNull(dataspace) { "dataspace must be provided" }
            val re = checkNotNull(revokedEpoch) { "revokedEpoch must be provided" }
            return RevokeSpaceDirectoryManifestInstruction(
                u, ds, re, reason,
                canonicalArguments(u, ds, re, reason),
            )
        }

        private fun canonicalArguments(u: String, ds: Long, re: Long, reason: String?): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("uaid", u)
                put("dataspace", ds.toString())
                put("revoked_epoch", re.toString())
                if (!reason.isNullOrBlank()) put("reason", reason)
            }
    }

    companion object {
        private const val ACTION = "RevokeSpaceDirectoryManifest"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RevokeSpaceDirectoryManifestInstruction {
            val u = require(arguments, "uaid")
            val ds = requireLong(arguments, "dataspace")
            val re = requireLong(arguments, "revoked_epoch")
            val reason = arguments["reason"]
            return RevokeSpaceDirectoryManifestInstruction(u, ds, re, reason, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = require(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("Instruction argument '$key' must be a number: $value", ex)
            }
        }
    }
}
