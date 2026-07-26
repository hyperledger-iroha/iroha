package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Typed builder for `SetKeyValue` instructions targeting domains, accounts, asset
 * definitions, NFTs, RWAs, and triggers.
 */
class SetKeyValueInstruction private constructor(
    /** Returns the instruction target describing which entity should be updated. */
    val target: Target,
    /** Returns the identifier of the target entity (domain id, account id, trigger id, etc.). */
    val objectId: String,
    /** Returns the metadata key being set. */
    val key: String,
    /** Returns the metadata value encoded as a string. */
    val value: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.SET_KEY_VALUE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetKeyValueInstruction) return false
        return target == other.target
            && objectId == other.objectId
            && key == other.key
            && value == other.value
    }

    override fun hashCode(): Int = listOf(target, objectId, key, value).hashCode()

    enum class Target(val action: String, val argumentKey: String) {
        DOMAIN("SetDomainKeyValue", "domain"),
        ACCOUNT("SetAccountKeyValue", "account"),
        ASSET_DEFINITION("SetAssetDefinitionKeyValue", "definition"),
        NFT("SetNftKeyValue", "nft"),
        RWA("SetRwaKeyValue", "rwa"),
        TRIGGER("SetTriggerKeyValue", "trigger");

        companion object {
            @JvmStatic
            fun fromAction(action: String): Target =
                entries.find { it.action == action }
                    ?: throw IllegalArgumentException("Unknown SetKeyValue action: $action")
        }
    }

    class Builder internal constructor() {
        private var target: Target? = null
        private var objectId: String? = null
        private var key: String? = null
        private var value: String? = null

        fun setDomainId(domainId: String) = setTarget(Target.DOMAIN, domainId)
        fun setAccountId(accountId: String) = setTarget(Target.ACCOUNT, accountId)
        fun setAssetDefinitionId(assetDefinitionId: String) = setTarget(Target.ASSET_DEFINITION, assetDefinitionId)
        fun setNftId(nftId: String) = setTarget(Target.NFT, nftId)
        fun setRwaId(rwaId: String) = setTarget(Target.RWA, rwaId)
        fun setTriggerId(triggerId: String) = setTarget(Target.TRIGGER, triggerId)

        internal fun setTarget(target: Target, id: String) = apply {
            requireNotNull(id) { "id" }
            if (this.target != null && this.target != target) {
                throw IllegalStateException("Instruction target already set to ${this.target}")
            }
            this.target = target
            this.objectId = id
        }

        fun setKey(key: String) = apply {
            this.key = requireNotNull(key) { "key" }
        }

        fun setValue(value: String) = apply {
            this.value = requireNotNull(value) { "value" }
        }

        fun build(): SetKeyValueInstruction {
            val t = checkNotNull(target) { "target must be set" }
            val oid = objectId
            check(!oid.isNullOrBlank()) { "target identifier must be provided" }
            val k = key
            check(!k.isNullOrBlank()) { "key must be provided" }
            val v = checkNotNull(value) { "value must be provided" }
            return SetKeyValueInstruction(t, oid, k, v, canonicalArguments(t, oid, k, v))
        }

        private fun canonicalArguments(t: Target, oid: String, k: String, v: String): Map<String, String> =
            buildMap {
                put("action", t.action)
                put(t.argumentKey, oid)
                put("key", k)
                put("value", v)
            }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetKeyValueInstruction {
            val action = require(arguments, "action")
            val target = Target.fromAction(action)
            val oid = require(arguments, target.argumentKey)
            val key = require(arguments, "key")
            val value = require(arguments, "value")
            return SetKeyValueInstruction(target, oid, key, value, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
