package org.hyperledger.iroha.android.sorafs;

/** Payload kind accepted by heterogeneous SoraFS fixture-bundle validation. */
public enum SorafsFixtureBundlePayloadKind {
  PROVIDER_ADVERT(1, "provider-advert.to"),
  PROVIDER_ADMISSION_ENVELOPE(2, "provider-admission-envelope.to"),
  REPLICATION_ORDER(3, "replication-order.to"),
  POR_CHALLENGE(4, "por-challenge.to"),
  POR_PROOF(5, "por-proof.to"),
  POTR_RECEIPT(6, "potr-receipt.to"),
  REPAIR_EVIDENCE(7, "repair-evidence.to"),
  REPAIR_REPORT(8, "repair-report.to"),
  REPAIR_TASK_RECORD(9, "repair-task-record.to"),
  REPAIR_SLASH_PROPOSAL(10, "repair-slash-proposal.to"),
  REPAIR_TASK_EVENT(11, "repair-task-event.to"),
  ORDERBOOK_ORDER_REQUEST(12, "orderbook-order-request.to"),
  ORDERBOOK_ORDER_CANCEL(13, "orderbook-order-cancel.to"),
  ORDERBOOK_TRADE_EVENT(14, "orderbook-trade-event.to"),
  ORDERBOOK_SETTLEMENT_CHANNEL(15, "orderbook-settlement-channel.to"),
  ORDERBOOK_SETTLEMENT_RECEIPT(16, "orderbook-settlement-receipt.to"),
  PDP_COMMITMENT(17, "pdp-commitment.to"),
  PDP_CHALLENGE(18, "pdp-challenge.to"),
  PDP_PROOF(19, "pdp-proof.to");

  private final int bridgeCode;
  private final String defaultLabel;

  SorafsFixtureBundlePayloadKind(final int bridgeCode, final String defaultLabel) {
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
