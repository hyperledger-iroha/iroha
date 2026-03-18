package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for granting a permission to a role. */
public final class GrantRolePermissionInstruction implements InstructionTemplate {

  private static final String ACTION = "GrantRolePermission";

  private final String destinationRoleId;
  private final String permissionName;
  private final String permissionPayload;
  private final Map<String, String> arguments;

  private GrantRolePermissionInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private GrantRolePermissionInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.destinationRoleId = builder.destinationRoleId;
    this.permissionName = builder.permissionName;
    this.permissionPayload = builder.permissionPayload;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String destinationRoleId() {
    return destinationRoleId;
  }

  public String permissionName() {
    return permissionName;
  }

  public String permissionPayload() {
    return permissionPayload;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.GRANT;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static GrantRolePermissionInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDestinationRoleId(require(arguments, "destination"))
            .setPermissionName(require(arguments, "permission"));
    if (arguments.containsKey("permission_payload")) {
      builder.setPermissionPayload(arguments.get("permission_payload"));
    }
    return new GrantRolePermissionInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof GrantRolePermissionInstruction other)) {
      return false;
    }
    return Objects.equals(destinationRoleId, other.destinationRoleId)
        && Objects.equals(permissionName, other.permissionName)
        && Objects.equals(permissionPayload, other.permissionPayload);
  }

  @Override
  public int hashCode() {
    return Objects.hash(destinationRoleId, permissionName, permissionPayload);
  }

  public static final class Builder {
    private String destinationRoleId;
    private String permissionName;
    private String permissionPayload;

    private Builder() {}

    public Builder setDestinationRoleId(final String destinationRoleId) {
      this.destinationRoleId =
          Objects.requireNonNull(destinationRoleId, "destinationRoleId");
      return this;
    }

    public Builder setPermissionName(final String permissionName) {
      this.permissionName = Objects.requireNonNull(permissionName, "permissionName");
      return this;
    }

    public Builder setPermissionPayload(final String permissionPayload) {
      if (permissionPayload != null && permissionPayload.isBlank()) {
        this.permissionPayload = null;
      } else {
        this.permissionPayload = permissionPayload;
      }
      return this;
    }

    public GrantRolePermissionInstruction build() {
      if (destinationRoleId == null || destinationRoleId.isBlank()) {
        throw new IllegalStateException("destinationRoleId must be set");
      }
      if (permissionName == null || permissionName.isBlank()) {
        throw new IllegalStateException("permissionName must be set");
      }
      return new GrantRolePermissionInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("destination", destinationRoleId);
      args.put("permission", permissionName);
      if (permissionPayload != null && !permissionPayload.isBlank()) {
        args.put("permission_payload", permissionPayload);
      }
      return args;
    }
  }
}
