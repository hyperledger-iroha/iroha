package org.hyperledger.iroha.android.offline;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.client.JsonParser;

/** Immutable view over `/v1/offline/transfers` responses. */
public final class OfflineTransferList {

  private final List<OfflineTransferItem> items;
  private final long total;

  public OfflineTransferList(final List<OfflineTransferItem> items, final long total) {
    this.items = List.copyOf(Objects.requireNonNull(items, "items"));
    this.total = total;
  }

  public List<OfflineTransferItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  public static final class OfflineTransferItem {
    private final String bundleIdHex;
    private final String receiverId;
    private final String receiverDisplay;
    private final String depositAccountId;
    private final String depositAccountDisplay;
    private final String assetId;
    private final long receiptCount;
    private final String totalAmount;
    private final String claimedDelta;
    private final String platformPolicy;
    private final PlatformTokenSnapshot platformTokenSnapshot;
    private final String transferJson;

    public OfflineTransferItem(
        final String bundleIdHex,
        final String receiverId,
        final String receiverDisplay,
        final String depositAccountId,
        final String depositAccountDisplay,
        final String assetId,
        final long receiptCount,
        final String totalAmount,
        final String claimedDelta,
        final String platformPolicy,
        final PlatformTokenSnapshot platformTokenSnapshot,
        final String transferJson) {
      this.bundleIdHex = Objects.requireNonNull(bundleIdHex, "bundleIdHex");
      this.receiverId = Objects.requireNonNull(receiverId, "receiverId");
      this.receiverDisplay = Objects.requireNonNull(receiverDisplay, "receiverDisplay");
      this.depositAccountId = Objects.requireNonNull(depositAccountId, "depositAccountId");
      this.depositAccountDisplay =
          Objects.requireNonNull(depositAccountDisplay, "depositAccountDisplay");
      this.assetId = assetId;
      this.receiptCount = receiptCount;
      this.totalAmount = Objects.requireNonNull(totalAmount, "totalAmount");
      this.claimedDelta = Objects.requireNonNull(claimedDelta, "claimedDelta");
      this.platformPolicy = platformPolicy;
      this.platformTokenSnapshot = platformTokenSnapshot;
      this.transferJson = transferJson;
    }

    public String bundleIdHex() {
      return bundleIdHex;
    }

    public String receiverId() {
      return receiverId;
    }

    public String receiverDisplay() {
      return receiverDisplay;
    }

    public String depositAccountId() {
      return depositAccountId;
    }

    public String depositAccountDisplay() {
      return depositAccountDisplay;
    }

    public String assetId() {
      return assetId;
    }

    public long receiptCount() {
      return receiptCount;
    }

    public String totalAmount() {
      return totalAmount;
    }

    public String claimedDelta() {
      return claimedDelta;
    }

    public String platformPolicy() {
      return platformPolicy;
    }

    public PlatformTokenSnapshot platformTokenSnapshot() {
      return platformTokenSnapshot;
    }

    /** Raw Norito JSON of the submitted bundle. */
    public String transferJson() {
      return transferJson;
    }

    /**
     * Serialises this transfer item into a JSON-ready map.
     *
     * <p>Use {@link org.hyperledger.iroha.android.client.JsonEncoder#encode(Object)} to cache the
     * resulting structure locally.</p>
     */
    public Map<String, Object> toJsonMap() {
      final Map<String, Object> json = new LinkedHashMap<>();
      json.put("bundle_id_hex", bundleIdHex);
      json.put("receiver_id", receiverId);
      json.put("receiver_display", receiverDisplay);
      json.put("deposit_account_id", depositAccountId);
      json.put("deposit_account_display", depositAccountDisplay);
      if (assetId != null) {
        json.put("asset_id", assetId);
      }
      json.put("receipt_count", receiptCount);
      json.put("total_amount", totalAmount);
      json.put("claimed_delta", claimedDelta);
      if (platformPolicy != null) {
        json.put("platform_policy", platformPolicy);
      }
      if (platformTokenSnapshot != null) {
        json.put("platform_token_snapshot", platformTokenSnapshot.toJsonMap());
      }
      json.put("transfer", transferAsMap());
      return Collections.unmodifiableMap(json);
    }

    /**
     * Parses the raw transfer JSON into an immutable map representation.
     *
     * @throws IllegalStateException if the JSON payload is malformed or not an object
     */
    @SuppressWarnings("unchecked")
    public Map<String, Object> transferAsMap() {
      if (transferJson == null || transferJson.isBlank()) {
        return Collections.emptyMap();
      }
      final Object parsed = JsonParser.parse(transferJson);
      if (!(parsed instanceof Map)) {
        throw new IllegalStateException("transfer is not a JSON object");
      }
      return Collections.unmodifiableMap((Map<String, Object>) parsed);
    }

    /**
     * Returns a summary of the first receipt embedded in the transfer payload, when present.
     *
     * <p>Transfers aggregate multiple receipts; for audit logging we only need the first entry to
     * capture the sender/receiver/asset context associated with the bundle.</p>
     */
    @SuppressWarnings("unchecked")
    public Optional<ReceiptSummary> firstReceiptSummary() {
      final Map<String, Object> payload = transferAsMap();
      if (payload.isEmpty()) {
        return Optional.empty();
      }
      final Object receiptsNode = payload.get("receipts");
      if (!(receiptsNode instanceof List<?> receipts) || receipts.isEmpty()) {
        return Optional.empty();
      }
      final Object first = receipts.get(0);
      if (!(first instanceof Map<?, ?> rawReceipt)) {
        return Optional.empty();
      }
      final Map<String, Object> receipt = (Map<String, Object>) rawReceipt;
      final String sender = stringValue(receipt.get("from"));
      final String receiver = stringValue(receipt.get("to"));
      final String asset = stringValue(receipt.get("asset"));
      final String amount = stringValue(receipt.get("amount"));
      if (sender == null || receiver == null || amount == null) {
        return Optional.empty();
      }
      return Optional.of(new ReceiptSummary(sender, receiver, asset, amount));
    }

    private static String stringValue(final Object value) {
      if (value == null) {
        return null;
      }
      if (value instanceof String string) {
        return string;
      }
      final String converted = String.valueOf(value).trim();
      return converted.isEmpty() ? null : converted;
    }

    /** Lightweight projection of receipt details for audit logging. */
    public static final class ReceiptSummary {
      private final String senderId;
      private final String receiverId;
      private final String assetId;
      private final String amount;

      private ReceiptSummary(
          final String senderId,
          final String receiverId,
          final String assetId,
          final String amount) {
        this.senderId = Objects.requireNonNull(senderId, "senderId");
        this.receiverId = Objects.requireNonNull(receiverId, "receiverId");
        this.assetId = assetId;
        this.amount = Objects.requireNonNull(amount, "amount");
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
    }

    /** Snapshot of the platform token captured at settlement time. */
    public static final class PlatformTokenSnapshot {
      private final String policy;
      private final String attestationJwsB64;

      public PlatformTokenSnapshot(final String policy, final String attestationJwsB64) {
        this.policy = Objects.requireNonNull(policy, "policy");
        this.attestationJwsB64 = Objects.requireNonNull(attestationJwsB64, "attestationJwsB64");
      }

      public String policy() {
        return policy;
      }

      public String attestationJwsB64() {
        return attestationJwsB64;
      }

      /** Returns a JSON-ready map representation of the snapshot. */
      public Map<String, String> toJsonMap() {
        final Map<String, String> snapshot = new LinkedHashMap<>();
        snapshot.put("policy", policy);
        snapshot.put("attestation_jws_b64", attestationJwsB64);
        return Collections.unmodifiableMap(snapshot);
      }
    }
  }
}
