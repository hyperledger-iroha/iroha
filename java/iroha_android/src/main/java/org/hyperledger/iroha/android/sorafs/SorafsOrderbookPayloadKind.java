package org.hyperledger.iroha.android.sorafs;

/** Orderbook payload kind accepted by the Rust-backed SoraFS reference validator. */
public enum SorafsOrderbookPayloadKind {
  ORDER_REQUEST(1, "order-request.to", true),
  ORDER_CANCEL(2, "order-cancel.to", true),
  TRADE_EVENT(3, "trade-event.to", false),
  SETTLEMENT_CHANNEL(4, "settlement-channel.to", false),
  SETTLEMENT_RECEIPT(5, "settlement-receipt.to", true),
  RUNTIME_SNAPSHOT(6, "orderbook-runtime-snapshot.to", false);

  private final int bridgeCode;
  private final String defaultLabel;
  private final boolean userSignedPayload;

  SorafsOrderbookPayloadKind(
      final int bridgeCode, final String defaultLabel, final boolean userSignedPayload) {
    this.bridgeCode = bridgeCode;
    this.defaultLabel = defaultLabel;
    this.userSignedPayload = userSignedPayload;
  }

  /** Numeric selector used by {@code connect_norito_bridge}. */
  public int bridgeCode() {
    return bridgeCode;
  }

  /** Default diagnostic label passed to the reference validator. */
  public String defaultLabel() {
    return defaultLabel;
  }

  /** Returns true for orderbook payloads signed directly by SDK users. */
  public boolean isUserSignedPayload() {
    return userSignedPayload;
  }
}
