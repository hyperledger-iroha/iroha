package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Summary entry returned by `GET /v1/identifier-policies`. */
public final class IdentifierPolicySummary {
  private final String policyId;
  private final String owner;
  private final boolean active;
  private final IdentifierNormalization normalization;
  private final String resolverPublicKey;
  private final String backend;
  private final String inputEncryption;
  private final String inputEncryptionPublicParameters;
  private final String note;

  public IdentifierPolicySummary(
      final String policyId,
      final String owner,
      final boolean active,
      final IdentifierNormalization normalization,
      final String resolverPublicKey,
      final String backend,
      final String inputEncryption,
      final String inputEncryptionPublicParameters,
      final String note) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.owner = Objects.requireNonNull(owner, "owner");
    this.active = active;
    this.normalization = Objects.requireNonNull(normalization, "normalization");
    this.resolverPublicKey = Objects.requireNonNull(resolverPublicKey, "resolverPublicKey");
    this.backend = Objects.requireNonNull(backend, "backend");
    this.inputEncryption = inputEncryption;
    this.inputEncryptionPublicParameters = inputEncryptionPublicParameters;
    this.note = note;
  }

  public String policyId() {
    return policyId;
  }

  public String owner() {
    return owner;
  }

  public boolean active() {
    return active;
  }

  public IdentifierNormalization normalization() {
    return normalization;
  }

  public String resolverPublicKey() {
    return resolverPublicKey;
  }

  public String backend() {
    return backend;
  }

  public String inputEncryption() {
    return inputEncryption;
  }

  public String inputEncryptionPublicParameters() {
    return inputEncryptionPublicParameters;
  }

  public String note() {
    return note;
  }
}
