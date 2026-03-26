package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code TransferRwa} instruction. */
public final class TransferRwaInstruction implements InstructionTemplate {

  private static final String ACTION = "TransferRwa";

  private final String sourceAccountId;
  private final String rwaId;
  private final String quantity;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private TransferRwaInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private TransferRwaInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.sourceAccountId = builder.sourceAccountId;
    this.rwaId = builder.rwaId;
    this.quantity = builder.quantity;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String sourceAccountId() {
    return sourceAccountId;
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

  public static TransferRwaInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setSourceAccountId(require(arguments, "source"))
            .setRwaId(require(arguments, "rwa"))
            .setQuantity(require(arguments, "quantity"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new TransferRwaInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof TransferRwaInstruction other)) {
      return false;
    }
    return Objects.equals(sourceAccountId, other.sourceAccountId)
        && Objects.equals(rwaId, other.rwaId)
        && Objects.equals(quantity, other.quantity)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(sourceAccountId, rwaId, quantity, destinationAccountId);
  }

  public static final class Builder {
    private String sourceAccountId;
    private String rwaId;
    private String quantity;
    private String destinationAccountId;

    private Builder() {}

    public Builder setSourceAccountId(final String sourceAccountId) {
      this.sourceAccountId =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              sourceAccountId, "sourceAccountId");
      return this;
    }

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

    public TransferRwaInstruction build() {
      if (sourceAccountId == null || sourceAccountId.isBlank()) {
        throw new IllegalStateException("sourceAccountId must be set");
      }
      if (rwaId == null || rwaId.isBlank()) {
        throw new IllegalStateException("rwaId must be set");
      }
      if (quantity == null || quantity.isBlank()) {
        throw new IllegalStateException("quantity must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new TransferRwaInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("source", sourceAccountId);
      args.put("rwa", rwaId);
      args.put("quantity", quantity);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}
