package org.hyperledger.iroha.sdk.client.testing

import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ConcurrentLinkedQueue
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/**
 * In-memory [HttpTransportExecutor] that returns pre-seeded responses without performing
 * network I/O. Useful for deterministic tests on both Android and JVM targets.
 */
class FakeHttpTransportExecutor : HttpTransportExecutor {

    private val globalResponses = ConcurrentLinkedQueue<TransportResponse>()
    private val pathResponses = ConcurrentHashMap<String, ConcurrentLinkedQueue<TransportResponse>>()

    @Volatile
    private var defaultResponse = TransportResponse(200, ByteArray(0), "", emptyMap())

    /** Enqueue a response that will be returned for any request when no path-specific response exists. */
    fun enqueueResponse(response: TransportResponse) {
        globalResponses.add(response)
    }

    /** Enqueue a response for the given request path (e.g., `/transaction`). */
    fun enqueueResponse(path: String, response: TransportResponse) {
        pathResponses
            .computeIfAbsent(path) { ConcurrentLinkedQueue() }
            .add(response)
    }

    /** Sets the fallback response used when no queued response is available. */
    fun setDefaultResponse(response: TransportResponse) {
        this.defaultResponse = response
    }

    /** Clears queued responses and restores the queues to an empty state. */
    fun clear() {
        globalResponses.clear()
        pathResponses.clear()
    }

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
        val path = request.uri.path
        var response = pathResponses[path]?.poll()
        if (response == null) {
            response = globalResponses.poll()
        }
        if (response == null) {
            response = defaultResponse
        }
        return CompletableFuture.completedFuture(
            TransportResponse(response.statusCode, response.body, response.message, response.headers)
        )
    }
}
