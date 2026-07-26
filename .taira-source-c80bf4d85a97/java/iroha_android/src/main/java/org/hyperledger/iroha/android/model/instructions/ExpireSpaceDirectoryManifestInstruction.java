package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code ExpireSpaceDirectoryManifest} instruction. */
public final class ExpireSpaceDirectoryManifestInstruction implements InstructionTemplate {

  private static final String ACTION = "ExpireSpaceDirectoryManifest";

  private final String uaid;
  private final long dataspace;
  private final long expiredEpoch;
  private final Map<String, String> arguments;

  private ExpireSpaceDirectoryManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ExpireSpaceDirectoryManifestInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.uaid = builder.uaid;
    this.dataspace = builder.dataspace;
    this.expiredEpoch = builder.expiredEpoch;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String uaid() {
    return uaid;
  }

  public long dataspace() {
    return dataspace;
  }

  public long expiredEpoch() {
    return expiredEpoch;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ExpireSpaceDirectoryManifestInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setUaid(require(arguments, "uaid"))
            .setDataspace(requireLong(arguments, "dataspace"))
            .setExpiredEpoch(requireLong(arguments, "expired_epoch"));
    return new ExpireSpaceDirectoryManifestInstruction(builder, new LinkedHashMap<>(arguments));
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

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ExpireSpaceDirectoryManifestInstruction other)) {
      return false;
    }
    return dataspace == other.dataspace
        && expiredEpoch == other.expiredEpoch
        && Objects.equals(uaid, other.uaid);
  }

  @Override
  public int hashCode() {
    return Objects.hash(uaid, dataspace, expiredEpoch);
  }

  public static final class Builder {
    private String uaid;
    private Long dataspace;
    private Long expiredEpoch;

    private Builder() {}

    public Builder setUaid(final String uaid) {
      this.uaid = Objects.requireNonNull(uaid, "uaid");
      return this;
    }

    public Builder setDataspace(final long dataspace) {
      this.dataspace = dataspace;
      return this;
    }

    public Builder setExpiredEpoch(final long expiredEpoch) {
      this.expiredEpoch = expiredEpoch;
      return this;
    }

    public ExpireSpaceDirectoryManifestInstruction build() {
      if (uaid == null || uaid.isBlank()) {
        throw new IllegalStateException("uaid must be provided");
      }
      if (dataspace == null) {
        throw new IllegalStateException("dataspace must be provided");
      }
      if (expiredEpoch == null) {
        throw new IllegalStateException("expiredEpoch must be provided");
      }
      return new ExpireSpaceDirectoryManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("uaid", uaid);
      args.put("dataspace", Long.toString(dataspace));
      args.put("expired_epoch", Long.toString(expiredEpoch));
      return args;
    }
  }
}
