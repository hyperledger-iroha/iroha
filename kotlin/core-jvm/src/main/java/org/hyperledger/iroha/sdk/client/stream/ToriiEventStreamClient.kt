package org.hyperledger.iroha.sdk.client.stream

import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.TransportSecurity
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag
import java.io.BufferedReader
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStreamReader
import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.LinkedHashMap
import java.util.concurrent.CancellationException
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference

private const val EVENT_STREAM_CONTENT_TYPE = "text/event-stream"
private const val DEFAULT_EVENT_NAME = "message"

/**
 * Minimal streaming client used to consume Torii server-sent event (SSE) feeds. The implementation
 * shares the same configuration surface as `HttpClientTransport`
 * so telemetry observers and authentication headers behave consistently across transports.
 */
class ToriiEventStreamClient(
    @JvmField val baseUri: URI,
    private val transport: TransportExecutor,
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
) {
    private val defaultHeaders: Map<String, String> = defaultHeaders.toMap()
    private val observers: List<ClientObserver> = observers.toList()

    /**
     * Opens an SSE stream against `path` using the supplied options.
     *
     * Callers must close the returned `ToriiEventStream`; the listener is notified when the
     * stream receives frames, fails, or terminates.
     *
     * When the configured transport supports streaming responses, frames are parsed as they
     * arrive; otherwise, the response body is buffered before parsing.
     */
    fun openSseStream(
        path: String?,
        options: ToriiEventStreamOptions?,
        listener: ToriiEventStreamListener,
    ): ToriiEventStream {
        val resolved = options ?: ToriiEventStreamOptions.defaultOptions()
        val request = buildRequest(path, resolved)
        notifyRequest(request)
        if (transport is StreamingTransportExecutor) {
            val responseFuture = transport.openStream(request)
            val stream = ActiveStream(request, responseFuture)
            responseFuture.whenComplete { response, throwable ->
                handleStreamResponse(request, listener, stream, response, throwable)
            }
            return stream
        }

        val responseFuture = transport.execute(request)
        val stream = ActiveStream(request, responseFuture)
        responseFuture.whenComplete { response, throwable ->
            handleBufferedResponse(request, listener, stream, response, throwable)
        }
        return stream
    }

    private fun buildRequest(path: String?, options: ToriiEventStreamOptions): TransportRequest {
        val target = appendQueryParameters(
            normalizeEventSseUriFilter(resolvePath(path)),
            normalizeEventSseQueryParameters(options.queryParameters),
        )
        val headers = LinkedHashMap(defaultHeaders)
        headers.putIfAbsent("Accept", EVENT_STREAM_CONTENT_TYPE)
        headers.putIfAbsent("Cache-Control", "no-cache")
        headers.putIfAbsent("Connection", "keep-alive")
        options.headers.forEach { (k, v) -> headers[k] = v }
        TransportSecurity.requireHttpRequestAllowed(
            "ToriiEventStreamClient",
            baseUri,
            target,
            headers,
            null,
        )
        val builder = TransportRequest.builder().setUri(target).setMethod("GET")
        val timeout = options.timeout
        if (timeout != null) {
            builder.setTimeout(timeout)
        }
        headers.forEach { (k, v) -> builder.addHeader(k, v) }
        return builder.build()
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return baseUri
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        val joined = if (base.endsWith("/")) "$base$normalized" else "$base/$normalized"
        return URI.create(joined)
    }

    private fun parseEventStream(
        stream: java.io.InputStream,
        listener: ToriiEventStreamListener,
        activeStream: ActiveStream,
    ) {
        try {
            BufferedReader(InputStreamReader(stream, StandardCharsets.UTF_8)).use { reader ->
                val data = StringBuilder()
                var eventName: String? = null
                var eventId: String? = null
                var line: String? = null
                while (!activeStream.closed() && reader.readLine().also { line = it } != null) {
                    val currentLine = line!!
                    if (currentLine.isEmpty()) {
                        dispatchEvent(listener, data, eventName, eventId)
                        eventName = null
                        eventId = null
                        continue
                    }
                    if (currentLine.startsWith(":")) continue
                    val colon = currentLine.indexOf(':')
                    val field: String
                    val value: String
                    if (colon == -1) {
                        field = currentLine
                        value = ""
                    } else {
                        field = currentLine.substring(0, colon)
                        val raw = currentLine.substring(colon + 1)
                        value = if (raw.startsWith(" ")) raw.substring(1) else raw
                    }
                    when (field) {
                        "data" -> data.append(value).append('\n')
                        "event" -> eventName = value
                        "id" -> eventId = value
                        "retry" -> {
                            try {
                                val millis = value.toLong()
                                if (millis >= 0) {
                                    listener.onRetryHint(Duration.ofMillis(millis))
                                }
                            } catch (_: NumberFormatException) {
                            }
                        }
                    }
                }
                dispatchEvent(listener, data, eventName, eventId)
            }
        } catch (ex: IOException) {
            if (!activeStream.closed()) {
                throw RuntimeException("Failed to parse Torii SSE stream", ex)
            }
        }
    }

    private fun handleBufferedResponse(
        request: TransportRequest,
        listener: ToriiEventStreamListener,
        stream: ActiveStream,
        response: TransportResponse?,
        throwable: Throwable?,
    ) {
        if (throwable != null) {
            val cause = unwrapCompletion(throwable)
            if (cause is CancellationException && stream.closedByCaller()) {
                stream.signalSuccess()
                return
            }
            stream.signalFailure(cause)
            notifyFailure(request, cause)
            listener.onError(cause)
            return
        }
        response!!
        if (response.statusCode < 200 || response.statusCode >= 300) {
            val body = response.body
            val message = if (body.isEmpty()) "" else String(body, StandardCharsets.UTF_8)
            val error = IOException(
                "Torii SSE request failed with status ${response.statusCode}" +
                    if (message.isEmpty()) "" else ": $message"
            )
            stream.signalFailure(error)
            notifyFailure(request, error)
            listener.onError(error)
            return
        }

        val clientResponse = ClientResponse(response.statusCode, ByteArray(0))
        notifyResponse(request, clientResponse)
        listener.onOpen()

        val readerFuture = CompletableFuture.runAsync {
            parseEventStream(ByteArrayInputStream(response.body), listener, stream)
        }
        stream.attach(readerFuture)
        readerFuture.whenComplete { _, parseError ->
            handleParseCompletion(request, listener, stream, parseError)
        }
    }

    private fun handleStreamResponse(
        request: TransportRequest,
        listener: ToriiEventStreamListener,
        stream: ActiveStream,
        response: TransportStreamResponse?,
        throwable: Throwable?,
    ) {
        if (throwable != null) {
            val cause = unwrapCompletion(throwable)
            if (cause is CancellationException && stream.closedByCaller()) {
                stream.signalSuccess()
                return
            }
            stream.signalFailure(cause)
            notifyFailure(request, cause)
            listener.onError(cause)
            return
        }
        response!!
        if (response.statusCode < 200 || response.statusCode >= 300) {
            val message = readBody(response)
            val error = IOException(
                "Torii SSE request failed with status ${response.statusCode}" +
                    if (message.isEmpty()) "" else ": $message"
            )
            stream.signalFailure(error)
            notifyFailure(request, error)
            listener.onError(error)
            return
        }

        val clientResponse = ClientResponse(response.statusCode, ByteArray(0))
        notifyResponse(request, clientResponse)
        listener.onOpen()
        stream.attachStream(response)

        val readerFuture = CompletableFuture.runAsync {
            parseEventStream(response.body, listener, stream)
        }
        stream.attach(readerFuture)
        readerFuture.whenComplete { _, parseError ->
            handleParseCompletion(request, listener, stream, parseError)
        }
    }

    private fun handleParseCompletion(
        request: TransportRequest,
        listener: ToriiEventStreamListener,
        stream: ActiveStream,
        parseError: Throwable?,
    ) {
        stream.closeStreamResponse()
        if (parseError != null && parseError !is CancellationException) {
            stream.signalFailure(parseError)
            notifyFailure(request, parseError)
            listener.onError(parseError)
        } else if (!stream.closedByCaller()) {
            listener.onClosed()
            stream.signalSuccess()
        } else {
            stream.signalSuccess()
        }
    }

    private fun notifyRequest(request: TransportRequest) {
        for (observer in observers) {
            observer.onRequest(request)
        }
    }

    private fun notifyResponse(request: TransportRequest, response: ClientResponse) {
        for (observer in observers) {
            observer.onResponse(request, response)
        }
    }

    private fun notifyFailure(request: TransportRequest, error: Throwable) {
        for (observer in observers) {
            observer.onFailure(request, error)
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        private fun appendQueryParameters(target: URI, params: Map<String, String>): URI {
            if (params.isEmpty()) return target
            val sb = StringBuilder(target.toString())
            val query = encodeQuery(params)
            if (target.query.isNullOrEmpty()) {
                sb.append(if (target.toString().contains("?")) "&" else "?")
            } else {
                sb.append("&")
            }
            sb.append(query)
            return URI.create(sb.toString())
        }

        private fun normalizeEventSseUriFilter(target: URI): URI {
            val rawQuery = target.rawQuery ?: return target
            val normalizedQuery = normalizeEventSseQuery(rawQuery)
            if (normalizedQuery == rawQuery) return target
            val targetText = target.toString()
            val fragmentIndex = targetText.indexOf('#')
            val withoutFragment = if (fragmentIndex >= 0) targetText.substring(0, fragmentIndex) else targetText
            val fragment = if (fragmentIndex >= 0) targetText.substring(fragmentIndex) else ""
            val base = withoutFragment.substringBefore('?')
            return URI.create("$base?$normalizedQuery$fragment")
        }

        private fun normalizeEventSseQueryParameters(params: Map<String, String>): Map<String, String> {
            if (!params.containsKey("filter")) return params
            val normalized = LinkedHashMap(params)
            normalized["filter"] = normalizeEventFilterPayload(params.getValue("filter"), "eventFilter")
            return normalized
        }

        private fun normalizeEventSseQuery(rawQuery: String): String {
            val segments = rawQuery.split('&').toMutableList()
            var changed = false
            for (index in segments.indices) {
                val segment = segments[index]
                if (segment.isEmpty()) continue
                val equals = segment.indexOf('=')
                val rawName = if (equals >= 0) segment.substring(0, equals) else segment
                val rawValue = if (equals >= 0) segment.substring(equals + 1) else ""
                val name = decodeQueryComponent(rawName)
                if (name != "filter") continue
                val value = decodeQueryComponent(rawValue)
                val normalized = normalizeEventFilterPayload(value, "eventFilter")
                if (normalized != value) {
                    segments[index] = "${URLEncoder.encode(name, "UTF-8")}=${URLEncoder.encode(normalized, "UTF-8")}"
                    changed = true
                }
            }
            return if (changed) segments.joinToString("&") else rawQuery
        }

        private fun decodeQueryComponent(value: String): String =
            URLDecoder.decode(value.replace("+", " "), "UTF-8")

        private fun normalizeEventFilterPayload(filter: String, context: String): String {
            val trimmed = filter.trim()
            if (trimmed.isEmpty() || (trimmed[0] != '{' && trimmed[0] != '[')) return filter
            val parsed = try {
                JsonParser.parse(trimmed)
            } catch (_: IllegalStateException) {
                return filter
            }
            @Suppress("UNCHECKED_CAST")
            val obj = parsed as? MutableMap<String, Any?> ?: return filter
            return if (normalizeProductionEventFilterObject(obj, context)) JsonEncoder.encode(obj) else filter
        }

        private fun normalizeProductionEventFilterObject(filter: MutableMap<String, Any?>, context: String): Boolean {
            var changed = false
            for (eventKind in listOf("VerifyingKey", "Proof")) {
                @Suppress("UNCHECKED_CAST")
                val body = filter[eventKind] as? MutableMap<String, Any?> ?: continue
                @Suppress("UNCHECKED_CAST")
                val matcher = body["id_matcher"] as? MutableMap<String, Any?> ?: continue
                if (!matcher.containsKey("backend")) continue
                val backendContext = "$context.$eventKind.id_matcher.backend"
                val backend = matcher["backend"] as? String
                    ?: throw IllegalArgumentException("$backendContext must be a string")
                val normalizedBackend =
                    VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend, backendContext)
                if (normalizedBackend != backend) {
                    matcher["backend"] = normalizedBackend
                    changed = true
                }
                if (eventKind == "Proof") {
                    changed = normalizeProofHashMatcher(matcher, "hash_hex", "$context.$eventKind.id_matcher.hash_hex") || changed
                    changed = normalizeProofHashMatcher(
                        matcher,
                        "proof_hash_hex",
                        "$context.$eventKind.id_matcher.proof_hash_hex",
                    ) || changed
                } else {
                    changed = normalizeVerifyingKeyNameMatcher(
                        matcher,
                        "$context.$eventKind.id_matcher.name",
                    ) || changed
                }
            }
            return changed
        }

        private fun normalizeVerifyingKeyNameMatcher(
            matcher: MutableMap<String, Any?>,
            context: String,
        ): Boolean {
            if (!matcher.containsKey("name")) return false
            val raw = matcher["name"] as? String
                ?: throw IllegalArgumentException("$context must be a string")
            val normalized = raw.trim()
            require(normalized.isNotEmpty()) {
                "$context must be a non-empty string"
            }
            require(!normalized.contains(':')) {
                "$context must not contain ':' characters"
            }
            if (normalized == raw) return false
            matcher["name"] = normalized
            return true
        }

        private fun normalizeProofHashMatcher(
            matcher: MutableMap<String, Any?>,
            propertyName: String,
            context: String,
        ): Boolean {
            if (!matcher.containsKey(propertyName)) return false
            val raw = matcher[propertyName] as? String
                ?: throw IllegalArgumentException("$context must be a string")
            val normalized = normalizeHex32String(raw, context)
            if (normalized == raw) return false
            matcher[propertyName] = normalized
            return true
        }

        private fun normalizeHex32String(raw: String, context: String): String {
            var normalized = raw.trim().lowercase()
            if (normalized.startsWith("0x")) {
                normalized = normalized.substring(2)
            }
            require(normalized.length == 64 && normalized.all { it in '0'..'9' || it in 'a'..'f' }) {
                "$context must be a 32-byte hex string"
            }
            return normalized
        }

        private fun encodeQuery(params: Map<String, String>): String {
            val sb = StringBuilder()
            var first = true
            for ((key, value) in params) {
                if (!first) sb.append('&')
                sb.append(URLEncoder.encode(key, "UTF-8"))
                    .append('=')
                    .append(URLEncoder.encode(value, "UTF-8"))
                first = false
            }
            return sb.toString()
        }

        private fun dispatchEvent(
            listener: ToriiEventStreamListener,
            data: StringBuilder,
            eventName: String?,
            eventId: String?,
        ) {
            if (data.isEmpty() && eventName == null && eventId == null) return
            if (data.isNotEmpty() && data[data.length - 1] == '\n') {
                data.deleteCharAt(data.length - 1)
            }
            val payload = data.toString()
            data.setLength(0)
            val name = if (eventName.isNullOrEmpty()) DEFAULT_EVENT_NAME else eventName
            listener.onEvent(ServerSentEvent(name, payload, eventId))
        }

        private fun unwrapCompletion(error: Throwable): Throwable {
            if (error is CompletionException && error.cause != null) {
                return error.cause!!
            }
            return error
        }

        private fun readBody(response: TransportStreamResponse): String {
            val data: ByteArray
            try {
                response.body.use { body ->
                    ByteArrayOutputStream().use { buffer ->
                        val chunk = ByteArray(4096)
                        var read: Int
                        while (body.read(chunk).also { read = it } != -1) {
                            buffer.write(chunk, 0, read)
                        }
                        data = buffer.toByteArray()
                    }
                }
            } catch (_: IOException) {
                return ""
            }
            return if (data.isEmpty()) "" else String(data, StandardCharsets.UTF_8)
        }
    }

    class Builder {
        private var baseUri: URI = URI.create("http://localhost:8080")
        private var transport: TransportExecutor = PlatformHttpTransportExecutor.createDefault()
        private val defaultHeaders: MutableMap<String, String> = LinkedHashMap()
        private val observers: MutableList<ClientObserver> = ArrayList()

        fun setBaseUri(baseUri: URI): Builder {
            this.baseUri = baseUri
            return this
        }

        fun setTransportExecutor(transport: TransportExecutor): Builder {
            this.transport = transport
            return this
        }

        fun putDefaultHeader(name: String, value: String): Builder {
            defaultHeaders[name] = value
            return this
        }

        fun defaultHeaders(headers: Map<String, String>?): Builder {
            defaultHeaders.clear()
            headers?.forEach { (k, v) -> putDefaultHeader(k, v) }
            return this
        }

        fun addObserver(observer: ClientObserver): Builder {
            observers.add(observer)
            return this
        }

        fun observers(values: List<ClientObserver>?): Builder {
            observers.clear()
            values?.forEach { addObserver(it) }
            return this
        }

        fun build(): ToriiEventStreamClient {
            return ToriiEventStreamClient(baseUri, transport, defaultHeaders, observers)
        }
    }

    private class ActiveStream(
        private val request: TransportRequest,
        private val responseFuture: CompletableFuture<*>,
    ) : ToriiEventStream {

        private val completion = CompletableFuture<Void>()
        private val closed = AtomicBoolean(false)
        private val _closedByCaller = AtomicBoolean(false)
        private val streamResponse = AtomicReference<TransportStreamResponse?>(null)
        @Volatile private var readerFuture: CompletableFuture<Void>? = null

        fun attach(readerFuture: CompletableFuture<Void>) {
            this.readerFuture = readerFuture
        }

        fun attachStream(response: TransportStreamResponse) {
            streamResponse.set(response)
        }

        fun closeStreamResponse() {
            streamResponse.getAndSet(null)?.close()
        }

        fun signalFailure(error: Throwable) {
            completion.completeExceptionally(error)
        }

        fun signalSuccess() {
            completion.complete(null)
        }

        fun closed(): Boolean = closed.get()

        fun closedByCaller(): Boolean = _closedByCaller.get()

        override fun isOpen(): Boolean = !closed.get() && !completion.isDone

        override fun completion(): CompletableFuture<Void> = completion

        override fun close() {
            if (!closed.compareAndSet(false, true)) return
            _closedByCaller.set(true)
            streamResponse.getAndSet(null)?.close()
            readerFuture?.cancel(true)
            responseFuture.cancel(true)
            completion.complete(null)
        }
    }
}
