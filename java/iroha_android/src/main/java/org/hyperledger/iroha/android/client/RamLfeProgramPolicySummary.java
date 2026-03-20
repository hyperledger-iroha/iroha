package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Summary entry returned by `GET /v1/ram-lfe/program-policies`. */
public final class RamLfeProgramPolicySummary {
  private final String programId;
  private final String owner;
  private final boolean active;
  private final String resolverPublicKey;
  private final String backend;
  private final String verificationMode;
  private final String inputEncryption;
  private final String inputEncryptionPublicParameters;
  private final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded;
  private final String note;

  public RamLfeProgramPolicySummary(
      final String programId,
      final String owner,
      final boolean active,
      final String resolverPublicKey,
      final String backend,
      final String verificationMode,
      final String inputEncryption,
      final String inputEncryptionPublicParameters,
      final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded,
      final String note) {
    this.programId = Objects.requireNonNull(programId, "programId");
    this.owner = Objects.requireNonNull(owner, "owner");
    this.active = active;
    this.resolverPublicKey = Objects.requireNonNull(resolverPublicKey, "resolverPublicKey");
    this.backend = Objects.requireNonNull(backend, "backend");
    this.verificationMode = Objects.requireNonNull(verificationMode, "verificationMode");
    this.inputEncryption = inputEncryption;
    this.inputEncryptionPublicParameters = inputEncryptionPublicParameters;
    this.inputEncryptionPublicParametersDecoded = inputEncryptionPublicParametersDecoded;
    this.note = note;
  }

  public String programId() {
    return programId;
  }

  public String owner() {
    return owner;
  }

  public boolean active() {
    return active;
  }

  public String resolverPublicKey() {
    return resolverPublicKey;
  }

  public String backend() {
    return backend;
  }

  public String verificationMode() {
    return verificationMode;
  }

  public String inputEncryption() {
    return inputEncryption;
  }

  public String inputEncryptionPublicParameters() {
    return inputEncryptionPublicParameters;
  }

  public IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded() {
    return inputEncryptionPublicParametersDecoded;
  }

  public String note() {
    return note;
  }
}
