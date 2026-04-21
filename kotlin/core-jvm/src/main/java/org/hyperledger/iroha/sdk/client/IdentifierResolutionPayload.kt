package org.hyperledger.iroha.sdk.client

/** Canonical RAM-LFE execution payload nested inside an identifier-resolution receipt. */
class IdentifierResolutionExecutionPayload(
    @JvmField val programId: String,
    @JvmField val programDigest: String,
    @JvmField val backend: String,
    @JvmField val verificationMode: String,
    @JvmField val outputHash: String,
    @JvmField val associatedDataHash: String,
    @JvmField val executedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
)

/** Canonical payload covered by an identifier-resolution receipt attestation. */
class IdentifierResolutionPayload(
    @JvmField val policyId: String,
    @JvmField val execution: IdentifierResolutionExecutionPayload,
    @JvmField val opaqueId: String,
    @JvmField val receiptHash: String,
    @JvmField val uaid: String,
    @JvmField val accountId: String,
)
