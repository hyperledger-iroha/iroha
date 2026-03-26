package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "TransferRwa"

/** Typed representation of the `TransferRwa` instruction. */
class TransferRwaInstruction private constructor(
    @JvmField val sourceAccountId: String,
    @JvmField val rwaId: String,
    @JvmField val quantity: String,
    @JvmField val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        sourceAccountId: String,
        rwaId: String,
        quantity: String,
        destinationAccountId: String,
    ) : this(
        sourceAccountId = validated(sourceAccountId, "sourceAccountId"),
        rwaId = validated(rwaId, "rwaId"),
        quantity = validatedQuantity(quantity),
        destinationAccountId = validated(destinationAccountId, "destinationAccountId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "source" to sourceAccountId,
            "rwa" to rwaId,
            "quantity" to quantity,
            "destination" to destinationAccountId,
        ),
    )

    constructor(
        sourceAccountId: String,
        rwaId: String,
        quantity: Number,
        destinationAccountId: String,
    ) : this(sourceAccountId, rwaId, quantity.toString(), destinationAccountId)

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransferRwaInstruction) return false
        return sourceAccountId == other.sourceAccountId
            && rwaId == other.rwaId
            && quantity == other.quantity
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int {
        var result = sourceAccountId.hashCode()
        result = 31 * result + rwaId.hashCode()
        result = 31 * result + quantity.hashCode()
        result = 31 * result + destinationAccountId.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): TransferRwaInstruction {
            val sourceAccountId = require(arguments, "source")
            val rwaId = require(arguments, "rwa")
            val quantity = require(arguments, "quantity")
            val destinationAccountId = require(arguments, "destination")
            return TransferRwaInstruction(
                sourceAccountId = sourceAccountId,
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
