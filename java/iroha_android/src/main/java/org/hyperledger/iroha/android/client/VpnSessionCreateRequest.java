package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Request body for `POST /v1/vpn/sessions`. */
public final class VpnSessionCreateRequest {
  private final String exitClass;
  private final String quoteId;
  private final String paymentTxHash;
  private final String meteringPublicKeyHex;

  public VpnSessionCreateRequest(
      final String quoteId,
      final String paymentTxHash,
      final String meteringPublicKeyHex) {
    this(null, quoteId, paymentTxHash, meteringPublicKeyHex);
  }

  public VpnSessionCreateRequest(
      final String exitClass,
      final String quoteId,
      final String paymentTxHash,
      final String meteringPublicKeyHex) {
    this.exitClass = exitClass;
    this.quoteId = Objects.requireNonNull(quoteId, "quoteId");
    this.paymentTxHash = Objects.requireNonNull(paymentTxHash, "paymentTxHash");
    this.meteringPublicKeyHex = Objects.requireNonNull(meteringPublicKeyHex, "meteringPublicKeyHex");
  }

  public String exitClass() { return exitClass; }
  public String quoteId() { return quoteId; }
  public String paymentTxHash() { return paymentTxHash; }
  public String meteringPublicKeyHex() { return meteringPublicKeyHex; }
}
