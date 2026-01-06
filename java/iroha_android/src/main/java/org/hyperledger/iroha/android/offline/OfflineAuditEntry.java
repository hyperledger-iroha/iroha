package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Immutable log entry describing an offline transfer for audit exports. */
public final class OfflineAuditEntry {

  private final String txId;
  private final String senderId;
  private final String receiverId;
  private final String assetId;
  private final String amount;
  private final long timestampMs;

  public OfflineAuditEntry(
      final String txId,
      final String senderId,
      final String receiverId,
      final String assetId,
      final String amount,
      final long timestampMs) {
    this.txId = Objects.requireNonNull(txId, "txId");
    this.senderId = Objects.requireNonNull(senderId, "senderId");
    this.receiverId = Objects.requireNonNull(receiverId, "receiverId");
    this.assetId = Objects.requireNonNull(assetId, "assetId");
    this.amount = Objects.requireNonNull(amount, "amount");
    this.timestampMs = timestampMs;
  }

  public String txId() {
    return txId;
  }

  public String senderId() {
    return senderId;
  }

  public String receiverId() {
    return receiverId;
  }

  public String assetId() {
    return assetId;
  }

  public String amount() {
    return amount;
  }

  public long timestampMs() {
    return timestampMs;
  }

  Map<String, Object> toJson() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("tx_id", txId);
    json.put("sender_id", senderId);
    json.put("receiver_id", receiverId);
    json.put("asset_id", assetId);
    json.put("amount", amount);
    json.put("timestamp_ms", timestampMs);
    return json;
  }

  static OfflineAuditEntry fromJsonMap(final Map<String, Object> json) {
    return new OfflineAuditEntry(
        requireString(json.get("tx_id"), "tx_id"),
        requireString(json.get("sender_id"), "sender_id"),
        requireString(json.get("receiver_id"), "receiver_id"),
        requireString(json.get("asset_id"), "asset_id"),
        requireString(json.get("amount"), "amount"),
        requireLong(json.get("timestamp_ms"), "timestamp_ms"));
  }

  private static String requireString(final Object value, final String field) {
    if (value == null) {
      throw new IllegalStateException(field + " is missing");
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static long requireLong(final Object value, final String field) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(field + " is not numeric");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(field + " must be an integer");
    }
    return number.longValue();
  }
}
