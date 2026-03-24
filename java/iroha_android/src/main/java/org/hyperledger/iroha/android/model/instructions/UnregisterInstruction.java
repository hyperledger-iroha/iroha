package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/**
 * Typed builder for {@code Unregister} instructions spanning the supported entity families.
 */
public final class UnregisterInstruction implements InstructionTemplate {

  private final Target target;
  private final String objectId;
  private final Map<String, String> arguments;

  private UnregisterInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private UnregisterInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.target = builder.target;
    this.objectId = builder.objectId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the instruction target describing which entity should be unregistered. */
  public Target target() {
    return target;
  }

  /** Returns the identifier of the entity slated for unregistration. */
  public String objectId() {
    return objectId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.UNREGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static UnregisterInstruction fromArguments(final Map<String, String> arguments) {
    final String action = require(arguments, "action");
    final Target target = Target.fromAction(action);
    final Builder builder = builder().setTarget(target, require(arguments, target.argumentKey()));
    return new UnregisterInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof UnregisterInstruction other)) {
      return false;
    }
    return target == other.target && Objects.equals(objectId, other.objectId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(target, objectId);
  }

  /** Supported unregistration targets. */
  public enum Target {
    PEER("UnregisterPeer", "peer"),
    DOMAIN("UnregisterDomain", "domain"),
    ACCOUNT("UnregisterAccount", "account"),
    ASSET_DEFINITION("UnregisterAssetDefinition", "definition"),
    NFT("UnregisterNft", "nft"),
    ROLE("UnregisterRole", "role"),
    TRIGGER("UnregisterTrigger", "trigger");

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
      throw new IllegalArgumentException("Unknown Unregister action: " + action);
    }
  }

  public static final class Builder {
    private Target target;
    private String objectId;

    private Builder() {}

    public Builder setPeerId(final String peerId) {
      return setTarget(Target.PEER, peerId);
    }

    public Builder setDomainId(final String domainId) {
      return setTarget(Target.DOMAIN, domainId);
    }

    public Builder setAccountId(final String accountId) {
      return setTarget(
          Target.ACCOUNT,
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              accountId, "accountId"));
    }

    public Builder setAssetDefinitionId(final String assetDefinitionId) {
      return setTarget(Target.ASSET_DEFINITION, assetDefinitionId);
    }

    public Builder setNftId(final String nftId) {
      return setTarget(Target.NFT, nftId);
    }

    public Builder setRoleId(final String roleId) {
      return setTarget(Target.ROLE, roleId);
    }

    public Builder setTriggerId(final String triggerId) {
      return setTarget(Target.TRIGGER, triggerId);
    }

    Builder setTarget(final Target target, final String id) {
      Objects.requireNonNull(target, "target");
      Objects.requireNonNull(id, "identifier");
      if (this.target != null && this.target != target) {
        throw new IllegalStateException("Instruction target already set to " + this.target);
      }
      this.target = target;
      this.objectId = id;
      return this;
    }

    public UnregisterInstruction build() {
      if (target == null) {
        throw new IllegalStateException("target must be set");
      }
      if (objectId == null || objectId.isBlank()) {
        throw new IllegalStateException("target identifier must be provided");
      }
      return new UnregisterInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", target.action());
      args.put(target.argumentKey(), objectId);
      return args;
    }
  }
}
