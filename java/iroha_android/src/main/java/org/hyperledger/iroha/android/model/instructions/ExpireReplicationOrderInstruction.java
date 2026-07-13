package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code ExpireReplicationOrder} instruction. */
public final class ExpireReplicationOrderInstruction implements InstructionTemplate {

  private static final String ACTION = "ExpireReplicationOrder";

  private final String orderIdHex;
  private final long expirationEpoch;
  private final Map<String, String> arguments;

  private ExpireReplicationOrderInstruction(final Builder builder) {
    this.orderIdHex = builder.orderIdHex;
    this.expirationEpoch = builder.expirationEpoch;
    this.arguments = Collections.unmodifiableMap(builder.canonicalArguments());
  }

  public String orderIdHex() {
    return orderIdHex;
  }

  public long expirationEpoch() {
    return expirationEpoch;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ExpireReplicationOrderInstruction fromArguments(
      final Map<String, String> arguments) {
    ReplicationOrderInstructionValidation.requireArguments(
        arguments, ACTION, "order_id_hex", "expiration_epoch");
    return builder()
        .setOrderIdHex(require(arguments, "order_id_hex"))
        .setExpirationEpoch(requireLong(arguments, "expiration_epoch"))
        .build();
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.trim().isEmpty()) {
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
    if (!(obj instanceof ExpireReplicationOrderInstruction)) {
      return false;
    }
    final ExpireReplicationOrderInstruction other = (ExpireReplicationOrderInstruction) obj;
    return expirationEpoch == other.expirationEpoch && Objects.equals(orderIdHex, other.orderIdHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(orderIdHex, expirationEpoch);
  }

  /** Builder for canonical expiry instruction arguments. */
  public static final class Builder {
    private String orderIdHex;
    private Long expirationEpoch;

    private Builder() {}

    public Builder setOrderIdHex(final String orderIdHex) {
      this.orderIdHex = ReplicationOrderInstructionValidation.requireOrderId(orderIdHex);
      return this;
    }

    public Builder setExpirationEpoch(final long expirationEpoch) {
      this.expirationEpoch =
          ReplicationOrderInstructionValidation.requireEpoch(expirationEpoch, "expirationEpoch");
      return this;
    }

    public ExpireReplicationOrderInstruction build() {
      if (orderIdHex == null || orderIdHex.isEmpty()) {
        throw new IllegalStateException("orderIdHex must be provided");
      }
      if (expirationEpoch == null) {
        throw new IllegalStateException("expirationEpoch must be provided");
      }
      return new ExpireReplicationOrderInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("order_id_hex", orderIdHex);
      args.put("expiration_epoch", Long.toString(expirationEpoch));
      return args;
    }
  }
}
