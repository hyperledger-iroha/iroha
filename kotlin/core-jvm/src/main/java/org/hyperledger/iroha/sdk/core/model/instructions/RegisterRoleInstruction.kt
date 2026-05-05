package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RegisterRole"

/** Typed builder for `RegisterRole` instructions. */
class RegisterRoleInstruction private constructor(
    @JvmField val roleId: String,
    @JvmField val ownerAccountId: String,
    @JvmField val permissions: List<String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REGISTER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RegisterRoleInstruction) return false
        return roleId == other.roleId
            && ownerAccountId == other.ownerAccountId
            && permissions == other.permissions
    }

    override fun hashCode(): Int = listOf(roleId, ownerAccountId, permissions).hashCode()

    class Builder internal constructor() {
        private var roleId: String? = null
        private var ownerAccountId: String? = null
        private val permissions: MutableList<String> = mutableListOf()

        fun setRoleId(roleId: String) = apply {
            require(roleId.isNotBlank()) { "roleId must not be blank" }
            this.roleId = roleId
        }

        fun setOwnerAccountId(ownerAccountId: String) = apply {
            require(ownerAccountId.isNotBlank()) { "ownerAccountId must not be blank" }
            this.ownerAccountId = ownerAccountId
        }

        fun addPermission(permission: String) = apply {
            require(permission.isNotBlank()) { "permission must not be blank" }
            this.permissions.add(permission)
        }

        fun setPermissions(permissions: List<String>) = apply {
            this.permissions.clear()
            permissions.forEach { addPermission(it) }
        }

        fun build(): RegisterRoleInstruction {
            val rid = checkNotNull(roleId) { "roleId must be provided" }
            val owner = checkNotNull(ownerAccountId) { "ownerAccountId must be provided" }
            val args = buildMap {
                put("action", ACTION)
                put("role", rid)
                put("owner", owner)
                if (permissions.isNotEmpty()) {
                    put("permissions", permissions.joinToString(","))
                }
            }
            return RegisterRoleInstruction(rid, owner, permissions.toList(), args)
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterRoleInstruction {
            val roleId = requireArg(arguments, "role")
            val owner = requireArg(arguments, "owner")
            val permissionsValue = arguments["permissions"]
            val permissions = if (!permissionsValue.isNullOrBlank()) {
                permissionsValue.split(",").map { it.trim() }.filter { it.isNotEmpty() }
            } else {
                emptyList()
            }
            return RegisterRoleInstruction(roleId, owner, permissions, LinkedHashMap(arguments))
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
