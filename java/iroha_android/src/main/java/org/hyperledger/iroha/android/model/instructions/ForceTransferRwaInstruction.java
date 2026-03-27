package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code ForceTransferRwa} instruction. */
public final class ForceTransferRwaInstruction implements InstructionTemplate {

  private static final String ACTION = "ForceTransferRwa";

  private final String rwaId;
  private final String quantity;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private ForceTransferRwaInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ForceTransferRwaInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.rwaId = builder.rwaId;
    this.quantity = builder.quantity;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String rwaId() {
    return rwaId;
  }

  public String quantity() {
    return quantity;
  }

  public String destinationAccountId() {
    return destinationAccountId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ForceTransferRwaInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setRwaId(require(arguments, "rwa"))
            .setQuantity(require(arguments, "quantity"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new ForceTransferRwaInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof ForceTransferRwaInstruction other)) {
      return false;
    }
    return Objects.equals(rwaId, other.rwaId)
        && Objects.equals(quantity, other.quantity)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(rwaId, quantity, destinationAccountId);
  }

  public static final class Builder {
    private String rwaId;
    private String quantity;
    private String destinationAccountId;

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

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              destinationAccountId, "destinationAccountId");
      return this;
    }

    public ForceTransferRwaInstruction build() {
      if (rwaId == null || rwaId.isBlank()) {
        throw new IllegalStateException("rwaId must be set");
      }
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalStateException("quantity must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new ForceTransferRwaInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("rwa", rwaId);
      args.put("quantity", quantity);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}
