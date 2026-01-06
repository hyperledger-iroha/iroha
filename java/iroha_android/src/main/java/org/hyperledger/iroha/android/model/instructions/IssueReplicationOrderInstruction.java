package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code IssueReplicationOrder} instruction. */
public final class IssueReplicationOrderInstruction implements InstructionTemplate {

  private static final String ACTION = "IssueReplicationOrder";

  private final String orderIdHex;
  private final String orderPayloadBase64;
  private final long issuedEpoch;
  private final long deadlineEpoch;
  private final Map<String, String> arguments;

  private IssueReplicationOrderInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private IssueReplicationOrderInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.orderIdHex = builder.orderIdHex;
    this.orderPayloadBase64 = builder.orderPayloadBase64;
    this.issuedEpoch = builder.issuedEpoch;
    this.deadlineEpoch = builder.deadlineEpoch;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String orderIdHex() {
    return orderIdHex;
  }

  public String orderPayloadBase64() {
    return orderPayloadBase64;
  }

  public long issuedEpoch() {
    return issuedEpoch;
  }

  public long deadlineEpoch() {
    return deadlineEpoch;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static IssueReplicationOrderInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setOrderIdHex(require(arguments, "order_id_hex"))
            .setOrderPayloadBase64(require(arguments, "order_payload_base64"))
            .setIssuedEpoch(requireLong(arguments, "issued_epoch"))
            .setDeadlineEpoch(requireLong(arguments, "deadline_epoch"));
    return new IssueReplicationOrderInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String requireBase64(final String value, final String fieldName) {
    final String trimmed = Objects.requireNonNull(value, fieldName).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
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
    if (!(obj instanceof IssueReplicationOrderInstruction other)) {
      return false;
    }
    return issuedEpoch == other.issuedEpoch
        && deadlineEpoch == other.deadlineEpoch
        && Objects.equals(orderIdHex, other.orderIdHex)
        && Objects.equals(orderPayloadBase64, other.orderPayloadBase64);
  }

  @Override
  public int hashCode() {
    return Objects.hash(orderIdHex, orderPayloadBase64, issuedEpoch, deadlineEpoch);
  }

  public static final class Builder {
    private String orderIdHex;
    private String orderPayloadBase64;
    private Long issuedEpoch;
    private Long deadlineEpoch;

    private Builder() {}

    public Builder setOrderIdHex(final String orderIdHex) {
      this.orderIdHex = Objects.requireNonNull(orderIdHex, "orderIdHex");
      return this;
    }

    /** Convenience helper that accepts raw bytes for the order identifier. */
    public Builder setOrderId(final byte[] orderId) {
      Objects.requireNonNull(orderId, "orderId");
      final StringBuilder sb = new StringBuilder(orderId.length * 2);
      for (final byte b : orderId) {
        sb.append(String.format("%02x", b));
      }
      return setOrderIdHex(sb.toString());
    }

    public Builder setOrderPayloadBase64(final String orderPayloadBase64) {
      this.orderPayloadBase64 = requireBase64(orderPayloadBase64, "orderPayloadBase64");
      return this;
    }

    public Builder setOrderPayload(final byte[] orderPayload) {
      Objects.requireNonNull(orderPayload, "orderPayload");
      final String encoded = Base64.getEncoder().encodeToString(orderPayload);
      return setOrderPayloadBase64(encoded);
    }

    public Builder setIssuedEpoch(final long issuedEpoch) {
      if (issuedEpoch < 0) {
        throw new IllegalArgumentException("issuedEpoch must be non-negative");
      }
      this.issuedEpoch = issuedEpoch;
      return this;
    }

    public Builder setDeadlineEpoch(final long deadlineEpoch) {
      if (deadlineEpoch < 0) {
        throw new IllegalArgumentException("deadlineEpoch must be non-negative");
      }
      this.deadlineEpoch = deadlineEpoch;
      return this;
    }

    public IssueReplicationOrderInstruction build() {
      if (orderIdHex == null || orderIdHex.isBlank()) {
        throw new IllegalStateException("orderIdHex must be provided");
      }
      if (orderPayloadBase64 == null || orderPayloadBase64.isBlank()) {
        throw new IllegalStateException("orderPayloadBase64 must be provided");
      }
      if (issuedEpoch == null) {
        throw new IllegalStateException("issuedEpoch must be provided");
      }
      if (deadlineEpoch == null) {
        throw new IllegalStateException("deadlineEpoch must be provided");
      }
      return new IssueReplicationOrderInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("order_id_hex", orderIdHex);
      args.put("order_payload_base64", orderPayloadBase64);
      args.put("issued_epoch", Long.toString(issuedEpoch));
      args.put("deadline_epoch", Long.toString(deadlineEpoch));
      return args;
    }
  }
}
