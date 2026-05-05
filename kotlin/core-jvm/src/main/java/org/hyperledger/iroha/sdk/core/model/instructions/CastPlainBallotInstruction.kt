package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger

private const val CAST_BALLOT_ACTION = "CastPlainBallot"

/** Typed representation of `CastPlainBallot` instructions. */
class CastPlainBallotInstruction private constructor(
    val referendumId: String,
    val ownerAccountId: String,
    val amount: String,
    val durationBlocks: Long,
    val direction: Int,
    private val _arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override val arguments: Map<String, String> get() = _arguments

    constructor(
        referendumId: String,
        ownerAccountId: String,
        amount: String,
        durationBlocks: Long,
        direction: Int,
    ) : this(
        referendumId = referendumId.also {
            require(it.isNotBlank()) { "referendumId must not be blank" }
        },
        ownerAccountId = ownerAccountId.also {
            require(it.isNotBlank()) { "ownerAccountId must not be blank" }
        },
        amount = validateAmount(amount),
        durationBlocks = durationBlocks.also {
            require(it >= 0) { "durationBlocks must be non-negative" }
        },
        direction = direction.also {
            require(it in 0..0xFF) { "direction must be between 0 and 255" }
        },
        _arguments = linkedMapOf(
            "action" to CAST_BALLOT_ACTION,
            "referendum_id" to referendumId,
            "owner" to ownerAccountId,
            "amount" to amount,
            "duration_blocks" to durationBlocks.toString(),
            "direction" to direction.toString(),
        ),
    )

    constructor(
        referendumId: String,
        ownerAccountId: String,
        amount: BigInteger,
        durationBlocks: Long,
        direction: Int,
    ) : this(
        referendumId = referendumId,
        ownerAccountId = ownerAccountId,
        amount = amount.also {
            require(it.signum() >= 0) { "amount must be non-negative" }
        }.toString(),
        durationBlocks = durationBlocks,
        direction = direction,
    )

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is CastPlainBallotInstruction) return false
        return referendumId == other.referendumId
            && ownerAccountId == other.ownerAccountId
            && amount == other.amount
            && durationBlocks == other.durationBlocks
            && direction == other.direction
    }

    override fun hashCode(): Int {
        var result = referendumId.hashCode()
        result = 31 * result + ownerAccountId.hashCode()
        result = 31 * result + amount.hashCode()
        result = 31 * result + durationBlocks.hashCode()
        result = 31 * result + direction
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): CastPlainBallotInstruction {
            return CastPlainBallotInstruction(
                referendumId = require(arguments, "referendum_id"),
                ownerAccountId = require(arguments, "owner"),
                amount = require(arguments, "amount"),
                durationBlocks = parseLong(require(arguments, "duration_blocks"), "duration_blocks"),
                direction = parseDirection(require(arguments, "direction")),
                _arguments = LinkedHashMap(arguments),
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun parseLong(value: String, field: String): Long {
            try {
                return value.toLong()
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("$field must be a number: $value", ex)
            }
        }

        private fun parseDirection(value: String): Int {
            try {
                val parsed = value.toInt()
                if (parsed < 0 || parsed > 0xFF) {
                    throw IllegalArgumentException("direction must be between 0 and 255")
                }
                return parsed
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("direction must be numeric: $value", ex)
            }
        }

        private fun validateAmount(amount: String): String {
            require(amount.isNotBlank()) { "amount must not be blank" }
            try {
                val parsed = BigInteger(amount)
                require(parsed.signum() >= 0) { "amount must be non-negative" }
            } catch (ex: NumberFormatException) {
                throw IllegalArgumentException("amount must be a non-negative integer", ex)
            }
            return amount
        }
    }
}
