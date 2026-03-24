package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for `RemoveKeyValue` instructions spanning all supported entity types. */
class RemoveKeyValueInstruction private constructor(
    /** Returns the instruction target describing which entity should be updated. */
    val target: Target,
    /** Returns the identifier of the target entity (domain id, account id, trigger id, etc.). */
    val objectId: String,
    /** Returns the metadata key slated for removal. */
    val key: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REMOVE_KEY_VALUE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RemoveKeyValueInstruction) return false
        return target == other.target
            && objectId == other.objectId
            && key == other.key
    }

    override fun hashCode(): Int = listOf(target, objectId, key).hashCode()

    enum class Target(val action: String, val argumentKey: String) {
        DOMAIN("RemoveDomainKeyValue", "domain"),
        ACCOUNT("RemoveAccountKeyValue", "account"),
        ASSET_DEFINITION("RemoveAssetDefinitionKeyValue", "definition"),
        NFT("RemoveNftKeyValue", "nft"),
        TRIGGER("RemoveTriggerKeyValue", "trigger");

        companion object {
            @JvmStatic
            fun fromAction(action: String): Target =
                entries.find { it.action == action }
                    ?: throw IllegalArgumentException("Unknown RemoveKeyValue action: $action")
        }
    }

    class Builder internal constructor() {
        private var target: Target? = null
        private var objectId: String? = null
        private var key: String? = null

        fun setDomainId(domainId: String) = setTarget(Target.DOMAIN, domainId)
        fun setAccountId(accountId: String) = setTarget(Target.ACCOUNT, accountId)
        fun setAssetDefinitionId(assetDefinitionId: String) = setTarget(Target.ASSET_DEFINITION, assetDefinitionId)
        fun setNftId(nftId: String) = setTarget(Target.NFT, nftId)
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

        fun build(): RemoveKeyValueInstruction {
            val t = checkNotNull(target) { "target must be set" }
            val oid = objectId
            check(!oid.isNullOrBlank()) { "target identifier must be provided" }
            val k = this.key
            check(!k.isNullOrBlank()) { "key must be provided" }
            return RemoveKeyValueInstruction(t, oid, k, canonicalArguments(t, oid, k))
        }

        private fun canonicalArguments(t: Target, oid: String, k: String): Map<String, String> =
            buildMap {
                put("action", t.action)
                put(t.argumentKey, oid)
                put("key", k)
            }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RemoveKeyValueInstruction {
            val action = require(arguments, "action")
            val target = Target.fromAction(action)
            val oid = require(arguments, target.argumentKey)
            val key = require(arguments, "key")
            return RemoveKeyValueInstruction(target, oid, key, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
