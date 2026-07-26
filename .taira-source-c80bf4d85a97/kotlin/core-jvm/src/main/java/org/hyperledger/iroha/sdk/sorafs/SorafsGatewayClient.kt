package org.hyperledger.iroha.sdk.sorafs

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.net.URI
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse

private const val DEFAULT_PATH = "/v1/sorafs/gateway/fetch"
private const val MAX_GATEWAY_REQUEST_BYTES = 32 * 1024 * 1024
private const val MAX_GATEWAY_RESPONSE_BYTES = 16 * 1024 * 1024

/**
 * Minimal HTTP client that posts orchestrator fetch requests to a SoraFS gateway endpoint.
 *
 * The client mirrors the CLI/SDK JSON schema and routes requests through `HttpTransportExecutor`
 * so tests can provide deterministic transport fakes. Responses surface the raw HTTP payload allowing
 * callers to parse orchestrator summaries or binary artefacts as needed. A zero timeout is valid;
 * negative durations are rejected instead of being rewritten.
 *
 * Host syntax and literal addresses are validated here. A transport implementation resolving DNS
 * names must additionally reject non-public answers and pin the approved answer for the request;
 * the generic executor interface cannot enforce that resolver-level DNS-rebinding boundary.
 */
class SorafsGatewayClient(
    baseUri: URI,
    val executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault(),
    timeout: Duration? = Duration.ofSeconds(15),
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
    fetchPath: String = DEFAULT_PATH,
) {
    /** Canonical public HTTPS origin used for every gateway request. */
    val baseUri: URI =
        SorafsInputValidator.requireCanonicalGatewayBaseUri(baseUri, "baseUri")

    /** Canonical path resolved only against [baseUri]; absolute URL overrides are forbidden. */
    val fetchPath: String =
        SorafsInputValidator.requireCanonicalGatewayFetchPath(fetchPath, "fetchPath")

    val timeout: Duration? = timeout?.also {
        require(!it.isNegative) { "timeout must be non-negative" }
    }
    private val _defaultHeaders: Map<String, String> = LinkedHashMap(defaultHeaders)
    private val _observers: List<ClientObserver> = observers.toList()

    val defaultHeaders: Map<String, String> get() = _defaultHeaders
    val observers: List<ClientObserver> get() = _observers

    /**
     * Executes a SoraFS gateway fetch request.
     *
     * @param request structured request built via `GatewayFetchRequest`.
     * @return future resolving to the HTTP response.
     * @throws SorafsStorageException when the gateway rejects the request or the transport fails.
     */
    fun fetch(request: GatewayFetchRequest): CompletableFuture<ClientResponse> {
        val httpRequest = buildRequest(request)
        notifyRequest(httpRequest)
        val result = CompletableFuture<ClientResponse>()
        executeBounded(httpRequest).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = if (cause == null)
                    SorafsStorageException("SoraFS gateway fetch request failed")
                else
                    SorafsStorageException("SoraFS gateway fetch request failed", cause)
                notifyFailure(httpRequest, error)
                result.completeExceptionally(error)
                return@whenComplete
            }
            if (response.body.size > MAX_GATEWAY_RESPONSE_BYTES) {
                val error = SorafsStorageException(
                    "SoraFS gateway response exceeds the $MAX_GATEWAY_RESPONSE_BYTES-byte limit",
                )
                notifyFailure(httpRequest, error)
                result.completeExceptionally(error)
                return@whenComplete
            }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null)
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = errorForResponse(response)
                notifyFailure(httpRequest, error)
                result.completeExceptionally(error)
                return@whenComplete
            }
            notifyResponse(httpRequest, clientResponse)
            result.complete(clientResponse)
        }
        return result
    }

    /** Executes a gateway fetch and parses the response JSON into a `GatewayFetchSummary`. */
    fun fetchSummary(request: GatewayFetchRequest): CompletableFuture<GatewayFetchSummary> =
        fetch(request).thenApply { response ->
            try {
                GatewayFetchSummary.fromJsonBytes(response.body)
            } catch (ex: RuntimeException) {
                throw CompletionException(SorafsStorageException("Failed to parse gateway fetch summary", ex))
            }
        }

    private fun buildRequest(request: GatewayFetchRequest): TransportRequest {
        val target = resolvePath(fetchPath)
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setTimeout(timeout)
        mergeHeaders().forEach { (name, value) -> builder.addHeader(name, value) }
        val body = encodeRequestPayload(request)
        require(body.size <= MAX_GATEWAY_REQUEST_BYTES) {
            "SoraFS gateway request exceeds the $MAX_GATEWAY_REQUEST_BYTES-byte limit"
        }
        builder.setBody(body)
        return builder.build()
    }

    private fun executeBounded(request: TransportRequest): CompletableFuture<TransportResponse> {
        val streaming = executor as? StreamingTransportExecutor
        if (streaming == null) {
            // Injected transports that cannot stream must enforce the same network-read ceiling.
            // The post-materialisation check in fetch() still prevents oversized parsing/use.
            return executor.execute(request)
        }
        return streaming.openStream(request).thenApply { response ->
            response.use {
                val declaredLength = canonicalContentLength(response)
                if (declaredLength != null && declaredLength > MAX_GATEWAY_RESPONSE_BYTES.toLong()) {
                    throw IOException(
                        "SoraFS gateway declared a response larger than " +
                            "$MAX_GATEWAY_RESPONSE_BYTES bytes",
                    )
                }
                val initialCapacity = declaredLength
                    ?.coerceAtMost(MAX_GATEWAY_RESPONSE_BYTES.toLong())
                    ?.toInt()
                    ?: 8_192
                val output = ByteArrayOutputStream(initialCapacity)
                val chunk = ByteArray(8_192)
                var total = 0
                while (true) {
                    val read = response.body.read(chunk)
                    if (read == -1) break
                    if (read == 0) {
                        throw IOException("SoraFS gateway response stream made no progress")
                    }
                    total = try {
                        Math.addExact(total, read)
                    } catch (ex: ArithmeticException) {
                        throw IOException("SoraFS gateway response length overflow", ex)
                    }
                    if (total > MAX_GATEWAY_RESPONSE_BYTES) {
                        throw IOException(
                            "SoraFS gateway response exceeds the " +
                                "$MAX_GATEWAY_RESPONSE_BYTES-byte limit",
                        )
                    }
                    output.write(chunk, 0, read)
                }
                if (declaredLength != null && declaredLength != total.toLong()) {
                    throw IOException("SoraFS gateway response length does not match Content-Length")
                }
                TransportResponse(
                    response.statusCode,
                    output.toByteArray(),
                    response.message,
                    response.headers,
                )
            }
        }
    }

    private fun canonicalContentLength(response: TransportStreamResponse): Long? {
        val matching = response.headers.entries
            .filter { it.key.equals("Content-Length", ignoreCase = true) }
        if (matching.isEmpty()) return null
        if (matching.size != 1) {
            throw IOException("SoraFS gateway returned ambiguous Content-Length headers")
        }
        val values = matching[0].value
        if (values.size != 1) {
            throw IOException("SoraFS gateway returned ambiguous Content-Length headers")
        }
        val value = values[0]
        if (value.isEmpty() || value.any { it !in '0'..'9' } ||
            (value.length > 1 && value.startsWith('0'))
        ) {
            throw IOException("SoraFS gateway returned a noncanonical Content-Length")
        }
        return value.toLongOrNull()
            ?: throw IOException("SoraFS gateway returned an overflowing Content-Length")
    }

    private fun resolvePath(path: String): URI = baseUri.resolve(path)

    private fun mergeHeaders(): Map<String, String> {
        val headers = LinkedHashMap(_defaultHeaders)
        ensureHeader(headers, "Content-Type", "application/json")
        ensureHeader(headers, "Accept", "application/json")
        return headers
    }

    private fun errorForResponse(response: TransportResponse): SorafsStorageException {
        val message = StringBuilder("SoraFS gateway fetch failed with status ${response.statusCode}")
        if (response.message.isNotEmpty()) {
            message.append(" (${response.message})")
        }
        val body = response.body
        if (body.isNotEmpty()) {
            val snippet = String(body, Charsets.UTF_8)
            if (snippet.isNotBlank()) {
                val trimmed = if (snippet.length > 256) snippet.substring(0, 256) + "\u2026" else snippet
                message.append(": $trimmed")
            }
        }
        return SorafsStorageException(message.toString())
    }

    private fun notifyRequest(request: TransportRequest) {
        for (observer in _observers) observer.onRequest(request)
    }

    private fun notifyResponse(request: TransportRequest, response: ClientResponse) {
        for (observer in _observers) observer.onResponse(request, response)
    }

    private fun notifyFailure(request: TransportRequest, error: Throwable) {
        for (observer in _observers) observer.onFailure(request, error)
    }

    /** Strategy for encoding gateway fetch requests into HTTP payloads. */
    fun interface RequestPayloadEncoder {
        fun encode(request: GatewayFetchRequest): ByteArray
    }

    companion object {
        private val DEFAULT_JSON_ENCODER = RequestPayloadEncoder { request ->
            JsonWriter.encodeBytes(request.toJson())
        }

        @Volatile
        private var currentEncoder: RequestPayloadEncoder = DEFAULT_JSON_ENCODER

        /**
         * Configure a custom payload encoder used when serialising `GatewayFetchRequest` instances.
         *
         * This is primarily intended for the Android orchestrator bridge so the same encoder can be
         * reused for HTTP requests and local bindings.
         */
        @JvmStatic
        fun setRequestPayloadEncoder(encoder: RequestPayloadEncoder) {
            currentEncoder = encoder
        }

        /** Restore the default JSON encoder for gateway requests. */
        @JvmStatic
        fun resetRequestPayloadEncoder() {
            currentEncoder = DEFAULT_JSON_ENCODER
        }

        /** Returns the encoder currently in use for gateway request payloads. */
        @JvmStatic
        fun requestPayloadEncoder(): RequestPayloadEncoder = currentEncoder

        /** Encode a gateway fetch request using the configured encoder. */
        @JvmStatic
        fun encodeRequestPayload(request: GatewayFetchRequest): ByteArray =
            currentEncoder.encode(request)
    }
}

private fun ensureHeader(headers: MutableMap<String, String>, name: String, value: String) {
    val existing = headers.keys.find { it.equals(name, ignoreCase = true) }
    if (existing == null) {
        headers[name] = value
        return
    }
    val current = headers[existing]
    if (current.isNullOrBlank()) {
        headers[existing] = value
    }
}
