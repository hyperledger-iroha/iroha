package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "ActivateRuntimeUpgrade"
private const val ID_HEX = "id_hex"

/** Typed representation of `ActivateRuntimeUpgrade` instructions. */
class ActivateRuntimeUpgradeInstruction private constructor(
    val idHex: String,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(idHex: String) : this(
        idHex = GovernanceInstructionUtils.requireHex(idHex, "idHex", 32),
        _arguments = linkedMapOf(
            "action" to ACTION,
            ID_HEX to GovernanceInstructionUtils.requireHex(idHex, "idHex", 32),
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ActivateRuntimeUpgradeInstruction) return false
        return idHex == other.idHex
    }

    override fun hashCode(): Int = idHex.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ActivateRuntimeUpgradeInstruction {
            val idHex = GovernanceInstructionUtils.requireHex(
                require(arguments, ID_HEX), ID_HEX, 32,
            )
            return ActivateRuntimeUpgradeInstruction(
                idHex = idHex,
                _arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}
