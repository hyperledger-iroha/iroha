package org.hyperledger.iroha.sdk.core.model.instructions

/** Typed builder for the `TransferAssetDefinition` instruction. */
class TransferAssetDefinitionInstruction private constructor(
    val sourceAccountId: String,
    val assetDefinitionId: String,
    val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.TRANSFER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransferAssetDefinitionInstruction) return false
        return sourceAccountId == other.sourceAccountId
            && assetDefinitionId == other.assetDefinitionId
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int = listOf(sourceAccountId, assetDefinitionId, destinationAccountId).hashCode()

    class Builder internal constructor() {
        private var sourceAccountId: String? = null
        private var assetDefinitionId: String? = null
        private var destinationAccountId: String? = null

        fun setSourceAccountId(sourceAccountId: String) = apply {
            this.sourceAccountId = requireNotNull(sourceAccountId) { "sourceAccountId" }
        }

        fun setAssetDefinitionId(assetDefinitionId: String) = apply {
            this.assetDefinitionId = requireNotNull(assetDefinitionId) { "assetDefinitionId" }
        }

        fun setDestinationAccountId(destinationAccountId: String) = apply {
            this.destinationAccountId = requireNotNull(destinationAccountId) { "destinationAccountId" }
        }

        fun build(): TransferAssetDefinitionInstruction {
            val src = requireNotBlank(sourceAccountId, "sourceAccountId")
            val def = requireNotBlank(assetDefinitionId, "assetDefinitionId")
            val dst = requireNotBlank(destinationAccountId, "destinationAccountId")
            return TransferAssetDefinitionInstruction(src, def, dst, canonicalArguments(src, def, dst))
        }

        private fun canonicalArguments(src: String, def: String, dst: String): Map<String, String> =
            buildMap {
                put("action", ACTION)
                put("source", src)
                put("definition", def)
                put("destination", dst)
            }
    }

    companion object {
        private const val ACTION = "TransferAssetDefinition"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): TransferAssetDefinitionInstruction {
            val src = require(arguments, "source")
            val def = require(arguments, "definition")
            val dst = require(arguments, "destination")
            return TransferAssetDefinitionInstruction(src, def, dst, LinkedHashMap(arguments))
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireNotBlank(value: String?, name: String): String {
            check(!value.isNullOrBlank()) { "$name must be set" }
            return value
        }
    }
}
