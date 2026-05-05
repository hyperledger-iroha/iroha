package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "FreezeRwa"

/** Typed representation of the `FreezeRwa` instruction. */
class FreezeRwaInstruction private constructor(
    @JvmField val rwaId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind = InstructionKind.CUSTOM

    constructor(rwaId: String) : this(
        rwaId = validated(rwaId, "rwaId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "rwa" to rwaId,
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is FreezeRwaInstruction) return false
        return rwaId == other.rwaId
    }

    override fun hashCode(): Int = rwaId.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): FreezeRwaInstruction {
            val rwaId = require(arguments, "rwa")
            return FreezeRwaInstruction(
                rwaId = rwaId,
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
