package org.hyperledger.iroha.sdk.core.model.instructions

/**
 * Typed representation of `Unregister` instructions spanning the supported entity families.
 */
class UnregisterInstruction private constructor(
    @JvmField val target: Target,
    @JvmField val objectId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(target: Target, objectId: String) : this(
        target = target,
        objectId = validatedObjectId(objectId),
        arguments = linkedMapOf(
            "action" to target.action,
            target.argumentKey to objectId,
        ),
    )

    override val kind: InstructionKind get() = InstructionKind.UNREGISTER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is UnregisterInstruction) return false
        return target == other.target && objectId == other.objectId
    }

    override fun hashCode(): Int {
        var result = target.hashCode()
        result = 31 * result + objectId.hashCode()
        return result
    }

    /** Supported unregistration targets. */
    enum class Target(@JvmField val action: String, @JvmField val argumentKey: String) {
        PEER("UnregisterPeer", "peer"),
        DOMAIN("UnregisterDomain", "domain"),
        ACCOUNT("UnregisterAccount", "account"),
        ASSET_DEFINITION("UnregisterAssetDefinition", "definition"),
        NFT("UnregisterNft", "nft"),
        ROLE("UnregisterRole", "role"),
        TRIGGER("UnregisterTrigger", "trigger");

        companion object {
            @JvmStatic
            fun fromAction(action: String): Target =
                entries.find { it.action == action }
                    ?: throw IllegalArgumentException("Unknown Unregister action: $action")
        }
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UnregisterInstruction {
            val action = require(arguments, "action")
            val target = Target.fromAction(action)
            val objectId = require(arguments, target.argumentKey)
            return UnregisterInstruction(
                target = target,
                objectId = objectId,
                arguments = LinkedHashMap(arguments),
            )
        }

        @JvmStatic
        fun peer(peerId: String): UnregisterInstruction = UnregisterInstruction(Target.PEER, peerId)

        @JvmStatic
        fun domain(domainId: String): UnregisterInstruction = UnregisterInstruction(Target.DOMAIN, domainId)

        @JvmStatic
        fun account(accountId: String): UnregisterInstruction = UnregisterInstruction(Target.ACCOUNT, accountId)

        @JvmStatic
        fun assetDefinition(assetDefinitionId: String): UnregisterInstruction =
            UnregisterInstruction(Target.ASSET_DEFINITION, assetDefinitionId)

        @JvmStatic
        fun nft(nftId: String): UnregisterInstruction = UnregisterInstruction(Target.NFT, nftId)

        @JvmStatic
        fun role(roleId: String): UnregisterInstruction = UnregisterInstruction(Target.ROLE, roleId)

        @JvmStatic
        fun trigger(triggerId: String): UnregisterInstruction = UnregisterInstruction(Target.TRIGGER, triggerId)

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun validatedObjectId(value: String): String {
            require(value.isNotBlank()) { "target identifier must be provided" }
            return value
        }
    }
}
