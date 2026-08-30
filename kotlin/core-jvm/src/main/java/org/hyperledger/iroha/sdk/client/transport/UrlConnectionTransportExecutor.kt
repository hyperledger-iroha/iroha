package org.hyperledger.iroha.sdk.client.transport

import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executor

/**
 * `HttpTransportExecutor` implementation backed by `HttpURLConnection`.
 *
 * This executor avoids `java.net.http` so it can serve JVM and Android targets with the same
 * canonical implementation. It deliberately rejects credential-free requests: JVM and Android
 * URLConnection stacks expose process-wide authentication caches that cannot be disabled or
 * inspected portably on an individual connection.
 *
 * @param asyncExecutor optional [Executor] used to run the synchronous HTTP work for
 *   [execute] and [openStream]. When `null`, falls back to `ForkJoinPool.commonPool()`
 *   via [CompletableFuture.supplyAsync].
 *
 *   Why this parameter exists (it is not gratuitous):
 *   - On Android, sockets opened on threads with no traffic-stats tag (default `0`) trip
 *     `StrictMode.VmPolicy.detectUntaggedSockets`. With `penaltyDeath` the process is killed.
 *   - `ForkJoinPool.commonPool()` worker threads cannot be retagged externally — the consumer
 *     has no hook to influence what tag commonPool's threads carry.
 *   - Supplying [asyncExecutor] lets the consumer provide its own pool whose `ThreadFactory`
 *     pre-tags each worker via `TrafficStats.setThreadStatsTag(...)` before any socket is
 *     created on that thread.
 *   - Pure-JVM consumers can ignore the parameter (default `null` → `commonPool()`); the
 *     parameter keeps `core-jvm` free of Android-only APIs while still giving Android callers
 *     a clean injection point. The same hook also works for any other thread-local context
 *     (auth MDC, tracing) the consumer wants set per request.
 * @param maximumResponseBytes maximum number of response-body bytes buffered by [execute].
 *   The default is 64 MiB. Streaming responses opened through [openStream] are intentionally
 *   not subject to this buffered-response limit.
 */
class UrlConnectionTransportExecutor(
    private val connectTimeout: Duration? = null,
    private val readTimeout: Duration? = null,
    private val asyncExecutor: Executor? = null,
    private val maximumResponseBytes: Long = DEFAULT_MAXIMUM_RESPONSE_BYTES,
) : HttpTransportExecutor, StreamingTransportExecutor {

    init {
        require(maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
            "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
        }
    }

    /** Creates an executor that applies the same timeout to connect and read operations. */
    constructor(timeout: Duration?) : this(timeout, timeout, null, DEFAULT_MAXIMUM_RESPONSE_BYTES)

    /** Creates an executor with distinct connect/read timeouts (nullable to use defaults). */
    constructor(
        connectTimeout: Duration?,
        readTimeout: Duration?,
    ) : this(connectTimeout, readTimeout, null, DEFAULT_MAXIMUM_RESPONSE_BYTES)

    /** Creates an executor with distinct timeouts and a custom asynchronous executor. */
    constructor(
        connectTimeout: Duration?,
        readTimeout: Duration?,
        asyncExecutor: Executor?,
    ) : this(connectTimeout, readTimeout, asyncExecutor, DEFAULT_MAXIMUM_RESPONSE_BYTES)

    /** Creates an executor with default timeouts and a custom buffered-response limit. */
    constructor(maximumResponseBytes: Long) : this(null, null, null, maximumResponseBytes)

    /** Creates an executor with distinct timeouts and a custom buffered-response limit. */
    constructor(
        connectTimeout: Duration?,
        readTimeout: Duration?,
        maximumResponseBytes: Long,
    ) : this(connectTimeout, readTimeout, null, maximumResponseBytes)

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        if (asyncExecutor != null) {
            CompletableFuture.supplyAsync({ executeSync(request) }, asyncExecutor)
        } else {
            CompletableFuture.supplyAsync { executeSync(request) }
        }

    override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
        if (asyncExecutor != null) {
            CompletableFuture.supplyAsync({ openStreamSync(request) }, asyncExecutor)
        } else {
            CompletableFuture.supplyAsync { openStreamSync(request) }
        }

    private fun executeSync(request: TransportRequest): TransportResponse {
        if (!request.allowAmbientCredentials) {
            throw credentialFreeUnsupported()
        }
        return executeSyncInternal(request)
    }

    private fun executeSyncInternal(request: TransportRequest): TransportResponse {
        var connection: HttpURLConnection? = null
        try {
            connection = openConnection(request)
            writeRequestBody(request, connection)
            val status = connection.responseCode
            val message = connection.responseMessage ?: ""
            val responseLimit = minOf(
                maximumResponseBytes,
                request.maximumResponseBytes ?: maximumResponseBytes,
            )
            val body = readBody(connection, status, request.method, responseLimit)
            val headers = normalizeHeaders(connection.headerFields)
            return TransportResponse(status, body, message, headers)
        } catch (ex: IOException) {
            throw RuntimeException("HTTP request failed", ex)
        } finally {
            connection?.disconnect()
        }
    }

    private fun openStreamSync(request: TransportRequest): TransportStreamResponse {
        if (!request.allowAmbientCredentials) {
            throw credentialFreeUnsupported()
        }
        return openStreamSyncInternal(request)
    }

    private fun openStreamSyncInternal(request: TransportRequest): TransportStreamResponse {
        var connection: HttpURLConnection? = null
        try {
            connection = openConnection(request)
            writeRequestBody(request, connection)
            val status = connection.responseCode
            val message = connection.responseMessage ?: ""
            val stream = responseStream(connection, status)
            val headers = normalizeHeaders(connection.headerFields)
            val target = connection
            return TransportStreamResponse(status, stream, message, headers, Runnable { target.disconnect() })
        } catch (ex: IOException) {
            connection?.disconnect()
            throw RuntimeException("HTTP request failed", ex)
        }
    }

    private fun openConnection(request: TransportRequest): HttpURLConnection {
        val url = request.uri.toURL()
        val connection = url.openConnection() as HttpURLConnection
        connection.requestMethod = request.method
        // Canonical signatures bind the original URI, and onboarding tokens must never be
        // forwarded to a redirect target by the platform HTTP stack.
        connection.instanceFollowRedirects = false
        connection.doInput = true
        val timeout = request.timeout
        val connectMs = toMillis(timeout ?: connectTimeout, connection.connectTimeout)
        val readMs = toMillis(timeout ?: readTimeout, connection.readTimeout)
        connection.connectTimeout = connectMs
        connection.readTimeout = readMs
        for ((name, values) in request.headers) {
            for (value in values) {
                connection.addRequestProperty(name, value)
            }
        }
        val body = request.body
        val hasBody = body.isNotEmpty() && !request.method.equals("GET", ignoreCase = true)
        if (request.replayPolicy == RequestReplayPolicy.ONE_SHOT) {
            // Avoid stale pooled connections, the main source of transparent URLConnection
            // replays. Fixed-length streaming disables URLConnection's internal body retry path.
            connection.useCaches = false
            connection.setRequestProperty("Connection", "close")
            if (hasBody) connection.setFixedLengthStreamingMode(body.size)
        }
        connection.doOutput = hasBody
        return connection
    }

    companion object {
        /** Default maximum body size buffered by [execute], in bytes (64 MiB). */
        const val DEFAULT_MAXIMUM_RESPONSE_BYTES: Long = 64L * 1024L * 1024L

        private fun credentialFreeUnsupported(): IllegalStateException =
            IllegalStateException(
                "credential-free requests are unsupported by URLConnection because process-wide " +
                    "authentication caches cannot be isolated",
            )

        private fun writeRequestBody(request: TransportRequest, connection: HttpURLConnection) {
            val body = request.body
            val hasBody = body.isNotEmpty() && !request.method.equals("GET", ignoreCase = true)
            if (!hasBody) return
            connection.outputStream.write(body)
        }

        private fun readBody(
            connection: HttpURLConnection,
            status: Int,
            requestMethod: String,
            maximumResponseBytes: Long,
        ): ByteArray {
            val headers = connection.headerFields
            val declaredLength = canonicalContentLength(headers)
            rejectAmbiguousFraming(headers, declaredLength)
            if (responseMustNotHaveBody(requestMethod, status)) {
                return ByteArray(0)
            }
            val bufferedLength = contentLengthForBufferedBody(headers, declaredLength)
            if (bufferedLength != null && bufferedLength > maximumResponseBytes) {
                throw IOException(
                    "HTTP response Content-Length $bufferedLength exceeds the " +
                        "$maximumResponseBytes-byte limit",
                )
            }
            val stream = responseStream(connection, status)
            if (stream == null) {
                if (declaredLength != null && declaredLength != 0L) {
                    throw IOException(
                        "HTTP response ended before its $declaredLength-byte Content-Length",
                    )
                }
                return ByteArray(0)
            }
            return stream.use { input ->
                readBoundedBody(input, maximumResponseBytes, bufferedLength)
            }
        }

        internal fun readBoundedBody(
            input: InputStream,
            maximumResponseBytes: Long,
            declaredLength: Long?,
        ): ByteArray {
            require(maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
                "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
            }
            val initialCapacity = minOf(declaredLength ?: 32L, maximumResponseBytes, 8192L).toInt()
            ByteArrayOutputStream(initialCapacity).use { buffer ->
                val chunk = ByteArray(8192)
                var total = 0L
                while (true) {
                    val remaining = maximumResponseBytes - total
                    val requested = minOf(chunk.size.toLong(), remaining + 1L).toInt()
                    val read = input.read(chunk, 0, requested)
                    if (read == -1) break
                    if (read < -1 || read > requested) {
                        throw IOException("HTTP response body stream returned an invalid read count")
                    }
                    if (read == 0) {
                        throw IOException("HTTP response body stream made no read progress")
                    }
                    if (read.toLong() > remaining) {
                        throw IOException(
                            "HTTP response body exceeds the $maximumResponseBytes-byte limit",
                        )
                    }
                    buffer.write(chunk, 0, read)
                    total += read.toLong()
                }
                if (declaredLength != null && total != declaredLength) {
                    throw IOException(
                        "HTTP response body length $total does not match Content-Length " +
                            declaredLength,
                    )
                }
                return buffer.toByteArray()
            }
        }

        private fun canonicalContentLength(raw: Map<String?, List<String>?>?): Long? {
            if (raw == null) return null
            var value: String? = null
            for ((name, values) in raw) {
                if (name == null || !name.equals("Content-Length", ignoreCase = true)) continue
                if (value != null || values == null || values.size != 1) {
                    throw IOException(
                        "HTTP response must contain at most one Content-Length value",
                    )
                }
                value = values[0]
            }
            return value?.let(::parseCanonicalContentLength)
        }

        internal fun parseCanonicalContentLength(value: String): Long {
            if (value.isEmpty() || (value.length > 1 && value[0] == '0')) {
                throw IOException(
                    "HTTP response Content-Length must be a canonical unsigned decimal",
                )
            }
            var parsed = 0L
            for (character in value) {
                if (character !in '0'..'9') {
                    throw IOException(
                        "HTTP response Content-Length must be a canonical unsigned decimal",
                    )
                }
                val digit = character.code - '0'.code
                if (parsed > (Long.MAX_VALUE - digit) / 10L) {
                    throw IOException("HTTP response Content-Length exceeds the supported range")
                }
                parsed = parsed * 10L + digit
            }
            return parsed
        }

        internal fun contentLengthForBufferedBody(
            raw: Map<String?, List<String>?>?,
            declaredLength: Long?,
        ): Long? {
            if (declaredLength == null || raw == null) return declaredLength
            for ((name, values) in raw) {
                if (name == null || !name.equals("Content-Encoding", ignoreCase = true)) continue
                if (values == null || values.isEmpty()) return null
                for (value in values) {
                    val encodings = value.split(',')
                    if (encodings.any { !it.trim().equals("identity", ignoreCase = true) }) {
                        return null
                    }
                }
            }
            return declaredLength
        }

        private fun rejectAmbiguousFraming(
            raw: Map<String?, List<String>?>?,
            declaredLength: Long?,
        ) {
            if (declaredLength == null || raw == null) return
            val hasTransferEncoding = raw.keys.any { name ->
                name != null && name.equals("Transfer-Encoding", ignoreCase = true)
            }
            if (hasTransferEncoding) {
                throw IOException(
                    "HTTP response must not combine Content-Length with Transfer-Encoding",
                )
            }
        }

        private fun responseMustNotHaveBody(requestMethod: String, status: Int): Boolean =
            requestMethod.equals("HEAD", ignoreCase = true) ||
                status in 100..199 ||
                status == HttpURLConnection.HTTP_NO_CONTENT ||
                status == HttpURLConnection.HTTP_NOT_MODIFIED

        private fun responseStream(connection: HttpURLConnection, status: Int): InputStream? {
            if (status >= 400) {
                val error = connection.errorStream
                if (error != null) return error
                return try {
                    connection.inputStream
                } catch (_: IOException) {
                    null
                }
            }
            return connection.inputStream
        }

        private fun normalizeHeaders(raw: Map<String?, List<String>>?): Map<String, List<String>> {
            if (raw == null) return emptyMap()
            val out = LinkedHashMap<String, List<String>>()
            for ((key, value) in raw) {
                if (key == null) continue
                out[key] = value.toList()
            }
            return out
        }

        private fun toMillis(timeout: Duration?, defaultValue: Int): Int {
            if (timeout == null) return defaultValue
            val millis = timeout.toMillis()
            if (millis > Int.MAX_VALUE) return Int.MAX_VALUE
            return maxOf(0, millis.toInt())
        }
    }
}
