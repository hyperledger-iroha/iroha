package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;

/** Typed builder for the `RegisterDomain` instruction. */
public final class RegisterDomainInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterDomain";

  private final String domainName;
  private final String logo;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterDomainInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterDomainInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.domainName = builder.domainName;
    this.logo = builder.logo;
    this.metadata = Map.copyOf(builder.metadata);
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String domainName() {
    return domainName;
  }

  public String logo() {
    return logo;
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

  public static RegisterDomainInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setDomainName(require(arguments, "domain"));
    if (arguments.containsKey("logo")) {
      builder.setLogo(arguments.get("logo"));
    }
    arguments.forEach((key, value) -> {
      if (key.startsWith("metadata.")) {
        builder.putMetadata(key.substring("metadata.".length()), value);
      }
    });
    return new RegisterDomainInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterDomainInstruction other)) {
      return false;
    }
    return Objects.equals(domainName, other.domainName)
        && Objects.equals(logo, other.logo)
        && Objects.equals(metadata, other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(domainName, logo, metadata);
  }

  public static final class Builder {
    private String domainName;
    private String logo;
    private final Map<String, String> metadata = new LinkedHashMap<>();

    private Builder() {}

    public Builder setDomainName(final String domainName) {
      this.domainName = Objects.requireNonNull(domainName, "domainName");
      return this;
    }

    public Builder setLogo(final String logo) {
      this.logo = logo;
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(Objects.requireNonNull(key, "key"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterDomainInstruction build() {
      if (domainName == null || domainName.isBlank()) {
        throw new IllegalStateException("domainName must be set");
      }
      return new RegisterDomainInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("domain", domainName);
      if (logo != null) {
        args.put("logo", logo);
      }
      metadata.forEach((key, value) -> args.put("metadata." + key, value));
      return args;
    }
  }
}
