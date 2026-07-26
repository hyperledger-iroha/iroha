package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code RevokeSpaceDirectoryManifest} instruction. */
public final class RevokeSpaceDirectoryManifestInstruction implements InstructionTemplate {

  private static final String ACTION = "RevokeSpaceDirectoryManifest";

  private final String uaid;
  private final long dataspace;
  private final long revokedEpoch;
  private final String reason;
  private final Map<String, String> arguments;

  private RevokeSpaceDirectoryManifestInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RevokeSpaceDirectoryManifestInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.uaid = builder.uaid;
    this.dataspace = builder.dataspace;
    this.revokedEpoch = builder.revokedEpoch;
    this.reason = builder.reason;
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

  public long revokedEpoch() {
    return revokedEpoch;
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

  public static RevokeSpaceDirectoryManifestInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setUaid(require(arguments, "uaid"))
            .setDataspace(requireLong(arguments, "dataspace"))
            .setRevokedEpoch(requireLong(arguments, "revoked_epoch"));
    final String reason = arguments.get("reason");
    if (reason != null) {
      builder.setReason(reason);
    }
    return new RevokeSpaceDirectoryManifestInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RevokeSpaceDirectoryManifestInstruction other)) {
      return false;
    }
    return dataspace == other.dataspace
        && revokedEpoch == other.revokedEpoch
        && Objects.equals(uaid, other.uaid)
        && Objects.equals(reason, other.reason);
  }

  @Override
  public int hashCode() {
    return Objects.hash(uaid, dataspace, revokedEpoch, reason);
  }

  public static final class Builder {
    private String uaid;
    private Long dataspace;
    private Long revokedEpoch;
    private String reason;

    private Builder() {}

    public Builder setUaid(final String uaid) {
      this.uaid = Objects.requireNonNull(uaid, "uaid");
      return this;
    }

    public Builder setDataspace(final long dataspace) {
      this.dataspace = dataspace;
      return this;
    }

    public Builder setRevokedEpoch(final long revokedEpoch) {
      this.revokedEpoch = revokedEpoch;
      return this;
    }

    public Builder setReason(final String reason) {
      this.reason = reason;
      return this;
    }

    public RevokeSpaceDirectoryManifestInstruction build() {
      if (uaid == null || uaid.isBlank()) {
        throw new IllegalStateException("uaid must be provided");
      }
      if (dataspace == null) {
        throw new IllegalStateException("dataspace must be provided");
      }
      if (revokedEpoch == null) {
        throw new IllegalStateException("revokedEpoch must be provided");
      }
      return new RevokeSpaceDirectoryManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("uaid", uaid);
      args.put("dataspace", Long.toString(dataspace));
      args.put("revoked_epoch", Long.toString(revokedEpoch));
      if (reason != null && !reason.isBlank()) {
        args.put("reason", reason);
      }
      return args;
    }
  }
}
