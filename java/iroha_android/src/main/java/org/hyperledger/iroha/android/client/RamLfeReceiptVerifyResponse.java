package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Stateless result emitted by `POST /v1/ram-lfe/receipts/verify`. */
public final class RamLfeReceiptVerifyResponse {
  private final boolean valid;
  private final String programId;
  private final String backend;
  private final String verificationMode;
  private final String outputHash;
  private final String associatedDataHash;
  private final Boolean outputHashMatches;
  private final String error;

  public RamLfeReceiptVerifyResponse(
      final boolean valid,
      final String programId,
      final String backend,
      final String verificationMode,
      final String outputHash,
      final String associatedDataHash,
      final Boolean outputHashMatches,
      final String error) {
    this.valid = valid;
    this.programId = Objects.requireNonNull(programId, "programId");
    this.backend = Objects.requireNonNull(backend, "backend");
    this.verificationMode = Objects.requireNonNull(verificationMode, "verificationMode");
    this.outputHash = Objects.requireNonNull(outputHash, "outputHash");
    this.associatedDataHash = Objects.requireNonNull(associatedDataHash, "associatedDataHash");
    this.outputHashMatches = outputHashMatches;
    this.error = error;
  }

  public boolean valid() {
    return valid;
  }

  public String programId() {
    return programId;
  }

  public String backend() {
    return backend;
  }

  public String verificationMode() {
    return verificationMode;
  }

  public String outputHash() {
    return outputHash;
  }

  public String associatedDataHash() {
    return associatedDataHash;
  }

  public Boolean outputHashMatches() {
    return outputHashMatches;
  }

  public String error() {
    return error;
  }
}
