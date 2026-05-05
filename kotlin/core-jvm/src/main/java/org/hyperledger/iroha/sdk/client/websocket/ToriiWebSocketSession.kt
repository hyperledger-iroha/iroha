package org.hyperledger.iroha.sdk.client.websocket

import java.nio.ByteBuffer
import java.util.concurrent.CompletableFuture

/** Handle for an active Torii WebSocket session. */
interface ToriiWebSocketSession : AutoCloseable {

    /** Sends a text frame. */
    fun sendText(data: CharSequence, last: Boolean): CompletableFuture<Void>

    /** Sends a binary frame. */
    fun sendBinary(data: ByteBuffer, last: Boolean): CompletableFuture<Void>

    /** Sends a ping frame. */
    fun sendPing(message: ByteBuffer): CompletableFuture<Void>

    /** Sends a pong frame. */
    fun sendPong(message: ByteBuffer): CompletableFuture<Void>

    /** Initiates a close handshake. */
    fun close(statusCode: Int, reason: String): CompletableFuture<Void>

    /** Returns `true` when the underlying transport is still open. */
    val isOpen: Boolean

    /** Returns the negotiated subprotocol (if any). */
    val subprotocol: String?

    override fun close() {
        close(NORMAL_CLOSURE, "client closing")
    }

    companion object {
        const val NORMAL_CLOSURE: Int = 1000
    }
}
