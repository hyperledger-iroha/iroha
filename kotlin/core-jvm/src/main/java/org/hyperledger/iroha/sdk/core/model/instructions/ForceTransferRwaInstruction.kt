package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "ForceTransferRwa"

/** Typed representation of the `ForceTransferRwa` instruction. */
class ForceTransferRwaInstruction private constructor(
    @JvmField val rwaId: String,
    @JvmField val quantity: String,
    @JvmField val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        rwaId: String,
        quantity: String,
        destinationAccountId: String,
    ) : this(
        rwaId = validated(rwaId, "rwaId"),
        quantity = validatedQuantity(quantity),
        destinationAccountId = validated(destinationAccountId, "destinationAccountId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "rwa" to rwaId,
            "quantity" to quantity,
            "destination" to destinationAccountId,
        ),
    )

    constructor(
        rwaId: String,
        quantity: Number,
        destinationAccountId: String,
    ) : this(rwaId, quantity.toString(), destinationAccountId)

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ForceTransferRwaInstruction) return false
        return rwaId == other.rwaId
            && quantity == other.quantity
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int = listOf(rwaId, quantity, destinationAccountId).hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ForceTransferRwaInstruction {
            val rwaId = require(arguments, "rwa")
            val quantity = require(arguments, "quantity")
            val destinationAccountId = require(arguments, "destination")
            return ForceTransferRwaInstruction(
                rwaId = rwaId,
                quantity = quantity,
                destinationAccountId = destinationAccountId,
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
