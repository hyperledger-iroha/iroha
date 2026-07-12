package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

private const val ACTION = "MintAsset"

/** Typed representation of a `MintAsset` instruction. */
class MintAssetInstruction(
    @JvmField val assetId: String,
    @JvmField val quantity: String,
) : InstructionTemplate {

    init {
        require(assetId.isNotBlank()) { "assetId must not be blank" }
        requireCanonicalQuantity(quantity)
    }

    /** Construct from a lossless validated quantity value. */
    constructor(assetId: String, quantity: KotodamaQuantity) : this(assetId, quantity.toString())

    override val kind: InstructionKind = InstructionKind.MINT

    override val arguments: Map<String, String> by lazy {
        linkedMapOf(
            "action" to ACTION,
            "asset" to assetId,
            "quantity" to quantity,
        )
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MintAssetInstruction) return false
        return assetId == other.assetId && quantity == other.quantity
    }

    override fun hashCode(): Int {
        var result = assetId.hashCode()
        result = 31 * result + quantity.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MintAssetInstruction {
            val assetId = requireArg(arguments, "asset")
            val quantity = requireArg(arguments, "quantity")
            return MintAssetInstruction(assetId = assetId, quantity = quantity)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            if (value.isNullOrBlank()) {
                throw IllegalArgumentException("Instruction argument '$key' is required")
            }
            return value
        }
    }
}
