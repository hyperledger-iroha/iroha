package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "TransferAsset"

/** Typed representation of the `TransferAsset` instruction. */
class TransferAssetInstruction private constructor(
    @JvmField val assetId: String,
    @JvmField val quantity: String,
    @JvmField val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        assetId: String,
        quantity: String,
        destinationAccountId: String,
    ) : this(
        assetId = validated(assetId, "assetId"),
        quantity = validatedQuantity(quantity),
        destinationAccountId = validated(destinationAccountId, "destinationAccountId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "asset" to assetId,
            "quantity" to quantity,
            "destination" to destinationAccountId,
        ),
    )

    constructor(
        assetId: String,
        quantity: Number,
        destinationAccountId: String,
    ) : this(assetId, quantity.toString(), destinationAccountId)

    override val kind: InstructionKind get() = InstructionKind.TRANSFER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransferAssetInstruction) return false
        return assetId == other.assetId
            && quantity == other.quantity
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int {
        var result = assetId.hashCode()
        result = 31 * result + quantity.hashCode()
        result = 31 * result + destinationAccountId.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): TransferAssetInstruction {
            val assetId = require(arguments, "asset")
            val quantity = require(arguments, "quantity")
            val destinationAccountId = require(arguments, "destination")
            return TransferAssetInstruction(
                assetId = assetId,
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
