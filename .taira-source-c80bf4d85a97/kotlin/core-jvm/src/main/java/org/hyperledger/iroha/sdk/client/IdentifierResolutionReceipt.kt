package org.hyperledger.iroha.sdk.client

/** Resolution receipt returned by identifier resolve and claim-receipt endpoints. */
class IdentifierReceiptAttestation(
    @JvmField val kind: String,
    @JvmField val signature: String?,
    @JvmField val proofBackend: String?,
    @JvmField val proofB64: String?,
)

/** Resolution receipt returned by identifier resolve and claim-receipt endpoints. */
class IdentifierResolutionReceipt(
    @JvmField val payload: IdentifierResolutionPayload,
    @JvmField val attestation: IdentifierReceiptAttestation,
) {
    val policyId: String get() = payload.policyId
    val opaqueId: String get() = payload.opaqueId
    val receiptHash: String get() = payload.receiptHash
    val uaid: String get() = payload.uaid
    val accountId: String get() = payload.accountId
    val resolvedAtMs: Long get() = payload.execution.executedAtMs
    val expiresAtMs: Long? get() = payload.execution.expiresAtMs
    val backend: String get() = payload.execution.backend

    fun verifyAttestation(policy: IdentifierPolicySummary): Boolean =
        IdentifierReceiptVerifier.verify(this, policy)
}
