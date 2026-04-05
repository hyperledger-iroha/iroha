package org.hyperledger.iroha.android.client;

/** Successful response emitted by `POST /v1/contracts/call`. */
public final class ContractCallResponse {
  private final boolean ok;
  private final boolean submitted;
  private final String dataspace;
  private final String codeHashHex;
  private final String abiHashHex;
  private final long creationTimeMs;
  private final String contractAddress;
  private final String txHashHex;
  private final String entrypoint;
  private final String transactionScaffoldB64;
  private final String signedTransactionB64;
  private final String signingMessageB64;

  public ContractCallResponse(
      final boolean ok,
      final boolean submitted,
      final String dataspace,
      final String codeHashHex,
      final String abiHashHex,
      final long creationTimeMs,
      final String contractAddress,
      final String txHashHex,
      final String entrypoint,
      final String transactionScaffoldB64,
      final String signedTransactionB64,
      final String signingMessageB64) {
    this.ok = ok;
    this.submitted = submitted;
    this.dataspace = dataspace;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
    this.creationTimeMs = creationTimeMs;
    this.contractAddress = contractAddress;
    this.txHashHex = txHashHex;
    this.entrypoint = entrypoint;
    this.transactionScaffoldB64 = transactionScaffoldB64;
    this.signedTransactionB64 = signedTransactionB64;
    this.signingMessageB64 = signingMessageB64;
  }

  public boolean ok() { return ok; }
  public boolean submitted() { return submitted; }
  public String dataspace() { return dataspace; }
  public String codeHashHex() { return codeHashHex; }
  public String abiHashHex() { return abiHashHex; }
  public long creationTimeMs() { return creationTimeMs; }
  public String contractAddress() { return contractAddress; }
  public String txHashHex() { return txHashHex; }
  public String entrypoint() { return entrypoint; }
  public String transactionScaffoldB64() { return transactionScaffoldB64; }
  public String signedTransactionB64() { return signedTransactionB64; }
  public String signingMessageB64() { return signingMessageB64; }
}
