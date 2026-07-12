package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.offline.OfflineJsonParser
import org.hyperledger.iroha.sdk.offline.OfflineOperationCodec
import org.hyperledger.iroha.sdk.offline.OfflineOperationKind
import org.hyperledger.iroha.sdk.offline.OfflineOperationReference
import org.hyperledger.iroha.sdk.offline.OfflineOperationStatus
import org.hyperledger.iroha.sdk.offline.OfflineReadiness
import org.hyperledger.iroha.sdk.offline.OfflineRedeemRequest
import org.hyperledger.iroha.sdk.offline.OfflineToriiException
import org.hyperledger.iroha.sdk.offline.OfflineTopUpRequest

/**
 * Lightweight HTTP client for the maintained Torii Offline endpoint.
 *
 * The client exposes the first-release Offline readiness, top-up, redemption,
 * and asynchronous operation resources.
 */
class OfflineToriiClient private constructor(builder: Builder) {

    private val executor: HttpTransportExecutor = builder.executor
    private val baseUri: URI = builder.baseUri
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> = Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))
    private val observers: List<ClientObserver> = builder.observers.toList()

    fun getOfflineReadiness(assetDefinitionId: String): CompletableFuture<OfflineReadiness> {
        require(assetDefinitionId.isNotEmpty() && assetDefinitionId == assetDefinitionId.trim()) {
            "assetDefinitionId must be exact non-empty text"
        }
        val encoded = URLEncoder.encode(assetDefinitionId, StandardCharsets.UTF_8.name())
        return executeGet(
            "$OFFLINE_READINESS_PATH?asset_definition_id=$encoded",
            OfflineJsonParser::parseOfflineReadiness,
        )
    }

    /** Submit the final first-release Offline top-up request. */
    fun submitOfflineTopUp(request: OfflineTopUpRequest): CompletableFuture<OfflineOperationReference> =
        executeNoritoPost(
            OFFLINE_TOP_UP_PATH,
            request.operationId,
            request.noritoArchive(),
            OfflineOperationKind.TOP_UP,
        )

    /** Submit the final first-release Offline redemption request. */
    fun submitOfflineRedeem(request: OfflineRedeemRequest): CompletableFuture<OfflineOperationReference> =
        executeNoritoPost(
            OFFLINE_REDEEM_PATH,
            request.operationId,
            request.noritoArchive(),
            OfflineOperationKind.REDEEM,
        )

    /** Fetch the final first-release Offline operation status resource. */
    fun getOfflineOperationStatus(operationId: String): CompletableFuture<OfflineOperationStatus> {
        val canonicalId = org.hyperledger.iroha.sdk.offline.requireOperationId(operationId)
        return executeNoritoGet("$OFFLINE_OPERATIONS_PATH/$canonicalId", canonicalId)
    }

    fun executor(): HttpTransportExecutor = executor

    private fun <T> executeGet(path: String, parser: (ByteArray) -> T): CompletableFuture<T> {
        val request = buildGetRequest(path)
        notifyRequest(request)
        return executeHttpRequest(request, 200) { response ->
            requireResponseMediaType(response, "application/json")
            parser(response.body)
        }
    }

    private fun buildGetRequest(path: String): TransportRequest {
        val target = resolvePath(path)
        val headers = mergeHeaders()
        TransportSecurity.requireHttpRequestAllowed(
            "OfflineToriiClient",
            baseUri,
            target,
            headers,
            null,
        )
        val builder = TransportRequest.builder().setUri(target).setMethod("GET").setTimeout(timeout)
        headers.forEach { (k, v) -> builder.addHeader(k, v) }
        return builder.build()
    }

    private fun executeNoritoPost(
        path: String,
        idempotencyKey: String,
        body: ByteArray,
        expectedKind: OfflineOperationKind,
    ): CompletableFuture<OfflineOperationReference> {
        val request = buildNoritoRequest(path, "POST", body, idempotencyKey)
        notifyRequest(request)
        return executeHttpRequest(request, 202) { response ->
            requireResponseMediaType(response, NORITO_MEDIA_TYPE)
            val reference = OfflineOperationCodec.decodeReference(response.body)
            require(reference.operationId == idempotencyKey && reference.kind == expectedKind) {
                "Offline operation reference does not match the submitted command"
            }
            val location = requireSingleResponseHeader(response.headers, "Location")
            require(location == reference.statusUri) {
                "Offline operation response Location does not match its typed statusUri"
            }
            reference
        }
    }

    private fun executeNoritoGet(
        path: String,
        expectedOperationId: String,
    ): CompletableFuture<OfflineOperationStatus> {
        val request = buildNoritoRequest(path, "GET", null, null)
        notifyRequest(request)
        return executeHttpRequest(request, 200) { response ->
            requireResponseMediaType(response, NORITO_MEDIA_TYPE)
            val status = OfflineOperationCodec.decodeStatus(response.body)
            require(status.operationId == expectedOperationId) {
                "Offline operation status operationId does not match the requested resource"
            }
            status
        }
    }

    private fun buildNoritoRequest(
        path: String,
        method: String,
        body: ByteArray?,
        idempotencyKey: String?,
    ): TransportRequest {
        val target = resolvePath(path)
        val headers = LinkedHashMap(defaultHeaders)
        ensureHeader(headers, "Accept", NORITO_MEDIA_TYPE)
        if (body != null) ensureHeader(headers, "Content-Type", NORITO_MEDIA_TYPE)
        if (idempotencyKey != null) ensureHeader(headers, "Idempotency-Key", idempotencyKey)
        TransportSecurity.requireHttpRequestAllowed(
            "OfflineToriiClient",
            baseUri,
            target,
            headers,
            body,
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .setTimeout(timeout)
        if (body != null) builder.setBody(body.copyOf())
        headers.forEach { (key, value) -> builder.addHeader(key, value) }
        return builder.build()
    }

    private fun mergeHeaders(): Map<String, String> {
        val headers = LinkedHashMap(defaultHeaders)
        ensureHeader(headers, "Accept", "application/json")
        return headers
    }

    private fun ensureHeader(headers: MutableMap<String, String>, name: String, value: String) {
        headers[findHeader(headers, name) ?: name] = value
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return baseUri
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun notifyRequest(request: TransportRequest) { for (observer in observers) observer.onRequest(request) }
    private fun notifyResponse(request: TransportRequest, response: ClientResponse) { for (observer in observers) observer.onResponse(request, response) }
    private fun notifyFailure(request: TransportRequest, error: Throwable) { for (observer in observers) observer.onFailure(request, error) }

    private fun <T> executeHttpRequest(
        request: TransportRequest,
        expectedStatus: Int,
        parser: (TransportResponse) -> T,
    ): CompletableFuture<T> {
        val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = OfflineToriiException("Offline request failed: ${summarizeCauseMessage(cause)}", cause, null, null, null)
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            val rejectCode = extractRejectCode(response.headers, response.body)
            val bodyPreview = decodeBodyPreview(response.body)
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, rejectCode)
            if (response.statusCode != expectedStatus) {
                val error = OfflineToriiException(buildHttpFailureMessage(request, response.statusCode, response.message, rejectCode, bodyPreview), response.statusCode, rejectCode, bodyPreview)
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            try {
                val parsed = parser(response)
                notifyResponse(request, clientResponse)
                future.complete(parsed)
            } catch (ex: RuntimeException) {
                val error = OfflineToriiException(buildParseFailureMessage(request, response.statusCode, bodyPreview), ex, response.statusCode, rejectCode, bodyPreview)
                notifyFailure(request, error)
                future.completeExceptionally(error)
            }
        }
        return future
    }

    class Builder internal constructor() {
        internal var executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault()
        internal var baseUri: URI = URI.create("http://localhost:8080")
        internal var timeout: Duration? = Duration.ofSeconds(15)
        internal val defaultHeaders = LinkedHashMap<String, String>()
        internal val observers = ArrayList<ClientObserver>()
        fun executor(executor: HttpTransportExecutor): Builder { this.executor = executor; return this }
        fun baseUri(baseUri: URI): Builder { this.baseUri = baseUri; return this }
        fun timeout(timeout: Duration?): Builder { this.timeout = timeout; return this }
        fun addHeader(name: String, value: String): Builder { defaultHeaders[name] = value; return this }
        fun defaultHeaders(headers: Map<String, String>?): Builder { defaultHeaders.clear(); headers?.forEach { (k, v) -> defaultHeaders[k] = v }; return this }
        fun addObserver(observer: ClientObserver?): Builder { if (observer != null) observers.add(observer); return this }
        fun observers(observers: List<ClientObserver>?): Builder { this.observers.clear(); observers?.forEach { addObserver(it) }; return this }
        fun build(): OfflineToriiClient = OfflineToriiClient(this)
    }

    companion object {
        private const val OFFLINE_READINESS_PATH = "/v1/offline/readiness"
        private const val OFFLINE_TOP_UP_PATH = "/v1/offline/top-up"
        private const val OFFLINE_REDEEM_PATH = "/v1/offline/redeem"
        private const val OFFLINE_OPERATIONS_PATH = "/v1/offline/operations"
        private const val NORITO_MEDIA_TYPE = "application/x-norito"

        @JvmStatic fun builder(): Builder = Builder()
        private fun extractRejectCode(headers: Map<String, List<String>>, body: ByteArray?): String? =
            HttpErrorMessageExtractor.extractRejectCode(headers, "x-iroha-reject-code", body)
        private fun decodeBodyPreview(payload: ByteArray): String? = HttpErrorMessageExtractor.extractMessage(payload)
        private fun summarizeCauseMessage(cause: Throwable?): String = cause?.message?.takeIf { it.isNotBlank() } ?: cause?.javaClass?.simpleName ?: "unknown transport error"
        private fun buildHttpFailureMessage(request: TransportRequest?, statusCode: Int, statusMessage: String?, rejectCode: String?, bodyPreview: String?): String {
            val sb = StringBuilder("Offline request failed with HTTP $statusCode")
            if (!statusMessage.isNullOrBlank()) sb.append(" ($statusMessage)")
            request?.uri?.path?.let { sb.append(" on $it") }
            if (!rejectCode.isNullOrBlank()) sb.append(". reject_code=$rejectCode")
            if (!bodyPreview.isNullOrBlank()) sb.append(". body=$bodyPreview")
            return sb.toString()
        }
        private fun buildParseFailureMessage(request: TransportRequest?, statusCode: Int, bodyPreview: String?): String {
            val sb = StringBuilder("Failed to parse offline response (HTTP $statusCode)")
            request?.uri?.path?.let { sb.append(" for $it") }
            if (!bodyPreview.isNullOrBlank()) sb.append(". body=$bodyPreview")
            return sb.toString()
        }
        private fun findHeader(headers: Map<String, String>, name: String): String? {
            for (key in headers.keys) if (key.equals(name, ignoreCase = true)) return key
            return null
        }

        private fun requireSingleResponseHeader(
            headers: Map<String, List<String>>,
            name: String,
        ): String {
            val values = headers.entries
                .filter { (headerName, _) -> headerName.equals(name, ignoreCase = true) }
                .flatMap { (_, headerValues) -> headerValues }
            require(values.size == 1) { "$name response header must occur exactly once" }
            return values.single()
        }

        private fun requireResponseMediaType(response: TransportResponse, expected: String) {
            val raw = requireSingleResponseHeader(response.headers, "Content-Type")
            val mediaType = raw.substringBefore(';').trim()
            require(mediaType.equals(expected, ignoreCase = true)) {
                "Offline response Content-Type must be $expected"
            }
        }
    }
}
