package org.hyperledger.iroha.sdk.client.transport

import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection
import java.time.Duration
import java.util.concurrent.CompletableFuture

/**
 * `HttpTransportExecutor` implementation backed by `HttpURLConnection`.
 *
 * This executor avoids `java.net.http` so it can serve as a portable fallback across JVM
 * and Android targets. Callers should prefer platform-optimised executors (OkHttp on Android, JDK
 * HTTP client on JVM) when available.
 */
class UrlConnectionTransportExecutor(
    private val connectTimeout: Duration? = null,
    private val readTimeout: Duration? = null,
) : HttpTransportExecutor, StreamingTransportExecutor {

    /** Creates an executor that applies the same timeout to connect and read operations. */
    constructor(timeout: Duration?) : this(timeout, timeout)

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        CompletableFuture.supplyAsync { executeSync(request) }

    override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
        CompletableFuture.supplyAsync { openStreamSync(request) }

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
                return error ?: connection.inputStream
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

        private fun toMillis(timeout: Duration?, fallback: Int): Int {
            if (timeout == null) return fallback
            val millis = timeout.toMillis()
            if (millis > Int.MAX_VALUE) return Int.MAX_VALUE
            return maxOf(0, millis.toInt())
        }
    }
}
