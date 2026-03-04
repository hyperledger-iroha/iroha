package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code UpsertProviderCredit} instruction (SoraFS provider ledger).
 */
public final class UpsertProviderCreditInstruction implements InstructionTemplate {

  public static final String ACTION = "UpsertProviderCredit";

  private final String providerIdHex;
  private final BigInteger availableCreditNano;
  private final BigInteger bondedNano;
  private final BigInteger requiredBondNano;
  private final BigInteger expectedSettlementNano;
  private final long onboardingEpoch;
  private final long lastSettlementEpoch;
  private final Long lowBalanceSinceEpoch;
  private final BigInteger slashedNano;
  private final Integer underDeliveryStrikes;
  private final Long lastPenaltyEpoch;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private UpsertProviderCreditInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private UpsertProviderCreditInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.providerIdHex = builder.providerIdHex;
    this.availableCreditNano = builder.availableCreditNano;
    this.bondedNano = builder.bondedNano;
    this.requiredBondNano = builder.requiredBondNano;
    this.expectedSettlementNano = builder.expectedSettlementNano;
    this.onboardingEpoch = builder.onboardingEpoch;
    this.lastSettlementEpoch = builder.lastSettlementEpoch;
    this.lowBalanceSinceEpoch = builder.lowBalanceSinceEpoch;
    this.slashedNano = builder.slashedNano;
    this.underDeliveryStrikes = builder.underDeliveryStrikes;
    this.lastPenaltyEpoch = builder.lastPenaltyEpoch;
    this.metadata = Map.copyOf(builder.metadata);
    this.arguments =
        Map.copyOf(Objects.requireNonNull(canonicalArguments, "canonicalArguments"));
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public BigInteger availableCreditNano() {
    return availableCreditNano;
  }

  public BigInteger bondedNano() {
    return bondedNano;
  }

  public BigInteger requiredBondNano() {
    return requiredBondNano;
  }

  public BigInteger expectedSettlementNano() {
    return expectedSettlementNano;
  }

  public long onboardingEpoch() {
    return onboardingEpoch;
  }

  public long lastSettlementEpoch() {
    return lastSettlementEpoch;
  }

  public Long lowBalanceSinceEpoch() {
    return lowBalanceSinceEpoch;
  }

  public BigInteger slashedNano() {
    return slashedNano;
  }

  public Integer underDeliveryStrikes() {
    return underDeliveryStrikes;
  }

  public Long lastPenaltyEpoch() {
    return lastPenaltyEpoch;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static UpsertProviderCreditInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setProviderIdHex(require(arguments, "record.provider_id_hex"))
            .setAvailableCreditNano(new BigInteger(require(arguments, "record.available_credit_nano")))
            .setBondedNano(new BigInteger(require(arguments, "record.bonded_nano")))
            .setRequiredBondNano(new BigInteger(require(arguments, "record.required_bond_nano")))
            .setExpectedSettlementNano(
                new BigInteger(require(arguments, "record.expected_settlement_nano")))
            .setOnboardingEpoch(parseLong(arguments, "record.onboarding_epoch"))
            .setLastSettlementEpoch(parseLong(arguments, "record.last_settlement_epoch"));

    final String lowBalance = arguments.get("record.low_balance_since_epoch");
    if (lowBalance != null && !lowBalance.isBlank()) {
      builder.setLowBalanceSinceEpoch(Long.parseLong(lowBalance));
    }
    final String slashed = arguments.get("record.slashed_nano");
    if (slashed != null && !slashed.isBlank()) {
      builder.setSlashedNano(new BigInteger(slashed));
    }
    final String strikes = arguments.get("record.under_delivery_strikes");
    if (strikes != null && !strikes.isBlank()) {
      builder.setUnderDeliveryStrikes(Integer.parseInt(strikes));
    }
    final String lastPenalty = arguments.get("record.last_penalty_epoch");
    if (lastPenalty != null && !lastPenalty.isBlank()) {
      builder.setLastPenaltyEpoch(Long.parseLong(lastPenalty));
    }
    arguments.forEach(
        (key, value) -> {
          if (key.startsWith("record.metadata.") && value != null) {
            builder.putMetadata(key.substring("record.metadata.".length()), value);
          }
        });
    return builder.build();
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long parseLong(final Map<String, String> arguments, final String key) {
    return Long.parseLong(require(arguments, key));
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof UpsertProviderCreditInstruction other)) {
      return false;
    }
    return onboardingEpoch == other.onboardingEpoch
        && lastSettlementEpoch == other.lastSettlementEpoch
        && Objects.equals(providerIdHex, other.providerIdHex)
        && Objects.equals(availableCreditNano, other.availableCreditNano)
        && Objects.equals(bondedNano, other.bondedNano)
        && Objects.equals(requiredBondNano, other.requiredBondNano)
        && Objects.equals(expectedSettlementNano, other.expectedSettlementNano)
        && Objects.equals(lowBalanceSinceEpoch, other.lowBalanceSinceEpoch)
        && Objects.equals(slashedNano, other.slashedNano)
        && Objects.equals(underDeliveryStrikes, other.underDeliveryStrikes)
        && Objects.equals(lastPenaltyEpoch, other.lastPenaltyEpoch)
        && Objects.equals(metadata, other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        providerIdHex,
        availableCreditNano,
        bondedNano,
        requiredBondNano,
        expectedSettlementNano,
        onboardingEpoch,
        lastSettlementEpoch,
        lowBalanceSinceEpoch,
        slashedNano,
        underDeliveryStrikes,
        lastPenaltyEpoch,
        metadata);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String providerIdHex;
    private BigInteger availableCreditNano;
    private BigInteger bondedNano;
    private BigInteger requiredBondNano;
    private BigInteger expectedSettlementNano;
    private Long onboardingEpoch;
    private Long lastSettlementEpoch;
    private Long lowBalanceSinceEpoch;
    private BigInteger slashedNano = BigInteger.ZERO;
    private Integer underDeliveryStrikes = 0;
    private Long lastPenaltyEpoch;
    private final Map<String, String> metadata = new LinkedHashMap<>();

    private Builder() {}

    public Builder setProviderIdHex(final String providerIdHex) {
      final String normalized = Objects.requireNonNull(providerIdHex, "providerIdHex").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("providerIdHex must not be blank");
      }
      this.providerIdHex = normalized;
      return this;
    }

    public Builder setAvailableCreditNano(final BigInteger value) {
      this.availableCreditNano = requireNonNegative(value, "availableCreditNano");
      return this;
    }

    public Builder setBondedNano(final BigInteger value) {
      this.bondedNano = requireNonNegative(value, "bondedNano");
      return this;
    }

    public Builder setRequiredBondNano(final BigInteger value) {
      this.requiredBondNano = requireNonNegative(value, "requiredBondNano");
      return this;
    }

    public Builder setExpectedSettlementNano(final BigInteger value) {
      this.expectedSettlementNano = requireNonNegative(value, "expectedSettlementNano");
      return this;
    }

    public Builder setOnboardingEpoch(final long onboardingEpoch) {
      this.onboardingEpoch = ensureNonNegative(onboardingEpoch, "onboardingEpoch");
      return this;
    }

    public Builder setLastSettlementEpoch(final long lastSettlementEpoch) {
      this.lastSettlementEpoch = ensureNonNegative(lastSettlementEpoch, "lastSettlementEpoch");
      return this;
    }

    public Builder setLowBalanceSinceEpoch(final Long lowBalanceSinceEpoch) {
      if (lowBalanceSinceEpoch != null) {
        ensureNonNegative(lowBalanceSinceEpoch, "lowBalanceSinceEpoch");
      }
      this.lowBalanceSinceEpoch = lowBalanceSinceEpoch;
      return this;
    }

    public Builder setSlashedNano(final BigInteger slashedNano) {
      this.slashedNano = requireNonNegative(slashedNano, "slashedNano");
      return this;
    }

    public Builder setUnderDeliveryStrikes(final Integer strikes) {
      if (strikes != null && strikes < 0) {
        throw new IllegalArgumentException("underDeliveryStrikes must be non-negative");
      }
      this.underDeliveryStrikes = strikes;
      return this;
    }

    public Builder setLastPenaltyEpoch(final Long lastPenaltyEpoch) {
      if (lastPenaltyEpoch != null) {
        ensureNonNegative(lastPenaltyEpoch, "lastPenaltyEpoch");
      }
      this.lastPenaltyEpoch = lastPenaltyEpoch;
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(
          Objects.requireNonNull(key, "metadata key"),
          Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public UpsertProviderCreditInstruction build() {
      if (providerIdHex == null
          || availableCreditNano == null
          || bondedNano == null
          || requiredBondNano == null
          || expectedSettlementNano == null
          || onboardingEpoch == null
          || lastSettlementEpoch == null) {
        throw new IllegalStateException("provider credit requires all primary fields");
      }
      return new UpsertProviderCreditInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("record.provider_id_hex", providerIdHex);
      args.put("record.available_credit_nano", availableCreditNano.toString());
      args.put("record.bonded_nano", bondedNano.toString());
      args.put("record.required_bond_nano", requiredBondNano.toString());
      args.put("record.expected_settlement_nano", expectedSettlementNano.toString());
      args.put("record.onboarding_epoch", Long.toString(onboardingEpoch));
      args.put("record.last_settlement_epoch", Long.toString(lastSettlementEpoch));
      if (lowBalanceSinceEpoch != null) {
        args.put("record.low_balance_since_epoch", Long.toString(lowBalanceSinceEpoch));
      }
      if (slashedNano != null) {
        args.put("record.slashed_nano", slashedNano.toString());
      }
      if (underDeliveryStrikes != null) {
        args.put("record.under_delivery_strikes", Integer.toString(underDeliveryStrikes));
      }
      if (lastPenaltyEpoch != null) {
        args.put("record.last_penalty_epoch", Long.toString(lastPenaltyEpoch));
      }
      metadata.forEach((key, value) -> args.put("record.metadata." + key, value));
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
