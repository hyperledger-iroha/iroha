package org.hyperledger.iroha.sdk.core.model.instructions

private const val EXPIRE_REPLICATION_ACTION = "ExpireReplicationOrder"

/** Typed representation of the `ExpireReplicationOrder` instruction. */
class ExpireReplicationOrderInstruction(
    orderIdHex: String,
    expirationEpoch: Long,
) : InstructionTemplate {
    val orderIdHex = ReplicationOrderInstructionValidation.requireOrderId(orderIdHex)
    val expirationEpoch = ReplicationOrderInstructionValidation.requireEpoch(
        expirationEpoch,
        "expirationEpoch",
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> = linkedMapOf(
        "action" to EXPIRE_REPLICATION_ACTION,
        "order_id_hex" to this.orderIdHex,
        "expiration_epoch" to this.expirationEpoch.toString(),
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ExpireReplicationOrderInstruction) return false
        return orderIdHex == other.orderIdHex && expirationEpoch == other.expirationEpoch
    }

    override fun hashCode(): Int = 31 * orderIdHex.hashCode() + expirationEpoch.hashCode()

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ExpireReplicationOrderInstruction {
            ReplicationOrderInstructionValidation.requireArguments(
                arguments,
                EXPIRE_REPLICATION_ACTION,
                setOf("order_id_hex", "expiration_epoch"),
            )
            return ExpireReplicationOrderInstruction(
                require(arguments, "order_id_hex"),
                requireLong(arguments, "expiration_epoch"),
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
                    "Instruction argument '$key' must be a number: $value",
                    ex,
                )
            }
        }
    }
}
