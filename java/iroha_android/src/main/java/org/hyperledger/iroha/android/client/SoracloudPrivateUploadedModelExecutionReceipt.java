package org.hyperledger.iroha.android.client;

import java.util.Objects;
import org.hyperledger.iroha.android.model.NetworkId;

/** Committed deterministic private uploaded-model execution receipt. */
public final class SoracloudPrivateUploadedModelExecutionReceipt {
  private final long schemaVersion;
  private final String networkId;
  private final String receiptId;
  private final String serviceName;
  private final String serviceVersion;
  private final String modelId;
  private final String weightVersion;
  private final String runtimeVersion;
  private final String modelManifestDigest;
  private final String modelBundleRoot;
  private final String policyId;
  private final String decryptionRequestId;
  private final SoracloudRuntimeDeterministicValidatorHost attestingValidator;
  private final SoracloudPrivateModelArtifactRef inputArtifact;
  private final SoracloudPrivateModelArtifactRef outputArtifact;
  private final String inputCommitment;
  private final String outputCommitment;
  private final SoracloudUploadedModelEncryptionRecipient outputRecipient;
  private final String requestCommitment;
  private final String resultCommitment;
  private final long emittedSequence;
  private final long emittedBlockHeight;

  public SoracloudPrivateUploadedModelExecutionReceipt(
      final long schemaVersion,
      final String networkId,
      final String receiptId,
      final String serviceName,
      final String serviceVersion,
      final String modelId,
      final String weightVersion,
      final String runtimeVersion,
      final String modelManifestDigest,
      final String modelBundleRoot,
      final String policyId,
      final String decryptionRequestId,
      final SoracloudRuntimeDeterministicValidatorHost attestingValidator,
      final SoracloudPrivateModelArtifactRef inputArtifact,
      final SoracloudPrivateModelArtifactRef outputArtifact,
      final String inputCommitment,
      final String outputCommitment,
      final SoracloudUploadedModelEncryptionRecipient outputRecipient,
      final String requestCommitment,
      final String resultCommitment,
      final long emittedSequence,
      final long emittedBlockHeight) {
    this.schemaVersion = schemaVersion;
    this.networkId =
        NetworkId.parse(Objects.requireNonNull(networkId, "networkId")).literal();
    this.receiptId = Objects.requireNonNull(receiptId, "receiptId");
    this.serviceName = Objects.requireNonNull(serviceName, "serviceName");
    this.serviceVersion = Objects.requireNonNull(serviceVersion, "serviceVersion");
    this.modelId = Objects.requireNonNull(modelId, "modelId");
    this.weightVersion = Objects.requireNonNull(weightVersion, "weightVersion");
    this.runtimeVersion = Objects.requireNonNull(runtimeVersion, "runtimeVersion");
    this.modelManifestDigest = Objects.requireNonNull(modelManifestDigest, "modelManifestDigest");
    this.modelBundleRoot = Objects.requireNonNull(modelBundleRoot, "modelBundleRoot");
    this.policyId = Objects.requireNonNull(policyId, "policyId");
    this.decryptionRequestId =
        Objects.requireNonNull(decryptionRequestId, "decryptionRequestId");
    this.attestingValidator = Objects.requireNonNull(attestingValidator, "attestingValidator");
    this.inputArtifact = Objects.requireNonNull(inputArtifact, "inputArtifact");
    this.outputArtifact = Objects.requireNonNull(outputArtifact, "outputArtifact");
    this.inputCommitment = Objects.requireNonNull(inputCommitment, "inputCommitment");
    this.outputCommitment = Objects.requireNonNull(outputCommitment, "outputCommitment");
    this.outputRecipient = Objects.requireNonNull(outputRecipient, "outputRecipient");
    this.requestCommitment = Objects.requireNonNull(requestCommitment, "requestCommitment");
    this.resultCommitment = Objects.requireNonNull(resultCommitment, "resultCommitment");
    this.emittedSequence = emittedSequence;
    this.emittedBlockHeight = emittedBlockHeight;
  }

  public long schemaVersion() { return schemaVersion; }

  public String networkId() { return networkId; }

  public String receiptId() { return receiptId; }

  public String serviceName() { return serviceName; }

  public String serviceVersion() { return serviceVersion; }

  public String modelId() { return modelId; }

  public String weightVersion() { return weightVersion; }

  public String runtimeVersion() { return runtimeVersion; }

  public String modelManifestDigest() { return modelManifestDigest; }

  public String modelBundleRoot() { return modelBundleRoot; }

  public String policyId() { return policyId; }

  public String decryptionRequestId() { return decryptionRequestId; }

  public SoracloudRuntimeDeterministicValidatorHost attestingValidator() {
    return attestingValidator;
  }

  public SoracloudPrivateModelArtifactRef inputArtifact() { return inputArtifact; }

  public SoracloudPrivateModelArtifactRef outputArtifact() { return outputArtifact; }

  public String inputCommitment() { return inputCommitment; }

  public String outputCommitment() { return outputCommitment; }

  public SoracloudUploadedModelEncryptionRecipient outputRecipient() { return outputRecipient; }

  public String requestCommitment() { return requestCommitment; }

  public String resultCommitment() { return resultCommitment; }

  public long emittedSequence() { return emittedSequence; }

  public long emittedBlockHeight() { return emittedBlockHeight; }
}
