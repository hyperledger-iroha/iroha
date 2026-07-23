package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class SetPricingScheduleInstructionTest {

    @Test
    fun `fromArguments rejects integer values that would wrap`() {
        val base = pricingSchedule().arguments
        for (key in INT_ARGUMENT_KEYS) {
            for (value in listOf("4294967296", "-2147483649")) {
                val error = assertFailsWith<IllegalArgumentException> {
                    SetPricingScheduleInstruction.fromArguments(base + (key to value))
                }
                assertEquals(
                    "Instruction argument '$key' is outside the signed 32-bit integer range",
                    error.message,
                )
            }
        }
    }

    @Test
    fun `fromArguments accepts integer maximum without narrowing long commitment`() {
        val arguments = pricingSchedule().arguments.toMutableMap()
        INT_ARGUMENT_KEYS.forEach { key -> arguments[key] = Int.MAX_VALUE.toString() }
        arguments[MINIMUM_COMMITMENT_KEY] = "4294967296"

        val parsed = SetPricingScheduleInstruction.fromArguments(arguments)

        INT_ARGUMENT_KEYS.forEach { key ->
            assertEquals(Int.MAX_VALUE.toString(), parsed.arguments[key])
        }
        assertEquals("4294967296", parsed.arguments[MINIMUM_COMMITMENT_KEY])
    }

    private fun pricingSchedule(): SetPricingScheduleInstruction =
        SetPricingScheduleInstruction.builder()
            .setVersion(1)
            .setCurrencyCode("xor")
            .setDefaultStorageClass(SetPricingScheduleInstruction.StorageClass.HOT)
            .addTier(
                SetPricingScheduleInstruction.TierRate.builder()
                    .setStorageClass(SetPricingScheduleInstruction.StorageClass.HOT)
                    .setStoragePriceNanoPerGibMonth(BigInteger.ONE)
                    .setEgressPriceNanoPerGib(BigInteger.ONE)
                    .build(),
            )
            .setCollateralPolicy(
                SetPricingScheduleInstruction.CollateralPolicy.builder()
                    .setMultiplierBps(10_000)
                    .setOnboardingDiscountBps(1_000)
                    .setOnboardingPeriodSecs(3_600)
                    .build(),
            )
            .setCreditPolicy(
                SetPricingScheduleInstruction.CreditPolicy.builder()
                    .setSettlementWindowSecs(3_600)
                    .setSettlementGraceSecs(60)
                    .setLowBalanceAlertBps(1_000)
                    .build(),
            )
            .setDiscountSchedule(
                SetPricingScheduleInstruction.DiscountSchedule.builder()
                    .setLoyaltyMonthsRequired(12)
                    .setLoyaltyDiscountBps(1_000)
                    .addCommitmentTier(
                        SetPricingScheduleInstruction.CommitmentDiscountTier.builder()
                            .setMinimumCommitmentGibMonth(500)
                            .setDiscountBps(500)
                            .build(),
                    )
                    .build(),
            )
            .build()

    private companion object {
        const val MINIMUM_COMMITMENT_PREFIX = "schedule.discounts.commitment_tiers.0."
        const val MINIMUM_COMMITMENT_KEY = "${MINIMUM_COMMITMENT_PREFIX}minimum_commitment_gib_month"

        val INT_ARGUMENT_KEYS =
            listOf(
                "schedule.credit.low_balance_alert_bps",
                "schedule.discounts.loyalty_months_required",
                "schedule.discounts.loyalty_discount_bps",
                "${MINIMUM_COMMITMENT_PREFIX}discount_bps",
            )
    }
}
