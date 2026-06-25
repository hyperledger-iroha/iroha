package org.hyperledger.iroha.android.sorafs;

/** Cancellation reason selector for field-level SoraFS orderbook cancel builders. */
public enum SorafsOrderbookCancelReason {
  OWNER_REQUESTED(1),
  EXPIRED(2),
  GOVERNANCE(3),
  REPLACED(4);

  private final int bridgeCode;

  SorafsOrderbookCancelReason(final int bridgeCode) {
    this.bridgeCode = bridgeCode;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }
}
