package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `TransferAssetDefinition` instruction. */
public final class TransferAssetDefinitionInstruction implements InstructionTemplate {

  private static final String ACTION = "TransferAssetDefinition";

  private final String sourceAccountId;
  private final String assetDefinitionId;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private TransferAssetDefinitionInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private TransferAssetDefinitionInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.sourceAccountId = builder.sourceAccountId;
    this.assetDefinitionId = builder.assetDefinitionId;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String sourceAccountId() {
    return sourceAccountId;
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String destinationAccountId() {
    return destinationAccountId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.TRANSFER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static TransferAssetDefinitionInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setSourceAccountId(require(arguments, "source"))
            .setAssetDefinitionId(require(arguments, "definition"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new TransferAssetDefinitionInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof TransferAssetDefinitionInstruction other)) {
      return false;
    }
    return Objects.equals(sourceAccountId, other.sourceAccountId)
        && Objects.equals(assetDefinitionId, other.assetDefinitionId)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(sourceAccountId, assetDefinitionId, destinationAccountId);
  }

  public static final class Builder {
    private String sourceAccountId;
    private String assetDefinitionId;
    private String destinationAccountId;

    private Builder() {}

    public Builder setSourceAccountId(final String sourceAccountId) {
      this.sourceAccountId = Objects.requireNonNull(sourceAccountId, "sourceAccountId");
      return this;
    }

    public Builder setAssetDefinitionId(final String assetDefinitionId) {
      this.assetDefinitionId = Objects.requireNonNull(assetDefinitionId, "assetDefinitionId");
      return this;
    }

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          Objects.requireNonNull(destinationAccountId, "destinationAccountId");
      return this;
    }

    public TransferAssetDefinitionInstruction build() {
      if (sourceAccountId == null || sourceAccountId.isBlank()) {
        throw new IllegalStateException("sourceAccountId must be set");
      }
      if (assetDefinitionId == null || assetDefinitionId.isBlank()) {
        throw new IllegalStateException("assetDefinitionId must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new TransferAssetDefinitionInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("source", sourceAccountId);
      args.put("definition", assetDefinitionId);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}
