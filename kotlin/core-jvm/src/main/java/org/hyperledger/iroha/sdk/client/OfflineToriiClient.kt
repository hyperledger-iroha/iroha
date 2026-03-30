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
import org.hyperledger.iroha.sdk.offline.*

/**
 * Lightweight HTTP client for Torii offline inspection endpoints (`/v1/offline/`).
 *
 * The client reuses the shared [HttpTransportExecutor] abstraction so telemetry hooks and
 * custom HTTP stacks can be injected by SDK consumers. Responses are parsed into immutable model
 * types under `org.hyperledger.iroha.sdk.offline`.
 */
class OfflineToriiClient private constructor(builder: Builder) {

    private val executor: HttpTransportExecutor = builder.executor
    private val baseUri: URI = builder.baseUri
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> = Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))
    private val observers: List<ClientObserver> = builder.observers.toList()

    fun listTransfers(params: OfflineListParams): CompletableFuture<OfflineTransferList> = executeRequest(TRANSFERS_PATH, params, OfflineJsonParser::parseTransfers)
    fun listRevocations(params: OfflineListParams): CompletableFuture<OfflineRevocationList> = executeRequest(REVOCATIONS_PATH, params, OfflineJsonParser::parseRevocations)
    fun queryTransfers(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineTransferList> = executeQuery(TRANSFERS_QUERY_PATH, envelope, OfflineJsonParser::parseTransfers)
    fun getTransfer(bundleIdHex: String): CompletableFuture<OfflineTransferList.OfflineTransferItem> = executeGet("$TRANSFERS_PATH/${urlEncode(bundleIdHex.trim())}", OfflineJsonParser::parseTransferItem)
    fun getRevocationBundleJson(): CompletableFuture<String> = executeGet(REVOCATIONS_BUNDLE_PATH, ::decodeJsonPayload)

    fun setupCash(requestJson: String): CompletableFuture<String> = executeJsonPost(CASH_SETUP_PATH, requestJson)
    fun loadCash(requestJson: String): CompletableFuture<String> = executeJsonPost(CASH_LOAD_PATH, requestJson)
    fun refreshCash(requestJson: String): CompletableFuture<String> = executeJsonPost(CASH_REFRESH_PATH, requestJson)
    fun syncCash(requestJson: String): CompletableFuture<String> = executeJsonPost(CASH_SYNC_PATH, requestJson)
    fun redeemCash(requestJson: String): CompletableFuture<String> = executeJsonPost(CASH_REDEEM_PATH, requestJson)

    fun issueBuildClaim(requestBody: OfflineBuildClaimIssueRequest): CompletableFuture<OfflineBuildClaimIssueResponse> {
        val body = JsonEncoder.encode(requestBody.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        val request = buildPostRequest(BUILD_CLAIM_ISSUE_PATH, body)
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseBuildClaimIssueResponse)
    }

    fun executor(): HttpTransportExecutor = executor

    private fun <T> executeRequest(path: String, params: OfflineListParams, parser: (ByteArray) -> T): CompletableFuture<T> {
        val request = buildGetRequest(path, params)
        notifyRequest(request)
        return executeHttpRequest(request, parser)
    }

    private fun <T> executeGet(path: String, parser: (ByteArray) -> T): CompletableFuture<T> {
        val request = buildGetRequest(path, emptyMap<String, String>())
        notifyRequest(request)
        return executeHttpRequest(request, parser)
    }

    private fun executeJsonPost(path: String, requestJson: String): CompletableFuture<String> {
        val trimmed = requestJson.trim()
        require(trimmed.isNotEmpty()) { "requestJson must not be blank" }
        val request = buildPostRequest(path, trimmed.toByteArray(StandardCharsets.UTF_8))
        notifyRequest(request)
        return executeHttpRequest(request, ::decodeJsonPayload)
    }

    private fun buildGetRequest(path: String, params: OfflineListParams?): TransportRequest = buildGetRequest(path, params?.toQueryParameters() ?: emptyMap())

    private fun buildGetRequest(path: String, query: Map<String, String>): TransportRequest {
        val target = appendQuery(resolvePath(path), query)
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

    private fun <T> executeQuery(path: String, envelope: OfflineQueryEnvelope?, parser: (ByteArray) -> T): CompletableFuture<T> {
        val resolved = envelope ?: OfflineQueryEnvelope.builder().build()
        val request = buildPostRequest(path, resolved.toJsonBytes())
        notifyRequest(request)
        return executeHttpRequest(request, parser)
    }

    private fun buildPostRequest(path: String, body: ByteArray): TransportRequest {
        val target = resolvePath(path)
        val headers = mergeHeaders()
        TransportSecurity.requireHttpRequestAllowed(
            "OfflineToriiClient",
            baseUri,
            target,
            headers,
            body,
        )
        val builder = TransportRequest.builder().setUri(target).setMethod("POST").setTimeout(timeout).setBody(body)
        headers.forEach { (k, v) -> builder.addHeader(k, v) }
        builder.addHeader("Content-Type", "application/json")
        return builder.build()
    }

    private fun mergeHeaders(): Map<String, String> { val h = LinkedHashMap(defaultHeaders); ensureHeader(h, "Accept", "application/json"); return h }
    private fun ensureHeader(headers: MutableMap<String, String>, name: String, value: String) { headers[findHeader(headers, name) ?: name] = value }
    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return baseUri
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun notifyRequest(request: TransportRequest) { for (o in observers) o.onRequest(request) }
    private fun notifyResponse(request: TransportRequest, response: ClientResponse) { for (o in observers) o.onResponse(request, response) }
    private fun notifyFailure(request: TransportRequest, error: Throwable) { for (o in observers) o.onFailure(request, error) }

    private fun <T> executeHttpRequest(request: TransportRequest, parser: (ByteArray) -> T): CompletableFuture<T> {
        val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = OfflineToriiException("Offline request failed: ${summarizeCauseMessage(cause)}", cause, null, null, null)
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            val rejectCode = extractRejectCode(response.headers)
            val bodyPreview = decodeBodyPreview(response.body)
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, rejectCode)
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = OfflineToriiException(buildHttpFailureMessage(request, response.statusCode, response.message, rejectCode, bodyPreview), response.statusCode, rejectCode, bodyPreview)
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            try { val parsed = parser(response.body); notifyResponse(request, clientResponse); future.complete(parsed) }
            catch (ex: RuntimeException) {
                val error = OfflineToriiException(buildParseFailureMessage(request, response.statusCode, bodyPreview), ex, response.statusCode, rejectCode, bodyPreview)
                notifyFailure(request, error); future.completeExceptionally(error)
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
        private const val TRANSFERS_PATH = "/v1/offline/transfers"
        private const val REVOCATIONS_PATH = "/v1/offline/revocations"
        private const val REVOCATIONS_BUNDLE_PATH = "/v1/offline/revocations/bundle"
        private const val CASH_SETUP_PATH = "/v1/offline/cash/setup"
        private const val CASH_LOAD_PATH = "/v1/offline/cash/load"
        private const val CASH_REFRESH_PATH = "/v1/offline/cash/refresh"
        private const val CASH_SYNC_PATH = "/v1/offline/cash/sync"
        private const val CASH_REDEEM_PATH = "/v1/offline/cash/redeem"
        private const val TRANSFERS_QUERY_PATH = "/v1/offline/transfers/query"
        private const val BUILD_CLAIM_ISSUE_PATH = "/v1/offline/build-claims/issue"

        @JvmStatic fun builder(): Builder = Builder()
        private fun extractRejectCode(headers: Map<String, List<String>>): String? = HttpErrorMessageExtractor.extractRejectCode(headers, "x-iroha-reject-code")
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
        private fun decodeJsonPayload(payload: ByteArray): String = String(payload, StandardCharsets.UTF_8)
        private fun findHeader(headers: Map<String, String>, name: String): String? { for (key in headers.keys) if (key.equals(name, ignoreCase = true)) return key; return null }
        private fun appendQuery(target: URI, params: Map<String, String>): URI {
            if (params.isEmpty()) return target
            val sb = StringBuilder(target.toString()).append(if (target.toString().contains("?")) "&" else "?").append(encodeQuery(params))
            return URI.create(sb.toString())
        }
        private fun encodeQuery(params: Map<String, String>): String = params.entries.joinToString("&") { (k, v) -> "${urlEncode(k)}=${urlEncode(v)}" }
        private fun urlEncode(value: String): String = URLEncoder.encode(value, StandardCharsets.UTF_8.name())
    }
}
