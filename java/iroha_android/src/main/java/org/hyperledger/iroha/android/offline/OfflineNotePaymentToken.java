package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Payment token produced by a payer and accepted by the recipient. */
public final class OfflineNotePaymentToken {
  private final String chainId;
  private final String paymentRequestId;
  private final byte[] tokenNonce;
  private final byte[] tokenId;
  private final OfflineNote.AuditBundle audit;
  private final List<OfflineNote.AuditBundle> bearerAuditTrail;
  private final long createdAtMs;

  public OfflineNotePaymentToken(
      final String chainId,
      final String paymentRequestId,
      final byte[] tokenNonce,
      final byte[] tokenId,
      final OfflineNote.AuditBundle audit,
      final long createdAtMs) {
    this(chainId, paymentRequestId, tokenNonce, tokenId, audit, Collections.singletonList(audit), createdAtMs);
  }

  public OfflineNotePaymentToken(
      final String chainId,
      final String paymentRequestId,
      final byte[] tokenNonce,
      final byte[] tokenId,
      final OfflineNote.AuditBundle audit,
      final List<OfflineNote.AuditBundle> bearerAuditTrail,
      final long createdAtMs) {
    this.chainId = chainId;
    this.paymentRequestId = paymentRequestId;
    this.tokenNonce = Arrays.copyOf(tokenNonce, tokenNonce.length);
    this.tokenId = Arrays.copyOf(tokenId, tokenId.length);
    this.audit = audit;
    this.bearerAuditTrail = Collections.unmodifiableList(new ArrayList<>(bearerAuditTrail));
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
    return OfflineNoteWallet.hexLower(tokenId);
  }

  public OfflineNote.AuditBundle audit() {
    return audit;
  }

  public List<OfflineNote.AuditBundle> bearerAuditTrail() {
    return bearerAuditTrail;
  }

  public long createdAtMs() {
    return createdAtMs;
  }
}
