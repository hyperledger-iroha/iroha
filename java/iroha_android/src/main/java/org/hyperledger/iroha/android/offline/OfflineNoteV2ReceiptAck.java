package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** Receipt ACK returned by a recipient after accepting an Offline Note V2 payment token. */
public final class OfflineNoteV2ReceiptAck {
  private final String chainId;
  private final String paymentRequestId;
  private final byte[] tokenId;
  private final String recipientAccountId;
  private final long acceptedAtMs;

  public OfflineNoteV2ReceiptAck(
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

  public static OfflineNoteV2ReceiptAck fromPaymentToken(
      final OfflineNoteV2PaymentToken token,
      final String recipientAccountId,
      final long acceptedAtMs) {
    final OfflineNoteV2PaymentToken checkedToken = Objects.requireNonNull(token, "token");
    final String checkedRecipient = requireNonBlank(recipientAccountId, "recipientAccountId");
    if (!tokenHasRecipientOutput(checkedToken, checkedRecipient)) {
      throw new IllegalArgumentException("payment token does not contain recipient output");
    }
    return new OfflineNoteV2ReceiptAck(
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
    return OfflineNoteV2Wallet.hexLower(tokenId);
  }

  public String recipientAccountId() {
    return recipientAccountId;
  }

  public long acceptedAtMs() {
    return acceptedAtMs;
  }

  public boolean matchesPaymentToken(final OfflineNoteV2PaymentToken token) {
    final OfflineNoteV2PaymentToken checkedToken = Objects.requireNonNull(token, "token");
    return chainId.equals(checkedToken.chainId())
        && paymentRequestId.equals(checkedToken.paymentRequestId())
        && Arrays.equals(tokenId, checkedToken.tokenId())
        && tokenHasRecipientOutput(checkedToken, recipientAccountId);
  }

  public void requireMatchesPaymentToken(final OfflineNoteV2PaymentToken token) {
    if (!matchesPaymentToken(token)) {
      throw new IllegalArgumentException("receipt ACK does not match payment token");
    }
  }

  private static boolean tokenHasRecipientOutput(
      final OfflineNoteV2PaymentToken token, final String recipientAccountId) {
    for (final OfflineNoteV2.AuditOutputClaimV2 claim : token.audit().outputClaims()) {
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
