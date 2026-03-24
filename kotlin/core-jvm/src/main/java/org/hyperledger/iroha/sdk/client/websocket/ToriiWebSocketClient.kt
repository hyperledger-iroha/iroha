package org.hyperledger.iroha.sdk.client.websocket

import java.net.URI
import java.net.URLEncoder
import java.nio.ByteBuffer
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket

/**
 * WebSocket client built on the transport abstractions shared between JVM and Android targets.
 *
 * By default the platform connector is used (OkHttp on Android, JDK connector on JVM). Android
 * callers can still inject an explicit OkHttp connector to reuse a shared client instance.
 */
class ToriiWebSocketClient private constructor(builder: Builder) {

    /** Connector hook so platforms can supply their preferred WebSocket implementation. */
    fun interface WebSocketConnector {
        fun connect(
            request: TransportRequest,
            options: ToriiWebSocketOptions,
            listener: TransportWebSocket.Listener,
            defaultHeaders: Map<String, String>
        ): CompletableFuture<TransportWebSocket>
    }

    private val baseUri: URI = builder.baseUri
    private val defaultHeaders: Map<String, String> = builder.defaultHeaders.toMap()
    private val observers: List<ClientObserver> = builder.observers.toList()
    private val connector: WebSocketConnector = builder.connector ?: PlatformWebSocketConnector.createDefault()

    /** Opens a WebSocket session against the given path. */
    fun connect(path: String, options: ToriiWebSocketOptions?, listener: ToriiWebSocketListener): ToriiWebSocketSession {
        val resolved = options ?: ToriiWebSocketOptions.defaultOptions()
        val request = buildRequest(path, resolved)
        notifyRequest(request)
        val session = ToriiWebSocketSessionImpl()
        val adapter = Adapter(listener, session)
        connector.connect(request, resolved, adapter, defaultHeaders)
            .whenComplete { socket, throwable ->
                if (throwable != null) {
                    session.fail(throwable)
                    notifyFailure(request, throwable)
                    listener.onError(session, throwable)
                    return@whenComplete
                }
                session.attach(socket)
                notifyResponse(request)
            }
        return session
    }

    private fun buildRequest(path: String, options: ToriiWebSocketOptions): TransportRequest {
        val httpUri = appendQueryParameters(resolvePath(path), options.queryParameters)
        val wsUri = toWebSocketUri(httpUri)
        val builder = TransportRequest.builder()
            .setMethod("GET")
            .setUri(wsUri)
            .setTimeout(options.connectTimeout)
        defaultHeaders.forEach { (k, v) -> builder.addHeader(k, v) }
        options.headers.forEach { (k, v) -> builder.addHeader(k, v) }
        if (options.subprotocols.isNotEmpty()) {
            builder.addHeader("Sec-WebSocket-Protocol", options.subprotocols.joinToString(","))
        }
        return builder.build()
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return baseUri
        if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("ws://") || path.startsWith("wss://")) {
            return URI.create(path)
        }
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        val joined = if (base.endsWith("/")) base + normalized else "$base/$normalized"
        return URI.create(joined)
    }

    private fun notifyRequest(request: TransportRequest) {
        for (observer in observers) observer.onRequest(request)
    }

    private fun notifyResponse(request: TransportRequest) {
        val response = ClientResponse(101, ByteArray(0), "websocket_handshake", null)
        for (observer in observers) observer.onResponse(request, response)
    }

    private fun notifyFailure(request: TransportRequest, error: Throwable) {
        for (observer in observers) observer.onFailure(request, error)
    }

    private class Adapter(
        private val delegate: ToriiWebSocketListener,
        private val session: ToriiWebSocketSessionImpl
    ) : TransportWebSocket.Listener {
        override fun onOpen(socket: TransportWebSocket) { delegate.onOpen(session) }
        override fun onText(socket: TransportWebSocket, data: CharSequence, last: Boolean) { delegate.onText(session, data, last) }
        override fun onBinary(socket: TransportWebSocket, data: ByteBuffer, last: Boolean) { delegate.onBinary(session, data, last) }
        override fun onPing(socket: TransportWebSocket, message: ByteBuffer) { delegate.onPing(session, message) }
        override fun onPong(socket: TransportWebSocket, message: ByteBuffer) { delegate.onPong(session, message) }
        override fun onError(socket: TransportWebSocket, error: Throwable) { delegate.onError(session, error); session.fail(error) }
        override fun onClose(socket: TransportWebSocket, statusCode: Int, reason: String) { delegate.onClose(session, statusCode, reason); session.close(statusCode, reason) }
    }

    private class ToriiWebSocketSessionImpl : ToriiWebSocketSession {
        private val ready = CompletableFuture<TransportWebSocket>()

        fun attach(socket: TransportWebSocket) { ready.complete(socket) }
        fun fail(error: Throwable) { ready.completeExceptionally(error) }

        override fun sendText(data: CharSequence, last: Boolean): CompletableFuture<Void> = ready.thenCompose { it.sendText(data, last) }
        override fun sendBinary(data: ByteBuffer, last: Boolean): CompletableFuture<Void> = ready.thenCompose { it.sendBinary(data, last) }
        override fun sendPing(message: ByteBuffer): CompletableFuture<Void> = ready.thenCompose { it.ping(message) }
        override fun sendPong(message: ByteBuffer): CompletableFuture<Void> = ready.thenCompose { it.pong(message) }
        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> = ready.thenCompose { ws ->
            if (!ws.isOpen()) CompletableFuture.completedFuture(null) else ws.close(statusCode, reason)
        }
        override val isOpen: Boolean get() = ready.getNow(null)?.isOpen() == true
        override val subprotocol: String? get() = ready.getNow(null)?.subprotocol() ?: ""
    }

    class Builder {
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal val defaultHeaders = LinkedHashMap<String, String>()
        internal val observers = ArrayList<ClientObserver>()
        internal var connector: WebSocketConnector? = null

        fun setBaseUri(baseUri: URI): Builder { this.baseUri = baseUri; return this }
        fun putDefaultHeader(name: String, value: String): Builder { defaultHeaders[name] = value; return this }
        fun defaultHeaders(headers: Map<String, String>?): Builder { defaultHeaders.clear(); headers?.forEach { (k, v) -> putDefaultHeader(k, v) }; return this }
        fun addObserver(observer: ClientObserver): Builder { observers.add(observer); return this }
        fun observers(values: List<ClientObserver>?): Builder { observers.clear(); values?.forEach { addObserver(it) }; return this }
        fun setWebSocketConnector(connector: WebSocketConnector): Builder { this.connector = connector; return this }
        fun build(): ToriiWebSocketClient = ToriiWebSocketClient(this)
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun appendQueryParameters(target: URI, params: Map<String, String>): URI {
            if (params.isEmpty()) return target
            val builder = StringBuilder(target.toString())
            val query = encodeQuery(params)
            if (target.query == null || target.query.isEmpty()) {
                builder.append(if (target.toString().contains("?")) "&" else "?")
            } else {
                builder.append("&")
            }
            builder.append(query)
            return URI.create(builder.toString())
        }

        private fun encodeQuery(params: Map<String, String>): String =
            params.entries.joinToString("&") { (k, v) ->
                "${URLEncoder.encode(k, StandardCharsets.UTF_8)}=${URLEncoder.encode(v, StandardCharsets.UTF_8)}"
            }

        private fun toWebSocketUri(httpUri: URI): URI {
            val scheme = if ("https".equals(httpUri.scheme, ignoreCase = true) || "wss".equals(httpUri.scheme, ignoreCase = true)) "wss" else "ws"
            return URI.create(
                "$scheme://${httpUri.authority}${httpUri.rawPath ?: ""}${if (httpUri.rawQuery != null) "?${httpUri.rawQuery}" else ""}"
            )
        }
    }
}
