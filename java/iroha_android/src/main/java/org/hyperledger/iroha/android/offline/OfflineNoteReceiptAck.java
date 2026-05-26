package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** Receipt ACK returned by a recipient after accepting an Offline Note payment token. */
public final class OfflineNoteReceiptAck {
  private final String chainId;
  private final String paymentRequestId;
  private final byte[] tokenId;
  private final String recipientAccountId;
  private final long acceptedAtMs;

  public OfflineNoteReceiptAck(
      final String chainId,
      final String paymentRequestId,
      final byte[] tokenId,
      final String recipientAccountId,
      final long acceptedAtMs) {
    this.chainId = requireNonBlank(chainId, "chainId");
    this.paymentRequestId = requireNonBlank(paymentRequestId, "paymentRequestId");
    this.tokenId = Objects.requireNonNull(tokenId, "tokenId").clone();
    if (this.tokenId.length != 32) {
      throw new IllegalArgumentException("tokenId must be 32 bytes");
    }
    this.recipientAccountId = requireNonBlank(recipientAccountId, "recipientAccountId");
    if (acceptedAtMs < 0L) {
      throw new IllegalArgumentException("acceptedAtMs must be non-negative");
    }
    this.acceptedAtMs = acceptedAtMs;
  }

  public static OfflineNoteReceiptAck fromPaymentToken(
      final OfflineNotePaymentToken token,
      final String recipientAccountId,
      final long acceptedAtMs) {
    final OfflineNotePaymentToken checkedToken = Objects.requireNonNull(token, "token");
    final String checkedRecipient = requireNonBlank(recipientAccountId, "recipientAccountId");
    if (!tokenHasRecipientOutput(checkedToken, checkedRecipient)) {
      throw new IllegalArgumentException("payment token does not contain recipient output");
    }
    return new OfflineNoteReceiptAck(
        checkedToken.chainId(),
        checkedToken.paymentRequestId(),
        checkedToken.tokenId(),
        checkedRecipient,
        acceptedAtMs);
  }

  public String chainId() {
    return chainId;
  }

  public String paymentRequestId() {
    return paymentRequestId;
  }

  public byte[] tokenId() {
    return tokenId.clone();
  }

  public String tokenIdHex() {
    return OfflineNoteWallet.hexLower(tokenId);
  }

  public String recipientAccountId() {
    return recipientAccountId;
  }

  public long acceptedAtMs() {
    return acceptedAtMs;
  }

  public boolean matchesPaymentToken(final OfflineNotePaymentToken token) {
    final OfflineNotePaymentToken checkedToken = Objects.requireNonNull(token, "token");
    return chainId.equals(checkedToken.chainId())
        && paymentRequestId.equals(checkedToken.paymentRequestId())
        && Arrays.equals(tokenId, checkedToken.tokenId())
        && tokenHasRecipientOutput(checkedToken, recipientAccountId);
  }

  public void requireMatchesPaymentToken(final OfflineNotePaymentToken token) {
    if (!matchesPaymentToken(token)) {
      throw new IllegalArgumentException("receipt ACK does not match payment token");
    }
  }

  private static boolean tokenHasRecipientOutput(
      final OfflineNotePaymentToken token, final String recipientAccountId) {
    for (final OfflineNote.AuditOutputClaim claim : token.audit().outputClaims()) {
      if (claim.keyCertificate().accountId().equals(recipientAccountId)) {
        return true;
      }
    }
    return false;
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return value;
  }
}
