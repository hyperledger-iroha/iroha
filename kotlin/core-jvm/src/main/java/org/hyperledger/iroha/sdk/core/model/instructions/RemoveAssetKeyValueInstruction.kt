package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RemoveAssetKeyValue"

/** Typed builder for `RemoveAssetKeyValue` instructions targeting concrete asset balances. */
class RemoveAssetKeyValueInstruction private constructor(
    /** Returns the full asset identifier (definition + owner). */
    @JvmField val assetId: String,
    /** Returns the metadata key slated for removal. */
    @JvmField val key: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REMOVE_KEY_VALUE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RemoveAssetKeyValueInstruction) return false
        return assetId == other.assetId && key == other.key
    }

    override fun hashCode(): Int = listOf(assetId, key).hashCode()

    class Builder internal constructor() {
        private var assetId: String? = null
        private var key: String? = null

        fun setAssetId(assetId: String) = apply {
            this.assetId = requireNotNull(assetId) { "assetId" }
        }

        fun setKey(key: String) = apply {
            this.key = requireNotNull(key) { "key" }
        }

        fun build(): RemoveAssetKeyValueInstruction {
            val aid = assetId
            check(!aid.isNullOrBlank()) { "assetId must be provided" }
            val k = this.key
            check(!k.isNullOrBlank()) { "key must be provided" }
            val args = buildMap {
                put("action", ACTION)
                put("asset", aid)
                put("key", k)
            }
            return RemoveAssetKeyValueInstruction(aid, k, args)
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RemoveAssetKeyValueInstruction {
            val action = requireArg(arguments, "action")
            require(action == ACTION) { "Unsupported action for RemoveAssetKeyValue" }
            val assetId = requireArg(arguments, "asset")
            val key = requireArg(arguments, "key")
            return RemoveAssetKeyValueInstruction(assetId, key, LinkedHashMap(arguments))
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
