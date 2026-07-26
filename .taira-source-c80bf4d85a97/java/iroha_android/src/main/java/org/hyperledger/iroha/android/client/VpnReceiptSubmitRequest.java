package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Request body for `POST /v1/vpn/receipts`. */
public final class VpnReceiptSubmitRequest {
  private final String relayReceiptHex;
  private final String clientVoucherHex;
  private final String leaseIdHex;

  public VpnReceiptSubmitRequest(final String relayReceiptHex, final String clientVoucherHex) {
    this(relayReceiptHex, clientVoucherHex, null);
  }

  public VpnReceiptSubmitRequest(
      final String relayReceiptHex,
      final String clientVoucherHex,
      final String leaseIdHex) {
    this.relayReceiptHex = Objects.requireNonNull(relayReceiptHex, "relayReceiptHex");
    this.clientVoucherHex = Objects.requireNonNull(clientVoucherHex, "clientVoucherHex");
    this.leaseIdHex = leaseIdHex;
  }

  public String relayReceiptHex() { return relayReceiptHex; }
  public String clientVoucherHex() { return clientVoucherHex; }
  public String leaseIdHex() { return leaseIdHex; }
}
