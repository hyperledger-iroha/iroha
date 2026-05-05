package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "RedeemRwa"

/** Typed representation of the `RedeemRwa` instruction. */
class RedeemRwaInstruction private constructor(
    @JvmField val rwaId: String,
    @JvmField val quantity: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(rwaId: String, quantity: String) : this(
        rwaId = validated(rwaId, "rwaId"),
        quantity = validatedQuantity(quantity),
        arguments = linkedMapOf(
            "action" to ACTION,
            "rwa" to rwaId,
            "quantity" to quantity,
        ),
    )

    constructor(rwaId: String, quantity: Number) : this(rwaId, quantity.toString())

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is RedeemRwaInstruction) return false
        return rwaId == other.rwaId && quantity == other.quantity
    }

    override fun hashCode(): Int = listOf(rwaId, quantity).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RedeemRwaInstruction {
            val rwaId = require(arguments, "rwa")
            val quantity = require(arguments, "quantity")
            return RedeemRwaInstruction(
                rwaId = rwaId,
                quantity = quantity,
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

        private fun validatedQuantity(value: String): String {
            require(value.isNotBlank()) { "quantity must not be blank" }
            return value
        }
    }
}
