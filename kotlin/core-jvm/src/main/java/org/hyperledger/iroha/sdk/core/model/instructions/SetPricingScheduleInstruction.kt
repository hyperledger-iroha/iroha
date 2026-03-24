package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import java.util.Locale

/** Typed builder for the `SetPricingSchedule` instruction (SoraFS pricing/credit policy). */
class SetPricingScheduleInstruction private constructor(
    val version: Int,
    val currencyCode: String,
    val defaultStorageClass: StorageClass,
    val tiers: List<TierRate>,
    val collateralPolicy: CollateralPolicy,
    val creditPolicy: CreditPolicy,
    val discountSchedule: DiscountSchedule,
    val notes: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SetPricingScheduleInstruction) return false
        return version == other.version
            && currencyCode == other.currencyCode
            && defaultStorageClass == other.defaultStorageClass
            && tiers == other.tiers
            && collateralPolicy == other.collateralPolicy
            && creditPolicy == other.creditPolicy
            && discountSchedule == other.discountSchedule
            && notes == other.notes
    }

    override fun hashCode(): Int =
        listOf(version, currencyCode, defaultStorageClass, tiers, collateralPolicy, creditPolicy, discountSchedule, notes).hashCode()

    enum class StorageClass(val label: String) {
        HOT("hot"),
        WARM("warm"),
        COLD("cold");

        companion object {
            @JvmStatic
            fun fromLabel(label: String): StorageClass {
                val normalized = label.trim().lowercase(Locale.ROOT)
                return entries.find { it.label == normalized }
                    ?: throw IllegalArgumentException("Unknown storage class: $label")
            }
        }
    }

    class TierRate private constructor(
        val storageClass: StorageClass,
        val storagePriceNanoPerGibMonth: BigInteger,
        val egressPriceNanoPerGib: BigInteger,
    ) {
        internal fun appendArguments(args: MutableMap<String, String>, prefix: String) {
            args["$prefix.storage_class"] = storageClass.label
            args["$prefix.storage_price_nano_per_gib_month"] = storagePriceNanoPerGibMonth.toString()
            args["$prefix.egress_price_nano_per_gib"] = egressPriceNanoPerGib.toString()
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is TierRate) return false
            return storageClass == other.storageClass
                && storagePriceNanoPerGibMonth == other.storagePriceNanoPerGibMonth
                && egressPriceNanoPerGib == other.egressPriceNanoPerGib
        }

        override fun hashCode(): Int = listOf(storageClass, storagePriceNanoPerGibMonth, egressPriceNanoPerGib).hashCode()

        class Builder internal constructor() {
            private var storageClass: StorageClass? = null
            private var storagePriceNanoPerGibMonth: BigInteger? = null
            private var egressPriceNanoPerGib: BigInteger? = null

            fun setStorageClass(storageClass: StorageClass) = apply {
                this.storageClass = storageClass
            }

            fun setStoragePriceNanoPerGibMonth(price: BigInteger) = apply {
                this.storagePriceNanoPerGibMonth = requireNonNegative(price, "storage price")
            }

            fun setEgressPriceNanoPerGib(price: BigInteger) = apply {
                this.egressPriceNanoPerGib = requireNonNegative(price, "egress price")
            }

            fun build(): TierRate {
                val sc = checkNotNull(storageClass) { "storageClass must be provided" }
                val sp = checkNotNull(storagePriceNanoPerGibMonth) { "storagePriceNanoPerGibMonth must be provided" }
                val ep = checkNotNull(egressPriceNanoPerGib) { "egressPriceNanoPerGib must be provided" }
                return TierRate(sc, sp, ep)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()
        }
    }

    class CollateralPolicy private constructor(
        val multiplierBps: Long,
        val onboardingDiscountBps: Long,
        val onboardingPeriodSecs: Long,
    ) {
        internal fun appendArguments(args: MutableMap<String, String>) {
            args["schedule.collateral.multiplier_bps"] = multiplierBps.toString()
            args["schedule.collateral.onboarding_discount_bps"] = onboardingDiscountBps.toString()
            args["schedule.collateral.onboarding_period_secs"] = onboardingPeriodSecs.toString()
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CollateralPolicy) return false
            return multiplierBps == other.multiplierBps
                && onboardingDiscountBps == other.onboardingDiscountBps
                && onboardingPeriodSecs == other.onboardingPeriodSecs
        }

        override fun hashCode(): Int = listOf(multiplierBps, onboardingDiscountBps, onboardingPeriodSecs).hashCode()

        class Builder internal constructor() {
            private var multiplierBps: Long? = null
            private var onboardingDiscountBps: Long? = null
            private var onboardingPeriodSecs: Long? = null

            fun setMultiplierBps(multiplierBps: Long) = apply {
                this.multiplierBps = ensureNonNegative(multiplierBps, "multiplierBps")
            }

            fun setOnboardingDiscountBps(discountBps: Long) = apply {
                this.onboardingDiscountBps = ensureNonNegative(discountBps, "onboardingDiscountBps")
            }

            fun setOnboardingPeriodSecs(periodSecs: Long) = apply {
                this.onboardingPeriodSecs = ensureNonNegative(periodSecs, "onboardingPeriodSecs")
            }

            fun build(): CollateralPolicy {
                check(multiplierBps != null && onboardingDiscountBps != null && onboardingPeriodSecs != null) {
                    "collateral policy requires all fields"
                }
                return CollateralPolicy(multiplierBps!!, onboardingDiscountBps!!, onboardingPeriodSecs!!)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()
        }
    }

    class CreditPolicy private constructor(
        val settlementWindowSecs: Long,
        val settlementGraceSecs: Long,
        val lowBalanceAlertBps: Int,
    ) {
        internal fun appendArguments(args: MutableMap<String, String>) {
            args["schedule.credit.settlement_window_secs"] = settlementWindowSecs.toString()
            args["schedule.credit.settlement_grace_secs"] = settlementGraceSecs.toString()
            args["schedule.credit.low_balance_alert_bps"] = lowBalanceAlertBps.toString()
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CreditPolicy) return false
            return settlementWindowSecs == other.settlementWindowSecs
                && settlementGraceSecs == other.settlementGraceSecs
                && lowBalanceAlertBps == other.lowBalanceAlertBps
        }

        override fun hashCode(): Int = listOf(settlementWindowSecs, settlementGraceSecs, lowBalanceAlertBps).hashCode()

        class Builder internal constructor() {
            private var settlementWindowSecs: Long? = null
            private var settlementGraceSecs: Long? = null
            private var lowBalanceAlertBps: Int? = null

            fun setSettlementWindowSecs(windowSecs: Long) = apply {
                this.settlementWindowSecs = ensureNonNegative(windowSecs, "settlementWindowSecs")
            }

            fun setSettlementGraceSecs(graceSecs: Long) = apply {
                this.settlementGraceSecs = ensureNonNegative(graceSecs, "settlementGraceSecs")
            }

            fun setLowBalanceAlertBps(bps: Int) = apply {
                require(bps >= 0) { "lowBalanceAlertBps must be non-negative" }
                this.lowBalanceAlertBps = bps
            }

            fun build(): CreditPolicy {
                check(settlementWindowSecs != null && settlementGraceSecs != null && lowBalanceAlertBps != null) {
                    "credit policy requires all fields"
                }
                return CreditPolicy(settlementWindowSecs!!, settlementGraceSecs!!, lowBalanceAlertBps!!)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()
        }
    }

    class DiscountSchedule private constructor(
        val loyaltyMonthsRequired: Int,
        val loyaltyDiscountBps: Int,
        val commitmentTiers: List<CommitmentDiscountTier>,
    ) {
        internal fun appendArguments(args: MutableMap<String, String>) {
            args["schedule.discounts.loyalty_months_required"] = loyaltyMonthsRequired.toString()
            args["schedule.discounts.loyalty_discount_bps"] = loyaltyDiscountBps.toString()
            commitmentTiers.forEachIndexed { i, tier ->
                tier.appendArguments(args, "schedule.discounts.commitment_tiers.$i")
            }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is DiscountSchedule) return false
            return loyaltyMonthsRequired == other.loyaltyMonthsRequired
                && loyaltyDiscountBps == other.loyaltyDiscountBps
                && commitmentTiers == other.commitmentTiers
        }

        override fun hashCode(): Int = listOf(loyaltyMonthsRequired, loyaltyDiscountBps, commitmentTiers).hashCode()

        class Builder internal constructor() {
            private var loyaltyMonthsRequired: Int? = null
            private var loyaltyDiscountBps: Int? = null
            private val commitmentTiers: MutableList<CommitmentDiscountTier> = mutableListOf()

            fun setLoyaltyMonthsRequired(months: Int) = apply {
                require(months >= 0) { "loyaltyMonthsRequired must be non-negative" }
                this.loyaltyMonthsRequired = months
            }

            fun setLoyaltyDiscountBps(bps: Int) = apply {
                require(bps >= 0) { "loyaltyDiscountBps must be non-negative" }
                this.loyaltyDiscountBps = bps
            }

            fun addCommitmentTier(tier: CommitmentDiscountTier) = apply {
                commitmentTiers.add(tier)
            }

            fun build(): DiscountSchedule {
                check(loyaltyMonthsRequired != null && loyaltyDiscountBps != null) {
                    "discount schedule requires loyalty fields"
                }
                return DiscountSchedule(loyaltyMonthsRequired!!, loyaltyDiscountBps!!, commitmentTiers.toList())
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()
        }
    }

    class CommitmentDiscountTier private constructor(
        val minimumCommitmentGibMonth: Long,
        val discountBps: Int,
    ) {
        internal fun appendArguments(args: MutableMap<String, String>, prefix: String) {
            args["$prefix.minimum_commitment_gib_month"] = minimumCommitmentGibMonth.toString()
            args["$prefix.discount_bps"] = discountBps.toString()
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CommitmentDiscountTier) return false
            return minimumCommitmentGibMonth == other.minimumCommitmentGibMonth
                && discountBps == other.discountBps
        }

        override fun hashCode(): Int = listOf(minimumCommitmentGibMonth, discountBps).hashCode()

        class Builder internal constructor() {
            private var minimumCommitmentGibMonth: Long? = null
            private var discountBps: Int? = null

            fun setMinimumCommitmentGibMonth(commitment: Long) = apply {
                this.minimumCommitmentGibMonth = ensureNonNegative(commitment, "minimumCommitment")
            }

            fun setDiscountBps(bps: Int) = apply {
                require(bps >= 0) { "discountBps must be non-negative" }
                this.discountBps = bps
            }

            fun build(): CommitmentDiscountTier {
                check(minimumCommitmentGibMonth != null && discountBps != null) {
                    "commitment tier requires all fields"
                }
                return CommitmentDiscountTier(minimumCommitmentGibMonth!!, discountBps!!)
            }
        }

        companion object {
            @JvmStatic
            fun builder(): Builder = Builder()
        }
    }

    class Builder internal constructor() {
        private var version: Int? = null
        private var currencyCode: String? = null
        private var defaultStorageClass: StorageClass? = null
        private val tiers: MutableList<TierRate> = mutableListOf()
        private var collateralPolicy: CollateralPolicy? = null
        private var creditPolicy: CreditPolicy? = null
        private var discountSchedule: DiscountSchedule? = null
        private var notes: String? = null

        fun setVersion(version: Int) = apply {
            require(version > 0) { "version must be positive" }
            this.version = version
        }

        fun setCurrencyCode(currencyCode: String) = apply {
            val normalized = currencyCode.trim()
            require(normalized.length == 3) { "currencyCode must be a 3-letter code" }
            this.currencyCode = normalized.lowercase(Locale.ROOT)
        }

        fun setDefaultStorageClass(storageClass: StorageClass) = apply {
            this.defaultStorageClass = storageClass
        }

        fun addTier(tier: TierRate) = apply {
            tiers.add(tier)
        }

        internal fun addAllTiers(entries: List<TierRate>) = apply {
            tiers.addAll(entries)
        }

        internal fun clearTiers() = apply {
            tiers.clear()
        }

        fun setCollateralPolicy(policy: CollateralPolicy) = apply {
            this.collateralPolicy = policy
        }

        fun setCreditPolicy(policy: CreditPolicy) = apply {
            this.creditPolicy = policy
        }

        fun setDiscountSchedule(schedule: DiscountSchedule) = apply {
            this.discountSchedule = schedule
        }

        fun setNotes(notes: String?) = apply {
            this.notes = notes?.trim()
        }

        fun build(): SetPricingScheduleInstruction {
            val v = checkNotNull(version) { "version must be provided" }
            val cc = checkNotNull(currencyCode) { "currencyCode must be provided" }
            val dsc = checkNotNull(defaultStorageClass) { "defaultStorageClass must be provided" }
            check(tiers.isNotEmpty()) { "at least one tier must be provided" }
            val cp = checkNotNull(collateralPolicy) { "collateral, credit, and discount policies are required" }
            val crp = checkNotNull(creditPolicy) { "collateral, credit, and discount policies are required" }
            val ds = checkNotNull(discountSchedule) { "collateral, credit, and discount policies are required" }
            return SetPricingScheduleInstruction(
                v, cc, dsc, tiers.toList(), cp, crp, ds, notes,
                canonicalArguments(v, cc, dsc, tiers, cp, crp, ds, notes),
            )
        }

        private fun canonicalArguments(
            v: Int,
            cc: String,
            dsc: StorageClass,
            tiers: List<TierRate>,
            cp: CollateralPolicy,
            crp: CreditPolicy,
            ds: DiscountSchedule,
            notes: String?,
        ): Map<String, String> {
            val args = LinkedHashMap<String, String>()
            args["action"] = ACTION
            args["schedule.version"] = v.toString()
            args["schedule.currency_code"] = cc
            args["schedule.default_storage_class"] = dsc.label
            tiers.forEachIndexed { i, tier -> tier.appendArguments(args, "schedule.tiers.$i") }
            cp.appendArguments(args)
            crp.appendArguments(args)
            ds.appendArguments(args)
            if (!notes.isNullOrBlank()) args["schedule.notes"] = notes
            return args
        }
    }

    companion object {
        const val ACTION: String = "SetPricingSchedule"

        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): SetPricingScheduleInstruction {
            val builder = builder()
                .setVersion(require(arguments, "schedule.version").toInt())
                .setCurrencyCode(require(arguments, "schedule.currency_code"))
                .setDefaultStorageClass(StorageClass.fromLabel(require(arguments, "schedule.default_storage_class")))
                .setCollateralPolicy(
                    CollateralPolicy.builder()
                        .setMultiplierBps(parseLong(arguments, "schedule.collateral.multiplier_bps"))
                        .setOnboardingDiscountBps(parseLong(arguments, "schedule.collateral.onboarding_discount_bps"))
                        .setOnboardingPeriodSecs(parseLong(arguments, "schedule.collateral.onboarding_period_secs"))
                        .build(),
                )
                .setCreditPolicy(
                    CreditPolicy.builder()
                        .setSettlementWindowSecs(parseLong(arguments, "schedule.credit.settlement_window_secs"))
                        .setSettlementGraceSecs(parseLong(arguments, "schedule.credit.settlement_grace_secs"))
                        .setLowBalanceAlertBps(parseLong(arguments, "schedule.credit.low_balance_alert_bps").toInt())
                        .build(),
                )

            val discountBuilder = DiscountSchedule.builder()
                .setLoyaltyMonthsRequired(parseLong(arguments, "schedule.discounts.loyalty_months_required").toInt())
                .setLoyaltyDiscountBps(parseLong(arguments, "schedule.discounts.loyalty_discount_bps").toInt())
            var discountIndex = 0
            while (arguments.containsKey("schedule.discounts.commitment_tiers.$discountIndex.minimum_commitment_gib_month")) {
                val base = "schedule.discounts.commitment_tiers.$discountIndex."
                discountBuilder.addCommitmentTier(
                    CommitmentDiscountTier.builder()
                        .setMinimumCommitmentGibMonth(parseLong(arguments, "${base}minimum_commitment_gib_month"))
                        .setDiscountBps(parseLong(arguments, "${base}discount_bps").toInt())
                        .build(),
                )
                discountIndex++
            }
            builder.setDiscountSchedule(discountBuilder.build())

            val tierList = mutableListOf<TierRate>()
            var tierIndex = 0
            while (arguments.containsKey("schedule.tiers.$tierIndex.storage_class")) {
                val base = "schedule.tiers.$tierIndex."
                tierList.add(
                    TierRate.builder()
                        .setStorageClass(StorageClass.fromLabel(require(arguments, "${base}storage_class")))
                        .setStoragePriceNanoPerGibMonth(BigInteger(require(arguments, "${base}storage_price_nano_per_gib_month")))
                        .setEgressPriceNanoPerGib(BigInteger(require(arguments, "${base}egress_price_nano_per_gib")))
                        .build(),
                )
                tierIndex++
            }
            builder.clearTiers().addAllTiers(tierList)

            val maybeNotes = arguments["schedule.notes"]
            if (!maybeNotes.isNullOrBlank()) builder.setNotes(maybeNotes)

            return builder.build()
        }

        private fun parseLong(arguments: Map<String, String>, key: String): Long =
            require(arguments, key).toLong()

        private fun require(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }
    }
}

private fun ensureNonNegative(value: Long, label: String): Long {
    require(value >= 0) { "$label must be non-negative" }
    return value
}

private fun requireNonNegative(value: BigInteger?, label: String): BigInteger {
    require(value != null && value.signum() >= 0) { "$label must be a non-negative integer" }
    return value
}
