package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for the {@code RecordReplicationReceipt} instruction. */
public final class RecordReplicationReceiptInstruction implements InstructionTemplate {

  private static final String ACTION = "RecordReplicationReceipt";

  private final String orderIdHex;
  private final String providerIdHex;
  private final Status status;
  private final long timestamp;
  private final String porSampleDigestHex;
  private final Map<String, String> arguments;

  private RecordReplicationReceiptInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RecordReplicationReceiptInstruction(
      final Builder builder, final Map<String, String> canonicalArguments) {
    this.orderIdHex = builder.orderIdHex;
    this.providerIdHex = builder.providerIdHex;
    this.status = builder.status;
    this.timestamp = builder.timestamp;
    this.porSampleDigestHex = builder.porSampleDigestHex;
    this.arguments =
        Collections.unmodifiableMap(
            new LinkedHashMap<>(Objects.requireNonNull(canonicalArguments, "arguments")));
  }

  public String orderIdHex() {
    return orderIdHex;
  }

  public String providerIdHex() {
    return providerIdHex;
  }

  public Status status() {
    return status;
  }

  public long timestamp() {
    return timestamp;
  }

  public String porSampleDigestHex() {
    return porSampleDigestHex;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RecordReplicationReceiptInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setOrderIdHex(require(arguments, "order_id_hex"))
            .setProviderIdHex(require(arguments, "provider_id_hex"))
            .setStatus(Status.fromLabel(require(arguments, "status")))
            .setTimestamp(requireLong(arguments, "timestamp"));
    if (arguments.containsKey("por_sample_digest_hex")) {
      builder.setPorSampleDigestHex(arguments.get("por_sample_digest_hex"));
    }
    return new RecordReplicationReceiptInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof RecordReplicationReceiptInstruction other)) {
      return false;
    }
    return timestamp == other.timestamp
        && Objects.equals(orderIdHex, other.orderIdHex)
        && Objects.equals(providerIdHex, other.providerIdHex)
        && status == other.status
        && Objects.equals(porSampleDigestHex, other.porSampleDigestHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(orderIdHex, providerIdHex, status, timestamp, porSampleDigestHex);
  }

  public enum Status {
    ACCEPTED("Accepted"),
    COMPLETED("Completed"),
    REJECTED("Rejected");

    private final String label;

    Status(final String label) {
      this.label = label;
    }

    public String label() {
      return label;
    }

    public static Status fromLabel(final String value) {
      final String normalized = value.trim();
      for (final Status status : values()) {
        if (status.label.equalsIgnoreCase(normalized)) {
          return status;
        }
      }
      throw new IllegalArgumentException("Unknown replication receipt status: " + value);
    }
  }

  public static final class Builder {
    private String orderIdHex;
    private String providerIdHex;
    private Status status;
    private Long timestamp;
    private String porSampleDigestHex;

    private Builder() {}

    public Builder setOrderIdHex(final String orderIdHex) {
      this.orderIdHex = Objects.requireNonNull(orderIdHex, "orderIdHex");
      return this;
    }

    public Builder setProviderIdHex(final String providerIdHex) {
      this.providerIdHex = Objects.requireNonNull(providerIdHex, "providerIdHex");
      return this;
    }

    public Builder setStatus(final Status status) {
      this.status = Objects.requireNonNull(status, "status");
      return this;
    }

    public Builder setStatusLabel(final String label) {
      return setStatus(Status.fromLabel(Objects.requireNonNull(label, "label")));
    }

    public Builder setTimestamp(final long timestamp) {
      if (timestamp < 0) {
        throw new IllegalArgumentException("timestamp must be non-negative");
      }
      this.timestamp = timestamp;
      return this;
    }

    public Builder setPorSampleDigestHex(final String porSampleDigestHex) {
      this.porSampleDigestHex = porSampleDigestHex;
      return this;
    }

    public RecordReplicationReceiptInstruction build() {
      if (orderIdHex == null || orderIdHex.isBlank()) {
        throw new IllegalStateException("orderIdHex must be provided");
      }
      if (providerIdHex == null || providerIdHex.isBlank()) {
        throw new IllegalStateException("providerIdHex must be provided");
      }
      if (status == null) {
        throw new IllegalStateException("status must be provided");
      }
      if (timestamp == null) {
        throw new IllegalStateException("timestamp must be provided");
      }
      return new RecordReplicationReceiptInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("order_id_hex", orderIdHex);
      args.put("provider_id_hex", providerIdHex);
      args.put("status", status.label());
      args.put("timestamp", Long.toString(timestamp));
      if (porSampleDigestHex != null && !porSampleDigestHex.isBlank()) {
        args.put("por_sample_digest_hex", porSampleDigestHex);
      }
      return args;
    }
  }
}
