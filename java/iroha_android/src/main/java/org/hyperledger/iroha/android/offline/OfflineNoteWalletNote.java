package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Structured persisted note record; encrypted stores should serialize this shape. */
public final class OfflineNoteWalletNote {
  private final String chainId;
  private final String accountId;
  private final String assetId;
  private final String amount;
  private final String canonicalAmount;
  private final OfflineNote.KeyCertificate keyCertificate;
  private final byte[] noteCommitment;
  private final byte[] noteSecret;
  private final OfflineNote.CommitmentOrigin origin;
  private final List<OfflineNote.AuditBundle> bearerAuditTrail;
  private final OfflineNoteWalletNoteState state;
  private final long createdAtMs;
  private final long updatedAtMs;
  private final String spentPaymentRequestId;

  public OfflineNoteWalletNote(
      final String chainId,
      final String accountId,
      final String assetId,
      final String amount,
      final OfflineNote.KeyCertificate keyCertificate,
      final byte[] noteCommitment,
      final byte[] noteSecret,
      final OfflineNote.CommitmentOrigin origin,
      final OfflineNoteWalletNoteState state,
      final long createdAtMs,
      final long updatedAtMs) {
    this(
        chainId,
        accountId,
        assetId,
        amount,
        keyCertificate,
        noteCommitment,
        noteSecret,
        origin,
        Collections.emptyList(),
        state,
        createdAtMs,
        updatedAtMs,
        null);
  }

  public OfflineNoteWalletNote(
      final String chainId,
      final String accountId,
      final String assetId,
      final String amount,
      final OfflineNote.KeyCertificate keyCertificate,
      final byte[] noteCommitment,
      final byte[] noteSecret,
      final OfflineNote.CommitmentOrigin origin,
      final List<OfflineNote.AuditBundle> bearerAuditTrail,
      final OfflineNoteWalletNoteState state,
      final long createdAtMs,
      final long updatedAtMs) {
    this(
        chainId,
        accountId,
        assetId,
        amount,
        keyCertificate,
        noteCommitment,
        noteSecret,
        origin,
        bearerAuditTrail,
        state,
        createdAtMs,
        updatedAtMs,
        null);
  }

  public OfflineNoteWalletNote(
      final String chainId,
      final String accountId,
      final String assetId,
      final String amount,
      final OfflineNote.KeyCertificate keyCertificate,
      final byte[] noteCommitment,
      final byte[] noteSecret,
      final OfflineNote.CommitmentOrigin origin,
      final List<OfflineNote.AuditBundle> bearerAuditTrail,
      final OfflineNoteWalletNoteState state,
      final long createdAtMs,
      final long updatedAtMs,
      final String spentPaymentRequestId) {
    this.chainId = Objects.requireNonNull(chainId, "chainId");
    this.accountId = Objects.requireNonNull(accountId, "accountId");
    this.assetId = Objects.requireNonNull(assetId, "assetId");
    this.amount = Objects.requireNonNull(amount, "amount");
    this.keyCertificate = Objects.requireNonNull(keyCertificate, "keyCertificate");
    this.noteCommitment = Arrays.copyOf(noteCommitment, noteCommitment.length);
    this.noteSecret = Arrays.copyOf(noteSecret, noteSecret.length);
    if (this.noteSecret.length != 32) {
      throw new IllegalArgumentException("note_secret must be exactly 32 bytes");
    }
    this.origin = Objects.requireNonNull(origin, "origin");
    this.bearerAuditTrail = Collections.unmodifiableList(new ArrayList<>(bearerAuditTrail));
    this.state = Objects.requireNonNull(state, "state");
    this.createdAtMs = createdAtMs;
    this.updatedAtMs = updatedAtMs;
    this.spentPaymentRequestId = normalizeOptionalString(spentPaymentRequestId);
    this.canonicalAmount =
        new OfflineNote.IssuedClaim(
                this.noteCommitment, keyCertificate.payloadHash(), assetId, amount)
            .canonicalAmount();
  }

  public String chainId() {
    return chainId;
  }

  public String accountId() {
    return accountId;
  }

  public String assetId() {
    return assetId;
  }

  public String amount() {
    return amount;
  }

  public String canonicalAmount() {
    return canonicalAmount;
  }

  public OfflineNote.KeyCertificate keyCertificate() {
    return keyCertificate;
  }

  public byte[] noteCommitment() {
    return Arrays.copyOf(noteCommitment, noteCommitment.length);
  }

  public byte[] noteSecret() {
    return Arrays.copyOf(noteSecret, noteSecret.length);
  }

  public String noteCommitmentHex() {
    return OfflineNoteWallet.hexLower(noteCommitment);
  }

  public OfflineNote.CommitmentOrigin origin() {
    return origin;
  }

  public List<OfflineNote.AuditBundle> bearerAuditTrail() {
    return bearerAuditTrail;
  }

  public OfflineNoteWalletNoteState state() {
    return state;
  }

  public long createdAtMs() {
    return createdAtMs;
  }

  public long updatedAtMs() {
    return updatedAtMs;
  }

  public String spentPaymentRequestId() {
    return spentPaymentRequestId;
  }

  public OfflineNote.IssuedClaim issuedClaim() {
    return new OfflineNote.IssuedClaim(
        noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
  }

  public OfflineNoteWalletNote withState(
      final OfflineNoteWalletNoteState state, final long updatedAtMs) {
    return new OfflineNoteWalletNote(
        chainId,
        accountId,
        assetId,
        canonicalAmount,
        keyCertificate,
        noteCommitment(),
        noteSecret(),
        origin,
        bearerAuditTrail,
        state,
        createdAtMs,
        updatedAtMs,
        spentPaymentRequestId);
  }

  public OfflineNoteWalletNote withBearerAuditTrail(
      final List<OfflineNote.AuditBundle> bearerAuditTrail, final long updatedAtMs) {
    return new OfflineNoteWalletNote(
        chainId,
        accountId,
        assetId,
        canonicalAmount,
        keyCertificate,
        noteCommitment(),
        noteSecret(),
        origin,
        bearerAuditTrail,
        state,
        createdAtMs,
        updatedAtMs,
        spentPaymentRequestId);
  }

  public OfflineNoteWalletNote withSpentPaymentRequestId(
      final String spentPaymentRequestId, final long updatedAtMs) {
    return new OfflineNoteWalletNote(
        chainId,
        accountId,
        assetId,
        canonicalAmount,
        keyCertificate,
        noteCommitment(),
        noteSecret(),
        origin,
        bearerAuditTrail,
        state,
        createdAtMs,
        updatedAtMs,
        spentPaymentRequestId);
  }

  private static String normalizeOptionalString(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }
}
