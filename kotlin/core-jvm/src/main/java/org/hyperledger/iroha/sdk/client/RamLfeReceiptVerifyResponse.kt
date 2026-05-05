package org.hyperledger.iroha.sdk.client

/** Stateless result emitted by `POST /v1/ram-lfe/receipts/verify`. */
class RamLfeReceiptVerifyResponse(
    @JvmField val valid: Boolean,
    @JvmField val programId: String,
    @JvmField val backend: String,
    @JvmField val verificationMode: String,
    @JvmField val outputHash: String,
    @JvmField val associatedDataHash: String,
    @JvmField val outputHashMatches: Boolean?,
    @JvmField val error: String?,
)
