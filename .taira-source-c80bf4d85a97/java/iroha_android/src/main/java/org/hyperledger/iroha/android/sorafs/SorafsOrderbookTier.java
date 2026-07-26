package org.hyperledger.iroha.android.sorafs;

/** Storage tier selector for field-level SoraFS orderbook order builders. */
public enum SorafsOrderbookTier {
  HOT(1),
  WARM(2),
  ARCHIVE(3);

  private final int bridgeCode;

  SorafsOrderbookTier(final int bridgeCode) {
    this.bridgeCode = bridgeCode;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }
}
