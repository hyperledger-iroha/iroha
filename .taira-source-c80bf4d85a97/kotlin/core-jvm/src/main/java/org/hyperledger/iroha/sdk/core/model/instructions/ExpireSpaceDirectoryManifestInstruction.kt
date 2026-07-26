package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "ExpireSpaceDirectoryManifest"

/** Typed representation of an `ExpireSpaceDirectoryManifest` instruction. */
class ExpireSpaceDirectoryManifestInstruction(
    val uaid: String,
    val dataspace: Long,
    val expiredEpoch: Long,
) : InstructionTemplate {

    init {
        require(uaid.isNotBlank()) { "uaid must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> = buildMap {
        put("action", ACTION)
        put("uaid", uaid)
        put("dataspace", dataspace.toString())
        put("expired_epoch", expiredEpoch.toString())
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ExpireSpaceDirectoryManifestInstruction) return false
        return dataspace == other.dataspace
            && expiredEpoch == other.expiredEpoch
            && uaid == other.uaid
    }

    override fun hashCode(): Int {
        var result = uaid.hashCode()
        result = 31 * result + dataspace.hashCode()
        result = 31 * result + expiredEpoch.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ExpireSpaceDirectoryManifestInstruction {
            return ExpireSpaceDirectoryManifestInstruction(
                uaid = requireArg(arguments, "uaid"),
                dataspace = requireLong(arguments, "dataspace"),
                expiredEpoch = requireLong(arguments, "expired_epoch"),
            )
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
                    "Instruction argument '$key' must be a number: $value",
                    ex,
                )
            }
        }
    }
}
