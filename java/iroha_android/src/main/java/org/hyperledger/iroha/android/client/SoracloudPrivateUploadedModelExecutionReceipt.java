package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.util.Objects;

/**
 * Deterministic private uploaded-model receipt; coordinates are zero for submission and positive
 * once committed.
 */
public final class SoracloudPrivateUploadedModelExecutionReceipt {
  private final long schemaVersion;
  private final String networkId;
  private final String receiptId;
  private final String serviceName;
  private final String serviceVersion;
  private final String modelId;
  private final String weightVersion;
  private final String runtimeVersion;
  private final byte[] modelManifestDigest;
  private final String modelBundleRoot;
  private final String policyId;
  private final String decryptionRequestId;
  private final SoracloudRuntimeDeterministicValidatorHost attestingValidator;
  private final SoracloudPrivateModelArtifactRef inputArtifact;
  private final SoracloudPrivateModelArtifactRef outputArtifact;
  private final byte[] outputReplicationOrderId;
  private final String inputCommitment;
  private final String outputCommitment;
  private final SoracloudUploadedModelEncryptionRecipient outputRecipient;
  private final String requestCommitment;
  private final String resultCommitment;
  private final BigInteger authorizationClaimBlockHeight;
  private final BigInteger authorizationClaimEpoch;
  private final BigInteger emittedSequence;
  private final BigInteger emittedBlockHeight;
  private final BigInteger emittedEpoch;

  public SoracloudPrivateUploadedModelExecutionReceipt(
      final long schemaVersion,
      final String networkId,
      final String receiptId,
      final String serviceName,
      final String serviceVersion,
      final String modelId,
      final String weightVersion,
      final String runtimeVersion,
      final byte[] modelManifestDigest,
      final String modelBundleRoot,
      final String policyId,
      final String decryptionRequestId,
      final SoracloudRuntimeDeterministicValidatorHost attestingValidator,
      final SoracloudPrivateModelArtifactRef inputArtifact,
      final SoracloudPrivateModelArtifactRef outputArtifact,
      final byte[] outputReplicationOrderId,
      final String inputCommitment,
      final String outputCommitment,
      final SoracloudUploadedModelEncryptionRecipient outputRecipient,
      final String requestCommitment,
      final String resultCommitment,
      final BigInteger authorizationClaimBlockHeight,
      final BigInteger authorizationClaimEpoch,
      final BigInteger emittedSequence,
      final BigInteger emittedBlockHeight,
      final BigInteger emittedEpoch) {
    SoracloudPrivateModelValidation.requireSchemaVersion(schemaVersion, "schemaVersion");
    this.schemaVersion = schemaVersion;
    this.networkId = SoracloudPrivateModelValidation.requireNetworkId(networkId, "networkId");
    this.receiptId =
        SoracloudPrivateModelValidation.requireSoracloudHash(receiptId, "receiptId");
    this.serviceName =
        SoracloudPrivateModelValidation.requireCanonicalName(serviceName, "serviceName");
    this.serviceVersion =
        SoracloudPrivateModelValidation.requireServiceVersion(serviceVersion, "serviceVersion");
    this.modelId = SoracloudPrivateModelValidation.requireIdentifier(modelId, "modelId");
    this.weightVersion =
        SoracloudPrivateModelValidation.requireIdentifier(weightVersion, "weightVersion");
    this.runtimeVersion = canonicalString(runtimeVersion, "runtimeVersion");
    if (!SoracloudPrivateModelValidation.RUNTIME_VERSION_V1.equals(this.runtimeVersion)) {
      throw new IllegalArgumentException(
          "runtimeVersion must equal " + SoracloudPrivateModelValidation.RUNTIME_VERSION_V1);
    }
    this.modelManifestDigest = canonicalManifestDigest(modelManifestDigest);
    this.modelBundleRoot =
        SoracloudPrivateModelValidation.requireSoracloudHash(
            modelBundleRoot, "modelBundleRoot");
    this.policyId = canonicalString(policyId, "policyId");
    this.decryptionRequestId = canonicalString(decryptionRequestId, "decryptionRequestId");
    this.attestingValidator = Objects.requireNonNull(attestingValidator, "attestingValidator");
    this.inputArtifact = Objects.requireNonNull(inputArtifact, "inputArtifact");
    this.outputArtifact = Objects.requireNonNull(outputArtifact, "outputArtifact");
    if (!"input".equals(this.inputArtifact.artifactRole())) {
      throw new IllegalArgumentException("inputArtifact.artifactRole must equal input");
    }
    if (!"output".equals(this.outputArtifact.artifactRole())) {
      throw new IllegalArgumentException("outputArtifact.artifactRole must equal output");
    }
    if (this.inputArtifact.artifactHash().equals(this.outputArtifact.artifactHash())) {
      throw new IllegalArgumentException(
          "outputArtifact.artifactHash must differ from inputArtifact.artifactHash");
    }
    this.outputReplicationOrderId =
        SoracloudPrivateModelValidation.requireSorafsAutoReplicationOrderIdV1(
            outputReplicationOrderId,
            this.outputArtifact.sorafsManifestDigest(),
            "outputReplicationOrderId");
    this.inputCommitment =
        SoracloudPrivateModelValidation.requireSoracloudHash(
            inputCommitment, "inputCommitment");
    this.outputCommitment =
        SoracloudPrivateModelValidation.requireSoracloudHash(
            outputCommitment, "outputCommitment");
    this.outputRecipient = Objects.requireNonNull(outputRecipient, "outputRecipient");
    this.requestCommitment =
        SoracloudPrivateModelValidation.requireSoracloudHash(
            requestCommitment, "requestCommitment");
    this.resultCommitment =
        SoracloudPrivateModelValidation.requireSoracloudHash(
            resultCommitment, "resultCommitment");
    SoracloudPrivateModelValidation.requireLedgerCoordinates(
        authorizationClaimBlockHeight,
        authorizationClaimEpoch,
        emittedSequence,
        emittedBlockHeight,
        emittedEpoch);
    this.authorizationClaimBlockHeight = authorizationClaimBlockHeight;
    this.authorizationClaimEpoch = authorizationClaimEpoch;
    this.emittedSequence = emittedSequence;
    this.emittedBlockHeight = emittedBlockHeight;
    this.emittedEpoch = emittedEpoch;
  }

  public long schemaVersion() { return schemaVersion; }

  public String networkId() { return networkId; }

  public String receiptId() { return receiptId; }

  public String serviceName() { return serviceName; }

  public String serviceVersion() { return serviceVersion; }

  public String modelId() { return modelId; }

  public String weightVersion() { return weightVersion; }

  public String runtimeVersion() { return runtimeVersion; }

  public byte[] modelManifestDigest() { return modelManifestDigest.clone(); }

  public String modelBundleRoot() { return modelBundleRoot; }

  public String policyId() { return policyId; }

  public String decryptionRequestId() { return decryptionRequestId; }

  public SoracloudRuntimeDeterministicValidatorHost attestingValidator() {
    return attestingValidator;
  }

  public SoracloudPrivateModelArtifactRef inputArtifact() { return inputArtifact; }

  public SoracloudPrivateModelArtifactRef outputArtifact() { return outputArtifact; }

  /** Return a defensive copy of the deterministic SoraFS replication-order identifier. */
  public byte[] outputReplicationOrderId() { return outputReplicationOrderId.clone(); }

  public String inputCommitment() { return inputCommitment; }

  public String outputCommitment() { return outputCommitment; }

  public SoracloudUploadedModelEncryptionRecipient outputRecipient() { return outputRecipient; }

  public String requestCommitment() { return requestCommitment; }

  public String resultCommitment() { return resultCommitment; }

  /** Unsigned 64-bit block height at which consensus froze the execution authorization. */
  public BigInteger authorizationClaimBlockHeight() { return authorizationClaimBlockHeight; }

  /** Unsigned 64-bit consensus Unix-seconds epoch at which authorization was frozen. */
  public BigInteger authorizationClaimEpoch() { return authorizationClaimEpoch; }

  /** Unsigned 64-bit ledger sequence, represented without signed narrowing. */
  public BigInteger emittedSequence() { return emittedSequence; }

  /** Unsigned 64-bit block height, represented without signed narrowing. */
  public BigInteger emittedBlockHeight() { return emittedBlockHeight; }

  /** Unsigned 64-bit consensus Unix-seconds epoch, represented without signed narrowing. */
  public BigInteger emittedEpoch() { return emittedEpoch; }

  private static byte[] canonicalManifestDigest(final byte[] value) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException("modelManifestDigest must contain exactly 32 bytes");
    }
    return value.clone();
  }

  private static String canonicalString(final String value, final String field) {
    return SoracloudPrivateModelValidation.requireCanonicalString(value, field);
  }
}
