package org.hyperledger.iroha.sdk.core.model.instructions

private const val ACTION = "TransferDomain"

/** Typed representation of the `TransferDomain` instruction. */
class TransferDomainInstruction private constructor(
    @JvmField val sourceAccountId: String,
    @JvmField val domainId: String,
    @JvmField val destinationAccountId: String,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    constructor(
        sourceAccountId: String,
        domainId: String,
        destinationAccountId: String,
    ) : this(
        sourceAccountId = validated(sourceAccountId, "sourceAccountId"),
        domainId = validated(domainId, "domainId"),
        destinationAccountId = validated(destinationAccountId, "destinationAccountId"),
        arguments = linkedMapOf(
            "action" to ACTION,
            "source" to sourceAccountId,
            "domain" to domainId,
            "destination" to destinationAccountId,
        ),
    )

    override val kind: InstructionKind get() = InstructionKind.TRANSFER

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TransferDomainInstruction) return false
        return sourceAccountId == other.sourceAccountId
            && domainId == other.domainId
            && destinationAccountId == other.destinationAccountId
    }

    override fun hashCode(): Int {
        var result = sourceAccountId.hashCode()
        result = 31 * result + domainId.hashCode()
        result = 31 * result + destinationAccountId.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): TransferDomainInstruction {
            val sourceAccountId = require(arguments, "source")
            val domainId = require(arguments, "domain")
            val destinationAccountId = require(arguments, "destination")
            return TransferDomainInstruction(
                sourceAccountId = sourceAccountId,
                domainId = domainId,
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
