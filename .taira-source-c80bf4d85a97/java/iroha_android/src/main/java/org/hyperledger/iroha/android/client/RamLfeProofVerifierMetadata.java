package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Proof verifier metadata returned by identifier and RAM-LFE policy summaries. */
public final class RamLfeProofVerifierMetadata {
  private final String proofBackend;
  private final String circuitId;
  private final String publicInputsSchemaHash;
  private final String verifyingKeyBytesB64;

  public RamLfeProofVerifierMetadata(
      final String proofBackend,
      final String circuitId,
      final String publicInputsSchemaHash,
      final String verifyingKeyBytesB64) {
    this.proofBackend = Objects.requireNonNull(proofBackend, "proofBackend");
    this.circuitId = Objects.requireNonNull(circuitId, "circuitId");
    this.publicInputsSchemaHash =
        Objects.requireNonNull(publicInputsSchemaHash, "publicInputsSchemaHash");
    this.verifyingKeyBytesB64 = Objects.requireNonNull(verifyingKeyBytesB64, "verifyingKeyBytesB64");
  }

  public String proofBackend() {
    return proofBackend;
  }

  public String circuitId() {
    return circuitId;
  }

  public String publicInputsSchemaHash() {
    return publicInputsSchemaHash;
  }

  public String verifyingKeyBytesB64() {
    return verifyingKeyBytesB64;
  }
}
