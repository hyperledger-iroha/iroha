package org.hyperledger.iroha.sdk.client.transport

import java.nio.ByteBuffer
import java.util.concurrent.CompletableFuture

/** Complete-message WebSocket transport. Protocol fragmentation and control frames stay internal. */
interface TransportWebSocket {
    /** Sends one complete immutable text message. */
    fun sendText(data: String): CompletableFuture<Void>

    /** Sends an owned copy of the buffer's remaining bytes without changing its position. */
    fun sendBinary(data: ByteBuffer): CompletableFuture<Void>

    /** Initiates the close handshake and completes when the connection terminates. */
    fun close(statusCode: Int, reason: String): CompletableFuture<Void>

    /** Whether application messages may currently be sent. */
    fun isOpen(): Boolean

    /** Negotiated subprotocol, or empty string when none was negotiated. */
    fun subprotocol(): String

    /** Complete messages and lifecycle events; binary callbacks own read-only buffers. */
    interface Listener {
        fun onOpen(socket: TransportWebSocket) {}
        fun onText(socket: TransportWebSocket, data: String) {}
        fun onBinary(socket: TransportWebSocket, data: ByteBuffer) {}
        fun onError(socket: TransportWebSocket, error: Throwable) {}
        fun onClose(socket: TransportWebSocket, statusCode: Int, reason: String) {}
    }
}
