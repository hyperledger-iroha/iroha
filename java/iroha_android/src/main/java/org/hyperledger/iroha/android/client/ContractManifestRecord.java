package org.hyperledger.iroha.android.client;

/** Full response from `GET /v1/contracts/code/{code_hash}`. */
public final class ContractManifestRecord {
  private final ContractManifest manifest;
  private final String codeHashHex;
  private final String abiHashHex;

  ContractManifestRecord(
      final ContractManifest manifest, final String codeHashHex, final String abiHashHex) {
    this.manifest = manifest;
    this.codeHashHex = codeHashHex;
    this.abiHashHex = abiHashHex;
  }

  public ContractManifest manifest() {
    return manifest;
  }

  public String codeHashHex() {
    return codeHashHex;
  }

  public String abiHashHex() {
    return abiHashHex;
  }
}
