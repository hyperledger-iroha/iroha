package org.hyperledger.iroha.sdk.client.transport

import java.nio.ByteBuffer
import java.util.concurrent.CompletableFuture

/** Minimal transport-level WebSocket abstraction decoupled from `java.net.http.WebSocket`. */
interface TransportWebSocket {

    fun sendText(data: CharSequence, last: Boolean): CompletableFuture<Void>

    fun sendBinary(data: ByteBuffer, last: Boolean): CompletableFuture<Void>

    /**
     * Sends a ping frame if supported by the underlying transport.
     *
     * Implementations should return a failed future when the transport does not expose ping
     * controls (e.g., OkHttp), allowing callers to handle the unsupported path explicitly.
     */
    fun ping(data: ByteBuffer): CompletableFuture<Void> = unsupported("ping")

    /**
     * Sends a pong frame if supported by the underlying transport.
     *
     * Implementations should return a failed future when the transport does not expose pong
     * controls, mirroring `ping` semantics.
     */
    fun pong(data: ByteBuffer): CompletableFuture<Void> = unsupported("pong")

    fun close(statusCode: Int, reason: String): CompletableFuture<Void>

    /** Returns `true` when the underlying connection is open. */
    fun isOpen(): Boolean = false

    /** Returns the negotiated subprotocol (or empty string when none was negotiated). */
    fun subprotocol(): String = ""

    fun validateCloseCode(statusCode: Int) {
        require(statusCode in 1000..4999) { "Invalid WebSocket close status: $statusCode" }
    }

    private fun unsupported(operation: String): CompletableFuture<Void> {
        val failed = CompletableFuture<Void>()
        failed.completeExceptionally(
            UnsupportedOperationException("$operation is not supported by this transport")
        )
        return failed
    }

    /** Listener for WebSocket events. */
    interface Listener {
        fun onOpen(socket: TransportWebSocket) {}

        fun onText(socket: TransportWebSocket, data: CharSequence, last: Boolean) {}

        fun onBinary(socket: TransportWebSocket, data: ByteBuffer, last: Boolean) {}

        fun onPing(socket: TransportWebSocket, data: ByteBuffer) {}

        fun onPong(socket: TransportWebSocket, data: ByteBuffer) {}

        fun onError(socket: TransportWebSocket, error: Throwable) {}

        fun onClose(socket: TransportWebSocket, statusCode: Int, reason: String) {}
    }
}
