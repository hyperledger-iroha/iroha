package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed builder for the `GrantRole` instruction. */
public final class GrantRoleInstruction implements InstructionTemplate {

  private static final String ACTION = "GrantRole";

  private final String destinationAccountId;
  private final String roleId;
  private final Map<String, String> arguments;

  private GrantRoleInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private GrantRoleInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.destinationAccountId = builder.destinationAccountId;
    this.roleId = builder.roleId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String destinationAccountId() {
    return destinationAccountId;
  }

  public String roleId() {
    return roleId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.GRANT;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static GrantRoleInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDestinationAccountId(require(arguments, "destination"))
            .setRoleId(require(arguments, "role"));
    return new GrantRoleInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof GrantRoleInstruction other)) {
      return false;
    }
    return Objects.equals(destinationAccountId, other.destinationAccountId)
        && Objects.equals(roleId, other.roleId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(destinationAccountId, roleId);
  }

  public static final class Builder {
    private String destinationAccountId;
    private String roleId;

    private Builder() {}

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          AccountIdLiteral.extractI105Address(
              Objects.requireNonNull(destinationAccountId, "destinationAccountId"));
      return this;
    }

    public Builder setRoleId(final String roleId) {
      this.roleId = Objects.requireNonNull(roleId, "roleId");
      return this;
    }

    public GrantRoleInstruction build() {
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      if (roleId == null || roleId.isBlank()) {
        throw new IllegalStateException("roleId must be set");
      }
      return new GrantRoleInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("destination", destinationAccountId);
      args.put("role", roleId);
      return args;
    }
  }
}
