package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.Objects;

/** Structured persisted note record; encrypted stores should serialize this shape. */
public final class OfflineNoteV2WalletNote {
  private final String chainId;
  private final String accountId;
  private final String assetId;
  private final String amount;
  private final String canonicalAmount;
  private final OfflineNoteV2.KeyCertificateV2 keyCertificate;
  private final byte[] noteCommitment;
  private final byte[] noteSecret;
  private final OfflineNoteV2.CommitmentOriginV2 origin;
  private final OfflineNoteV2WalletNoteState state;
  private final long createdAtMs;
  private final long updatedAtMs;

  public OfflineNoteV2WalletNote(
      final String chainId,
      final String accountId,
      final String assetId,
      final String amount,
      final OfflineNoteV2.KeyCertificateV2 keyCertificate,
      final byte[] noteCommitment,
      final byte[] noteSecret,
      final OfflineNoteV2.CommitmentOriginV2 origin,
      final OfflineNoteV2WalletNoteState state,
      final long createdAtMs,
      final long updatedAtMs) {
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
    this.state = Objects.requireNonNull(state, "state");
    this.createdAtMs = createdAtMs;
    this.updatedAtMs = updatedAtMs;
    this.canonicalAmount =
        new OfflineNoteV2.IssuedClaimV2(
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

  public OfflineNoteV2.KeyCertificateV2 keyCertificate() {
    return keyCertificate;
  }

  public byte[] noteCommitment() {
    return Arrays.copyOf(noteCommitment, noteCommitment.length);
  }

  public byte[] noteSecret() {
    return Arrays.copyOf(noteSecret, noteSecret.length);
  }

  public String noteCommitmentHex() {
    return OfflineNoteV2Wallet.hexLower(noteCommitment);
  }

  public OfflineNoteV2.CommitmentOriginV2 origin() {
    return origin;
  }

  public OfflineNoteV2WalletNoteState state() {
    return state;
  }

  public long createdAtMs() {
    return createdAtMs;
  }

  public long updatedAtMs() {
    return updatedAtMs;
  }

  public OfflineNoteV2.IssuedClaimV2 issuedClaim() {
    return new OfflineNoteV2.IssuedClaimV2(
        noteCommitment(), keyCertificate.payloadHash(), assetId, canonicalAmount);
  }

  public OfflineNoteV2WalletNote withState(
      final OfflineNoteV2WalletNoteState state, final long updatedAtMs) {
    return new OfflineNoteV2WalletNote(
        chainId,
        accountId,
        assetId,
        canonicalAmount,
        keyCertificate,
        noteCommitment(),
        noteSecret(),
        origin,
        state,
        createdAtMs,
        updatedAtMs);
  }
}
