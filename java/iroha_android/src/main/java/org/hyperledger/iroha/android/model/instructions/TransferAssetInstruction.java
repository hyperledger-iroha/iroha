package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `TransferAsset` instruction. */
public final class TransferAssetInstruction implements InstructionTemplate {

  private static final String ACTION = "TransferAsset";

  private final String assetId;
  private final String quantity;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private TransferAssetInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private TransferAssetInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.assetId = builder.assetId;
    this.quantity = builder.quantity;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String assetId() {
    return assetId;
  }

  public String quantity() {
    return quantity;
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

  public static TransferAssetInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setAssetId(require(arguments, "asset"))
            .setQuantity(arguments.get("quantity"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new TransferAssetInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof TransferAssetInstruction other)) {
      return false;
    }
    return Objects.equals(assetId, other.assetId)
        && Objects.equals(quantity, other.quantity)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetId, quantity, destinationAccountId);
  }

  public static final class Builder {
    private String assetId;
    private String quantity;
    private String destinationAccountId;

    private Builder() {}

    public Builder setAssetId(final String assetId) {
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      return this;
    }

    public Builder setQuantity(final String quantity) {
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalArgumentException("quantity must not be blank");
      }
      this.quantity = quantity;
      return this;
    }

    public Builder setQuantity(final Number quantity) {
      Objects.requireNonNull(quantity, "quantity");
      return setQuantity(quantity.toString());
    }

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              destinationAccountId, "destinationAccountId");
      return this;
    }

    public TransferAssetInstruction build() {
      if (assetId == null || assetId.isBlank()) {
        throw new IllegalStateException("assetId must be set");
      }
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalStateException("quantity must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new TransferAssetInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("asset", assetId);
      args.put("quantity", quantity);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}
