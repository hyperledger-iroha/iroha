package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code ReleaseRwa} instruction. */
public final class ReleaseRwaInstruction implements InstructionTemplate {

  private static final String ACTION = "ReleaseRwa";

  private final String rwaId;
  private final String quantity;
  private final Map<String, String> arguments;

  private ReleaseRwaInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ReleaseRwaInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.rwaId = builder.rwaId;
    this.quantity = builder.quantity;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String rwaId() {
    return rwaId;
  }

  public String quantity() {
    return quantity;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ReleaseRwaInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setRwaId(require(arguments, "rwa"))
            .setQuantity(require(arguments, "quantity"));
    return new ReleaseRwaInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof ReleaseRwaInstruction other)) {
      return false;
    }
    return Objects.equals(rwaId, other.rwaId) && Objects.equals(quantity, other.quantity);
  }

  @Override
  public int hashCode() {
    return Objects.hash(rwaId, quantity);
  }

  public static final class Builder {
    private String rwaId;
    private String quantity;

    private Builder() {}

    public Builder setRwaId(final String rwaId) {
      this.rwaId = Objects.requireNonNull(rwaId, "rwaId");
      return this;
    }

    public Builder setQuantity(final String quantity) {
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalArgumentException("quantity must not be blank");
      }
      this.quantity = quantity;
      return this;
    }

    public Builder setQuantity(final Number quantity) {
      Objects.requireNonNull(quantity, "quantity");
      return setQuantity(quantity.toString());
    }

    public ReleaseRwaInstruction build() {
      if (rwaId == null || rwaId.isBlank()) {
        throw new IllegalStateException("rwaId must be set");
      }
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalStateException("quantity must be set");
      }
      return new ReleaseRwaInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("rwa", rwaId);
      args.put("quantity", quantity);
      return args;
    }
  }
}
