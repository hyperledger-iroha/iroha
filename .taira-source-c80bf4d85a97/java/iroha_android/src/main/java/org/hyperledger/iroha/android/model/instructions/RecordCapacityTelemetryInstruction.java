package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code RecordCapacityTelemetry} instruction. */
public final class RecordCapacityTelemetryInstruction implements InstructionTemplate {

  public static final String ACTION = "RecordCapacityTelemetry";

  private final String providerIdHex;
  private final long windowStartEpoch;
  private final long windowEndEpoch;
  private final long declaredGib;
  private final long effectiveGib;
  private final long utilisedGib;
  private final long ordersIssued;
  private final long ordersCompleted;
  private final int uptimeBps;
  private final int porSuccessBps;
  private final long egressBytes;
  private final int pdpChallenges;
  private final int pdpFailures;
  private final int potrWindows;
  private final int potrBreaches;
  private final long nonce;
  private final Map<String, String> arguments;

  private RecordCapacityTelemetryInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RecordCapacityTelemetryInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.providerIdHex = builder.providerIdHex;
    this.windowStartEpoch = builder.windowStartEpoch;
    this.windowEndEpoch = builder.windowEndEpoch;
    this.declaredGib = builder.declaredGib;
    this.effectiveGib = builder.effectiveGib;
    this.utilisedGib = builder.utilisedGib;
    this.ordersIssued = builder.ordersIssued;
    this.ordersCompleted = builder.ordersCompleted;
    this.uptimeBps = builder.uptimeBps;
    this.porSuccessBps = builder.porSuccessBps;
    this.egressBytes = builder.egressBytes == null ? 0L : builder.egressBytes;
    this.pdpChallenges = builder.pdpChallenges == null ? 0 : builder.pdpChallenges;
    this.pdpFailures = builder.pdpFailures == null ? 0 : builder.pdpFailures;
    this.potrWindows = builder.potrWindows == null ? 0 : builder.potrWindows;
    this.potrBreaches = builder.potrBreaches == null ? 0 : builder.potrBreaches;
    this.nonce = builder.nonce == null ? 0L : builder.nonce;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public long windowStartEpoch() {
    return windowStartEpoch;
  }

  public long windowEndEpoch() {
    return windowEndEpoch;
  }

  public long declaredGib() {
    return declaredGib;
  }

  public long effectiveGib() {
    return effectiveGib;
  }

  public long utilisedGib() {
    return utilisedGib;
  }

  public long ordersIssued() {
    return ordersIssued;
  }

  public long ordersCompleted() {
    return ordersCompleted;
  }

  public int uptimeBps() {
    return uptimeBps;
  }

  public int porSuccessBps() {
    return porSuccessBps;
  }

  public long egressBytes() {
    return egressBytes;
  }

  public int pdpChallenges() {
    return pdpChallenges;
  }

  public int pdpFailures() {
    return pdpFailures;
  }

  public int potrWindows() {
    return potrWindows;
  }

  public int potrBreaches() {
    return potrBreaches;
  }

  public long nonce() {
    return nonce;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RecordCapacityTelemetryInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setProviderIdHex(require(arguments, "provider_id_hex"))
            .setWindowStartEpoch(requireLong(arguments, "window_start_epoch"))
            .setWindowEndEpoch(requireLong(arguments, "window_end_epoch"))
            .setDeclaredGib(requireLong(arguments, "declared_gib"))
            .setEffectiveGib(requireLong(arguments, "effective_gib"))
            .setUtilisedGib(requireLong(arguments, "utilised_gib"))
            .setOrdersIssued(requireLong(arguments, "orders_issued"))
            .setOrdersCompleted(requireLong(arguments, "orders_completed"))
            .setUptimeBps(requireInt(arguments, "uptime_bps"))
            .setPorSuccessBps(requireInt(arguments, "por_success_bps"));

    final Long egressBytes = parseOptionalLong(arguments, "egress_bytes");
    if (egressBytes != null) {
      builder.setEgressBytes(egressBytes);
    }
    final Integer pdpChallenges = parseOptionalInt(arguments, "pdp_challenges");
    if (pdpChallenges != null) {
      builder.setPdpChallenges(pdpChallenges);
    }
    final Integer pdpFailures = parseOptionalInt(arguments, "pdp_failures");
    if (pdpFailures != null) {
      builder.setPdpFailures(pdpFailures);
    }
    final Integer potrWindows = parseOptionalInt(arguments, "potr_windows");
    if (potrWindows != null) {
      builder.setPotrWindows(potrWindows);
    }
    final Integer potrBreaches = parseOptionalInt(arguments, "potr_breaches");
    if (potrBreaches != null) {
      builder.setPotrBreaches(potrBreaches);
    }
    final Long nonce = parseOptionalLong(arguments, "nonce");
    if (nonce != null) {
      builder.setNonce(nonce);
    }

    return new RecordCapacityTelemetryInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String raw = require(arguments, key);
    try {
      return Long.parseLong(raw);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + raw, ex);
    }
  }

  private static int requireInt(final Map<String, String> arguments, final String key) {
    final String raw = require(arguments, key);
    try {
      return Integer.parseInt(raw);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + raw, ex);
    }
  }

  private static Long parseOptionalLong(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      return null;
    }
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(key + " must be a number: " + value, ex);
    }
  }

  private static Integer parseOptionalInt(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      return null;
    }
    try {
      return Integer.parseInt(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(key + " must be a number: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RecordCapacityTelemetryInstruction other)) {
      return false;
    }
    return windowStartEpoch == other.windowStartEpoch
        && windowEndEpoch == other.windowEndEpoch
        && declaredGib == other.declaredGib
        && effectiveGib == other.effectiveGib
        && utilisedGib == other.utilisedGib
        && ordersIssued == other.ordersIssued
        && ordersCompleted == other.ordersCompleted
        && uptimeBps == other.uptimeBps
        && porSuccessBps == other.porSuccessBps
        && egressBytes == other.egressBytes
        && pdpChallenges == other.pdpChallenges
        && pdpFailures == other.pdpFailures
        && potrWindows == other.potrWindows
        && potrBreaches == other.potrBreaches
        && nonce == other.nonce
        && Objects.equals(providerIdHex, other.providerIdHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        providerIdHex,
        windowStartEpoch,
        windowEndEpoch,
        declaredGib,
        effectiveGib,
        utilisedGib,
        ordersIssued,
        ordersCompleted,
        uptimeBps,
        porSuccessBps,
        egressBytes,
        pdpChallenges,
        pdpFailures,
        potrWindows,
        potrBreaches,
        nonce);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String providerIdHex;
    private Long windowStartEpoch;
    private Long windowEndEpoch;
    private Long declaredGib;
    private Long effectiveGib;
    private Long utilisedGib;
    private Long ordersIssued;
    private Long ordersCompleted;
    private Integer uptimeBps;
    private Integer porSuccessBps;
    private Long egressBytes;
    private Integer pdpChallenges;
    private Integer pdpFailures;
    private Integer potrWindows;
    private Integer potrBreaches;
    private Long nonce;

    private Builder() {}

    public Builder setProviderIdHex(final String providerIdHex) {
      final String normalized = Objects.requireNonNull(providerIdHex, "providerIdHex").trim();
      if (normalized.isEmpty()) {
        throw new IllegalArgumentException("providerIdHex must not be blank");
      }
      this.providerIdHex = normalized;
      return this;
    }

    public Builder setProviderId(final byte[] providerId) {
      Objects.requireNonNull(providerId, "providerId");
      final StringBuilder sb = new StringBuilder(providerId.length * 2);
      for (final byte b : providerId) {
        sb.append(String.format("%02x", b));
      }
      return setProviderIdHex(sb.toString());
    }

    public Builder setWindowStartEpoch(final long windowStartEpoch) {
      this.windowStartEpoch = ensureNonNegative(windowStartEpoch, "windowStartEpoch");
      return this;
    }

    public Builder setWindowEndEpoch(final long windowEndEpoch) {
      this.windowEndEpoch = ensureNonNegative(windowEndEpoch, "windowEndEpoch");
      return this;
    }

    public Builder setDeclaredGib(final long declaredGib) {
      this.declaredGib = ensureNonNegative(declaredGib, "declaredGib");
      return this;
    }

    public Builder setEffectiveGib(final long effectiveGib) {
      this.effectiveGib = ensureNonNegative(effectiveGib, "effectiveGib");
      return this;
    }

    public Builder setUtilisedGib(final long utilisedGib) {
      this.utilisedGib = ensureNonNegative(utilisedGib, "utilisedGib");
      return this;
    }

    public Builder setOrdersIssued(final long ordersIssued) {
      this.ordersIssued = ensureNonNegative(ordersIssued, "ordersIssued");
      return this;
    }

    public Builder setOrdersCompleted(final long ordersCompleted) {
      this.ordersCompleted = ensureNonNegative(ordersCompleted, "ordersCompleted");
      return this;
    }

    public Builder setUptimeBps(final int uptimeBps) {
      this.uptimeBps = ensureNonNegative(uptimeBps, "uptimeBps");
      return this;
    }

    public Builder setPorSuccessBps(final int porSuccessBps) {
      this.porSuccessBps = ensureNonNegative(porSuccessBps, "porSuccessBps");
      return this;
    }

    public Builder setEgressBytes(final long egressBytes) {
      this.egressBytes = ensureNonNegative(egressBytes, "egressBytes");
      return this;
    }

    public Builder setPdpChallenges(final int pdpChallenges) {
      this.pdpChallenges = ensureNonNegative(pdpChallenges, "pdpChallenges");
      return this;
    }

    public Builder setPdpFailures(final int pdpFailures) {
      this.pdpFailures = ensureNonNegative(pdpFailures, "pdpFailures");
      return this;
    }

    public Builder setPotrWindows(final int potrWindows) {
      this.potrWindows = ensureNonNegative(potrWindows, "potrWindows");
      return this;
    }

    public Builder setPotrBreaches(final int potrBreaches) {
      this.potrBreaches = ensureNonNegative(potrBreaches, "potrBreaches");
      return this;
    }

    public Builder setNonce(final long nonce) {
      this.nonce = ensureNonNegative(nonce, "nonce");
      return this;
    }

    public RecordCapacityTelemetryInstruction build() {
      if (providerIdHex == null || providerIdHex.isBlank()) {
        throw new IllegalStateException("providerIdHex must be provided");
      }
      if (windowStartEpoch == null) {
        throw new IllegalStateException("windowStartEpoch must be provided");
      }
      if (windowEndEpoch == null) {
        throw new IllegalStateException("windowEndEpoch must be provided");
      }
      if (declaredGib == null) {
        throw new IllegalStateException("declaredGib must be provided");
      }
      if (effectiveGib == null) {
        throw new IllegalStateException("effectiveGib must be provided");
      }
      if (utilisedGib == null) {
        throw new IllegalStateException("utilisedGib must be provided");
      }
      if (ordersIssued == null) {
        throw new IllegalStateException("ordersIssued must be provided");
      }
      if (ordersCompleted == null) {
        throw new IllegalStateException("ordersCompleted must be provided");
      }
      if (uptimeBps == null) {
        throw new IllegalStateException("uptimeBps must be provided");
      }
      if (porSuccessBps == null) {
        throw new IllegalStateException("porSuccessBps must be provided");
      }
      if (windowEndEpoch < windowStartEpoch) {
        throw new IllegalStateException("windowEndEpoch must be >= windowStartEpoch");
      }
      return new RecordCapacityTelemetryInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("provider_id_hex", providerIdHex);
      args.put("window_start_epoch", Long.toString(windowStartEpoch));
      args.put("window_end_epoch", Long.toString(windowEndEpoch));
      args.put("declared_gib", Long.toString(declaredGib));
      args.put("effective_gib", Long.toString(effectiveGib));
      args.put("utilised_gib", Long.toString(utilisedGib));
      args.put("orders_issued", Long.toString(ordersIssued));
      args.put("orders_completed", Long.toString(ordersCompleted));
      args.put("uptime_bps", Integer.toString(uptimeBps));
      args.put("por_success_bps", Integer.toString(porSuccessBps));
      if (egressBytes != null) {
        args.put("egress_bytes", Long.toString(egressBytes));
      }
      if (pdpChallenges != null) {
        args.put("pdp_challenges", Integer.toString(pdpChallenges));
      }
      if (pdpFailures != null) {
        args.put("pdp_failures", Integer.toString(pdpFailures));
      }
      if (potrWindows != null) {
        args.put("potr_windows", Integer.toString(potrWindows));
      }
      if (potrBreaches != null) {
        args.put("potr_breaches", Integer.toString(potrBreaches));
      }
      if (nonce != null) {
        args.put("nonce", Long.toString(nonce));
      }
      return args;
    }

    private static long ensureNonNegative(final long value, final String label) {
      if (value < 0) {
        throw new IllegalArgumentException(label + " must be non-negative");
      }
      return value;
    }

    private static int ensureNonNegative(final int value, final String label) {
      if (value < 0) {
        throw new IllegalArgumentException(label + " must be non-negative");
      }
      return value;
    }
  }
}
