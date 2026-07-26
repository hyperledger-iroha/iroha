package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Response emitted by `/v1/soracloud/model/upload/private/receipts`. */
public final class SoracloudPrivateUploadedModelReceiptListResponse {
  private final long schemaVersion;
  private final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts;
  private final Long total;
  private final long returnedItems;
  private final long remainingItems;
  private final boolean hasMore;
  private final String countMode;
  private final String continueCursor;

  public SoracloudPrivateUploadedModelReceiptListResponse(
      final long schemaVersion,
      final List<SoracloudPrivateUploadedModelExecutionReceipt> receipts,
      final Long total,
      final long returnedItems,
      final long remainingItems,
      final boolean hasMore,
      final String countMode,
      final String continueCursor) {
    this.schemaVersion = schemaVersion;
    this.receipts = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(receipts, "receipts")));
    this.total = total;
    this.returnedItems = returnedItems;
    this.remainingItems = remainingItems;
    this.hasMore = hasMore;
    this.countMode = Objects.requireNonNull(countMode, "countMode");
    this.continueCursor = continueCursor;
  }

  public long schemaVersion() { return schemaVersion; }

  public List<SoracloudPrivateUploadedModelExecutionReceipt> receipts() { return receipts; }

  public Long total() { return total; }

  public long returnedItems() { return returnedItems; }

  public long remainingItems() { return remainingItems; }

  public boolean hasMore() { return hasMore; }

  public String countMode() { return countMode; }

  public String continueCursor() { return continueCursor; }
}

