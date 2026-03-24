package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger

private const val ACTION = "UpsertProviderCredit"
private const val METADATA_PREFIX = "record.metadata."

/**
 * Typed representation of the `UpsertProviderCredit` instruction (SoraFS provider ledger).
 */
class UpsertProviderCreditInstruction private constructor(
    @JvmField val providerIdHex: String,
    @JvmField val availableCreditNano: BigInteger,
    @JvmField val bondedNano: BigInteger,
    @JvmField val requiredBondNano: BigInteger,
    @JvmField val expectedSettlementNano: BigInteger,
    @JvmField val onboardingEpoch: Long,
    @JvmField val lastSettlementEpoch: Long,
    @JvmField val lowBalanceSinceEpoch: Long?,
    @JvmField val slashedNano: BigInteger,
    @JvmField val underDeliveryStrikes: Int?,
    @JvmField val lastPenaltyEpoch: Long?,
    metadata: Map<String, String>,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    private val _metadata: Map<String, String> = metadata.toMap()

    val metadata: Map<String, String> get() = _metadata

    constructor(
        providerIdHex: String,
        availableCreditNano: BigInteger,
        bondedNano: BigInteger,
        requiredBondNano: BigInteger,
        expectedSettlementNano: BigInteger,
        onboardingEpoch: Long,
        lastSettlementEpoch: Long,
        lowBalanceSinceEpoch: Long? = null,
        slashedNano: BigInteger = BigInteger.ZERO,
        underDeliveryStrikes: Int? = 0,
        lastPenaltyEpoch: Long? = null,
        metadata: Map<String, String> = emptyMap(),
    ) : this(
        providerIdHex = validatedProviderIdHex(providerIdHex),
        availableCreditNano = requireNonNegative(availableCreditNano, "availableCreditNano"),
        bondedNano = requireNonNegative(bondedNano, "bondedNano"),
        requiredBondNano = requireNonNegative(requiredBondNano, "requiredBondNano"),
        expectedSettlementNano = requireNonNegative(expectedSettlementNano, "expectedSettlementNano"),
        onboardingEpoch = ensureNonNegative(onboardingEpoch, "onboardingEpoch"),
        lastSettlementEpoch = ensureNonNegative(lastSettlementEpoch, "lastSettlementEpoch"),
        lowBalanceSinceEpoch = lowBalanceSinceEpoch?.also { ensureNonNegative(it, "lowBalanceSinceEpoch") },
        slashedNano = requireNonNegative(slashedNano, "slashedNano"),
        underDeliveryStrikes = underDeliveryStrikes?.also {
            require(it >= 0) { "underDeliveryStrikes must be non-negative" }
        },
        lastPenaltyEpoch = lastPenaltyEpoch?.also { ensureNonNegative(it, "lastPenaltyEpoch") },
        metadata = metadata,
        arguments = buildArguments(
            providerIdHex, availableCreditNano, bondedNano, requiredBondNano,
            expectedSettlementNano, onboardingEpoch, lastSettlementEpoch,
            lowBalanceSinceEpoch, slashedNano, underDeliveryStrikes, lastPenaltyEpoch, metadata,
        ),
    )

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is UpsertProviderCreditInstruction) return false
        return onboardingEpoch == other.onboardingEpoch
            && lastSettlementEpoch == other.lastSettlementEpoch
            && providerIdHex == other.providerIdHex
            && availableCreditNano == other.availableCreditNano
            && bondedNano == other.bondedNano
            && requiredBondNano == other.requiredBondNano
            && expectedSettlementNano == other.expectedSettlementNano
            && lowBalanceSinceEpoch == other.lowBalanceSinceEpoch
            && slashedNano == other.slashedNano
            && underDeliveryStrikes == other.underDeliveryStrikes
            && lastPenaltyEpoch == other.lastPenaltyEpoch
            && _metadata == other._metadata
    }

    override fun hashCode(): Int {
        var result = providerIdHex.hashCode()
        result = 31 * result + availableCreditNano.hashCode()
        result = 31 * result + bondedNano.hashCode()
        result = 31 * result + requiredBondNano.hashCode()
        result = 31 * result + expectedSettlementNano.hashCode()
        result = 31 * result + onboardingEpoch.hashCode()
        result = 31 * result + lastSettlementEpoch.hashCode()
        result = 31 * result + (lowBalanceSinceEpoch?.hashCode() ?: 0)
        result = 31 * result + slashedNano.hashCode()
        result = 31 * result + (underDeliveryStrikes?.hashCode() ?: 0)
        result = 31 * result + (lastPenaltyEpoch?.hashCode() ?: 0)
        result = 31 * result + _metadata.hashCode()
        return result
    }

    companion object {
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UpsertProviderCreditInstruction {
            val providerIdHex = require(arguments, "record.provider_id_hex")
            val availableCreditNano = BigInteger(require(arguments, "record.available_credit_nano"))
            val bondedNano = BigInteger(require(arguments, "record.bonded_nano"))
            val requiredBondNano = BigInteger(require(arguments, "record.required_bond_nano"))
            val expectedSettlementNano = BigInteger(require(arguments, "record.expected_settlement_nano"))
            val onboardingEpoch = require(arguments, "record.onboarding_epoch").toLong()
            val lastSettlementEpoch = require(arguments, "record.last_settlement_epoch").toLong()

            val lowBalanceSinceEpoch = arguments["record.low_balance_since_epoch"]
                ?.takeIf { it.isNotBlank() }?.toLong()
            val slashedNano = arguments["record.slashed_nano"]
                ?.takeIf { it.isNotBlank() }?.let { BigInteger(it) } ?: BigInteger.ZERO
            val underDeliveryStrikes = arguments["record.under_delivery_strikes"]
                ?.takeIf { it.isNotBlank() }?.toInt()
            val lastPenaltyEpoch = arguments["record.last_penalty_epoch"]
                ?.takeIf { it.isNotBlank() }?.toLong()

            val metadata = linkedMapOf<String, String>()
            arguments.forEach { (key, value) ->
                if (key.startsWith(METADATA_PREFIX)) {
                    metadata[key.substring(METADATA_PREFIX.length)] = value
                }
            }

            return UpsertProviderCreditInstruction(
                providerIdHex = providerIdHex,
                availableCreditNano = availableCreditNano,
                bondedNano = bondedNano,
                requiredBondNano = requiredBondNano,
                expectedSettlementNano = expectedSettlementNano,
                onboardingEpoch = onboardingEpoch,
                lastSettlementEpoch = lastSettlementEpoch,
                lowBalanceSinceEpoch = lowBalanceSinceEpoch,
                slashedNano = slashedNano,
                underDeliveryStrikes = underDeliveryStrikes,
                lastPenaltyEpoch = lastPenaltyEpoch,
                metadata = metadata,
            )
        }

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun buildArguments(
            providerIdHex: String,
            availableCreditNano: BigInteger,
            bondedNano: BigInteger,
            requiredBondNano: BigInteger,
            expectedSettlementNano: BigInteger,
            onboardingEpoch: Long,
            lastSettlementEpoch: Long,
            lowBalanceSinceEpoch: Long?,
            slashedNano: BigInteger,
            underDeliveryStrikes: Int?,
            lastPenaltyEpoch: Long?,
            metadata: Map<String, String>,
        ): Map<String, String> {
            val args = linkedMapOf<String, String>()
            args["action"] = ACTION
            args["record.provider_id_hex"] = providerIdHex
            args["record.available_credit_nano"] = availableCreditNano.toString()
            args["record.bonded_nano"] = bondedNano.toString()
            args["record.required_bond_nano"] = requiredBondNano.toString()
            args["record.expected_settlement_nano"] = expectedSettlementNano.toString()
            args["record.onboarding_epoch"] = onboardingEpoch.toString()
            args["record.last_settlement_epoch"] = lastSettlementEpoch.toString()
            if (lowBalanceSinceEpoch != null) {
                args["record.low_balance_since_epoch"] = lowBalanceSinceEpoch.toString()
            }
            args["record.slashed_nano"] = slashedNano.toString()
            if (underDeliveryStrikes != null) {
                args["record.under_delivery_strikes"] = underDeliveryStrikes.toString()
            }
            if (lastPenaltyEpoch != null) {
                args["record.last_penalty_epoch"] = lastPenaltyEpoch.toString()
            }
            metadata.forEach { (key, value) -> args["$METADATA_PREFIX$key"] = value }
            return args
        }
    }
}

private fun ensureNonNegative(value: Long, label: String): Long {
    require(value >= 0) { "$label must be non-negative" }
    return value
}

private fun requireNonNegative(value: BigInteger, label: String): BigInteger {
    require(value.signum() >= 0) { "$label must be a non-negative integer" }
    return value
}

private fun validatedProviderIdHex(value: String): String {
    val normalized = value.trim()
    require(normalized.isNotEmpty()) { "providerIdHex must not be blank" }
    return normalized
}
