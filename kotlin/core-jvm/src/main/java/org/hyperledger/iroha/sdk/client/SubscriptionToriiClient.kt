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
import org.hyperledger.iroha.sdk.subscriptions.*

/** Lightweight HTTP client for Torii subscription endpoints (`/v1/subscriptions/`). */
class SubscriptionToriiClient private constructor(builder: Builder) {

    private val executor: HttpTransportExecutor = builder.executor
    private val baseUri: URI = builder.baseUri
    private val timeout: Duration? = builder.timeout
    private val defaultHeaders: Map<String, String> = Collections.unmodifiableMap(LinkedHashMap(builder.defaultHeaders))
    private val observers: List<ClientObserver> = builder.observers.toList()

    fun listSubscriptionPlans(params: SubscriptionPlanListParams?): CompletableFuture<SubscriptionPlanListResponse> {
        val request = buildGetRequest(PLANS_PATH, params?.toQueryParameters() ?: emptyMap())
        notifyRequest(request)
        return executeHttpRequest(request, SubscriptionJsonParser::parsePlanList)
    }

    fun createSubscriptionPlan(request: SubscriptionPlanCreateRequest): CompletableFuture<SubscriptionPlanCreateResponse> {
        val transport = buildPostRequest(PLANS_PATH, request.toJsonBytes())
        notifyRequest(transport)
        return executeHttpRequest(transport, SubscriptionJsonParser::parsePlanCreateResponse)
    }

    fun listSubscriptions(params: SubscriptionListParams?): CompletableFuture<SubscriptionListResponse> {
        val request = buildGetRequest(SUBSCRIPTIONS_PATH, params?.toQueryParameters() ?: emptyMap())
        notifyRequest(request)
        return executeHttpRequest(request, SubscriptionJsonParser::parseSubscriptionList)
    }

    fun createSubscription(request: SubscriptionCreateRequest): CompletableFuture<SubscriptionCreateResponse> {
        val transport = buildPostRequest(SUBSCRIPTIONS_PATH, request.toJsonBytes())
        notifyRequest(transport)
        return executeHttpRequest(transport, SubscriptionJsonParser::parseSubscriptionCreateResponse)
    }

    fun getSubscription(subscriptionId: String): CompletableFuture<SubscriptionListResponse.SubscriptionRecord?> {
        val normalizedId = requireNonBlank(subscriptionId, "subscription_id")
        val path = "$SUBSCRIPTIONS_PATH/${urlEncode(normalizedId)}"
        val request = buildGetRequest(path, emptyMap())
        notifyRequest(request)
        return executeHttpRequestAllowingNotFound(request, SubscriptionJsonParser::parseSubscriptionRecord)
    }

    fun pauseSubscription(subscriptionId: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> = executeSubscriptionAction(subscriptionId, "pause", request)
    fun resumeSubscription(subscriptionId: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> = executeSubscriptionAction(subscriptionId, "resume", request)
    fun cancelSubscription(subscriptionId: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> = executeSubscriptionAction(subscriptionId, "cancel", request)
    fun keepSubscription(subscriptionId: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> = executeSubscriptionAction(subscriptionId, "keep", request)
    fun chargeSubscriptionNow(subscriptionId: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> = executeSubscriptionAction(subscriptionId, "charge-now", request)

    fun recordSubscriptionUsage(subscriptionId: String, request: SubscriptionUsageRequest): CompletableFuture<SubscriptionActionResponse> {
        val normalizedId = requireNonBlank(subscriptionId, "subscription_id")
        val path = "$SUBSCRIPTIONS_PATH/${urlEncode(normalizedId)}/usage"
        val transport = buildPostRequest(path, request.toJsonBytes())
        notifyRequest(transport)
        return executeHttpRequest(transport, SubscriptionJsonParser::parseActionResponse)
    }

    fun executor(): HttpTransportExecutor = executor

    private fun executeSubscriptionAction(subscriptionId: String, action: String, request: SubscriptionActionRequest): CompletableFuture<SubscriptionActionResponse> {
        val normalizedId = requireNonBlank(subscriptionId, "subscription_id")
        val path = "$SUBSCRIPTIONS_PATH/${urlEncode(normalizedId)}/$action"
        val transport = buildPostRequest(path, request.toJsonBytes())
        notifyRequest(transport)
        return executeHttpRequest(transport, SubscriptionJsonParser::parseActionResponse)
    }

    private fun buildGetRequest(path: String, query: Map<String, String>): TransportRequest {
        val target = appendQuery(resolvePath(path), query)
        val builder = TransportRequest.builder().setUri(target).setMethod("GET").setTimeout(timeout)
        mergeHeaders().forEach { (k, v) -> builder.addHeader(k, v) }
        return builder.build()
    }

    private fun buildPostRequest(path: String, body: ByteArray): TransportRequest {
        val target = resolvePath(path)
        val builder = TransportRequest.builder().setUri(target).setMethod("POST").setTimeout(timeout).setBody(body)
        mergeHeaders().forEach { (k, v) -> builder.addHeader(k, v) }
        builder.addHeader("Content-Type", "application/json")
        return builder.build()
    }

    private fun mergeHeaders(): Map<String, String> {
        val headers = LinkedHashMap(defaultHeaders)
        ensureHeader(headers, "Accept", "application/json")
        return headers
    }

    private fun ensureHeader(headers: MutableMap<String, String>, name: String, value: String) {
        val existing = findHeader(headers, name)
        headers[existing ?: name] = value
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

    private fun <T> executeHttpRequest(request: TransportRequest, parser: (ByteArray) -> T): CompletableFuture<T> {
        val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = SubscriptionToriiException("Subscription request failed", cause ?: throwable)
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            val clientResponse = ClientResponse(response.statusCode, response.body)
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = SubscriptionToriiException("Subscription request failed with status ${response.statusCode}")
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            try { val parsed = parser(response.body); notifyResponse(request, clientResponse); future.complete(parsed) }
            catch (ex: RuntimeException) { val error = SubscriptionToriiException("Failed to parse subscription response", ex); notifyFailure(request, error); future.completeExceptionally(error) }
        }
        return future
    }

    private fun <T> executeHttpRequestAllowingNotFound(request: TransportRequest, parser: (ByteArray) -> T): CompletableFuture<T?> {
        val future = CompletableFuture<T?>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                val error = SubscriptionToriiException("Subscription request failed", cause ?: throwable)
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            val clientResponse = ClientResponse(response.statusCode, response.body)
            if (response.statusCode == 404) { notifyResponse(request, clientResponse); future.complete(null); return@whenComplete }
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = SubscriptionToriiException("Subscription request failed with status ${response.statusCode}")
                notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete
            }
            try { val parsed = parser(response.body); notifyResponse(request, clientResponse); future.complete(parsed) }
            catch (ex: RuntimeException) { val error = SubscriptionToriiException("Failed to parse subscription response", ex); notifyFailure(request, error); future.completeExceptionally(error) }
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
        fun addHeader(name: String, value: String): Builder { if (name != null && value != null) defaultHeaders[name] = value; return this }
        fun defaultHeaders(headers: Map<String, String>?): Builder { defaultHeaders.clear(); headers?.forEach { (k, v) -> if (k != null && v != null) defaultHeaders[k] = v }; return this }
        fun addObserver(observer: ClientObserver?): Builder { if (observer != null) observers.add(observer); return this }
        fun observers(observers: List<ClientObserver>?): Builder { this.observers.clear(); observers?.forEach { addObserver(it) }; return this }
        fun build(): SubscriptionToriiClient { check(baseUri != null) { "baseUri is required" }; return SubscriptionToriiClient(this) }
    }

    companion object {
        private const val PLANS_PATH = "/v1/subscriptions/plans"
        private const val SUBSCRIPTIONS_PATH = "/v1/subscriptions"

        @JvmStatic fun builder(): Builder = Builder()

        private fun requireNonBlank(value: String?, field: String): String {
            check(!value.isNullOrBlank()) { "$field is required" }
            return value.trim()
        }

        private fun findHeader(headers: Map<String, String>, name: String): String? {
            for (key in headers.keys) if (key.equals(name, ignoreCase = true)) return key
            return null
        }

        private fun appendQuery(target: URI, params: Map<String, String>?): URI {
            if (params.isNullOrEmpty()) return target
            val builder = StringBuilder(target.toString())
            builder.append(if (target.toString().contains("?")) "&" else "?")
            builder.append(encodeQuery(params))
            return URI.create(builder.toString())
        }

        private fun encodeQuery(params: Map<String, String>): String =
            params.entries.joinToString("&") { (k, v) -> "${urlEncode(k)}=${urlEncode(v)}" }

        private fun urlEncode(value: String): String = URLEncoder.encode(value, StandardCharsets.UTF_8.name())
    }
}
