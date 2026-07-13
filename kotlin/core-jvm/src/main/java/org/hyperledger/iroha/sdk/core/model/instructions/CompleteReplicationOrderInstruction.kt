package org.hyperledger.iroha.sdk.core.model.instructions

private const val COMPLETE_REPLICATION_ACTION = "CompleteReplicationOrder"

/** Typed representation of the `CompleteReplicationOrder` instruction. */
class CompleteReplicationOrderInstruction private constructor(
    val orderIdHex: String,
    val completionEpoch: Long,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(orderIdHex: String, completionEpoch: Long) : this(
        orderIdHex = ReplicationOrderInstructionValidation.requireOrderId(orderIdHex),
        completionEpoch = ReplicationOrderInstructionValidation.requireEpoch(
            completionEpoch,
            "completionEpoch",
        ),
        _arguments = linkedMapOf(
            "action" to COMPLETE_REPLICATION_ACTION,
            "order_id_hex" to orderIdHex,
            "completion_epoch" to completionEpoch.toString(),
        ),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CompleteReplicationOrderInstruction) return false
        return completionEpoch == other.completionEpoch && orderIdHex == other.orderIdHex
    }

    override fun hashCode(): Int {
        var result = orderIdHex.hashCode()
        result = 31 * result + completionEpoch.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CompleteReplicationOrderInstruction {
            ReplicationOrderInstructionValidation.requireArguments(
                arguments,
                COMPLETE_REPLICATION_ACTION,
                setOf("order_id_hex", "completion_epoch"),
            )
            return CompleteReplicationOrderInstruction(
                require(arguments, "order_id_hex"),
                requireLong(arguments, "completion_epoch"),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = require(arguments, key)
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException(
                    "Instruction argument '$key' must be a number: $value", ex,
                )
            }
        }
    }
}
