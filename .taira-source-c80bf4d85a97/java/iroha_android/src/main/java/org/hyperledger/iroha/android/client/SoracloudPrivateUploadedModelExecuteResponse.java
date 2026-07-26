package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
public final class SoracloudPrivateUploadedModelExecuteResponse {
  private final long schemaVersion;
  private final Map<String, Object> status;
  private final SoracloudPrivateUploadedModelExecutionReceipt receipt;
  private final List<SoracloudTxInstruction> txInstructions;

  public SoracloudPrivateUploadedModelExecuteResponse(
      final long schemaVersion,
      final Map<String, Object> status,
      final SoracloudPrivateUploadedModelExecutionReceipt receipt,
      final List<SoracloudTxInstruction> txInstructions) {
    this.schemaVersion = schemaVersion;
    this.status = Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(status, "status")));
    this.receipt = Objects.requireNonNull(receipt, "receipt");
    this.txInstructions =
        Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(txInstructions, "txInstructions")));
  }

  public long schemaVersion() { return schemaVersion; }

  public Map<String, Object> status() { return status; }

  public SoracloudPrivateUploadedModelExecutionReceipt receipt() { return receipt; }

  public List<SoracloudTxInstruction> txInstructions() { return txInstructions; }

  public SoracloudTxInstruction receiptInstruction() {
    return SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(txInstructions);
  }
}

