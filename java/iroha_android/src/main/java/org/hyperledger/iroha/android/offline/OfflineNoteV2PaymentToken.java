package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Payment token produced by a payer and accepted by the recipient. */
public final class OfflineNoteV2PaymentToken {
  private final String chainId;
  private final String paymentRequestId;
  private final byte[] tokenNonce;
  private final byte[] tokenId;
  private final OfflineNoteV2.AuditBundleV2 audit;
  private final long createdAtMs;

  public OfflineNoteV2PaymentToken(
      final String chainId,
      final String paymentRequestId,
      final byte[] tokenNonce,
      final byte[] tokenId,
      final OfflineNoteV2.AuditBundleV2 audit,
      final long createdAtMs) {
    this.chainId = chainId;
    this.paymentRequestId = paymentRequestId;
    this.tokenNonce = Arrays.copyOf(tokenNonce, tokenNonce.length);
    this.tokenId = Arrays.copyOf(tokenId, tokenId.length);
    this.audit = audit;
    this.createdAtMs = createdAtMs;
  }

  public String chainId() {
    return chainId;
  }

  public String paymentRequestId() {
    return paymentRequestId;
  }

  public byte[] tokenId() {
    return Arrays.copyOf(tokenId, tokenId.length);
  }

  public byte[] tokenNonce() {
    return Arrays.copyOf(tokenNonce, tokenNonce.length);
  }

  public String tokenIdHex() {
    return OfflineNoteV2Wallet.hexLower(tokenId);
  }

  public OfflineNoteV2.AuditBundleV2 audit() {
    return audit;
  }

  public long createdAtMs() {
    return createdAtMs;
  }
}
