package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Request sent to an issuer adapter after the wallet derives a note commitment. */
public final class OfflineNoteV2IssueRequest {
  private final String chainId;
  private final String accountId;
  private final String assetDefinitionId;
  private final String assetId;
  private final String amount;
  private final OfflineNoteV2LoadContext loadContext;
  private final byte[] noteCommitment;

  public OfflineNoteV2IssueRequest(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final String assetId,
      final String amount,
      final OfflineNoteV2LoadContext loadContext,
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

  public OfflineNoteV2LoadContext loadContext() {
    return loadContext;
  }

  public byte[] noteCommitment() {
    return Arrays.copyOf(noteCommitment, noteCommitment.length);
  }

  public String noteCommitmentHex() {
    return OfflineNoteV2Wallet.hexLower(noteCommitment);
  }
}
