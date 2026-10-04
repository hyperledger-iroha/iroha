package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.core.model.NetworkId

/**
 * Account identity and application-owned signer for authenticated Torii endpoints.
 *
 * With the two-argument constructor every signed request gets a fresh timestamp and random nonce,
 * so one instance can sign any number of requests. Passing an explicit [timestampMs] and [nonce]
 * (for deterministic tests or externally reserved nonces) makes the instance single-use: Torii
 * rejects a replayed nonce, so a second signing attempt fails locally with
 * [IllegalStateException] instead of sending a request that cannot succeed.
 */
class ToriiCanonicalRequestAuth(
    @JvmField val accountId: String,
    @JvmField val signer: RequestSigner,
    @JvmField val timestampMs: Long?,
    @JvmField val nonce: String?,
) {
    private val explicitFreshnessUsed = AtomicBoolean(false)

    init {
        require((timestampMs == null) == (nonce == null)) { "timestampMs and nonce must be provided together" }
        require(timestampMs == null || timestampMs >= 0) { "timestampMs must be non-negative" }
    }

    constructor(accountId: String, signer: RequestSigner) : this(accountId, signer, null, null)

    /** Whether this instance carries a caller-chosen timestamp and nonce (single-use). */
    val hasExplicitFreshness: Boolean get() = nonce != null

    /**
     * The four canonical headers for one request to [uri].
     *
     * @throws IllegalStateException when an explicit nonce was already used
     */
    internal fun headers(networkId: NetworkId, method: String, uri: URI, body: ByteArray?): Map<String, String> {
        val explicitNonce = nonce
            ?: return CanonicalRequestSigner.buildHeaders(networkId, method, uri, body, accountId, signer)
        check(explicitFreshnessUsed.compareAndSet(false, true)) {
            "this ToriiCanonicalRequestAuth carries an explicit nonce that was already used; " +
                "create a new instance, or omit timestampMs/nonce for per-request freshness"
        }
        return CanonicalRequestSigner.buildHeaders(
            networkId,
            method,
            uri,
            body,
            accountId,
            signer,
            requireNotNull(timestampMs),
            explicitNonce,
        )
    }
}
