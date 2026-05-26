package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Receiver request handed to a payer; it contains no note secret. */
public final class OfflineNoteReceiveRequest {
  private final String chainId;
  private final String paymentRequestId;
  private final String accountId;
  private final String assetDefinitionId;
  private final String assetId;
  private final String amount;
  private final String canonicalAmount;
  private final OfflineNote.KeyCertificate keyCertificate;
  private final byte[] outputCommitment;

  public OfflineNoteReceiveRequest(
      final String chainId,
      final String paymentRequestId,
      final String accountId,
      final String assetDefinitionId,
      final String assetId,
      final String amount,
      final OfflineNote.KeyCertificate keyCertificate,
      final byte[] outputCommitment) {
    this.chainId = chainId;
    this.paymentRequestId = paymentRequestId;
    this.accountId = accountId;
    this.assetDefinitionId = assetDefinitionId;
    this.assetId = assetId;
    this.amount = amount;
    this.keyCertificate = keyCertificate;
    this.outputCommitment = Arrays.copyOf(outputCommitment, outputCommitment.length);
    this.canonicalAmount =
        new OfflineNote.AuditOutputClaim(
                this.outputCommitment, keyCertificate, assetId, amount)
            .canonicalAmount();
  }

  public String chainId() {
    return chainId;
  }

  public String paymentRequestId() {
    return paymentRequestId;
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

  public String canonicalAmount() {
    return canonicalAmount;
  }

  public OfflineNote.KeyCertificate keyCertificate() {
    return keyCertificate;
  }

  public byte[] outputCommitment() {
    return Arrays.copyOf(outputCommitment, outputCommitment.length);
  }

  public String outputCommitmentHex() {
    return OfflineNoteWallet.hexLower(outputCommitment);
  }
}
