package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "TransferNft"

/** Typed representation of the `TransferNft` instruction. */
class TransferNftInstruction private constructor(
    @JvmField val sourceAccountId: String,
    @JvmField val nftId: String,
    @JvmField val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        sourceAccountId: String,
        nftId: String,
        destinationAccountId: String,
    ) : this(
        sourceAccountId = validated(sourceAccountId, "sourceAccountId"),
        nftId = validated(nftId, "nftId"),
        destinationAccountId = validated(destinationAccountId, "destinationAccountId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "source" to sourceAccountId,
            "nft" to nftId,
            "destination" to destinationAccountId,
        ),
    )

    override val kind: InstructionKind get() = InstructionKind.TRANSFER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransferNftInstruction) return false
        return sourceAccountId == other.sourceAccountId
            && nftId == other.nftId
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int {
        var result = sourceAccountId.hashCode()
        result = 31 * result + nftId.hashCode()
        result = 31 * result + destinationAccountId.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): TransferNftInstruction {
            val sourceAccountId = require(arguments, "source")
            val nftId = require(arguments, "nft")
            val destinationAccountId = require(arguments, "destination")
            return TransferNftInstruction(
                sourceAccountId = sourceAccountId,
                nftId = nftId,
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
    }
}
