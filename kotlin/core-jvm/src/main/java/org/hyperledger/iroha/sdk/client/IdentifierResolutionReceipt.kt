package org.hyperledger.iroha.sdk.client

/** Resolution receipt returned by identifier resolve and claim-receipt endpoints. */
class IdentifierResolutionReceipt(
    @JvmField val policyId: String,
    @JvmField val opaqueId: String,
    @JvmField val receiptHash: String,
    @JvmField val uaid: String,
    @JvmField val accountId: String,
    @JvmField val resolvedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
    @JvmField val backend: String,
    @JvmField val signature: String,
    @JvmField val signaturePayloadHex: String,
    @JvmField val signaturePayload: IdentifierResolutionPayload,
) {
    fun verifySignature(policy: IdentifierPolicySummary): Boolean =
        IdentifierReceiptVerifier.verify(this, policy)
}
