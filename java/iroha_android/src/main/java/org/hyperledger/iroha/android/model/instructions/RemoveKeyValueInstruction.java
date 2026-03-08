package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code RemoveKeyValue} instructions spanning all supported entity types. */
public final class RemoveKeyValueInstruction implements InstructionTemplate {

  private final Target target;
  private final String objectId;
  private final String key;
  private final Map<String, String> arguments;

  private RemoveKeyValueInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RemoveKeyValueInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.target = builder.target;
    this.objectId = builder.objectId;
    this.key = builder.key;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the instruction target describing which entity should be updated. */
  public Target target() {
    return target;
  }

  /** Returns the identifier of the target entity (domain id, account id, trigger id, etc.). */
  public String objectId() {
    return objectId;
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

  public static RemoveKeyValueInstruction fromArguments(final Map<String, String> arguments) {
    final String action = require(arguments, "action");
    final Target target = Target.fromAction(action);
    final Builder builder =
        builder()
            .setTarget(target, require(arguments, target.argumentKey))
            .setKey(require(arguments, "key"));
    return new RemoveKeyValueInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RemoveKeyValueInstruction other)) {
      return false;
    }
    return target == other.target
        && Objects.equals(objectId, other.objectId)
        && Objects.equals(key, other.key);
  }

  @Override
  public int hashCode() {
    return Objects.hash(target, objectId, key);
  }

  public enum Target {
    DOMAIN("RemoveDomainKeyValue", "domain"),
    ACCOUNT("RemoveAccountKeyValue", "account"),
    ASSET_DEFINITION("RemoveAssetDefinitionKeyValue", "definition"),
    NFT("RemoveNftKeyValue", "nft"),
    TRIGGER("RemoveTriggerKeyValue", "trigger");

    private final String action;
    private final String argumentKey;

    Target(final String action, final String argumentKey) {
      this.action = action;
      this.argumentKey = argumentKey;
    }

    String action() {
      return action;
    }

    String argumentKey() {
      return argumentKey;
    }

    static Target fromAction(final String action) {
      for (final Target target : values()) {
        if (target.action.equals(action)) {
          return target;
        }
      }
      throw new IllegalArgumentException("Unknown RemoveKeyValue action: " + action);
    }
  }

  public static final class Builder {
    private Target target;
    private String objectId;
    private String key;

    private Builder() {}

    public Builder setDomainId(final String domainId) {
      return setTarget(Target.DOMAIN, domainId);
    }

    public Builder setAccountId(final String accountId) {
      return setTarget(Target.ACCOUNT, accountId);
    }

    public Builder setAssetDefinitionId(final String assetDefinitionId) {
      return setTarget(Target.ASSET_DEFINITION, assetDefinitionId);
    }

    public Builder setNftId(final String nftId) {
      return setTarget(Target.NFT, nftId);
    }

    public Builder setTriggerId(final String triggerId) {
      return setTarget(Target.TRIGGER, triggerId);
    }

    Builder setTarget(final Target target, final String id) {
      Objects.requireNonNull(target, "target");
      final String normalizedId = normalizeTargetId(target, id);
      if (this.target != null && this.target != target) {
        throw new IllegalStateException("Instruction target already set to " + this.target);
      }
      this.target = target;
      this.objectId = normalizedId;
      return this;
    }

    private static String normalizeTargetId(final Target target, final String id) {
      final String raw = Objects.requireNonNull(id, "id");
      if (target == Target.ACCOUNT) {
        return AccountIdLiteral.extractIh58Address(raw);
      }
      return raw;
    }

    public Builder setKey(final String key) {
      this.key = Objects.requireNonNull(key, "key");
      return this;
    }

    public RemoveKeyValueInstruction build() {
      if (target == null) {
        throw new IllegalStateException("target must be set");
      }
      if (objectId == null || objectId.isBlank()) {
        throw new IllegalStateException("target identifier must be provided");
      }
      if (key == null || key.isBlank()) {
        throw new IllegalStateException("key must be provided");
      }
      return new RemoveKeyValueInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", target.action());
      args.put(target.argumentKey(), objectId);
      args.put("key", key);
      return args;
    }
  }
}
