package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.client.transport.TransportRequest

/**
 * Observer hooks for `IrohaClient` implementations.
 *
 * Observers can be used to collect telemetry, emit logs, or thread tracing headers without
 * mutating the client transport. Implementations should execute quickly and avoid throwing unless
 * the request must be aborted.
 */
interface ClientObserver {

    /**
     * Invoked once the request has been constructed and before it is dispatched to the transport
     * executor.
     */
    fun onRequest(request: TransportRequest) {}

    /** Invoked when a response is received successfully. */
    fun onResponse(request: TransportRequest, response: ClientResponse) {}

    /** Invoked when request execution fails. */
    fun onFailure(request: TransportRequest, error: Throwable) {}
}
