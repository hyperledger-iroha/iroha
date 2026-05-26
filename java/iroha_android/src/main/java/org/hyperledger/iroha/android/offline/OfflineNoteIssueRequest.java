package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Request sent to an issuer adapter after the wallet derives a note commitment. */
public final class OfflineNoteIssueRequest {
  private final String chainId;
  private final String accountId;
  private final String assetDefinitionId;
  private final String assetId;
  private final String amount;
  private final OfflineNoteLoadContext loadContext;
  private final byte[] noteCommitment;

  public OfflineNoteIssueRequest(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final String assetId,
      final String amount,
      final OfflineNoteLoadContext loadContext,
      final byte[] noteCommitment) {
    this.chainId = chainId;
    this.accountId = accountId;
    this.assetDefinitionId = assetDefinitionId;
    this.assetId = assetId;
    this.amount = amount;
    this.loadContext = loadContext;
    this.noteCommitment = Arrays.copyOf(noteCommitment, noteCommitment.length);
  }

  public String chainId() {
    return chainId;
  }

  public String accountId() {
    return accountId;
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String assetId() {
    return assetId;
  }

  public String amount() {
    return amount;
  }

  public OfflineNoteLoadContext loadContext() {
    return loadContext;
  }

  public byte[] noteCommitment() {
    return Arrays.copyOf(noteCommitment, noteCommitment.length);
  }

  public String noteCommitmentHex() {
    return OfflineNoteWallet.hexLower(noteCommitment);
  }
}
