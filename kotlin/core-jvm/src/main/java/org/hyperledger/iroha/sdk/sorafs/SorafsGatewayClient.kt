package org.hyperledger.iroha.sdk.sorafs

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

private const val DEFAULT_PATH = "/v1/sorafs/gateway/fetch"

/**
 * Minimal HTTP client that posts orchestrator fetch requests to a SoraFS gateway endpoint.
 *
 * The client mirrors the CLI/SDK JSON schema and routes requests through `HttpTransportExecutor`
 * so tests can provide deterministic stubs. Responses surface the raw HTTP payload allowing
 * callers to parse orchestrator summaries or binary artefacts as needed.
 */
class SorafsGatewayClient(
    val executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault(),
    val baseUri: URI = URI.create("http://localhost:8080"),
    timeout: Duration? = Duration.ofSeconds(15),
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
    val fetchPath: String = DEFAULT_PATH,
) {
    val timeout: Duration? = timeout?.let { if (it.isNegative) Duration.ZERO else it }
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
        executor.execute(httpRequest).whenComplete { response, throwable ->
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
        if (timeout != null && timeout.isNegative) {
            builder.setTimeout(null)
        }
        mergeHeaders().forEach { (name, value) -> builder.addHeader(name, value) }
        builder.setBody(encodeRequestPayload(request))
        return builder.build()
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return baseUri
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalised = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        val joined = if (base.endsWith("/")) "$base$normalised" else "$base/$normalised"
        return URI.create(joined)
    }

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
