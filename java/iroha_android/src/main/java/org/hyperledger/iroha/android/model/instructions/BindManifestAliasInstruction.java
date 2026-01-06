package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code BindManifestAlias} instruction (SoraFS alias lifecycle).
 */
public final class BindManifestAliasInstruction implements InstructionTemplate {

  public static final String ACTION = "BindManifestAlias";

  private final String digestHex;
  private final RegisterPinManifestInstruction.AliasBinding aliasBinding;
  private final long boundEpoch;
  private final long expiryEpoch;
  private final Map<String, String> arguments;

  private BindManifestAliasInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private BindManifestAliasInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.digestHex = builder.digestHex;
    this.aliasBinding = builder.aliasBinding;
    this.boundEpoch = builder.boundEpoch;
    this.expiryEpoch = builder.expiryEpoch;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String digestHex() {
    return digestHex;
  }

  public RegisterPinManifestInstruction.AliasBinding aliasBinding() {
    return aliasBinding;
  }

  public long boundEpoch() {
    return boundEpoch;
  }

  public long expiryEpoch() {
    return expiryEpoch;
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

  public static BindManifestAliasInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDigestHex(require(arguments, "digest_hex"))
            .setBoundEpoch(requireLong(arguments, "bound_epoch"))
            .setExpiryEpoch(requireLong(arguments, "expiry_epoch"))
            .setAliasBinding(
                RegisterPinManifestInstruction.AliasBinding.fromArguments(arguments, true));
    return new BindManifestAliasInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof BindManifestAliasInstruction other)) {
      return false;
    }
    return boundEpoch == other.boundEpoch
        && expiryEpoch == other.expiryEpoch
        && Objects.equals(digestHex, other.digestHex)
        && Objects.equals(aliasBinding, other.aliasBinding);
  }

  @Override
  public int hashCode() {
    return Objects.hash(digestHex, aliasBinding, boundEpoch, expiryEpoch);
  }

  public static final class Builder {
    private String digestHex;
    private RegisterPinManifestInstruction.AliasBinding aliasBinding;
    private Long boundEpoch;
    private Long expiryEpoch;

    private Builder() {}

    public Builder setDigestHex(final String digestHex) {
      this.digestHex = Objects.requireNonNull(digestHex, "digestHex");
      return this;
    }

    public Builder setAliasBinding(
        final RegisterPinManifestInstruction.AliasBinding aliasBinding) {
      this.aliasBinding = Objects.requireNonNull(aliasBinding, "aliasBinding");
      return this;
    }

    public Builder setAliasBinding(
        final String aliasName, final String aliasNamespace, final String proofHex) {
      return setAliasBinding(
          RegisterPinManifestInstruction.AliasBinding.builder()
              .setName(aliasName)
              .setNamespace(aliasNamespace)
              .setProofHex(proofHex)
              .build());
    }

    public Builder setBoundEpoch(final long boundEpoch) {
      if (boundEpoch < 0) {
        throw new IllegalArgumentException("boundEpoch must be non-negative");
      }
      this.boundEpoch = boundEpoch;
      return this;
    }

    public Builder setExpiryEpoch(final long expiryEpoch) {
      if (expiryEpoch < 0) {
        throw new IllegalArgumentException("expiryEpoch must be non-negative");
      }
      this.expiryEpoch = expiryEpoch;
      return this;
    }

    public BindManifestAliasInstruction build() {
      if (digestHex == null || digestHex.isBlank()) {
        throw new IllegalStateException("digestHex must be set");
      }
      if (aliasBinding == null) {
        throw new IllegalStateException("aliasBinding must be set");
      }
      if (boundEpoch == null) {
        throw new IllegalStateException("boundEpoch must be set");
      }
      if (expiryEpoch == null) {
        throw new IllegalStateException("expiryEpoch must be set");
      }
      return new BindManifestAliasInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("digest_hex", digestHex);
      args.put("bound_epoch", Long.toString(boundEpoch));
      args.put("expiry_epoch", Long.toString(expiryEpoch));
      aliasBinding.appendArguments(args);
      return args;
    }
  }
}
