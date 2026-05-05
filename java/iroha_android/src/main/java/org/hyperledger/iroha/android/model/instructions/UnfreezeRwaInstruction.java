package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code UnfreezeRwa} instruction. */
public final class UnfreezeRwaInstruction implements InstructionTemplate {

  private static final String ACTION = "UnfreezeRwa";

  private final String rwaId;
  private final Map<String, String> arguments;

  private UnfreezeRwaInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private UnfreezeRwaInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.rwaId = builder.rwaId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String rwaId() {
    return rwaId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static UnfreezeRwaInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setRwaId(require(arguments, "rwa"));
    return new UnfreezeRwaInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof UnfreezeRwaInstruction other)) {
      return false;
    }
    return Objects.equals(rwaId, other.rwaId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(rwaId);
  }

  public static final class Builder {
    private String rwaId;

    private Builder() {}

    public Builder setRwaId(final String rwaId) {
      this.rwaId = Objects.requireNonNull(rwaId, "rwaId");
      return this;
    }

    public UnfreezeRwaInstruction build() {
      if (rwaId == null || rwaId.isBlank()) {
        throw new IllegalStateException("rwaId must be provided");
      }
      return new UnfreezeRwaInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("rwa", rwaId);
      return args;
    }
  }
}
