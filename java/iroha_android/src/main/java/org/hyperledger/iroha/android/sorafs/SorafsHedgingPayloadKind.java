package org.hyperledger.iroha.android.sorafs;

/** Hedging and billing payload kind accepted by the Rust-backed SoraFS reference validator. */
public enum SorafsHedgingPayloadKind {
  PRICE_FEED(1, "hedging-price-feed.to"),
  REFERENCE_PRICE_DECISION(2, "hedging-reference-price-decision.to"),
  BILLING_LINE_ITEM(3, "billing-line-item.to"),
  BILLING_STATEMENT(4, "billing-statement.to");

  private final int bridgeCode;
  private final String defaultLabel;

  SorafsHedgingPayloadKind(final int bridgeCode, final String defaultLabel) {
    this.bridgeCode = bridgeCode;
    this.defaultLabel = defaultLabel;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }

  /** Default diagnostic label passed to the reference validator. */
  public String defaultLabel() {
    return defaultLabel;
  }
}
