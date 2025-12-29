package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `MintAsset` instruction. */
public final class MintAssetInstruction implements InstructionTemplate {

  private static final String ACTION = "MintAsset";

  private final String assetId;
  private final String quantity;
  private final Map<String, String> arguments;

  private MintAssetInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private MintAssetInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.assetId = builder.assetId;
    this.quantity = builder.quantity;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String assetId() {
    return assetId;
  }

  public String quantity() {
    return quantity;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.MINT;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static MintAssetInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setAssetId(require(arguments, "asset"))
            .setQuantity(require(arguments, "quantity"));
    return new MintAssetInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof MintAssetInstruction other)) {
      return false;
    }
    return Objects.equals(assetId, other.assetId) && Objects.equals(quantity, other.quantity);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetId, quantity);
  }

  public static final class Builder {
    private String assetId;
    private String quantity;

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

    public MintAssetInstruction build() {
      if (assetId == null || assetId.isBlank()) {
        throw new IllegalStateException("assetId must be set");
      }
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalStateException("quantity must be set");
      }
      return new MintAssetInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("asset", assetId);
      args.put("quantity", quantity);
      return args;
    }
  }
}
