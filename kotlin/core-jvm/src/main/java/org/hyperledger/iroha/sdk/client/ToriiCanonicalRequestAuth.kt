package org.hyperledger.iroha.sdk.client

/** Account identity and application-owned signer for authenticated Torii endpoints. */
class ToriiCanonicalRequestAuth(
    @JvmField val accountId: String,
    @JvmField val signer: RequestSigner,
    @JvmField val timestampMs: Long?,
    @JvmField val nonce: String?,
) {
    constructor(accountId: String, signer: RequestSigner) : this(accountId, signer, null, null)
}
