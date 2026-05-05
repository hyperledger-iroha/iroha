package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Canonical RAM-LFE execution payload nested inside an identifier-resolution receipt. */
public final class IdentifierResolutionExecutionPayload {
  private final String programId;
  private final String programDigest;
  private final String backend;
  private final String verificationMode;
  private final String outputHash;
  private final String associatedDataHash;
  private final long executedAtMs;
  private final Long expiresAtMs;

  public IdentifierResolutionExecutionPayload(
      final String programId,
      final String programDigest,
      final String backend,
      final String verificationMode,
      final String outputHash,
      final String associatedDataHash,
      final long executedAtMs,
      final Long expiresAtMs) {
    this.programId = Objects.requireNonNull(programId, "programId");
    this.programDigest = Objects.requireNonNull(programDigest, "programDigest");
    this.backend = Objects.requireNonNull(backend, "backend");
    this.verificationMode = Objects.requireNonNull(verificationMode, "verificationMode");
    this.outputHash = Objects.requireNonNull(outputHash, "outputHash");
    this.associatedDataHash = Objects.requireNonNull(associatedDataHash, "associatedDataHash");
    this.executedAtMs = executedAtMs;
    this.expiresAtMs = expiresAtMs;
  }

  public String programId() {
    return programId;
  }

  public String programDigest() {
    return programDigest;
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

  public long executedAtMs() {
    return executedAtMs;
  }

  public Long expiresAtMs() {
    return expiresAtMs;
  }
}
