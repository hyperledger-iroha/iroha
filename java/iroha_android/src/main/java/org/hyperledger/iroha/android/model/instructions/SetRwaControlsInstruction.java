package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code SetRwaControls} instruction.
 *
 * <p>The payload stores the canonical JSON representation of {@code RwaControlPolicy}.
 */
public final class SetRwaControlsInstruction implements InstructionTemplate {

  private static final String ACTION = "SetRwaControls";

  private final String rwaId;
  private final String controlsJson;
  private final Map<String, String> arguments;

  private SetRwaControlsInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private SetRwaControlsInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.rwaId = builder.rwaId;
    this.controlsJson = builder.controlsJson;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  /** Returns the RWA identifier being updated. */
  public String rwaId() {
    return rwaId;
  }

  /** Returns the canonical JSON representation of the {@code RwaControlPolicy}. */
  public String controlsJson() {
    return controlsJson;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static SetRwaControlsInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setRwaId(require(arguments, "rwa"))
            .setControlsJson(require(arguments, "controls"));
    return new SetRwaControlsInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof SetRwaControlsInstruction other)) {
      return false;
    }
    return Objects.equals(rwaId, other.rwaId)
        && Objects.equals(controlsJson, other.controlsJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(rwaId, controlsJson);
  }

  public static final class Builder {
    private String rwaId;
    private String controlsJson;

    private Builder() {}

    public Builder setRwaId(final String rwaId) {
      this.rwaId = Objects.requireNonNull(rwaId, "rwaId");
      return this;
    }

    public Builder setControlsJson(final String controlsJson) {
      this.controlsJson = Objects.requireNonNull(controlsJson, "controlsJson");
      return this;
    }

    public SetRwaControlsInstruction build() {
      if (rwaId == null || rwaId.isBlank()) {
        throw new IllegalStateException("rwaId must be provided");
      }
      if (controlsJson == null || controlsJson.isBlank()) {
        throw new IllegalStateException("controlsJson must be provided");
      }
      return new SetRwaControlsInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("rwa", rwaId);
      args.put("controls", controlsJson);
      return args;
    }
  }
}
