package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code RemoveAssetKeyValue} instructions targeting concrete asset balances. */
public final class RemoveAssetKeyValueInstruction implements InstructionTemplate {

  private static final String ACTION = "RemoveAssetKeyValue";

  private final String assetId;
  private final String key;
  private final Map<String, String> arguments;

  private RemoveAssetKeyValueInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RemoveAssetKeyValueInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.assetId = builder.assetId;
    this.key = builder.key;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the full asset identifier (definition + owner). */
  public String assetId() {
    return assetId;
  }

  /** Returns the metadata key slated for removal. */
  public String key() {
    return key;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REMOVE_KEY_VALUE;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RemoveAssetKeyValueInstruction fromArguments(final Map<String, String> arguments) {
    if (!ACTION.equals(require(arguments, "action"))) {
      throw new IllegalArgumentException("Unsupported action for RemoveAssetKeyValue");
    }
    final Builder builder =
        builder()
            .setAssetId(require(arguments, "asset"))
            .setKey(require(arguments, "key"));
    return new RemoveAssetKeyValueInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RemoveAssetKeyValueInstruction other)) {
      return false;
    }
    return Objects.equals(assetId, other.assetId) && Objects.equals(key, other.key);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetId, key);
  }

  public static final class Builder {
    private String assetId;
    private String key;

    private Builder() {}

    public Builder setAssetId(final String assetId) {
      this.assetId = Objects.requireNonNull(assetId, "assetId");
      return this;
    }

    public Builder setKey(final String key) {
      this.key = Objects.requireNonNull(key, "key");
      return this;
    }

    public RemoveAssetKeyValueInstruction build() {
      if (assetId == null || assetId.isBlank()) {
        throw new IllegalStateException("assetId must be provided");
      }
      if (key == null || key.isBlank()) {
        throw new IllegalStateException("key must be provided");
      }
      return new RemoveAssetKeyValueInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("asset", assetId);
      args.put("key", key);
      return args;
    }
  }
}

