package org.hyperledger.iroha.sdk.client.websocket

import java.nio.ByteBuffer

/** Listener interface mirroring Torii WebSocket lifecycle events. */
interface ToriiWebSocketListener {
    fun onOpen(session: ToriiWebSocketSession) {}
    fun onText(session: ToriiWebSocketSession, data: CharSequence, last: Boolean) {}
    fun onBinary(session: ToriiWebSocketSession, data: ByteBuffer, last: Boolean) {}
    fun onPing(session: ToriiWebSocketSession, message: ByteBuffer) {}
    fun onPong(session: ToriiWebSocketSession, message: ByteBuffer) {}
    fun onClose(session: ToriiWebSocketSession, statusCode: Int, reason: String) {}
    fun onError(session: ToriiWebSocketSession, error: Throwable) {}
}
