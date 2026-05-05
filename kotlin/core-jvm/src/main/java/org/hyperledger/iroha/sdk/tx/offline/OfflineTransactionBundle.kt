package org.hyperledger.iroha.sdk.tx.offline

/**
 * Convenience container that bundles an [OfflineSigningEnvelope] together with optional
 * attestation material. SDK helpers return this when callers request an offline transaction that
 * should be paired with StrongBox/TEE attestation data.
 */
class OfflineTransactionBundle(
    @JvmField val envelope: OfflineSigningEnvelope,
    @JvmField val attestation: ByteArray? = null,
)
