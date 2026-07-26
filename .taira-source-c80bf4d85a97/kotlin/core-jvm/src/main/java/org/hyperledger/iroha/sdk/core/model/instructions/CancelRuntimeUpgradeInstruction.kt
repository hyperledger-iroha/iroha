package org.hyperledger.iroha.sdk.core.model.instructions

private const val CANCEL_ACTION = "CancelRuntimeUpgrade"
private const val CANCEL_ID_HEX = "id_hex"

/** Typed representation of `CancelRuntimeUpgrade` instructions. */
class CancelRuntimeUpgradeInstruction private constructor(
    val idHex: String,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(idHex: String) : this(
        idHex = GovernanceInstructionUtils.requireHex(idHex, "idHex", 32),
        _arguments = linkedMapOf(
            "action" to CANCEL_ACTION,
            CANCEL_ID_HEX to GovernanceInstructionUtils.requireHex(idHex, "idHex", 32),
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CancelRuntimeUpgradeInstruction) return false
        return idHex == other.idHex
    }

    override fun hashCode(): Int = idHex.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CancelRuntimeUpgradeInstruction {
            val idHex = GovernanceInstructionUtils.requireHex(
                require(arguments, CANCEL_ID_HEX), CANCEL_ID_HEX, 32,
            )
            return CancelRuntimeUpgradeInstruction(
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
