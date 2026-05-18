package org.hyperledger.iroha.sdk.client

/** Wire instruction skeleton returned by Soracloud app endpoints for external signing. */
data class SoracloudTxInstruction(
    @JvmField val wireId: String,
    @JvmField val payloadHex: String,
)

/** SoraFS-backed encrypted artifact reference used by private uploaded-model execution. */
data class SoracloudPrivateModelArtifactRef(
    @JvmField val schemaVersion: Long,
    @JvmField val sorafsManifestDigest: String,
    @JvmField val artifactHash: String,
    @JvmField val ciphertextBytes: Long,
    @JvmField val artifactRole: String,
)

/** Committed deterministic private uploaded-model execution receipt. */
data class SoracloudPrivateUploadedModelExecutionReceipt(
    @JvmField val schemaVersion: Long,
    @JvmField val receiptId: String,
    @JvmField val serviceName: String,
    @JvmField val modelId: String,
    @JvmField val weightVersion: String,
    @JvmField val runtimeVersion: String,
    @JvmField val modelManifestDigest: String,
    @JvmField val modelBundleRoot: String,
    @JvmField val policyId: String,
    @JvmField val inputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val inputCommitment: String,
    @JvmField val outputCommitment: String,
    @JvmField val requestCommitment: String,
    @JvmField val resultCommitment: String,
    @JvmField val emittedSequence: Long,
)

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
data class SoracloudPrivateUploadedModelExecuteResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val status: Map<String, Any?>,
    @JvmField val receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    @JvmField val txInstructions: List<SoracloudTxInstruction>,
) {
    /** Return the private receipt-recording instruction skeleton for external signing. */
    fun receiptInstruction(): SoracloudTxInstruction =
        SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(txInstructions)
}

/** Response emitted by `/v1/soracloud/model/upload/private/receipts`. */
data class SoracloudPrivateUploadedModelReceiptListResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val receipts: List<SoracloudPrivateUploadedModelExecutionReceipt>,
    @JvmField val total: Long?,
    @JvmField val returnedItems: Long,
    @JvmField val remainingItems: Long,
    @JvmField val hasMore: Boolean,
    @JvmField val countMode: String,
    @JvmField val continueCursor: String?,
)

