package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;

/** Parsed response for Torii offline transfer list APIs. */
public final class OfflineTransferList {
  private final List<OfflineTransferItem> items;
  private final long total;

  public OfflineTransferList(final List<OfflineTransferItem> items, final long total) {
    this.items = Collections.unmodifiableList(new ArrayList<>(items));
    this.total = total;
  }

  public List<OfflineTransferItem> items() {
    return items;
  }

  public long total() {
    return total;
  }

  /** One offline transfer record returned by Torii. */
  public static final class OfflineTransferItem {
    private final String bundleIdHex;
    private final String controllerId;
    private final String controllerDisplay;
    private final String receiverId;
    private final String receiverDisplay;
    private final String depositAccountId;
    private final String depositAccountDisplay;
    private final String assetId;
    private final String totalAmount;
    private final String claimedDelta;
    private final String status;
    private final long receiptCount;
    private final long recordedAtMs;
    private final long recordedAtHeight;
    private final Map<String, Object> transfer;
    private final List<ReceiptSummary> receiptSummaries;

    public OfflineTransferItem(
        final String bundleIdHex,
        final String controllerId,
        final String controllerDisplay,
        final String receiverId,
        final String receiverDisplay,
        final String depositAccountId,
        final String depositAccountDisplay,
        final String assetId,
        final String totalAmount,
        final String claimedDelta,
        final String status,
        final long receiptCount,
        final long recordedAtMs,
        final long recordedAtHeight,
        final Map<String, Object> transfer,
        final List<ReceiptSummary> receiptSummaries) {
      this.bundleIdHex = nullToEmpty(bundleIdHex);
      this.controllerId = nullToEmpty(controllerId);
      this.controllerDisplay = nullToEmpty(controllerDisplay);
      this.receiverId = nullToEmpty(receiverId);
      this.receiverDisplay = nullToEmpty(receiverDisplay);
      this.depositAccountId = nullToEmpty(depositAccountId);
      this.depositAccountDisplay = nullToEmpty(depositAccountDisplay);
      this.assetId = nullToEmpty(assetId);
      this.totalAmount = nullToEmpty(totalAmount);
      this.claimedDelta = nullToEmpty(claimedDelta);
      this.status = nullToEmpty(status);
      this.receiptCount = receiptCount;
      this.recordedAtMs = recordedAtMs;
      this.recordedAtHeight = recordedAtHeight;
      this.transfer = immutableMap(transfer);
      this.receiptSummaries = Collections.unmodifiableList(new ArrayList<>(receiptSummaries));
    }

    public String bundleIdHex() {
      return bundleIdHex;
    }

    public String controllerId() {
      return controllerId;
    }

    public String controllerDisplay() {
      return controllerDisplay;
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

    public String totalAmount() {
      return totalAmount;
    }

    public String claimedDelta() {
      return claimedDelta;
    }

    public String status() {
      return status;
    }

    public long receiptCount() {
      return receiptCount;
    }

    public long recordedAtMs() {
      return recordedAtMs;
    }

    public long recordedAtHeight() {
      return recordedAtHeight;
    }

    public Map<String, Object> transfer() {
      return transfer;
    }

    public List<ReceiptSummary> receiptSummaries() {
      return receiptSummaries;
    }

    public Optional<ReceiptSummary> firstReceiptSummary() {
      if (receiptSummaries.isEmpty()) {
        return Optional.empty();
      }
      return Optional.of(receiptSummaries.get(0));
    }

    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("bundle_id_hex", bundleIdHex);
      map.put("controller_id", controllerId);
      map.put("controller_display", controllerDisplay);
      map.put("receiver_id", receiverId);
      map.put("receiver_display", receiverDisplay);
      map.put("deposit_account_id", depositAccountId);
      map.put("deposit_account_display", depositAccountDisplay);
      map.put("asset_id", assetId);
      map.put("total_amount", totalAmount);
      map.put("claimed_delta", claimedDelta);
      map.put("status", status);
      map.put("receipt_count", receiptCount);
      map.put("recorded_at_ms", recordedAtMs);
      map.put("recorded_at_height", recordedAtHeight);
      if (!transfer.isEmpty()) {
        map.put("transfer", transfer);
      }
      if (!receiptSummaries.isEmpty()) {
        final List<Map<String, Object>> summaries = new ArrayList<>(receiptSummaries.size());
        for (final ReceiptSummary summary : receiptSummaries) {
          summaries.add(summary.toJsonMap());
        }
        map.put("receipt_summaries", summaries);
      }
      return map;
    }
  }

  /** Compact receipt fields used for offline transfer list previews. */
  public static final class ReceiptSummary {
    private final String senderId;
    private final String receiverId;
    private final String amount;
    private final String assetId;
    private final String status;

    public ReceiptSummary(
        final String senderId,
        final String receiverId,
        final String amount,
        final String assetId,
        final String status) {
      this.senderId = nullToEmpty(senderId);
      this.receiverId = nullToEmpty(receiverId);
      this.amount = nullToEmpty(amount);
      this.assetId = nullToEmpty(assetId);
      this.status = nullToEmpty(status);
    }

    public String senderId() {
      return senderId;
    }

    public String receiverId() {
      return receiverId;
    }

    public String amount() {
      return amount;
    }

    public String assetId() {
      return assetId;
    }

    public String status() {
      return status;
    }

    public Map<String, Object> toJsonMap() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("sender_id", senderId);
      map.put("receiver_id", receiverId);
      map.put("amount", amount);
      map.put("asset_id", assetId);
      map.put("status", status);
      return map;
    }
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }

  private static Map<String, Object> immutableMap(final Map<String, Object> value) {
    if (value == null || value.isEmpty()) {
      return Collections.emptyMap();
    }
    return Collections.unmodifiableMap(new LinkedHashMap<>(value));
  }
}
