package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.FeePaymentIntent;

/** Response emitted by Torii multisig participation endpoints. */
public final class MultisigResponse {
  private final boolean ok;
  private final String resolvedMultisigAccountId;
  private final boolean submitted;
  private final String proposalId;
  private final String instructionsHash;
  private final String txHashHex;
  private final String executedTxHashHex;
  private final Long creationTimeMs;
  private final FeePaymentIntent feePayment;
  private final String transactionPayloadB64;
  private final String signingMessageB64;

  public MultisigResponse(
      final boolean ok,
      final String resolvedMultisigAccountId,
      final boolean submitted,
      final String proposalId,
      final String instructionsHash,
      final String txHashHex,
      final String executedTxHashHex,
      final Long creationTimeMs,
      final FeePaymentIntent feePayment,
      final String transactionPayloadB64,
      final String signingMessageB64) {
    this.ok = ok;
    this.resolvedMultisigAccountId = resolvedMultisigAccountId;
    this.submitted = submitted;
    this.proposalId = proposalId;
    this.instructionsHash = instructionsHash;
    this.txHashHex = txHashHex;
    this.executedTxHashHex = executedTxHashHex;
    this.creationTimeMs = creationTimeMs;
    this.feePayment = Objects.requireNonNull(feePayment, "feePayment");
    this.transactionPayloadB64 = transactionPayloadB64;
    this.signingMessageB64 = signingMessageB64;
  }

  public boolean ok() { return ok; }
  public String resolvedMultisigAccountId() { return resolvedMultisigAccountId; }
  public boolean submitted() { return submitted; }
  public String proposalId() { return proposalId; }
  public String instructionsHash() { return instructionsHash; }
  public String txHashHex() { return txHashHex; }
  public String executedTxHashHex() { return executedTxHashHex; }
  public Long creationTimeMs() { return creationTimeMs; }
  public FeePaymentIntent feePayment() { return feePayment; }
  public String transactionPayloadB64() { return transactionPayloadB64; }
  public String signingMessageB64() { return signingMessageB64; }
}
