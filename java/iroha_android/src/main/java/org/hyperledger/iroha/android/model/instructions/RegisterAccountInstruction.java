package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `RegisterAccount` instruction. */
public final class RegisterAccountInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterAccount";

  private final String accountId;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;
  private final String uaid;

  private RegisterAccountInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterAccountInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.accountId = builder.accountId;
    this.metadata = Map.copyOf(builder.metadata);
    this.uaid = builder.uaid;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String accountId() {
    return accountId;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public String uaid() {
    return uaid;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterAccountInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setAccountId(require(arguments, "account"));
    if (arguments.containsKey("uaid")) {
      builder.setUaid(arguments.get("uaid"));
    }
    arguments.forEach((key, value) -> {
      if (key.startsWith("metadata.")) {
        builder.putMetadata(key.substring("metadata.".length()), value);
      }
    });
    return new RegisterAccountInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterAccountInstruction other)) {
      return false;
    }
    return Objects.equals(accountId, other.accountId)
        && Objects.equals(metadata, other.metadata)
        && Objects.equals(uaid, other.uaid);
  }

  @Override
  public int hashCode() {
    return Objects.hash(accountId, metadata, uaid);
  }

  public static final class Builder {
    private String accountId;
    private final Map<String, String> metadata = new LinkedHashMap<>();
    private String uaid;

    private Builder() {}

    public Builder setAccountId(final String accountId) {
      this.accountId =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              accountId, "accountId");
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(Objects.requireNonNull(key, "key"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setUaid(final String uaid) {
      this.uaid = Objects.requireNonNull(uaid, "uaid");
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterAccountInstruction build() {
      if (accountId == null || accountId.isBlank()) {
        throw new IllegalStateException("accountId must be set");
      }
      return new RegisterAccountInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("account", accountId);
       if (uaid != null && !uaid.isBlank()) {
         args.put("uaid", uaid);
       }
      metadata.forEach((key, value) -> args.put("metadata." + key, value));
      return args;
    }
  }
}
