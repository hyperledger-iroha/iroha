package org.hyperledger.iroha.android.client;

/** Response emitted by Torii multisig participation endpoints. */
public final class MultisigResponse {
  private final boolean ok;
  private final String resolvedMultisigAccountId;
  private final Boolean submitted;
  private final String proposalId;
  private final String instructionsHash;
  private final String txHashHex;
  private final String executedTxHashHex;
  private final Long creationTimeMs;
  private final String signingMessageB64;

  public MultisigResponse(
      final boolean ok,
      final String resolvedMultisigAccountId,
      final Boolean submitted,
      final String proposalId,
      final String instructionsHash,
      final String txHashHex,
      final String executedTxHashHex,
      final Long creationTimeMs,
      final String signingMessageB64) {
    this.ok = ok;
    this.resolvedMultisigAccountId = resolvedMultisigAccountId;
    this.submitted = submitted;
    this.proposalId = proposalId;
    this.instructionsHash = instructionsHash;
    this.txHashHex = txHashHex;
    this.executedTxHashHex = executedTxHashHex;
    this.creationTimeMs = creationTimeMs;
    this.signingMessageB64 = signingMessageB64;
  }

  public boolean ok() { return ok; }
  public String resolvedMultisigAccountId() { return resolvedMultisigAccountId; }
  public Boolean submitted() { return submitted; }
  public String proposalId() { return proposalId; }
  public String instructionsHash() { return instructionsHash; }
  public String txHashHex() { return txHashHex; }
  public String executedTxHashHex() { return executedTxHashHex; }
  public Long creationTimeMs() { return creationTimeMs; }
  public String signingMessageB64() { return signingMessageB64; }
}
