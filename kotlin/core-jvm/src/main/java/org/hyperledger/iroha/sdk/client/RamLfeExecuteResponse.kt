package org.hyperledger.iroha.sdk.client

/** Ciphertext and execution receipt; authenticated plaintext opening is a separate operation. */
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
) {
    @JvmField
    val receipt: Map<String, Any> = receipt.toMap()
}
