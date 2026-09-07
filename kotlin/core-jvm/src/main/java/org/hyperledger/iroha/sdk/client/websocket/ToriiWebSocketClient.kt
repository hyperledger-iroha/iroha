package org.hyperledger.iroha.sdk.client.websocket

import java.net.URI
import java.net.URLEncoder
import java.nio.ByteBuffer
import java.util.LinkedHashMap
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.TransportSecurity
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket

/**
 * WebSocket client built on the transport abstractions shared between JVM and Android targets.
 *
 * Inject a concrete WebSocket backend or an explicit application connector. The immutable request
 * contains the complete handshake inputs. A session is attached before its onOpen callback;
 * sends attempted before that callback fail immediately instead of retaining caller buffers.
 */
class ToriiWebSocketClient private constructor(builder: Builder) {

    /** Opens the final immutable handshake request; implementations must emit onOpen once ready. */
    fun interface WebSocketConnector {
        fun connect(
            request: TransportRequest,
            listener: TransportWebSocket.Listener
        ): CompletableFuture<TransportWebSocket>
    }

    private val baseUri: URI = builder.baseUri
    private val defaultHeaders: Map<String, String> = builder.defaultHeaders.toMap()
    private val observers: List<ClientObserver> = builder.observers.toList()
    private val connector: WebSocketConnector = requireNotNull(builder.connector) {
        "WebSocket connector is required; call setWebSocketConnector(...)"
    }

    /** Opens a WebSocket session against the given path. */
    fun connect(path: String, options: ToriiWebSocketOptions?, listener: ToriiWebSocketListener): ToriiWebSocketSession {
        val resolved = options ?: ToriiWebSocketOptions.defaultOptions()
        val request = buildRequest(path, resolved)
        notifyRequest(request)
        val session = ToriiWebSocketSessionImpl()
        val adapter = Adapter(listener, session, request)
        val connection = try {
            connector.connect(request, adapter)
        } catch (failure: Exception) {
            adapter.fail(failure)
            return session
        }
        session.bind(connection)
        connection.whenComplete { socket, error ->
            if (error != null) adapter.fail(error) else session.attach(socket)
        }
        return session
    }

    private fun buildRequest(path: String, options: ToriiWebSocketOptions): TransportRequest {
        val httpUri = appendQueryParameters(resolvePath(path), options.queryParameters)
        val wsUri = toWebSocketUri(httpUri)
        val headers = LinkedHashMap(defaultHeaders)
        options.headers.forEach { (k, v) -> headers[k] = v }
        TransportSecurity.requireWebSocketRequestAllowed(
            "ToriiWebSocketClient",
            baseUri,
            wsUri,
            headers,
        )
        val builder = TransportRequest.builder()
            .setMethod("GET")
            .setUri(wsUri)
            .setTimeout(options.connectTimeout)
        headers.forEach { (k, v) -> builder.addHeader(k, v) }
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

    private inner class Adapter(
        private val delegate: ToriiWebSocketListener,
        private val session: ToriiWebSocketSessionImpl,
        private val request: TransportRequest,
    ) : TransportWebSocket.Listener {
        private val opened = AtomicBoolean(false)

        override fun onOpen(socket: TransportWebSocket) {
            if (session.attach(socket) && opened.compareAndSet(false, true)) {
                notifyResponse(request)
                delegate.onOpen(session)
            }
        }
        override fun onText(socket: TransportWebSocket, data: String) { delegate.onText(session, data) }
        override fun onBinary(socket: TransportWebSocket, data: ByteBuffer) { delegate.onBinary(session, data) }
        override fun onError(socket: TransportWebSocket, error: Throwable) { fail(error) }
        override fun onClose(socket: TransportWebSocket, statusCode: Int, reason: String) {
            if (session.finish()) delegate.onClose(session, statusCode, reason)
        }
        fun fail(error: Throwable) {
            if (session.finish(error)) {
                notifyFailure(request, error)
                delegate.onError(session, error)
            }
        }
    }

    private class ToriiWebSocketSessionImpl : ToriiWebSocketSession {
        private val lock = Any()
        private var socket: TransportWebSocket? = null
        private var connection: CompletableFuture<TransportWebSocket>? = null
        private var finished = false
        private var failure: Throwable? = null

        fun bind(value: CompletableFuture<TransportWebSocket>) {
            val cancelled = synchronized(lock) { connection = value; finished }
            if (cancelled) value.cancel(false)
        }

        fun attach(value: TransportWebSocket): Boolean {
            val accepted = synchronized(lock) {
                if (finished) false else { socket = value; true }
            }
            if (!accepted) value.close(1000, "session closed")
            return accepted
        }

        fun finish(error: Throwable? = null): Boolean = synchronized(lock) {
            if (finished) false else { finished = true; failure = error; true }
        }

        private fun send(action: (TransportWebSocket) -> CompletableFuture<Void>): CompletableFuture<Void> {
            val target = synchronized(lock) {
                if (finished || socket == null) {
                    return CompletableFuture<Void>().also {
                        it.completeExceptionally(failure ?: IllegalStateException("WebSocket session is not open"))
                    }
                }
                requireNotNull(socket)
            }
            return action(target)
        }

        override fun sendText(data: String): CompletableFuture<Void> = send { it.sendText(data) }
        override fun sendBinary(data: ByteBuffer): CompletableFuture<Void> = send { it.sendBinary(data) }
        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> {
            val target: TransportWebSocket?
            val pending: CompletableFuture<TransportWebSocket>?
            synchronized(lock) {
                if (finished) return CompletableFuture.completedFuture(null)
                target = socket
                pending = connection
                if (target == null) {
                    finished = true
                    failure = CancellationException("WebSocket session closed before handshake")
                }
            }
            if (target != null) return target.close(statusCode, reason)
            pending?.cancel(false)
            return CompletableFuture.completedFuture(null)
        }
        override val isOpen: Boolean get() = synchronized(lock) { !finished && socket?.isOpen() == true }
        override val subprotocol: String get() = synchronized(lock) { if (finished) "" else socket?.subprotocol().orEmpty() }
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
            val targetText = target.toString()
            val fragmentIndex = targetText.indexOf('#').let { if (it >= 0) it else targetText.length }
            val builder = StringBuilder(targetText.length + 1)
                .append(targetText, 0, fragmentIndex)
            val query = encodeQuery(params)
            if (target.query == null || target.query.isEmpty()) {
                builder.append(if (builder.indexOf("?") >= 0) "&" else "?")
            } else {
                builder.append("&")
            }
            builder.append(query)
            builder.append(targetText, fragmentIndex, targetText.length)
            return URI.create(builder.toString())
        }

        private fun encodeQuery(params: Map<String, String>): String =
            params.entries.joinToString("&") { (k, v) ->
                "${URLEncoder.encode(k, "UTF-8")}=${URLEncoder.encode(v, "UTF-8")}"
            }

        private fun toWebSocketUri(httpUri: URI): URI {
            val scheme = if ("https".equals(httpUri.scheme, ignoreCase = true) || "wss".equals(httpUri.scheme, ignoreCase = true)) "wss" else "ws"
            return URI.create(
                "$scheme://${httpUri.authority}${httpUri.rawPath ?: ""}${if (httpUri.rawQuery != null) "?${httpUri.rawQuery}" else ""}"
            )
        }
    }
}
