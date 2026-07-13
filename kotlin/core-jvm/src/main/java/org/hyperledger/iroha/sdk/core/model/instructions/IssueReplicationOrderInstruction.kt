@file:OptIn(ExperimentalEncodingApi::class)

package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

private const val ACTION = "IssueReplicationOrder"

/** Typed representation of an `IssueReplicationOrder` instruction. */
class IssueReplicationOrderInstruction(
    val orderIdHex: String,
    val orderPayloadBase64: String,
    val issuedEpoch: Long,
    val deadlineEpoch: Long,
) : InstructionTemplate {

    init {
        ReplicationOrderInstructionValidation.requireOrderId(orderIdHex)
        ReplicationOrderInstructionValidation.requireCanonicalPayload(orderPayloadBase64)
        ReplicationOrderInstructionValidation.requireEpoch(issuedEpoch, "issuedEpoch")
        ReplicationOrderInstructionValidation.requireEpoch(deadlineEpoch, "deadlineEpoch")
        ReplicationOrderInstructionValidation.requireWindow(issuedEpoch, deadlineEpoch)
    }

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> = buildMap {
        put("action", ACTION)
        put("order_id_hex", orderIdHex)
        put("order_payload_base64", orderPayloadBase64)
        put("issued_epoch", issuedEpoch.toString())
        put("deadline_epoch", deadlineEpoch.toString())
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is IssueReplicationOrderInstruction) return false
        return issuedEpoch == other.issuedEpoch
            && deadlineEpoch == other.deadlineEpoch
            && orderIdHex == other.orderIdHex
            && orderPayloadBase64 == other.orderPayloadBase64
    }

    override fun hashCode(): Int {
        var result = orderIdHex.hashCode()
        result = 31 * result + orderPayloadBase64.hashCode()
        result = 31 * result + issuedEpoch.hashCode()
        result = 31 * result + deadlineEpoch.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): IssueReplicationOrderInstruction {
            ReplicationOrderInstructionValidation.requireArguments(
                arguments,
                ACTION,
                setOf("order_id_hex", "order_payload_base64", "issued_epoch", "deadline_epoch"),
            )
            return IssueReplicationOrderInstruction(
                orderIdHex = requireArg(arguments, "order_id_hex"),
                orderPayloadBase64 = requireArg(arguments, "order_payload_base64"),
                issuedEpoch = requireLong(arguments, "issued_epoch"),
                deadlineEpoch = requireLong(arguments, "deadline_epoch"),
            )
        }

        @JvmStatic
        fun fromOrderBytes(
            orderId: ByteArray,
            orderPayload: ByteArray,
            issuedEpoch: Long,
            deadlineEpoch: Long,
        ): IssueReplicationOrderInstruction {
            val hexId = ReplicationOrderInstructionValidation.encodeOrderId(orderId)
            val base64Payload = Base64.encode(orderPayload)
            return IssueReplicationOrderInstruction(hexId, base64Payload, issuedEpoch, deadlineEpoch)
        }

        private fun requireArg(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun requireLong(arguments: Map<String, String>, key: String): Long {
            val value = requireArg(arguments, key)
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
