package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code SetPricingSchedule} instruction (SoraFS pricing/credit policy).
 */
public final class SetPricingScheduleInstruction implements InstructionTemplate {

  public static final String ACTION = "SetPricingSchedule";

  private final int version;
  private final String currencyCode;
  private final StorageClass defaultStorageClass;
  private final List<TierRate> tiers;
  private final CollateralPolicy collateralPolicy;
  private final CreditPolicy creditPolicy;
  private final DiscountSchedule discountSchedule;
  private final String notes;
  private final Map<String, String> arguments;

  private SetPricingScheduleInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private SetPricingScheduleInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.version = builder.version;
    this.currencyCode = builder.currencyCode;
    this.defaultStorageClass = builder.defaultStorageClass;
    this.tiers =
        Collections.unmodifiableList(
            new ArrayList<>(Objects.requireNonNull(builder.tiers, "tiers")));
    this.collateralPolicy = builder.collateralPolicy;
    this.creditPolicy = builder.creditPolicy;
    this.discountSchedule = builder.discountSchedule;
    this.notes = builder.notes;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public int version() {
    return version;
  }

  public String currencyCode() {
    return currencyCode;
  }

  public StorageClass defaultStorageClass() {
    return defaultStorageClass;
  }

  public List<TierRate> tiers() {
    return tiers;
  }

  public CollateralPolicy collateralPolicy() {
    return collateralPolicy;
  }

  public CreditPolicy creditPolicy() {
    return creditPolicy;
  }

  public DiscountSchedule discountSchedule() {
    return discountSchedule;
  }

  public String notes() {
    return notes;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static SetPricingScheduleInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setVersion(Integer.parseInt(require(arguments, "schedule.version")))
            .setCurrencyCode(require(arguments, "schedule.currency_code"))
            .setDefaultStorageClass(StorageClass.fromLabel(require(arguments, "schedule.default_storage_class")))
            .setCollateralPolicy(
                CollateralPolicy.builder()
                    .setMultiplierBps(parseLong(arguments, "schedule.collateral.multiplier_bps"))
                    .setOnboardingDiscountBps(
                        parseLong(arguments, "schedule.collateral.onboarding_discount_bps"))
                    .setOnboardingPeriodSecs(
                        parseLong(arguments, "schedule.collateral.onboarding_period_secs"))
                    .build())
            .setCreditPolicy(
                CreditPolicy.builder()
                    .setSettlementWindowSecs(
                        parseLong(arguments, "schedule.credit.settlement_window_secs"))
                    .setSettlementGraceSecs(
                        parseLong(arguments, "schedule.credit.settlement_grace_secs"))
                    .setLowBalanceAlertBps(
                        (int) parseLong(arguments, "schedule.credit.low_balance_alert_bps"))
                    .build());

    final DiscountSchedule.Builder discountBuilder =
        DiscountSchedule.builder()
            .setLoyaltyMonthsRequired(
                (int) parseLong(arguments, "schedule.discounts.loyalty_months_required"))
            .setLoyaltyDiscountBps(
                (int) parseLong(arguments, "schedule.discounts.loyalty_discount_bps"));
    int discountIndex = 0;
    while (arguments.containsKey("schedule.discounts.commitment_tiers." + discountIndex + ".minimum_commitment_gib_month")) {
      final String base =
          "schedule.discounts.commitment_tiers." + discountIndex + ".";
      discountBuilder.addCommitmentTier(
          CommitmentDiscountTier.builder()
              .setMinimumCommitmentGibMonth(
                  parseLong(arguments, base + "minimum_commitment_gib_month"))
              .setDiscountBps((int) parseLong(arguments, base + "discount_bps"))
              .build());
      discountIndex++;
    }
    builder.setDiscountSchedule(discountBuilder.build());

    final List<TierRate> tiers = new ArrayList<>();
    int tierIndex = 0;
    while (arguments.containsKey("schedule.tiers." + tierIndex + ".storage_class")) {
      final String base = "schedule.tiers." + tierIndex + ".";
      tiers.add(
          TierRate.builder()
              .setStorageClass(StorageClass.fromLabel(require(arguments, base + "storage_class")))
              .setStoragePriceNanoPerGibMonth(
                  new BigInteger(require(arguments, base + "storage_price_nano_per_gib_month")))
              .setEgressPriceNanoPerGib(
                  new BigInteger(require(arguments, base + "egress_price_nano_per_gib")))
              .build());
      tierIndex++;
    }
    builder.clearTiers().addAllTiers(tiers);

    final String maybeNotes = arguments.get("schedule.notes");
    if (maybeNotes != null && !maybeNotes.isBlank()) {
      builder.setNotes(maybeNotes);
    }
    return builder.build();
  }

  private static long parseLong(final Map<String, String> arguments, final String key) {
    return Long.parseLong(require(arguments, key));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof SetPricingScheduleInstruction other)) {
      return false;
    }
    return version == other.version
        && Objects.equals(currencyCode, other.currencyCode)
        && defaultStorageClass == other.defaultStorageClass
        && tiers.equals(other.tiers)
        && Objects.equals(collateralPolicy, other.collateralPolicy)
        && Objects.equals(creditPolicy, other.creditPolicy)
        && Objects.equals(discountSchedule, other.discountSchedule)
        && Objects.equals(notes, other.notes);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        version, currencyCode, defaultStorageClass, tiers, collateralPolicy, creditPolicy, discountSchedule, notes);
  }

  public static Builder builder() {
    return new Builder();
  }

  public enum StorageClass {
    HOT("hot"),
    WARM("warm"),
    COLD("cold");

    private final String label;

    StorageClass(final String label) {
      this.label = label;
    }

    public String label() {
      return label;
    }

    public static StorageClass fromLabel(final String label) {
      final String normalized = label.trim().toLowerCase(Locale.ROOT);
      for (final StorageClass value : values()) {
        if (value.label.equals(normalized)) {
          return value;
        }
      }
      throw new IllegalArgumentException("Unknown storage class: " + label);
    }
  }

  public static final class TierRate {
    private final StorageClass storageClass;
    private final BigInteger storagePriceNanoPerGibMonth;
    private final BigInteger egressPriceNanoPerGib;

    private TierRate(final Builder builder) {
      this.storageClass = builder.storageClass;
      this.storagePriceNanoPerGibMonth = builder.storagePriceNanoPerGibMonth;
      this.egressPriceNanoPerGib = builder.egressPriceNanoPerGib;
    }

    public StorageClass storageClass() {
      return storageClass;
    }

    public BigInteger storagePriceNanoPerGibMonth() {
      return storagePriceNanoPerGibMonth;
    }

    public BigInteger egressPriceNanoPerGib() {
      return egressPriceNanoPerGib;
    }

    private void appendArguments(final Map<String, String> args, final String prefix) {
      args.put(prefix + ".storage_class", storageClass.label());
      args.put(prefix + ".storage_price_nano_per_gib_month", storagePriceNanoPerGibMonth.toString());
      args.put(prefix + ".egress_price_nano_per_gib", egressPriceNanoPerGib.toString());
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private StorageClass storageClass;
      private BigInteger storagePriceNanoPerGibMonth;
      private BigInteger egressPriceNanoPerGib;

      private Builder() {}

      public Builder setStorageClass(final StorageClass storageClass) {
        this.storageClass = Objects.requireNonNull(storageClass, "storageClass");
        return this;
      }

      public Builder setStoragePriceNanoPerGibMonth(final BigInteger price) {
        this.storagePriceNanoPerGibMonth = requireNonNegative(price, "storage price");
        return this;
      }

      public Builder setEgressPriceNanoPerGib(final BigInteger price) {
        this.egressPriceNanoPerGib = requireNonNegative(price, "egress price");
        return this;
      }

      public TierRate build() {
        if (storageClass == null) {
          throw new IllegalStateException("storageClass must be provided");
        }
        if (storagePriceNanoPerGibMonth == null) {
          throw new IllegalStateException("storagePriceNanoPerGibMonth must be provided");
        }
        if (egressPriceNanoPerGib == null) {
          throw new IllegalStateException("egressPriceNanoPerGib must be provided");
        }
        return new TierRate(this);
      }
    }
  }

  public static final class CollateralPolicy {
    private final long multiplierBps;
    private final long onboardingDiscountBps;
    private final long onboardingPeriodSecs;

    private CollateralPolicy(final Builder builder) {
      this.multiplierBps = builder.multiplierBps;
      this.onboardingDiscountBps = builder.onboardingDiscountBps;
      this.onboardingPeriodSecs = builder.onboardingPeriodSecs;
    }

    public long multiplierBps() {
      return multiplierBps;
    }

    public long onboardingDiscountBps() {
      return onboardingDiscountBps;
    }

    public long onboardingPeriodSecs() {
      return onboardingPeriodSecs;
    }

    private void appendArguments(final Map<String, String> args) {
      args.put("schedule.collateral.multiplier_bps", Long.toString(multiplierBps));
      args.put(
          "schedule.collateral.onboarding_discount_bps", Long.toString(onboardingDiscountBps));
      args.put(
          "schedule.collateral.onboarding_period_secs", Long.toString(onboardingPeriodSecs));
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private Long multiplierBps;
      private Long onboardingDiscountBps;
      private Long onboardingPeriodSecs;

      private Builder() {}

      public Builder setMultiplierBps(final long multiplierBps) {
        this.multiplierBps = ensureNonNegative(multiplierBps, "multiplierBps");
        return this;
      }

      public Builder setOnboardingDiscountBps(final long discountBps) {
        this.onboardingDiscountBps = ensureNonNegative(discountBps, "onboardingDiscountBps");
        return this;
      }

      public Builder setOnboardingPeriodSecs(final long periodSecs) {
        this.onboardingPeriodSecs = ensureNonNegative(periodSecs, "onboardingPeriodSecs");
        return this;
      }

      public CollateralPolicy build() {
        if (multiplierBps == null || onboardingDiscountBps == null || onboardingPeriodSecs == null) {
          throw new IllegalStateException("collateral policy requires all fields");
        }
        return new CollateralPolicy(this);
      }
    }
  }

  public static final class CreditPolicy {
    private final long settlementWindowSecs;
    private final long settlementGraceSecs;
    private final int lowBalanceAlertBps;

    private CreditPolicy(final Builder builder) {
      this.settlementWindowSecs = builder.settlementWindowSecs;
      this.settlementGraceSecs = builder.settlementGraceSecs;
      this.lowBalanceAlertBps = builder.lowBalanceAlertBps;
    }

    public long settlementWindowSecs() {
      return settlementWindowSecs;
    }

    public long settlementGraceSecs() {
      return settlementGraceSecs;
    }

    public int lowBalanceAlertBps() {
      return lowBalanceAlertBps;
    }

    private void appendArguments(final Map<String, String> args) {
      args.put("schedule.credit.settlement_window_secs", Long.toString(settlementWindowSecs));
      args.put("schedule.credit.settlement_grace_secs", Long.toString(settlementGraceSecs));
      args.put("schedule.credit.low_balance_alert_bps", Integer.toString(lowBalanceAlertBps));
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private Long settlementWindowSecs;
      private Long settlementGraceSecs;
      private Integer lowBalanceAlertBps;

      private Builder() {}

      public Builder setSettlementWindowSecs(final long windowSecs) {
        this.settlementWindowSecs = ensureNonNegative(windowSecs, "settlementWindowSecs");
        return this;
      }

      public Builder setSettlementGraceSecs(final long graceSecs) {
        this.settlementGraceSecs = ensureNonNegative(graceSecs, "settlementGraceSecs");
        return this;
      }

      public Builder setLowBalanceAlertBps(final int bps) {
        if (bps < 0) {
          throw new IllegalArgumentException("lowBalanceAlertBps must be non-negative");
        }
        this.lowBalanceAlertBps = bps;
        return this;
      }

      public CreditPolicy build() {
        if (settlementWindowSecs == null || settlementGraceSecs == null || lowBalanceAlertBps == null) {
          throw new IllegalStateException("credit policy requires all fields");
        }
        return new CreditPolicy(this);
      }
    }
  }

  public static final class DiscountSchedule {
    private final int loyaltyMonthsRequired;
    private final int loyaltyDiscountBps;
    private final List<CommitmentDiscountTier> commitmentTiers;

    private DiscountSchedule(final Builder builder) {
      this.loyaltyMonthsRequired = builder.loyaltyMonthsRequired;
      this.loyaltyDiscountBps = builder.loyaltyDiscountBps;
      this.commitmentTiers =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(builder.commitmentTiers)));
    }

    public int loyaltyMonthsRequired() {
      return loyaltyMonthsRequired;
    }

    public int loyaltyDiscountBps() {
      return loyaltyDiscountBps;
    }

    public List<CommitmentDiscountTier> commitmentTiers() {
      return commitmentTiers;
    }

    private void appendArguments(final Map<String, String> args) {
      args.put(
          "schedule.discounts.loyalty_months_required", Integer.toString(loyaltyMonthsRequired));
      args.put("schedule.discounts.loyalty_discount_bps", Integer.toString(loyaltyDiscountBps));
      for (int i = 0; i < commitmentTiers.size(); i++) {
        commitmentTiers.get(i)
            .appendArguments(args, "schedule.discounts.commitment_tiers." + i);
      }
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private Integer loyaltyMonthsRequired;
      private Integer loyaltyDiscountBps;
      private final List<CommitmentDiscountTier> commitmentTiers = new ArrayList<>();

      private Builder() {}

      public Builder setLoyaltyMonthsRequired(final int months) {
        if (months < 0) {
          throw new IllegalArgumentException("loyaltyMonthsRequired must be non-negative");
        }
        this.loyaltyMonthsRequired = months;
        return this;
      }

      public Builder setLoyaltyDiscountBps(final int bps) {
        if (bps < 0) {
          throw new IllegalArgumentException("loyaltyDiscountBps must be non-negative");
        }
        this.loyaltyDiscountBps = bps;
        return this;
      }

      public Builder addCommitmentTier(final CommitmentDiscountTier tier) {
        commitmentTiers.add(Objects.requireNonNull(tier, "tier"));
        return this;
      }

      public DiscountSchedule build() {
        if (loyaltyMonthsRequired == null || loyaltyDiscountBps == null) {
          throw new IllegalStateException("discount schedule requires loyalty fields");
        }
        return new DiscountSchedule(this);
      }
    }
  }

  public static final class CommitmentDiscountTier {
    private final long minimumCommitmentGibMonth;
    private final int discountBps;

    private CommitmentDiscountTier(final Builder builder) {
      this.minimumCommitmentGibMonth = builder.minimumCommitmentGibMonth;
      this.discountBps = builder.discountBps;
    }

    private void appendArguments(final Map<String, String> args, final String prefix) {
      args.put(prefix + ".minimum_commitment_gib_month", Long.toString(minimumCommitmentGibMonth));
      args.put(prefix + ".discount_bps", Integer.toString(discountBps));
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private Long minimumCommitmentGibMonth;
      private Integer discountBps;

      private Builder() {}

      public Builder setMinimumCommitmentGibMonth(final long commitment) {
        this.minimumCommitmentGibMonth = ensureNonNegative(commitment, "minimumCommitment");
        return this;
      }

      public Builder setDiscountBps(final int bps) {
        if (bps < 0) {
          throw new IllegalArgumentException("discountBps must be non-negative");
        }
        this.discountBps = bps;
        return this;
      }

      public CommitmentDiscountTier build() {
        if (minimumCommitmentGibMonth == null || discountBps == null) {
          throw new IllegalStateException("commitment tier requires all fields");
        }
        return new CommitmentDiscountTier(this);
      }
    }
  }

  public static final class Builder {
    private Integer version;
    private String currencyCode;
    private StorageClass defaultStorageClass;
    private final List<TierRate> tiers = new ArrayList<>();
    private CollateralPolicy collateralPolicy;
    private CreditPolicy creditPolicy;
    private DiscountSchedule discountSchedule;
    private String notes;

    private Builder() {}

    public Builder setVersion(final int version) {
      if (version <= 0) {
        throw new IllegalArgumentException("version must be positive");
      }
      this.version = version;
      return this;
    }

    public Builder setCurrencyCode(final String currencyCode) {
      final String normalized = Objects.requireNonNull(currencyCode, "currencyCode").trim();
      if (normalized.length() != 3) {
        throw new IllegalArgumentException("currencyCode must be a 3-letter code");
      }
      this.currencyCode = normalized.toLowerCase(Locale.ROOT);
      return this;
    }

    public Builder setDefaultStorageClass(final StorageClass storageClass) {
      this.defaultStorageClass = Objects.requireNonNull(storageClass, "defaultStorageClass");
      return this;
    }

    public Builder addTier(final TierRate tier) {
      tiers.add(Objects.requireNonNull(tier, "tier"));
      return this;
    }

    Builder addAllTiers(final List<TierRate> entries) {
      tiers.addAll(entries);
      return this;
    }

    Builder clearTiers() {
      tiers.clear();
      return this;
    }

    public Builder setCollateralPolicy(final CollateralPolicy policy) {
      this.collateralPolicy = Objects.requireNonNull(policy, "collateralPolicy");
      return this;
    }

    public Builder setCreditPolicy(final CreditPolicy policy) {
      this.creditPolicy = Objects.requireNonNull(policy, "creditPolicy");
      return this;
    }

    public Builder setDiscountSchedule(final DiscountSchedule schedule) {
      this.discountSchedule = Objects.requireNonNull(schedule, "discountSchedule");
      return this;
    }

    public Builder setNotes(final String notes) {
      this.notes = notes == null ? null : notes.trim();
      return this;
    }

    public SetPricingScheduleInstruction build() {
      if (version == null) {
        throw new IllegalStateException("version must be provided");
      }
      if (currencyCode == null) {
        throw new IllegalStateException("currencyCode must be provided");
      }
      if (defaultStorageClass == null) {
        throw new IllegalStateException("defaultStorageClass must be provided");
      }
      if (tiers.isEmpty()) {
        throw new IllegalStateException("at least one tier must be provided");
      }
      if (collateralPolicy == null || creditPolicy == null || discountSchedule == null) {
        throw new IllegalStateException("collateral, credit, and discount policies are required");
      }
      return new SetPricingScheduleInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("schedule.version", Integer.toString(version));
      args.put("schedule.currency_code", currencyCode);
      args.put("schedule.default_storage_class", defaultStorageClass.label());
      for (int i = 0; i < tiers.size(); i++) {
        tiers.get(i).appendArguments(args, "schedule.tiers." + i);
      }
      collateralPolicy.appendArguments(args);
      creditPolicy.appendArguments(args);
      discountSchedule.appendArguments(args);
      if (notes != null && !notes.isBlank()) {
        args.put("schedule.notes", notes);
      }
      return args;
    }
  }

  private static long ensureNonNegative(final long value, final String label) {
    if (value < 0) {
      throw new IllegalArgumentException(label + " must be non-negative");
    }
    return value;
  }

  private static BigInteger requireNonNegative(final BigInteger value, final String label) {
    if (value == null || value.signum() < 0) {
      throw new IllegalArgumentException(label + " must be a non-negative integer");
    }
    return value;
  }
}
