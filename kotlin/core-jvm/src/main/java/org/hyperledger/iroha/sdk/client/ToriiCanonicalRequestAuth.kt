package org.hyperledger.iroha.sdk.client

import java.security.PrivateKey

/** Canonical request signing material for Torii app endpoints. */
class ToriiCanonicalRequestAuth(
    @JvmField val accountId: String,
    @JvmField val privateKey: PrivateKey,
    @JvmField val timestampMs: Long?,
    @JvmField val nonce: String?,
) {
    constructor(accountId: String, privateKey: PrivateKey) : this(accountId, privateKey, null, null)
}
