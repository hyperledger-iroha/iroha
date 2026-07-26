package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/**
 * Typed builder for the {@code SetParameter} instruction.
 *
 * <p>The payload stores the Norito-encoded parameter as canonical JSON. Callers can surface the raw
 * JSON string via {@link #parameterJson()} and hand it to higher level decoders when richer
 * inspection is required.
 */
public final class SetParameterInstruction implements InstructionTemplate {

  private static final String ACTION = "SetParameter";

  private final String parameterJson;
  private final Map<String, String> arguments;

  private SetParameterInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private SetParameterInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.parameterJson = builder.parameterJson;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  /** Returns the canonical JSON representation of the parameter that will be set. */
  public String parameterJson() {
    return parameterJson;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.SET_PARAMETER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static SetParameterInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder().setParameterJson(require(arguments, "parameter"));
    return new SetParameterInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof SetParameterInstruction other)) {
      return false;
    }
    return Objects.equals(parameterJson, other.parameterJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(parameterJson);
  }

  public static final class Builder {
    private String parameterJson;

    private Builder() {}

    /**
     * Sets the canonical JSON representation of the parameter to apply.
     *
     * <p>The JSON must be a single-key object whose key matches one of the supported parameter
     * families (for example {@code Sumeragi}, {@code Block}, or {@code Custom}). The value mirrors
     * the Norito payload accepted by Iroha.
     */
    public Builder setParameterJson(final String parameterJson) {
      this.parameterJson = Objects.requireNonNull(parameterJson, "parameterJson");
      return this;
    }

    public SetParameterInstruction build() {
      if (parameterJson == null || parameterJson.isBlank()) {
        throw new IllegalStateException("parameterJson must be provided");
      }
      return new SetParameterInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("parameter", parameterJson);
      return args;
    }
  }

  /**
   * Convenience helper that builds an {@link InstructionBox} wrapping the supplied parameter JSON.
   */
  public InstructionBox toInstructionBox() {
    return InstructionBox.of(this);
  }
}

