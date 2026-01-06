package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code CompleteReplicationOrder} instruction. */
public final class CompleteReplicationOrderInstruction implements InstructionTemplate {

  private static final String ACTION = "CompleteReplicationOrder";

  private final String orderIdHex;
  private final long completionEpoch;
  private final Map<String, String> arguments;

  private CompleteReplicationOrderInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CompleteReplicationOrderInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.orderIdHex = builder.orderIdHex;
    this.completionEpoch = builder.completionEpoch;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String orderIdHex() {
    return orderIdHex;
  }

  public long completionEpoch() {
    return completionEpoch;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static CompleteReplicationOrderInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setOrderIdHex(require(arguments, "order_id_hex"))
            .setCompletionEpoch(requireLong(arguments, "completion_epoch"));
    return new CompleteReplicationOrderInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a number: " + value, ex);
    }
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CompleteReplicationOrderInstruction other)) {
      return false;
    }
    return completionEpoch == other.completionEpoch && Objects.equals(orderIdHex, other.orderIdHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(orderIdHex, completionEpoch);
  }

  public static final class Builder {
    private String orderIdHex;
    private Long completionEpoch;

    private Builder() {}

    public Builder setOrderIdHex(final String orderIdHex) {
      this.orderIdHex = Objects.requireNonNull(orderIdHex, "orderIdHex");
      return this;
    }

    public Builder setCompletionEpoch(final long completionEpoch) {
      if (completionEpoch < 0) {
        throw new IllegalArgumentException("completionEpoch must be non-negative");
      }
      this.completionEpoch = completionEpoch;
      return this;
    }

    public CompleteReplicationOrderInstruction build() {
      if (orderIdHex == null || orderIdHex.isBlank()) {
        throw new IllegalStateException("orderIdHex must be provided");
      }
      if (completionEpoch == null) {
        throw new IllegalStateException("completionEpoch must be provided");
      }
      return new CompleteReplicationOrderInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("order_id_hex", orderIdHex);
      args.put("completion_epoch", Long.toString(completionEpoch));
      return args;
    }
  }
}
