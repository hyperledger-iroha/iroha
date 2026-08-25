package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** SoraFS-backed encrypted artifact reference used by private uploaded-model execution. */
data class SoracloudPrivateModelArtifactRef(
    @JvmField val schemaVersion: Long,
    @JvmField val sorafsManifestDigest: String,
    @JvmField val sorafsRootCid: List<Int>,
    @JvmField val artifactHash: String,
    @JvmField val ciphertextBytes: Long,
    @JvmField val artifactRole: String,
) {
    init {
        require(sorafsRootCid.size == 36) { "sorafsRootCid must contain exactly 36 bytes" }
        require(sorafsRootCid.all { it in 0..255 }) {
            "sorafsRootCid elements must be unsigned bytes"
        }
        require(sorafsRootCid.subList(0, 4) == listOf(1, 0x71, 0x1f, 32)) {
            "sorafsRootCid must use canonical CIDv1/dag-cbor/BLAKE3-256 framing"
        }
        require(sorafsRootCid.subList(4, 36).any { it != 0 }) {
            "sorafsRootCid digest must be nonzero"
        }
    }
}

/** Exact active validator that attested a deterministic private execution receipt. */
data class SoracloudRuntimeDeterministicValidatorHost(
    @JvmField val laneId: Long,
    @JvmField val validatorAccountId: String,
    @JvmField val peerId: String,
)

/** Public key metadata to which a private execution output is encrypted. */
data class SoracloudUploadedModelEncryptionRecipient(
    @JvmField val schemaVersion: Long,
    @JvmField val keyId: String,
    @JvmField val keyVersion: Long,
    @JvmField val kem: String,
    @JvmField val aead: String,
    @JvmField val publicKeyBytesBase64: String,
    @JvmField val publicKeyFingerprint: String,
) {
    /** Return a fresh decoded copy of the recipient public key. */
    fun publicKeyBytes(): ByteArray = Base64.getDecoder().decode(publicKeyBytesBase64)
}

/** Committed deterministic private uploaded-model execution receipt. */
data class SoracloudPrivateUploadedModelExecutionReceipt(
    @JvmField val schemaVersion: Long,
    @JvmField val networkId: String,
    @JvmField val receiptId: String,
    @JvmField val serviceName: String,
    @JvmField val serviceVersion: String,
    @JvmField val modelId: String,
    @JvmField val weightVersion: String,
    @JvmField val runtimeVersion: String,
    @JvmField val modelManifestDigest: String,
    @JvmField val modelBundleRoot: String,
    @JvmField val policyId: String,
    @JvmField val decryptionRequestId: String,
    @JvmField val attestingValidator: SoracloudRuntimeDeterministicValidatorHost,
    @JvmField val inputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val inputCommitment: String,
    @JvmField val outputCommitment: String,
    @JvmField val outputRecipient: SoracloudUploadedModelEncryptionRecipient,
    @JvmField val requestCommitment: String,
    @JvmField val resultCommitment: String,
    @JvmField val emittedSequence: Long,
    @JvmField val emittedBlockHeight: Long,
) {
    init {
        require(NetworkId.parse(networkId).literal == networkId) {
            "networkId must be an exact canonical checksummed 32-byte NetworkId literal"
        }
    }
}

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
data class SoracloudPrivateUploadedModelExecuteResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val status: Map<String, Any?>,
    @JvmField val submissionStatus: String,
    @JvmField val transactionHash: String?,
    @JvmField val receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
)

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
