package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Objects;

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
public final class SoracloudPrivateUploadedModelExecuteResponse {
  private final long schemaVersion;
  private final Map<String, Object> status;
  private final String submissionStatus;
  private final String transactionHash;
  private final SoracloudPrivateUploadedModelExecutionReceipt receipt;
  private final SoracloudPrivateModelArtifactRef outputArtifact;

  public SoracloudPrivateUploadedModelExecuteResponse(
      final long schemaVersion,
      final Map<String, Object> status,
      final String submissionStatus,
      final String transactionHash,
      final SoracloudPrivateUploadedModelExecutionReceipt receipt,
      final SoracloudPrivateModelArtifactRef outputArtifact) {
    SoracloudPrivateModelValidation.requireSchemaVersion(schemaVersion, "schemaVersion");
    this.schemaVersion = schemaVersion;
    this.status = SoracloudPrivateModelValidation.snapshotUploadedModelStatus(status);
    this.submissionStatus =
        SoracloudPrivateModelValidation.requireSubmissionStatus(
            submissionStatus, "submissionStatus");
    this.transactionHash =
        transactionHash == null
            ? null
            : SoracloudPrivateModelValidation.requireSoracloudHash(
                transactionHash, "transactionHash");
    this.receipt = Objects.requireNonNull(receipt, "receipt");
    this.outputArtifact = Objects.requireNonNull(outputArtifact, "outputArtifact");
    SoracloudPrivateModelValidation.requireExecuteResponseState(
        this.submissionStatus, this.transactionHash, this.receipt, this.outputArtifact);
  }

  public long schemaVersion() { return schemaVersion; }

  public Map<String, Object> status() { return status; }

  public String submissionStatus() { return submissionStatus; }

  /** Current phase transaction hash when signed; absent while awaiting output durability or committed. */
  public String transactionHash() { return transactionHash; }

  public SoracloudPrivateUploadedModelExecutionReceipt receipt() { return receipt; }

  public SoracloudPrivateModelArtifactRef outputArtifact() { return outputArtifact; }
}
