package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for the `RevokePermission` instruction targeting an account. */
class RevokePermissionInstruction private constructor(
    val destinationId: String,
    val permissionName: String,
    val permissionPayload: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REVOKE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RevokePermissionInstruction) return false
        return destinationId == other.destinationId
            && permissionName == other.permissionName
            && permissionPayload == other.permissionPayload
    }

    override fun hashCode(): Int = listOf(destinationId, permissionName, permissionPayload).hashCode()

    class Builder internal constructor() {
        private var destinationId: String? = null
        private var permissionName: String? = null
        private var permissionPayload: String? = null

        fun setDestinationId(destinationId: String) = apply {
            this.destinationId = requireNotNull(destinationId) { "destinationId" }
        }

        fun setPermissionName(permissionName: String) = apply {
            this.permissionName = requireNotNull(permissionName) { "permissionName" }
        }

        fun setPermissionPayload(permissionPayload: String?) = apply {
            this.permissionPayload = if (permissionPayload != null && permissionPayload.isBlank()) null else permissionPayload
        }

        fun build(): RevokePermissionInstruction {
            val dest = requireNotBlank(destinationId, "destinationId")
            val perm = requireNotBlank(permissionName, "permissionName")
            return RevokePermissionInstruction(dest, perm, permissionPayload, canonicalArguments(dest, perm, permissionPayload))
        }

        private fun canonicalArguments(dest: String, perm: String, payload: String?): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("destination", dest)
                put("permission", perm)
                if (!payload.isNullOrBlank()) put("permission_payload", payload)
            }
    }

    companion object {
        private const val ACTION = "RevokePermission"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RevokePermissionInstruction {
            val dest = require(arguments, "destination")
            val perm = require(arguments, "permission")
            val payload = arguments["permission_payload"]?.takeIf { it.isNotBlank() }
            return RevokePermissionInstruction(dest, perm, payload, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireNotBlank(value: String?, name: String): String {
            check(!value.isNullOrBlank()) { "$name must be set" }
            return value
        }
    }
}
