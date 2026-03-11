package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed builder for {@code RegisterRole} instructions. */
public final class RegisterRoleInstruction implements InstructionTemplate {

  private final String roleId;
  private final String ownerAccountId;
  private final List<String> permissions;
  private final Map<String, String> arguments;

  private RegisterRoleInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterRoleInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.roleId = builder.roleId;
    this.ownerAccountId = builder.ownerAccountId;
    this.permissions = Collections.unmodifiableList(new ArrayList<>(builder.permissions));
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(argumentOrder));
  }

  /** Returns the role identifier. */
  public String roleId() {
    return roleId;
  }

  /** Returns the initial owner account id. */
  public String ownerAccountId() {
    return ownerAccountId;
  }

  /** Returns the list of permission token names associated with the role. */
  public List<String> permissions() {
    return permissions;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterRoleInstruction fromArguments(final Map<String, String> arguments) {
    final String permissionsValue = arguments.get("permissions");
    final Builder builder =
        builder()
            .setRoleId(require(arguments, "role"))
            .setOwnerAccountId(require(arguments, "owner"));
    if (permissionsValue != null && !permissionsValue.isBlank()) {
      for (final String permission : permissionsValue.split(",")) {
        final String trimmed = permission.trim();
        if (!trimmed.isEmpty()) {
          builder.addPermission(trimmed);
        }
      }
    }
    return new RegisterRoleInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterRoleInstruction other)) {
      return false;
    }
    return roleId.equals(other.roleId)
        && ownerAccountId.equals(other.ownerAccountId)
        && permissions.equals(other.permissions);
  }

  @Override
  public int hashCode() {
    return Objects.hash(roleId, ownerAccountId, permissions);
  }

  public static final class Builder {
    private String roleId;
    private String ownerAccountId;
    private final List<String> permissions = new ArrayList<>();

    private Builder() {}

    public Builder setRoleId(final String roleId) {
      if (roleId == null || roleId.isBlank()) {
        throw new IllegalArgumentException("roleId must not be blank");
      }
      this.roleId = roleId;
      return this;
    }

    public Builder setOwnerAccountId(final String ownerAccountId) {
      this.ownerAccountId = AccountIdLiteral.extractI105Address(ownerAccountId);
      return this;
    }

    public Builder addPermission(final String permission) {
      if (permission == null || permission.isBlank()) {
        throw new IllegalArgumentException("permission must not be blank");
      }
      this.permissions.add(permission);
      return this;
    }

    public Builder setPermissions(final List<String> permissions) {
      Objects.requireNonNull(permissions, "permissions");
      this.permissions.clear();
      for (final String permission : permissions) {
        addPermission(permission);
      }
      return this;
    }

    public RegisterRoleInstruction build() {
      if (roleId == null) {
        throw new IllegalStateException("roleId must be provided");
      }
      if (ownerAccountId == null) {
        throw new IllegalStateException("ownerAccountId must be provided");
      }
      return new RegisterRoleInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "RegisterRole");
      args.put("role", roleId);
      args.put("owner", ownerAccountId);
      if (!permissions.isEmpty()) {
        args.put("permissions", String.join(",", permissions));
      }
      return args;
    }
  }
}
