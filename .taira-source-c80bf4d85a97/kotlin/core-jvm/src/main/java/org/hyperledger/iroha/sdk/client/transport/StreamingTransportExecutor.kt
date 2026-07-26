package org.hyperledger.iroha.sdk.client.transport

import java.util.concurrent.CompletableFuture

/**
 * Transport executor that can surface streaming responses without buffering the full payload.
 */
interface StreamingTransportExecutor : TransportExecutor {

    /**
     * Executes the request and returns a streaming response once headers are available.
     */
    fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse>
}
