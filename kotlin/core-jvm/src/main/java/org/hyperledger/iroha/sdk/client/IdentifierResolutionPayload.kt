package org.hyperledger.iroha.sdk.client

/** Canonical payload covered by an identifier-resolution receipt signature. */
class IdentifierResolutionPayload(
    @JvmField val policyId: String,
    @JvmField val opaqueId: String,
    @JvmField val receiptHash: String,
    @JvmField val uaid: String,
    @JvmField val accountId: String,
    @JvmField val resolvedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
)
