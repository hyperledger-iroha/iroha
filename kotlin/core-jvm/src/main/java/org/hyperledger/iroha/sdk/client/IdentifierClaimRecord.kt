package org.hyperledger.iroha.sdk.client

/** Persisted identifier-claim binding returned by receipt-hash lookup. */
class IdentifierClaimRecord(
    @JvmField val policyId: String,
    @JvmField val opaqueId: String,
    @JvmField val receiptHash: String,
    @JvmField val uaid: String,
    @JvmField val accountId: String,
    @JvmField val verifiedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
)
