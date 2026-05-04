package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Response emitted by `GET /v1/vpn/receipts`. */
public final class VpnReceiptListResponse {
  private final List<VpnReceipt> items;
  private final long total;

  public VpnReceiptListResponse(final List<VpnReceipt> items, final long total) {
    this.items = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(items, "items")));
    this.total = total;
  }

  public List<VpnReceipt> items() { return items; }
  public long total() { return total; }
}
