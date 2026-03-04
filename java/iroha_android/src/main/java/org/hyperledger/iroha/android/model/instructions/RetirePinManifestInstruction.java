package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code RetirePinManifest} instruction (SoraFS manifest lifecycle). */
public final class RetirePinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "RetirePinManifest";

  private final String digestHex;
  private final long retiredEpoch;
  private final String reason;
  private final Map<String, String> arguments;

  private RetirePinManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RetirePinManifestInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.digestHex = builder.digestHex;
    this.retiredEpoch = builder.retiredEpoch;
    this.reason = builder.reason;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String digestHex() {
    return digestHex;
  }

  public long retiredEpoch() {
    return retiredEpoch;
  }

  public String reason() {
    return reason;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static RetirePinManifestInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDigestHex(require(arguments, "digest_hex"))
            .setRetiredEpoch(requireLong(arguments, "retired_epoch"));
    final String reason = arguments.get("reason");
    if (reason != null && !reason.isBlank()) {
      builder.setReason(reason);
    }
    return new RetirePinManifestInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RetirePinManifestInstruction other)) {
      return false;
    }
    return retiredEpoch == other.retiredEpoch
        && Objects.equals(digestHex, other.digestHex)
        && Objects.equals(reason, other.reason);
  }

  @Override
  public int hashCode() {
    return Objects.hash(digestHex, retiredEpoch, reason);
  }

  public static final class Builder {
    private String digestHex;
    private Long retiredEpoch;
    private String reason;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
      return this;
    }

    public Builder setRetiredEpoch(final long retiredEpoch) {
      if (retiredEpoch < 0) {
        throw new IllegalArgumentException("retiredEpoch must be non-negative");
      }
      this.retiredEpoch = retiredEpoch;
      return this;
    }

    public Builder setReason(final String reason) {
      this.reason = Objects.requireNonNull(reason, "reason");
      return this;
    }

    public RetirePinManifestInstruction build() {
      if (digestHex == null || digestHex.isBlank()) {
        throw new IllegalStateException("digestHex must be set");
      }
      if (retiredEpoch == null) {
        throw new IllegalStateException("retiredEpoch must be set");
      }
      return new RetirePinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
      args.put("retired_epoch", Long.toString(retiredEpoch));
      if (reason != null && !reason.isBlank()) {
        args.put("reason", reason);
      }
      return args;
    }
  }
}
