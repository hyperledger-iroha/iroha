package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Summary entry returned by `GET /v1/identifier-policies`. */
public final class IdentifierPolicySummary {
  private final String policyId;
  private final String programId;
  private final String owner;
  private final boolean active;
  private final IdentifierNormalization normalization;
  private final String resolverPublicKey;
  private final String outputOpeningPublicKey;
  private final String backend;
  private final String inputEncryption;
  private final String inputEncryptionPublicParameters;
  private final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded;
  private final RamLfeProofVerifierMetadata proofVerifier;
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
      final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded,
      final String note) {
    this(
        policyId,
        programIdFromPolicyId(policyId),
        owner,
        active,
        normalization,
        resolverPublicKey,
        resolverPublicKey,
        backend,
        inputEncryption,
        inputEncryptionPublicParameters,
        inputEncryptionPublicParametersDecoded,
        note,
        null);
  }

  public IdentifierPolicySummary(
      final String policyId,
      final String owner,
      final boolean active,
      final IdentifierNormalization normalization,
      final String resolverPublicKey,
      final String backend,
      final String inputEncryption,
      final String inputEncryptionPublicParameters,
      final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded,
      final String note,
      final RamLfeProofVerifierMetadata proofVerifier) {
    this(
        policyId,
        programIdFromPolicyId(policyId),
        owner,
        active,
        normalization,
        resolverPublicKey,
        resolverPublicKey,
        backend,
        inputEncryption,
        inputEncryptionPublicParameters,
        inputEncryptionPublicParametersDecoded,
        note,
        proofVerifier);
  }

  public IdentifierPolicySummary(
      final String policyId,
      final String programId,
      final String owner,
      final boolean active,
      final IdentifierNormalization normalization,
      final String resolverPublicKey,
      final String outputOpeningPublicKey,
      final String backend,
      final String inputEncryption,
      final String inputEncryptionPublicParameters,
      final IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded,
      final String note,
      final RamLfeProofVerifierMetadata proofVerifier) {
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.programId = Objects.requireNonNull(programId, "programId");
    this.owner = Objects.requireNonNull(owner, "owner");
    this.active = active;
    this.normalization = Objects.requireNonNull(normalization, "normalization");
    this.resolverPublicKey = Objects.requireNonNull(resolverPublicKey, "resolverPublicKey");
    this.outputOpeningPublicKey =
        Objects.requireNonNull(outputOpeningPublicKey, "outputOpeningPublicKey");
    this.backend = Objects.requireNonNull(backend, "backend");
    this.inputEncryption = inputEncryption;
    this.inputEncryptionPublicParameters = inputEncryptionPublicParameters;
    this.inputEncryptionPublicParametersDecoded = inputEncryptionPublicParametersDecoded;
    this.proofVerifier = proofVerifier;
    this.note = note;
  }

  public String policyId() {
    return policyId;
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

  public IdentifierNormalization normalization() {
    return normalization;
  }

  public String resolverPublicKey() {
    return resolverPublicKey;
  }

  public String outputOpeningPublicKey() {
    return outputOpeningPublicKey;
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

  public IdentifierBfvPublicParameters inputEncryptionPublicParametersDecoded() {
    return inputEncryptionPublicParametersDecoded;
  }

  public RamLfeProofVerifierMetadata proofVerifier() {
    return proofVerifier;
  }

  public String note() {
    return note;
  }

  public IdentifierResolveRequest encryptedRequest(
      final String encryptedInputHex, final RamLfeOutputOpening outputOpening) {
    return IdentifierResolveRequest.encrypted(this, encryptedInputHex, outputOpening);
  }

  public String encryptInput(final String input) {
    return IdentifierBfvEnvelopeBuilder.encrypt(this, input, null);
  }

  public String encryptInput(final String input, final byte[] seed) {
    return IdentifierBfvEnvelopeBuilder.encrypt(this, input, seed);
  }

  public IdentifierResolveRequest encryptedRequestFromInput(
      final String input, final RamLfeOutputOpening outputOpening) {
    return IdentifierResolveRequest.encryptedFromInput(this, input, outputOpening);
  }

  public IdentifierResolveRequest encryptedRequestFromInput(
      final String input, final RamLfeOutputOpening outputOpening, final byte[] seed) {
    return IdentifierResolveRequest.encryptedFromInput(this, input, outputOpening, seed);
  }

  private static String programIdFromPolicyId(final String policyId) {
    return Objects.requireNonNull(policyId, "policyId").trim().replace('#', '_');
  }
}
