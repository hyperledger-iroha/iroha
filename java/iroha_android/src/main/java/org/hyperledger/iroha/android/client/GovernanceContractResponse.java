package org.hyperledger.iroha.android.client;

/** Governance binding emitted by `GET /v1/gov/contracts/{contract_address}`. */
public final class GovernanceContractResponse {
  private final boolean found;
  private final String contractAddress;
  private final String contractSubjectAccount;
  private final String dataspace;
  private final Boolean active;
  private final GovernanceContractLifecycle lifecycle;
  private final Boolean emergencyHoldActive;
  private final String codeHashHex;
  private final String abiHashHex;
  private final java.util.List<String> publicEntrypoints;

  public GovernanceContractResponse(
      final boolean found,
      final String contractAddress,
      final String contractSubjectAccount,
      final String dataspace,
      final Boolean active,
      final GovernanceContractLifecycle lifecycle,
      final Boolean emergencyHoldActive,
      final String codeHashHex,
      final String abiHashHex,
      final java.util.List<String> publicEntrypoints) {
    this.found = found;
    this.contractAddress = contractAddress;
    this.contractSubjectAccount = contractSubjectAccount;
    this.dataspace = dataspace;
    this.active = active;
    this.lifecycle = lifecycle;
    this.emergencyHoldActive = emergencyHoldActive;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
    this.publicEntrypoints = publicEntrypoints == null
        ? null
        : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(publicEntrypoints));
  }

  public boolean found() { return found; }
  public String contractAddress() { return contractAddress; }
  public String contractSubjectAccount() { return contractSubjectAccount; }
  public String dataspace() { return dataspace; }
  public Boolean active() { return active; }
  public GovernanceContractLifecycle lifecycle() { return lifecycle; }
  public Boolean emergencyHoldActive() { return emergencyHoldActive; }
  public String codeHashHex() { return codeHashHex; }
  public String abiHashHex() { return abiHashHex; }
  public java.util.List<String> publicEntrypoints() { return publicEntrypoints; }
}
