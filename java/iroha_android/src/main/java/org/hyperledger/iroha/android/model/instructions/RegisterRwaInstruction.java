package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code RegisterRwa} instruction.
 *
 * <p>The payload stores the canonical JSON representation of {@code NewRwa}.
 */
public final class RegisterRwaInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterRwa";

  private final String rwaJson;
  private final Map<String, String> arguments;

  private RegisterRwaInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterRwaInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.rwaJson = builder.rwaJson;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the canonical JSON representation of the {@code NewRwa} payload. */
  public String rwaJson() {
    return rwaJson;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterRwaInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setRwaJson(require(arguments, "rwa"));
    return new RegisterRwaInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RegisterRwaInstruction other)) {
      return false;
    }
    return Objects.equals(rwaJson, other.rwaJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(rwaJson);
  }

  public static final class Builder {
    private String rwaJson;

    private Builder() {}

    public Builder setRwaJson(final String rwaJson) {
      this.rwaJson = Objects.requireNonNull(rwaJson, "rwaJson");
      return this;
    }

    public RegisterRwaInstruction build() {
      if (rwaJson == null || rwaJson.isBlank()) {
        throw new IllegalStateException("rwaJson must be provided");
      }
      return new RegisterRwaInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("rwa", rwaJson);
      return args;
    }
  }
}
