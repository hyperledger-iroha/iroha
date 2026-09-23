package org.hyperledger.iroha.sdk.client

/** Successful response emitted by `POST /v1/ram-lfe/programs/{program_id}/execute`. */
class RamLfeExecuteResponse(
    @JvmField val programId: String,
    @JvmField val opaqueHash: String,
    @JvmField val receiptHash: String,
    @JvmField val outputCiphertext: String,
    @JvmField val outputHash: String,
    @JvmField val associatedDataHash: String,
    @JvmField val executedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
    @JvmField val backend: String,
    @JvmField val verificationMode: String,
    receipt: Map<String, Any>,
    @JvmField val outputOpening: RamLfeOutputOpening,
) {
    init {
        require(outputOpening.payload.programId == programId) {
            "RAM-LFE output opening program does not match execution"
        }
        require(outputOpening.payload.openedOutputHash == outputHash) {
            "RAM-LFE output opening hash does not match execution"
        }
        require(outputOpening.payload.openedAtMs == executedAtMs) {
            "RAM-LFE output opening time does not match execution"
        }
        require(outputOpening.payload.expiresAtMs == expiresAtMs) {
            "RAM-LFE output opening expiry does not match execution"
        }
    }
    @JvmField
    val receipt: Map<String, Any> = receipt.toMap()
}
