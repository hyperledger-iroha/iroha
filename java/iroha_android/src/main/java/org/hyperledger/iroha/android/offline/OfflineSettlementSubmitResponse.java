package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Response payload for submitting an offline settlement bundle. */
public final class OfflineSettlementSubmitResponse {
  private final String bundleIdHex;
  private final String transactionHashHex;

  public OfflineSettlementSubmitResponse(
      final String bundleIdHex, final String transactionHashHex) {
    this.bundleIdHex = Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    this.transactionHashHex = Objects.requireNonNull(transactionHashHex, "transactionHashHex");
  }

  public String bundleIdHex() {
    return bundleIdHex;
  }

  public String transactionHashHex() {
    return transactionHashHex;
  }
}
