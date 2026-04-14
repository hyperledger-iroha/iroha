package org.hyperledger.iroha.android.client;

/** Governance binding emitted by `GET /v1/gov/contracts/{contract_address}`. */
public final class GovernanceContractResponse {
  private final boolean found;
  private final String contractAddress;
  private final String dataspace;
  private final String codeHashHex;

  public GovernanceContractResponse(
      final boolean found,
      final String contractAddress,
      final String dataspace,
      final String codeHashHex) {
    this.found = found;
    this.contractAddress = contractAddress;
    this.dataspace = dataspace;
    this.codeHashHex = codeHashHex;
  }

  public boolean found() { return found; }
  public String contractAddress() { return contractAddress; }
  public String dataspace() { return dataspace; }
  public String codeHashHex() { return codeHashHex; }
}
