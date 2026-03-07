package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AssetIdLiteral;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code SetAssetKeyValue} instructions targeting concrete asset balances. */
public final class SetAssetKeyValueInstruction implements InstructionTemplate {

  private static final String ACTION = "SetAssetKeyValue";

  private final String assetId;
  private final String key;
  private final String value;
  private final Map<String, String> arguments;

  private SetAssetKeyValueInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private SetAssetKeyValueInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.assetId = builder.assetId;
    this.key = builder.key;
    this.value = builder.value;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the full asset identifier (definition + owner). */
  public String assetId() {
    return assetId;
  }

  /** Returns the metadata key being set. */
  public String key() {
    return key;
  }

  /** Returns the metadata value encoded as a string. */
  public String value() {
    return value;
  }

  @Override
  public InstructionKind kind() {
    // Asset metadata edits share the SetKeyValue discriminant family.
    return InstructionKind.SET_KEY_VALUE;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static SetAssetKeyValueInstruction fromArguments(final Map<String, String> arguments) {
    if (!ACTION.equals(require(arguments, "action"))) {
      throw new IllegalArgumentException("Unsupported action for SetAssetKeyValue");
    }
    final Builder builder =
        builder()
            .setAssetId(require(arguments, "asset"))
            .setKey(require(arguments, "key"))
            .setValue(require(arguments, "value"));
    return new SetAssetKeyValueInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof SetAssetKeyValueInstruction other)) {
      return false;
    }
    return Objects.equals(assetId, other.assetId)
        && Objects.equals(key, other.key)
        && Objects.equals(value, other.value);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetId, key, value);
  }

  public static final class Builder {
    private String assetId;
    private String key;
    private String value;

    private Builder() {}

    public Builder setAssetId(final String assetId) {
      this.assetId = AssetIdLiteral.normalizeEncoded(assetId);
      return this;
    }

    public Builder setKey(final String key) {
      this.key = Objects.requireNonNull(key, "key");
      return this;
    }

    public Builder setValue(final String value) {
      this.value = Objects.requireNonNull(value, "value");
      return this;
    }

    public SetAssetKeyValueInstruction build() {
      if (assetId == null || assetId.isBlank()) {
        throw new IllegalStateException("assetId must be provided");
      }
      if (key == null || key.isBlank()) {
        throw new IllegalStateException("key must be provided");
      }
      if (value == null) {
        throw new IllegalStateException("value must be provided");
      }
      return new SetAssetKeyValueInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("asset", assetId);
      args.put("key", key);
      args.put("value", value);
      return args;
    }
  }
}
