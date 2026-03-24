package org.hyperledger.iroha.sdk.core.model.instructions

private const val BURN_ASSET_ACTION = "BurnAsset"

/** Typed representation of the `BurnAsset` instruction. */
class BurnAssetInstruction private constructor(
    val assetId: String,
    val quantity: String,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.BURN

    override val arguments: Map<String, String> get() = _arguments

    constructor(assetId: String, quantity: String) : this(
        assetId = assetId.also {
            require(it.isNotBlank()) { "assetId must not be blank" }
        },
        quantity = quantity.also {
            require(it.isNotBlank()) { "quantity must not be blank" }
        },
        _arguments = linkedMapOf(
            "action" to BURN_ASSET_ACTION,
            "asset" to assetId,
            "quantity" to quantity,
        ),
    )

    constructor(assetId: String, quantity: Number) : this(assetId, quantity.toString())

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BurnAssetInstruction) return false
        return assetId == other.assetId && quantity == other.quantity
    }

    override fun hashCode(): Int {
        var result = assetId.hashCode()
        result = 31 * result + quantity.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): BurnAssetInstruction {
            return BurnAssetInstruction(
                assetId = require(arguments, "asset"),
                quantity = require(arguments, "quantity"),
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
