package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `RegisterAssetDefinition` instruction. */
public final class RegisterAssetDefinitionInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterAssetDefinition";

  private final String assetDefinitionId;
  private final String displayName;
  private final String description;
  private final String logo;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterAssetDefinitionInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterAssetDefinitionInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.assetDefinitionId = builder.assetDefinitionId;
    this.displayName = builder.displayName;
    this.description = builder.description;
    this.logo = builder.logo;
    this.metadata = Map.copyOf(builder.metadata);
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String displayName() {
    return displayName;
  }

  public String description() {
    return description;
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

  public static RegisterAssetDefinitionInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setAssetDefinitionId(require(arguments, "definition"));
    if (arguments.containsKey("display_name")) {
      builder.setDisplayName(arguments.get("display_name"));
    }
    if (arguments.containsKey("description")) {
      builder.setDescription(arguments.get("description"));
    }
    if (arguments.containsKey("logo")) {
      builder.setLogo(arguments.get("logo"));
    }
    arguments.forEach((key, value) -> {
      if (key.startsWith("metadata.")) {
        builder.putMetadata(key.substring("metadata.".length()), value);
      }
    });
    return new RegisterAssetDefinitionInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterAssetDefinitionInstruction other)) {
      return false;
    }
    return Objects.equals(assetDefinitionId, other.assetDefinitionId)
        && Objects.equals(displayName, other.displayName)
        && Objects.equals(description, other.description)
        && Objects.equals(logo, other.logo)
        && Objects.equals(metadata, other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetDefinitionId, displayName, description, logo, metadata);
  }

  public static final class Builder {
    private String assetDefinitionId;
    private String displayName;
    private String description;
    private String logo;
    private final Map<String, String> metadata = new LinkedHashMap<>();

    public Builder setAssetDefinitionId(final String assetDefinitionId) {
      this.assetDefinitionId =
          Objects.requireNonNull(assetDefinitionId, "assetDefinitionId");
      return this;
    }

    public Builder setDisplayName(final String displayName) {
      this.displayName = displayName;
      return this;
    }

    public Builder setDescription(final String description) {
      this.description = description;
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

    public RegisterAssetDefinitionInstruction build() {
      if (assetDefinitionId == null || assetDefinitionId.isBlank()) {
        throw new IllegalStateException("assetDefinitionId must be set");
      }
      return new RegisterAssetDefinitionInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("definition", assetDefinitionId);
      if (displayName != null) {
        args.put("display_name", displayName);
      }
      if (description != null) {
        args.put("description", description);
      }
      if (logo != null) {
        args.put("logo", logo);
      }
      metadata.forEach((key, value) -> args.put("metadata." + key, value));
      return args;
    }
  }
}
