package org.hyperledger.iroha.sdk.client

/** Successful response emitted by `POST /v1/ram-lfe/programs/{program_id}/execute`. */
class RamLfeExecuteResponse(
    @JvmField val programId: String,
    @JvmField val opaqueHash: String,
    @JvmField val receiptHash: String,
    @JvmField val outputHex: String,
    @JvmField val outputHash: String,
    @JvmField val associatedDataHash: String,
    @JvmField val executedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
    @JvmField val backend: String,
    @JvmField val verificationMode: String,
    receipt: Map<String, Any>,
) {
    @JvmField
    val receipt: Map<String, Any> = receipt.toMap()
}
