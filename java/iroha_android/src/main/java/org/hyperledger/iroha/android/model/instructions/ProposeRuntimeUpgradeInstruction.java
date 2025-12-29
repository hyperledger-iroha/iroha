package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for {@code ProposeRuntimeUpgrade} instructions.
 *
 * <p>Encodes canonical runtime-upgrade manifest bytes using base64 so Android callers can round-trip
 * the same payloads Norito expects on the wire.
 */
public final class ProposeRuntimeUpgradeInstruction implements InstructionTemplate {

  public static final String ACTION = "ProposeRuntimeUpgrade";
  private static final String MANIFEST_BYTES_BASE64 = "manifest_bytes_base64";

  private final byte[] manifestBytes;
  private final Map<String, String> arguments;

  private ProposeRuntimeUpgradeInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ProposeRuntimeUpgradeInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.manifestBytes = builder.manifestBytes.clone();
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  /** Returns a defensive copy of the manifest bytes. */
  public byte[] manifestBytes() {
    return manifestBytes.clone();
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ProposeRuntimeUpgradeInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setManifestBytes(
                decodeBase64(require(arguments, MANIFEST_BYTES_BASE64), MANIFEST_BYTES_BASE64));
    return new ProposeRuntimeUpgradeInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static byte[] decodeBase64(final String value, final String fieldName) {
    try {
      return Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64 encoded", ex);
    }
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProposeRuntimeUpgradeInstruction other)) {
      return false;
    }
    return java.util.Arrays.equals(manifestBytes, other.manifestBytes);
  }

  @Override
  public int hashCode() {
    return java.util.Arrays.hashCode(manifestBytes);
  }

  public static final class Builder {
    private byte[] manifestBytes;

    private Builder() {}

    public Builder setManifestBytes(final byte[] manifestBytes) {
      this.manifestBytes = Objects.requireNonNull(manifestBytes, "manifestBytes").clone();
      if (this.manifestBytes.length == 0) {
        throw new IllegalArgumentException("manifestBytes must not be empty");
      }
      return this;
    }

    public ProposeRuntimeUpgradeInstruction build() {
      if (manifestBytes == null || manifestBytes.length == 0) {
        throw new IllegalStateException("manifestBytes must be provided");
      }
      return new ProposeRuntimeUpgradeInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put(MANIFEST_BYTES_BASE64, Base64.getEncoder().encodeToString(manifestBytes));
      return args;
    }
  }
}
