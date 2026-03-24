package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for the `RevokeRole` instruction. */
class RevokeRoleInstruction private constructor(
    val destinationAccountId: String,
    val roleId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.REVOKE

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RevokeRoleInstruction) return false
        return destinationAccountId == other.destinationAccountId
            && roleId == other.roleId
    }

    override fun hashCode(): Int = listOf(destinationAccountId, roleId).hashCode()

    class Builder internal constructor() {
        private var destinationAccountId: String? = null
        private var roleId: String? = null

        fun setDestinationAccountId(destinationAccountId: String) = apply {
            this.destinationAccountId = requireNotNull(destinationAccountId) { "destinationAccountId" }
        }

        fun setRoleId(roleId: String) = apply {
            this.roleId = requireNotNull(roleId) { "roleId" }
        }

        fun build(): RevokeRoleInstruction {
            val dest = requireNotBlank(destinationAccountId, "destinationAccountId")
            val role = requireNotBlank(roleId, "roleId")
            return RevokeRoleInstruction(dest, role, canonicalArguments(dest, role))
        }

        private fun canonicalArguments(dest: String, role: String): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("destination", dest)
                put("role", role)
            }
    }

    companion object {
        private const val ACTION = "RevokeRole"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RevokeRoleInstruction {
            val dest = require(arguments, "destination")
            val role = require(arguments, "role")
            return RevokeRoleInstruction(dest, role, LinkedHashMap(arguments))
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
