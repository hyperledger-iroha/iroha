package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the `GrantPermission` instruction targeting an account. */
public final class GrantPermissionInstruction implements InstructionTemplate {

  private static final String ACTION = "GrantPermission";

  private final String destinationId;
  private final String permissionName;
  private final String permissionPayload;
  private final Map<String, String> arguments;

  private GrantPermissionInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private GrantPermissionInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.destinationId = builder.destinationId;
    this.permissionName = builder.permissionName;
    this.permissionPayload = builder.permissionPayload;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String destinationId() {
    return destinationId;
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

  public static GrantPermissionInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setDestinationId(require(arguments, "destination"))
            .setPermissionName(require(arguments, "permission"));
    if (arguments.containsKey("permission_payload")) {
      builder.setPermissionPayload(arguments.get("permission_payload"));
    }
    return new GrantPermissionInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof GrantPermissionInstruction other)) {
      return false;
    }
    return Objects.equals(destinationId, other.destinationId)
        && Objects.equals(permissionName, other.permissionName)
        && Objects.equals(permissionPayload, other.permissionPayload);
  }

  @Override
  public int hashCode() {
    return Objects.hash(destinationId, permissionName, permissionPayload);
  }

  public static final class Builder {
    private String destinationId;
    private String permissionName;
    private String permissionPayload;

    private Builder() {}

    public Builder setDestinationId(final String destinationId) {
      this.destinationId = Objects.requireNonNull(destinationId, "destinationId");
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

    public GrantPermissionInstruction build() {
      if (destinationId == null || destinationId.isBlank()) {
        throw new IllegalStateException("destinationId must be set");
      }
      if (permissionName == null || permissionName.isBlank()) {
        throw new IllegalStateException("permissionName must be set");
      }
      return new GrantPermissionInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("destination", destinationId);
      args.put("permission", permissionName);
      if (permissionPayload != null && !permissionPayload.isBlank()) {
        args.put("permission_payload", permissionPayload);
      }
      return args;
    }
  }
}
