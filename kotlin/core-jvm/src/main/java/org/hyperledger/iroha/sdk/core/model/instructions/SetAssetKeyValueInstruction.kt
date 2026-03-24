package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for `SetAssetKeyValue` instructions targeting concrete asset balances. */
class SetAssetKeyValueInstruction private constructor(
    /** Returns the full asset identifier (definition + owner). */
    val assetId: String,
    /** Returns the metadata key being set. */
    val key: String,
    /** Returns the metadata value encoded as a string. */
    val value: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.SET_KEY_VALUE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetAssetKeyValueInstruction) return false
        return assetId == other.assetId
            && key == other.key
            && value == other.value
    }

    override fun hashCode(): Int = listOf(assetId, key, value).hashCode()

    class Builder internal constructor() {
        private var assetId: String? = null
        private var key: String? = null
        private var value: String? = null

        fun setAssetId(assetId: String) = apply {
            this.assetId = requireNotNull(assetId) { "assetId" }
        }

        fun setKey(key: String) = apply {
            this.key = requireNotNull(key) { "key" }
        }

        fun setValue(value: String) = apply {
            this.value = requireNotNull(value) { "value" }
        }

        fun build(): SetAssetKeyValueInstruction {
            val a = requireNotBlank(assetId, "assetId")
            val k = requireNotBlank(key, "key")
            val v = checkNotNull(value) { "value must be provided" }
            return SetAssetKeyValueInstruction(a, k, v, canonicalArguments(a, k, v))
        }

        private fun canonicalArguments(asset: String, key: String, value: String): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("asset", asset)
                put("key", key)
                put("value", value)
            }
    }

    companion object {
        private const val ACTION = "SetAssetKeyValue"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetAssetKeyValueInstruction {
            val action = require(arguments, "action")
            require(action == ACTION) { "Unsupported action for SetAssetKeyValue" }
            val asset = require(arguments, "asset")
            val key = require(arguments, "key")
            val value = require(arguments, "value")
            return SetAssetKeyValueInstruction(asset, key, value, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireNotBlank(value: String?, name: String): String {
            check(!value.isNullOrBlank()) { "$name must be provided" }
            return value
        }
    }
}
