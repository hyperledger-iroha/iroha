package org.hyperledger.iroha.sdk.client.transport

/**
 * Closed transport replay policy derived from the complete HTTP request.
 *
 * [ONE_SHOT] requests may carry a signature, nonce, credential, or mutating body. An executor
 * must make exactly one underlying network dispatch and must not follow redirects, authenticate
 * again, or retry after any response or transport failure. [RETRY_SAFE] is reserved for bodyless,
 * unsigned `GET`, `HEAD`, and `OPTIONS` requests.
 */
enum class RequestReplayPolicy {
    /** The exact request bytes and headers may be dispatched at most once. */
    ONE_SHOT,

    /** The request is a bodyless, unsigned read and may use an explicit retry policy. */
    RETRY_SAFE,
}
