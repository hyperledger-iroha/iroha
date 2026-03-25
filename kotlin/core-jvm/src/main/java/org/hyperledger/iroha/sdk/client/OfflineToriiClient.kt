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

    fun listAllowances(params: OfflineListParams): CompletableFuture<OfflineAllowanceList> = executeRequest(ALLOWANCES_PATH, params, OfflineJsonParser::parseAllowances)
    fun listTransfers(params: OfflineListParams): CompletableFuture<OfflineTransferList> = executeRequest(TRANSFERS_PATH, params, OfflineJsonParser::parseTransfers)
    fun listSummaries(params: OfflineListParams): CompletableFuture<OfflineSummaryList> = executeRequest(SUMMARIES_PATH, params, OfflineJsonParser::parseSummaries)
    fun listRevocations(params: OfflineListParams): CompletableFuture<OfflineRevocationList> = executeRequest(REVOCATIONS_PATH, params, OfflineJsonParser::parseRevocations)
    fun queryAllowances(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineAllowanceList> = executeQuery(ALLOWANCES_QUERY_PATH, envelope, OfflineJsonParser::parseAllowances)
    fun queryTransfers(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineTransferList> = executeQuery(TRANSFERS_QUERY_PATH, envelope, OfflineJsonParser::parseTransfers)
    fun querySummaries(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineSummaryList> = executeQuery(SUMMARIES_QUERY_PATH, envelope, OfflineJsonParser::parseSummaries)
    fun queryRevocations(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineRevocationList> = executeQuery(REVOCATIONS_QUERY_PATH, envelope, OfflineJsonParser::parseRevocations)

    fun submitSettlement(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String): CompletableFuture<OfflineSettlementSubmitResponse> =
        submitSettlement(transferPayload, authority, privateKeyHex, emptyList(), false)

    fun submitSettlementAndWait(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String, statusClient: IrohaClient): CompletableFuture<OfflineSettlementSubmitResponse> =
        submitSettlementAndWait(transferPayload, authority, privateKeyHex, emptyList(), false, statusClient, null)

    fun submitSettlementAndWait(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String, statusClient: IrohaClient, statusOptions: PipelineStatusOptions?): CompletableFuture<OfflineSettlementSubmitResponse> =
        submitSettlementAndWait(transferPayload, authority, privateKeyHex, emptyList(), false, statusClient, statusOptions)

    fun submitSettlement(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String, buildClaimOverrides: List<OfflineSettlementBuildClaimOverride>?, repairExistingBuildClaims: Boolean): CompletableFuture<OfflineSettlementSubmitResponse> {
        val overrides = buildClaimOverrides?.toList() ?: emptyList()
        val body = LinkedHashMap<String, Any?>()
        body["authority"] = authority; body["private_key"] = privateKeyHex; body["transfer"] = transferPayload
        if (overrides.isNotEmpty()) body["build_claim_overrides"] = overrides.map { it.toJsonMap() }
        if (repairExistingBuildClaims) body["repair_existing_build_claims"] = true
        val request = buildPostRequest(SETTLEMENTS_PATH, JsonEncoder.encode(body).toByteArray(StandardCharsets.UTF_8))
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseSettlementSubmitResponse)
    }

    fun submitSettlementAndWait(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String, buildClaimOverrides: List<OfflineSettlementBuildClaimOverride>?, repairExistingBuildClaims: Boolean, statusClient: IrohaClient): CompletableFuture<OfflineSettlementSubmitResponse> =
        submitSettlementAndWait(transferPayload, authority, privateKeyHex, buildClaimOverrides, repairExistingBuildClaims, statusClient, null)

    fun submitSettlementAndWait(transferPayload: Map<String, Any>, authority: String, privateKeyHex: String, buildClaimOverrides: List<OfflineSettlementBuildClaimOverride>?, repairExistingBuildClaims: Boolean, statusClient: IrohaClient, statusOptions: PipelineStatusOptions?): CompletableFuture<OfflineSettlementSubmitResponse> =
        submitSettlement(transferPayload, authority, privateKeyHex, buildClaimOverrides, repairExistingBuildClaims)
            .thenCompose { settlement -> statusClient.waitForTransactionStatus(settlement.transactionHashHex, statusOptions).thenApply { settlement } }

    fun getSettlement(bundleIdHex: String): CompletableFuture<OfflineTransferList.OfflineTransferItem> = executeGet("$SETTLEMENTS_PATH/${urlEncode(bundleIdHex.trim())}", OfflineJsonParser::parseTransferItem)
    fun getTransfer(bundleIdHex: String): CompletableFuture<OfflineTransferList.OfflineTransferItem> = executeGet("$TRANSFERS_PATH/${urlEncode(bundleIdHex.trim())}", OfflineJsonParser::parseTransferItem)

    fun getBundleProofStatus(bundleIdHex: String): CompletableFuture<OfflineBundleProofStatus> {
        val request = buildGetRequest(BUNDLE_PROOF_STATUS_PATH, mapOf("bundle_id_hex" to bundleIdHex.trim()))
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseBundleProofStatus)
    }

    fun buildProofRequest(params: OfflineProofRequestParams): CompletableFuture<OfflineProofRequestResult> {
        val request = buildPostRequest(TRANSFER_PROOF_PATH, params.toJsonBytes())
        notifyRequest(request)
        return executeHttpRequest(request) { payload -> OfflineProofRequestResult(params.kind, OfflineJsonParser.canonicalJson(payload)) }
    }

    fun issueCertificate(draft: OfflineWalletCertificateDraft): CompletableFuture<OfflineCertificateIssueResponse> {
        val body = JsonEncoder.encode(mapOf("certificate" to draft.toJsonMap())).toByteArray(StandardCharsets.UTF_8)
        val request = buildPostRequest(CERTIFICATE_ISSUE_PATH, body)
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseCertificateIssueResponse)
    }

    fun issueBuildClaim(requestBody: OfflineBuildClaimIssueRequest): CompletableFuture<OfflineBuildClaimIssueResponse> {
        val body = JsonEncoder.encode(requestBody.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        val request = buildPostRequest(BUILD_CLAIM_ISSUE_PATH, body)
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseBuildClaimIssueResponse)
    }

    fun registerAllowance(certificate: OfflineWalletCertificate, authority: String, privateKeyHex: String): CompletableFuture<Void> =
        registerAllowanceDetailed(certificate, authority, privateKeyHex).thenApply { null }

    fun registerAllowanceDetailed(certificate: OfflineWalletCertificate, authority: String, privateKeyHex: String): CompletableFuture<OfflineAllowanceRegisterResponse> {
        val request = buildPostRequest(ALLOWANCES_PATH, buildAllowanceRegisterBody(certificate, authority, privateKeyHex))
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseAllowanceRegisterResponse)
    }

    fun renewAllowance(certificateIdHex: String, certificate: OfflineWalletCertificate, authority: String, privateKeyHex: String): CompletableFuture<OfflineAllowanceRegisterResponse> {
        val path = "$ALLOWANCES_PATH/${urlEncode(certificateIdHex.trim())}/renew"
        val request = buildPostRequest(path, buildAllowanceRegisterBody(certificate, authority, privateKeyHex))
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseAllowanceRegisterResponse)
    }

    fun topUpAllowance(draft: OfflineWalletCertificateDraft, authority: String, privateKeyHex: String): CompletableFuture<OfflineTopUpResponse> =
        issueCertificate(draft).thenCompose { issued ->
            registerAllowanceDetailed(issued.certificate, authority, privateKeyHex).thenApply { registered ->
                ensureTopUpCertificateIdsMatch(issued.certificateIdHex, registered.certificateIdHex)
                OfflineTopUpResponse(issued, registered)
            }
        }

    fun topUpAllowanceRenewal(certificateIdHex: String, draft: OfflineWalletCertificateDraft, authority: String, privateKeyHex: String): CompletableFuture<OfflineTopUpResponse> =
        issueCertificateRenewal(certificateIdHex, draft).thenCompose { issued ->
            renewAllowance(certificateIdHex, issued.certificate, authority, privateKeyHex).thenApply { registered ->
                ensureTopUpCertificateIdsMatch(issued.certificateIdHex, registered.certificateIdHex)
                OfflineTopUpResponse(issued, registered)
            }
        }

    fun issueCertificateRenewal(certificateIdHex: String, draft: OfflineWalletCertificateDraft): CompletableFuture<OfflineCertificateIssueResponse> {
        val path = "$CERTIFICATE_RENEW_ISSUE_PATH/${urlEncode(certificateIdHex.trim())}/renew/issue"
        val body = JsonEncoder.encode(mapOf("certificate" to draft.toJsonMap())).toByteArray(StandardCharsets.UTF_8)
        val request = buildPostRequest(path, body)
        notifyRequest(request)
        return executeHttpRequest(request, OfflineJsonParser::parseCertificateIssueResponse)
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
        fun defaultHeaders(headers: Map<String, String>?): Builder { defaultHeaders.clear(); headers?.forEach { (k, v) -> if (k != null && v != null) defaultHeaders[k] = v }; return this }
        fun addObserver(observer: ClientObserver?): Builder { if (observer != null) observers.add(observer); return this }
        fun observers(observers: List<ClientObserver>?): Builder { this.observers.clear(); observers?.forEach { addObserver(it) }; return this }
        fun build(): OfflineToriiClient = OfflineToriiClient(this)
    }

    companion object {
        private const val ALLOWANCES_PATH = "/v1/offline/allowances"
        private const val TRANSFERS_PATH = "/v1/offline/transfers"
        private const val SETTLEMENTS_PATH = "/v1/offline/settlements"
        private const val SUMMARIES_PATH = "/v1/offline/summaries"
        private const val REVOCATIONS_PATH = "/v1/offline/revocations"
        private const val ALLOWANCES_QUERY_PATH = "/v1/offline/allowances/query"
        private const val TRANSFERS_QUERY_PATH = "/v1/offline/transfers/query"
        private const val SUMMARIES_QUERY_PATH = "/v1/offline/summaries/query"
        private const val REVOCATIONS_QUERY_PATH = "/v1/offline/revocations/query"
        private const val TRANSFER_PROOF_PATH = "/v1/offline/transfers/proof"
        private const val BUNDLE_PROOF_STATUS_PATH = "/v1/offline/bundle/proof_status"
        private const val CERTIFICATE_ISSUE_PATH = "/v1/offline/certificates/issue"
        private const val BUILD_CLAIM_ISSUE_PATH = "/v1/offline/build-claims/issue"
        private const val CERTIFICATE_RENEW_ISSUE_PATH = "/v1/offline/certificates"

        @JvmStatic fun builder(): Builder = Builder()

        private fun buildAllowanceRegisterBody(certificate: OfflineWalletCertificate, authority: String, privateKeyHex: String): ByteArray {
            val body = LinkedHashMap<String, Any>(); body["authority"] = authority; body["private_key"] = privateKeyHex; body["certificate"] = certificate.toJsonMap()
            return JsonEncoder.encode(body).toByteArray(StandardCharsets.UTF_8)
        }
        private fun ensureTopUpCertificateIdsMatch(issuedId: String?, registeredId: String?) {
            check(issuedId != null && registeredId != null) { "Missing certificate id in top-up responses" }
            check(issuedId.equals(registeredId, ignoreCase = true)) { "Top-up certificate id mismatch: issued=$issuedId registered=$registeredId" }
        }
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
