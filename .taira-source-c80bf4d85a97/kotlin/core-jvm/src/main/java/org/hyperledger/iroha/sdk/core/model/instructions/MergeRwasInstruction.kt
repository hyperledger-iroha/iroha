package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "MergeRwas"

/**
 * Typed representation of the `MergeRwas` instruction.
 *
 * The payload stores the canonical JSON representation of the merge request.
 */
class MergeRwasInstruction private constructor(
    /** Returns the canonical JSON representation of the `MergeRwas` payload. */
    @JvmField val mergeJson: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind = InstructionKind.CUSTOM

    constructor(mergeJson: String) : this(
        mergeJson = validated(mergeJson, "mergeJson"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "merge" to mergeJson,
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is MergeRwasInstruction) return false
        return mergeJson == other.mergeJson
    }

    override fun hashCode(): Int = mergeJson.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): MergeRwasInstruction {
            val mergeJson = require(arguments, "merge")
            return MergeRwasInstruction(
                mergeJson = mergeJson,
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
