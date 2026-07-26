package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "SetRwaControls"

/**
 * Typed representation of the `SetRwaControls` instruction.
 *
 * The payload stores the canonical JSON representation of `RwaControlPolicy`.
 */
class SetRwaControlsInstruction private constructor(
    /** Returns the RWA identifier being updated. */
    @JvmField val rwaId: String,
    /** Returns the canonical JSON representation of `RwaControlPolicy`. */
    @JvmField val controlsJson: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind = InstructionKind.CUSTOM

    constructor(rwaId: String, controlsJson: String) : this(
        rwaId = validated(rwaId, "rwaId"),
        controlsJson = validated(controlsJson, "controlsJson"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "rwa" to rwaId,
            "controls" to controlsJson,
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetRwaControlsInstruction) return false
        return rwaId == other.rwaId && controlsJson == other.controlsJson
    }

    override fun hashCode(): Int = listOf(rwaId, controlsJson).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetRwaControlsInstruction {
            val rwaId = require(arguments, "rwa")
            val controlsJson = require(arguments, "controls")
            return SetRwaControlsInstruction(
                rwaId = rwaId,
                controlsJson = controlsJson,
                arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun validated(value: String, name: String): String {
            require(value.isNotBlank()) { "$name must not be blank" }
            return value
        }
    }
}
