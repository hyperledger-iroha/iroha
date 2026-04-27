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
 * canonical implementation.
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
 */
class UrlConnectionTransportExecutor(
    private val connectTimeout: Duration? = null,
    private val readTimeout: Duration? = null,
    private val asyncExecutor: Executor? = null,
) : HttpTransportExecutor, StreamingTransportExecutor {

    /** Creates an executor that applies the same timeout to connect and read operations. */
    constructor(timeout: Duration?) : this(timeout, timeout, null)

    /** Creates an executor with distinct connect/read timeouts (nullable to use defaults). */
    constructor(connectTimeout: Duration?, readTimeout: Duration?) : this(connectTimeout, readTimeout, null)

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
        var connection: HttpURLConnection? = null
        try {
            connection = openConnection(request)
            writeRequestBody(request, connection)
            val status = connection.responseCode
            val message = connection.responseMessage ?: ""
            val body = readBody(connection, status)
            val headers = normalizeHeaders(connection.headerFields)
            return TransportResponse(status, body, message, headers)
        } catch (ex: IOException) {
            throw RuntimeException("HTTP request failed", ex)
        } finally {
            connection?.disconnect()
        }
    }

    private fun openStreamSync(request: TransportRequest): TransportStreamResponse {
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
        val hasBody = request.body.isNotEmpty() && !request.method.equals("GET", ignoreCase = true)
        connection.doOutput = hasBody
        return connection
    }

    companion object {
        private fun writeRequestBody(request: TransportRequest, connection: HttpURLConnection) {
            val hasBody = request.body.isNotEmpty() && !request.method.equals("GET", ignoreCase = true)
            if (!hasBody) return
            connection.outputStream.write(request.body)
        }

        private fun readBody(connection: HttpURLConnection, status: Int): ByteArray {
            val stream = responseStream(connection, status) ?: return ByteArray(0)
            stream.use { input ->
                ByteArrayOutputStream().use { buffer ->
                    val chunk = ByteArray(4096)
                    var read: Int
                    while (input.read(chunk).also { read = it } != -1) {
                        buffer.write(chunk, 0, read)
                    }
                    return buffer.toByteArray()
                }
            }
        }

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
                out[key] = value?.toList() ?: emptyList()
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
