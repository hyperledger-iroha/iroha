package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "GrantPermission"

/** Typed representation of a `GrantPermission` instruction targeting an account. */
class GrantPermissionInstruction(
    val destinationId: String,
    val permissionName: String,
    val permissionPayload: String? = null,
) : InstructionTemplate {

    init {
        require(destinationId.isNotBlank()) { "destinationId must not be blank" }
        require(permissionName.isNotBlank()) { "permissionName must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.GRANT

    override val arguments: Map<String, String> = buildMap {
        put("action", ACTION)
        put("destination", destinationId)
        put("permission", permissionName)
        if (!permissionPayload.isNullOrBlank()) {
            put("permission_payload", permissionPayload)
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is GrantPermissionInstruction) return false
        return destinationId == other.destinationId
            && permissionName == other.permissionName
            && permissionPayload == other.permissionPayload
    }

    override fun hashCode(): Int {
        var result = destinationId.hashCode()
        result = 31 * result + permissionName.hashCode()
        result = 31 * result + (permissionPayload?.hashCode() ?: 0)
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): GrantPermissionInstruction {
            val payload = arguments["permission_payload"]
            return GrantPermissionInstruction(
                destinationId = requireArg(arguments, "destination"),
                permissionName = requireArg(arguments, "permission"),
                permissionPayload = if (payload.isNullOrBlank()) null else payload,
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
