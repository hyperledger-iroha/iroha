package org.hyperledger.iroha.sdk.client.websocket

import java.time.Duration

/** Configuration for Torii WebSocket connections. */
class ToriiWebSocketOptions(
    queryParameters: Map<String, String> = emptyMap(),
    headers: Map<String, String> = emptyMap(),
    subprotocols: List<String> = emptyList(),
    @JvmField val connectTimeout: Duration? = null,
) {
    @JvmField val queryParameters: Map<String, String> = queryParameters.toMap()
    @JvmField val headers: Map<String, String> = headers.toMap()
    @JvmField val subprotocols: List<String> = subprotocols.toList()

    companion object {
        @JvmStatic
        fun defaultOptions(): ToriiWebSocketOptions = ToriiWebSocketOptions()
    }
}
