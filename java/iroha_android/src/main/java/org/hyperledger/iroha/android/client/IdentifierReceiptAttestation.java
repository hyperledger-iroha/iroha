package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Explicit attestation attached to an identifier-resolution receipt payload. */
public final class IdentifierReceiptAttestation {
  private final String kind;
  private final String signature;
  private final String proofBackend;
  private final String proofB64;

  public IdentifierReceiptAttestation(
      final String kind,
      final String signature,
      final String proofBackend,
      final String proofB64) {
    this.kind = Objects.requireNonNull(kind, "kind");
    this.signature = signature;
    this.proofBackend = proofBackend;
    this.proofB64 = proofB64;
  }

  public String kind() {
    return kind;
  }

  public String signature() {
    return signature;
  }

  public String proofBackend() {
    return proofBackend;
  }

  public String proofB64() {
    return proofB64;
  }
}
