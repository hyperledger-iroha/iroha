package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RegisterCapacityDeclaration} instruction (SoraFS capacity registry).
 */
public final class RegisterCapacityDeclarationInstruction implements InstructionTemplate {

public static final String ACTION = "RegisterCapacityDeclaration";

  private final String providerIdHex;
  private final String declarationBase64;
  private final long committedCapacityGib;
  private final long registeredEpoch;
  private final long validFromEpoch;
  private final long validUntilEpoch;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterCapacityDeclarationInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterCapacityDeclarationInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.providerIdHex = builder.providerIdHex;
    this.declarationBase64 = builder.declarationBase64;
    this.committedCapacityGib = builder.committedCapacityGib;
    this.registeredEpoch = builder.registeredEpoch;
    this.validFromEpoch = builder.validFromEpoch;
    this.validUntilEpoch = builder.validUntilEpoch;
    this.metadata =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(builder.metadata, "metadata")));
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public String declarationBase64() {
    return declarationBase64;
  }

  public long committedCapacityGib() {
    return committedCapacityGib;
  }

  public long registeredEpoch() {
    return registeredEpoch;
  }

  public long validFromEpoch() {
    return validFromEpoch;
  }

  public long validUntilEpoch() {
    return validUntilEpoch;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterCapacityDeclarationInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setProviderIdHex(require(arguments, "provider_id_hex"))
            .setDeclarationBase64(require(arguments, "declaration_b64"))
            .setCommittedCapacityGib(requireLong(arguments, "committed_capacity_gib"))
            .setRegisteredEpoch(requireLong(arguments, "registered_epoch"))
            .setValidFromEpoch(requireLong(arguments, "valid_from_epoch"))
            .setValidUntilEpoch(requireLong(arguments, "valid_until_epoch"));

    arguments.forEach(
        (key, value) -> {
          if (key.startsWith("metadata.")) {
            builder.putMetadata(key.substring("metadata.".length()), value);
          }
        });
    return new RegisterCapacityDeclarationInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String requireBase64(final String value, final String fieldName) {
    final String trimmed = Objects.requireNonNull(value, fieldName).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    final byte[] decoded;
    try {
      decoded = java.util.Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
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

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterCapacityDeclarationInstruction other)) {
      return false;
    }
    return committedCapacityGib == other.committedCapacityGib
        && registeredEpoch == other.registeredEpoch
        && validFromEpoch == other.validFromEpoch
        && validUntilEpoch == other.validUntilEpoch
        && Objects.equals(providerIdHex, other.providerIdHex)
        && Objects.equals(declarationBase64, other.declarationBase64)
        && metadata.equals(other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        providerIdHex,
        declarationBase64,
        committedCapacityGib,
        registeredEpoch,
        validFromEpoch,
        validUntilEpoch,
        metadata);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String providerIdHex;
    private String declarationBase64;
    private Long committedCapacityGib;
    private Long registeredEpoch;
    private Long validFromEpoch;
    private Long validUntilEpoch;
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

    public Builder setProviderId(final byte[] providerId) {
      Objects.requireNonNull(providerId, "providerId");
      final StringBuilder sb = new StringBuilder(providerId.length * 2);
      for (final byte b : providerId) {
        sb.append(String.format("%02x", b));
      }
      return setProviderIdHex(sb.toString());
    }

    public Builder setDeclarationBase64(final String declarationBase64) {
      this.declarationBase64 = requireBase64(declarationBase64, "declarationBase64");
      return this;
    }

    public Builder setDeclarationBytes(final byte[] declarationBytes) {
      Objects.requireNonNull(declarationBytes, "declarationBytes");
      final String encoded = java.util.Base64.getEncoder().encodeToString(declarationBytes);
      return setDeclarationBase64(encoded);
    }

    public Builder setCommittedCapacityGib(final long committedCapacityGib) {
      this.committedCapacityGib = ensureNonNegative(committedCapacityGib, "committedCapacityGib");
      return this;
    }

    public Builder setRegisteredEpoch(final long registeredEpoch) {
      this.registeredEpoch = ensureNonNegative(registeredEpoch, "registeredEpoch");
      return this;
    }

    public Builder setValidFromEpoch(final long validFromEpoch) {
      this.validFromEpoch = ensureNonNegative(validFromEpoch, "validFromEpoch");
      return this;
    }

    public Builder setValidUntilEpoch(final long validUntilEpoch) {
      this.validUntilEpoch = ensureNonNegative(validUntilEpoch, "validUntilEpoch");
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(
          Objects.requireNonNull(key, "metadata key"),
          Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> entries) {
      metadata.clear();
      if (entries != null) {
        entries.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterCapacityDeclarationInstruction build() {
      if (providerIdHex == null || providerIdHex.isBlank()) {
        throw new IllegalStateException("providerIdHex must be provided");
      }
      if (declarationBase64 == null || declarationBase64.isBlank()) {
        throw new IllegalStateException("declarationBase64 must be provided");
      }
      if (committedCapacityGib == null) {
        throw new IllegalStateException("committedCapacityGib must be provided");
      }
      if (registeredEpoch == null) {
        throw new IllegalStateException("registeredEpoch must be provided");
      }
      if (validFromEpoch == null) {
        throw new IllegalStateException("validFromEpoch must be provided");
      }
      if (validUntilEpoch == null) {
        throw new IllegalStateException("validUntilEpoch must be provided");
      }
      if (validUntilEpoch < validFromEpoch) {
        throw new IllegalStateException("validUntilEpoch must be >= validFromEpoch");
      }
      return new RegisterCapacityDeclarationInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("provider_id_hex", providerIdHex);
      args.put("declaration_b64", declarationBase64);
      args.put("committed_capacity_gib", Long.toString(committedCapacityGib));
      args.put("registered_epoch", Long.toString(registeredEpoch));
      args.put("valid_from_epoch", Long.toString(validFromEpoch));
      args.put("valid_until_epoch", Long.toString(validUntilEpoch));
      metadata.forEach((key, value) -> args.put("metadata." + key, value));
      return args;
    }

    private static long ensureNonNegative(final long value, final String label) {
      if (value < 0) {
        throw new IllegalArgumentException(label + " must be non-negative");
      }
      return value;
    }
  }
}
