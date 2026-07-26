package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "GrantRole"

/** Typed representation of a `GrantRole` instruction. */
class GrantRoleInstruction(
    val destinationAccountId: String,
    val roleId: String,
) : InstructionTemplate {

    init {
        require(destinationAccountId.isNotBlank()) { "destinationAccountId must not be blank" }
        require(roleId.isNotBlank()) { "roleId must not be blank" }
    }

    override val kind: InstructionKind = InstructionKind.GRANT

    override val arguments: Map<String, String> = buildMap {
        put("action", ACTION)
        put("destination", destinationAccountId)
        put("role", roleId)
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is GrantRoleInstruction) return false
        return destinationAccountId == other.destinationAccountId && roleId == other.roleId
    }

    override fun hashCode(): Int {
        var result = destinationAccountId.hashCode()
        result = 31 * result + roleId.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): GrantRoleInstruction {
            return GrantRoleInstruction(
                destinationAccountId = requireArg(arguments, "destination"),
                roleId = requireArg(arguments, "role"),
            )
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
