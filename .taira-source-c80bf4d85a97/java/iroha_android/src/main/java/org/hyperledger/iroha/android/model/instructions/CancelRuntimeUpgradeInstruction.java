package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code CancelRuntimeUpgrade} instructions. */
public final class CancelRuntimeUpgradeInstruction implements InstructionTemplate {

  public static final String ACTION = "CancelRuntimeUpgrade";
  private static final String ID_HEX = "id_hex";

  private final String idHex;
  private final Map<String, String> arguments;

  private CancelRuntimeUpgradeInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CancelRuntimeUpgradeInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.idHex = builder.idHex;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  /** Returns the runtime upgrade id in canonical lowercase hex form. */
  public String idHex() {
    return idHex;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static CancelRuntimeUpgradeInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setIdHex(
                GovernanceInstructionUtils.requireHex(
                    require(arguments, ID_HEX), ID_HEX, 32));
    return new CancelRuntimeUpgradeInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof CancelRuntimeUpgradeInstruction other)) {
      return false;
    }
    return Objects.equals(idHex, other.idHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(idHex);
  }

  public static final class Builder {
    private String idHex;

    private Builder() {}

    public Builder setIdHex(final String idHex) {
      this.idHex = GovernanceInstructionUtils.requireHex(idHex, "idHex", 32);
      return this;
    }

    public CancelRuntimeUpgradeInstruction build() {
      if (idHex == null || idHex.isBlank()) {
        throw new IllegalStateException("idHex must be provided");
      }
      return new CancelRuntimeUpgradeInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put(ID_HEX, idHex);
      return args;
    }
  }
}
