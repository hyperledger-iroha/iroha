package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Committed deterministic private uploaded-model execution receipt. */
public final class SoracloudPrivateUploadedModelExecutionReceipt {
  private final long schemaVersion;
  private final String receiptId;
  private final String serviceName;
  private final String modelId;
  private final String weightVersion;
  private final String runtimeVersion;
  private final String modelManifestDigest;
  private final String modelBundleRoot;
  private final String policyId;
  private final SoracloudPrivateModelArtifactRef inputArtifact;
  private final SoracloudPrivateModelArtifactRef outputArtifact;
  private final String inputCommitment;
  private final String outputCommitment;
  private final String requestCommitment;
  private final String resultCommitment;
  private final long emittedSequence;

  public SoracloudPrivateUploadedModelExecutionReceipt(
      final long schemaVersion,
      final String receiptId,
      final String serviceName,
      final String modelId,
      final String weightVersion,
      final String runtimeVersion,
      final String modelManifestDigest,
      final String modelBundleRoot,
      final String policyId,
      final SoracloudPrivateModelArtifactRef inputArtifact,
      final SoracloudPrivateModelArtifactRef outputArtifact,
      final String inputCommitment,
      final String outputCommitment,
      final String requestCommitment,
      final String resultCommitment,
      final long emittedSequence) {
    this.schemaVersion = schemaVersion;
    this.receiptId = Objects.requireNonNull(receiptId, "receiptId");
    this.serviceName = Objects.requireNonNull(serviceName, "serviceName");
    this.modelId = Objects.requireNonNull(modelId, "modelId");
    this.weightVersion = Objects.requireNonNull(weightVersion, "weightVersion");
    this.runtimeVersion = Objects.requireNonNull(runtimeVersion, "runtimeVersion");
    this.modelManifestDigest = Objects.requireNonNull(modelManifestDigest, "modelManifestDigest");
    this.modelBundleRoot = Objects.requireNonNull(modelBundleRoot, "modelBundleRoot");
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.inputArtifact = Objects.requireNonNull(inputArtifact, "inputArtifact");
    this.outputArtifact = Objects.requireNonNull(outputArtifact, "outputArtifact");
    this.inputCommitment = Objects.requireNonNull(inputCommitment, "inputCommitment");
    this.outputCommitment = Objects.requireNonNull(outputCommitment, "outputCommitment");
    this.requestCommitment = Objects.requireNonNull(requestCommitment, "requestCommitment");
    this.resultCommitment = Objects.requireNonNull(resultCommitment, "resultCommitment");
    this.emittedSequence = emittedSequence;
  }

  public long schemaVersion() { return schemaVersion; }

  public String receiptId() { return receiptId; }

  public String serviceName() { return serviceName; }

  public String modelId() { return modelId; }

  public String weightVersion() { return weightVersion; }

  public String runtimeVersion() { return runtimeVersion; }

  public String modelManifestDigest() { return modelManifestDigest; }

  public String modelBundleRoot() { return modelBundleRoot; }

  public String policyId() { return policyId; }

  public SoracloudPrivateModelArtifactRef inputArtifact() { return inputArtifact; }

  public SoracloudPrivateModelArtifactRef outputArtifact() { return outputArtifact; }

  public String inputCommitment() { return inputCommitment; }

  public String outputCommitment() { return outputCommitment; }

  public String requestCommitment() { return requestCommitment; }

  public String resultCommitment() { return resultCommitment; }

  public long emittedSequence() { return emittedSequence; }
}

