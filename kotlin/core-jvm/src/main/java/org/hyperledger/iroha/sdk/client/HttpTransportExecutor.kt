package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import java.util.concurrent.CompletableFuture

/**
 * Abstraction over the HTTP execution layer so transports can be tested without real network calls.
 *
 * The interface is intentionally expressed in terms of `TransportRequest` and
 * `TransportResponse` to avoid leaking JVM-specific HTTP client types into Android binaries.
 */
interface HttpTransportExecutor : TransportExecutor {

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse>

    /** Returns true when this executor can surface an underlying HTTP client for reuse. */
    fun supportsClientUnwrap(): Boolean = false

    /**
     * Cancels in-flight requests and releases any underlying resources when supported by the transport.
     *
     * Default implementation is a no-op so executors without lifecycle hooks are unaffected.
     */
    fun invalidateAndCancel() {}
}
