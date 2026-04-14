package org.hyperledger.iroha.android.client;

/** Successful response emitted by `POST /v1/contracts/deploy`. */
public final class ContractDeployResponse {
  private final boolean ok;
  private final String contractAlias;
  private final String contractAddress;
  private final String previousContractAddress;
  private final boolean upgraded;
  private final String dataspace;
  private final Long deployNonce;
  private final String txHashHex;
  private final String codeHashHex;
  private final String abiHashHex;

  public ContractDeployResponse(
      final boolean ok,
      final String contractAlias,
      final String contractAddress,
      final String previousContractAddress,
      final boolean upgraded,
      final String dataspace,
      final Long deployNonce,
      final String txHashHex,
      final String codeHashHex,
      final String abiHashHex) {
    this.ok = ok;
    this.contractAlias = contractAlias;
    this.contractAddress = contractAddress;
    this.previousContractAddress = previousContractAddress;
    this.upgraded = upgraded;
    this.dataspace = dataspace;
    this.deployNonce = deployNonce;
    this.txHashHex = txHashHex;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
  }

  public boolean ok() { return ok; }
  public String contractAlias() { return contractAlias; }
  public String contractAddress() { return contractAddress; }
  public String previousContractAddress() { return previousContractAddress; }
  public boolean upgraded() { return upgraded; }
  public String dataspace() { return dataspace; }
  public Long deployNonce() { return deployNonce; }
  public String txHashHex() { return txHashHex; }
  public String codeHashHex() { return codeHashHex; }
  public String abiHashHex() { return abiHashHex; }
}
