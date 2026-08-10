package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RetirePinManifest} instruction (SoraFS manifest lifecycle).
 * The recorded retirement epoch comes exclusively from the block consensus timestamp.
 */
public final class RetirePinManifestInstruction implements InstructionTemplate {

  public static final String ACTION = "RetirePinManifest";

  private final String digestHex;
  private final String reason;
  private final Map<String, String> arguments;

  private RetirePinManifestInstruction(final Builder builder) {
    this.digestHex = builder.digestHex;
    this.reason = builder.reason;
    this.arguments = Map.copyOf(builder.canonicalArguments());
  }

  public String digestHex() {
    return digestHex;
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
    Objects.requireNonNull(arguments, "arguments");
    if (!ACTION.equals(arguments.get("action"))) {
      throw new IllegalArgumentException("Instruction argument 'action' must be " + ACTION);
    }
    for (final String key : arguments.keySet()) {
      if (!key.equals("action") && !key.equals("digest_hex") && !key.equals("reason")) {
        throw new IllegalArgumentException("Unsupported RetirePinManifest argument: " + key);
      }
    }
    final Builder builder = builder().setDigestHex(require(arguments, "digest_hex"));
    final String reason = arguments.get("reason");
    if (reason != null && !reason.isBlank()) {
      builder.setReason(reason);
    }
    return builder.build();
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
    if (!(obj instanceof RetirePinManifestInstruction other)) {
      return false;
    }
    return Objects.equals(digestHex, other.digestHex)
        && Objects.equals(reason, other.reason);
  }

  @Override
  public int hashCode() {
    return Objects.hash(digestHex, reason);
  }

  public static final class Builder {
    private String digestHex;
    private String reason;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
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
      return new RetirePinManifestInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
      if (reason != null && !reason.isBlank()) {
        args.put("reason", reason);
      }
      return args;
    }
  }
}
