package org.hyperledger.iroha.android.sorafs;

/** Side selector for field-level SoraFS orderbook order builders. */
public enum SorafsOrderbookSide {
  BID(1),
  ASK(2);

  private final int bridgeCode;

  SorafsOrderbookSide(final int bridgeCode) {
    this.bridgeCode = bridgeCode;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }
}
