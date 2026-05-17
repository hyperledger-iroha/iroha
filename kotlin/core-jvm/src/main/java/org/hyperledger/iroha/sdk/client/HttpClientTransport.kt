package org.hyperledger.iroha.sdk.client

import java.io.IOException
import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.nio.file.Path
import java.time.Duration
import java.util.Arrays
import java.util.LinkedHashMap
import java.util.Locale
import java.util.Optional
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.function.Function
import org.hyperledger.iroha.sdk.crypto.KeyManagementException
import org.hyperledger.iroha.sdk.client.queue.PendingTransactionQueue
import org.hyperledger.iroha.sdk.crypto.export.KeyExportBundle
import org.hyperledger.iroha.sdk.crypto.export.KeyExportException
import org.hyperledger.iroha.sdk.nexus.*
import org.hyperledger.iroha.sdk.offline.OfflineJournalKey
import org.hyperledger.iroha.sdk.sorafs.GatewayFetchRequest
import org.hyperledger.iroha.sdk.sorafs.GatewayFetchSummary
import org.hyperledger.iroha.sdk.sorafs.SorafsGatewayClient
import org.hyperledger.iroha.sdk.telemetry.*
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamClient
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/**
 * HTTP-based client implementation that will forward transactions to an Iroha Torii endpoint.
 *
 * Serialization and endpoint construction follow the `/v1/pipeline/transactions` Torii route.
 * Network execution is delegated to [HttpTransportExecutor] so tests can run without making
 * outbound calls.
 */
class HttpClientTransport(
    private val executor: HttpTransportExecutor,
    private val config: ClientConfig
) : IrohaClient {

    private val sorafsGatewayClient: SorafsGatewayClient = SorafsGatewayClient(
        executor = executor, baseUri = config.sorafsGatewayUri(), timeout = config.requestTimeout(),
        defaultHeaders = config.defaultHeaders(), observers = config.observers())
    private val deviceProfileEmitted = AtomicBoolean(false)
    private val lazyScheduler = lazy {
        Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "iroha-http-retry").apply { isDaemon = true }
        }
    }
    private val scheduler: ScheduledExecutorService by lazyScheduler

    override fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse> {
        val hashHex = SignedTransactionHasher.hashHex(transaction)
        return flushPendingQueue().exceptionally { null }.thenCompose { submitWithRetryInternal(transaction, hashHex, 1, true) }
    }

    override fun submitTransactionEntrypoint(encodedVersionedEntrypoint: ByteArray): CompletableFuture<ClientResponse> {
        val request = ToriiRequestBuilder.buildSubmitEntrypointRequest(
            config.baseUri(),
            encodedVersionedEntrypoint,
            config.requestTimeout(),
            config.defaultHeaders(),
        )
        notifyRequest(request)
        return executor.execute(request).handle { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!)
                val failed = CompletableFuture<ClientResponse>()
                failed.completeExceptionally(cause)
                return@handle failed
            }
            val clientResponse = ClientResponse(
                response.statusCode,
                response.body,
                response.message,
                extractTransactionHash(response),
                extractRejectCode(response),
            )
            if (clientResponse.statusCode < 200 || clientResponse.statusCode >= 300) {
                notifyFailure(request, RuntimeException("Torii request failed with status ${clientResponse.statusCode}"))
            } else {
                notifyResponse(request, clientResponse)
            }
            CompletableFuture.completedFuture(clientResponse)
        }.thenCompose { it }
    }

    override fun waitForTransactionStatus(hashHex: String, options: PipelineStatusOptions?): CompletableFuture<Map<String, Any>> {
        val resolved = PipelineStatusOptions.resolve(options)
        val deadline = if (resolved.timeoutMillis == null) Long.MAX_VALUE else System.currentTimeMillis() + maxOf(0L, resolved.timeoutMillis!!)
        val future = CompletableFuture<Map<String, Any>>()
        pollPipelineStatus(hashHex, resolved, deadline, 0, null, future)
        return future
    }

    fun config(): ClientConfig = config
    fun invalidateAndCancel() {
        executor.invalidateAndCancel()
        if (lazyScheduler.isInitialized()) scheduler.shutdownNow()
    }
    fun newNoritoRpcClient(): NoritoRpcClient = config.toNoritoRpcClient(executor)
    fun newEventStreamClient(): ToriiEventStreamClient = ToriiEventStreamClient.builder().setBaseUri(config.baseUri()).setTransportExecutor(executor).defaultHeaders(config.defaultHeaders()).observers(config.observers()).build()
    fun newSorafsGatewayClient(): SorafsGatewayClient = newSorafsGatewayClient(config.sorafsGatewayUri())
    fun newSorafsGatewayClient(baseUri: URI): SorafsGatewayClient = SorafsGatewayClient(executor = executor, baseUri = baseUri, timeout = config.requestTimeout(), defaultHeaders = config.defaultHeaders(), observers = config.observers())
    fun sorafsGatewayClient(): SorafsGatewayClient = sorafsGatewayClient
    fun sorafsGatewayFetch(request: GatewayFetchRequest): CompletableFuture<ClientResponse> = sorafsGatewayClient.fetch(request)
    fun sorafsGatewayFetchSummary(request: GatewayFetchRequest): CompletableFuture<GatewayFetchSummary> = sorafsGatewayClient.fetchSummary(request)

    fun getUaidPortfolio(uaid: String): CompletableFuture<UaidPortfolioResponse> = getUaidPortfolio(uaid, null)
    fun getUaidPortfolio(uaid: String, query: UaidPortfolioQuery?): CompletableFuture<UaidPortfolioResponse> {
        val canonical = UaidLiteral.canonicalize(uaid, "uaid portfolio")
        val params = query?.toQueryParameters() ?: emptyMap()
        val request = buildJsonGetRequest("/v1/accounts/${encodePathSegment(canonical)}/portfolio", params)
        return fetchJson(request, UaidJsonParser::parsePortfolio, "UAID portfolio")
    }

    fun getUaidBindings(uaid: String): CompletableFuture<UaidBindingsResponse> = getUaidBindings(uaid, null)
    fun getUaidBindings(uaid: String, query: UaidBindingsQuery?): CompletableFuture<UaidBindingsResponse> {
        val canonical = UaidLiteral.canonicalize(uaid, "uaid bindings")
        val params = query?.toQueryParameters() ?: emptyMap()
        return fetchJson(buildJsonGetRequest("/v1/space-directory/uaids/${encodePathSegment(canonical)}", params), UaidJsonParser::parseBindings, "UAID bindings")
    }

    fun getUaidManifests(uaid: String, query: UaidManifestQuery?): CompletableFuture<UaidManifestsResponse> {
        val canonical = UaidLiteral.canonicalize(uaid, "uaid manifests")
        val params = query?.toQueryParameters() ?: emptyMap()
        return fetchJson(buildJsonGetRequest("/v1/space-directory/uaids/${encodePathSegment(canonical)}/manifests", params), UaidJsonParser::parseManifests, "UAID manifests")
    }

    fun listIdentifierPolicies(): CompletableFuture<IdentifierPolicyListResponse> = fetchJson(buildJsonGetRequest("/v1/identifier-policies", emptyMap()), IdentifierJsonParser::parsePolicyList, "identifier policy list")
    fun listRamLfeProgramPolicies(): CompletableFuture<RamLfeProgramPolicyListResponse> = fetchJson(buildJsonGetRequest("/v1/ram-lfe/program-policies", emptyMap()), RamLfeJsonParser::parsePolicyList, "ram-lfe program policy list")

    fun getIdentifierClaimByReceiptHash(receiptHash: String): CompletableFuture<Optional<IdentifierClaimRecord>> {
        val normalizedReceiptHash = normalizeHex32(receiptHash, "receiptHash")
        return fetchJsonAllowingNotFound(buildJsonGetRequest("/v1/identifiers/receipts/${encodePathSegment(normalizedReceiptHash)}", emptyMap()), IdentifierJsonParser::parseClaimRecord, "identifier claim lookup")
    }

    fun resolveIdentifier(requestBody: IdentifierResolveRequest): CompletableFuture<Optional<IdentifierResolutionReceipt>> {
        val body = encodeJsonBody(buildIdentifierResolvePayload(requestBody.policyId, requestBody.input, requestBody.encryptedInputHex))
        return fetchJsonAllowingNotFound(buildJsonPostRequest("/v1/identifiers/resolve", body), IdentifierJsonParser::parseResolutionReceipt, "identifier resolve")
    }

    fun resolveIdentifier(policyId: String, input: String?, encryptedInputHex: String?): CompletableFuture<Optional<IdentifierResolutionReceipt>> = resolveIdentifier(buildIdentifierResolveRequest(policyId, input, encryptedInputHex))

    override fun resolveAccountAlias(alias: String): CompletableFuture<Optional<AccountAliasResolution>> {
        val normalizedAlias = normalizeNonBlank(alias, "alias")
        val body = encodeJsonBody(linkedMapOf("alias" to normalizedAlias))
        return fetchJsonAllowingNotFound(buildJsonPostRequest("/v1/aliases/resolve", body), AccountAliasJsonParser::parseResolution, "account alias resolve")
    }

    fun issueIdentifierClaimReceipt(accountId: String, requestBody: IdentifierResolveRequest): CompletableFuture<Optional<IdentifierResolutionReceipt>> {
        val normalizedAccountId = normalizeNonBlank(accountId, "accountId")
        val body = encodeJsonBody(buildIdentifierResolvePayload(requestBody.policyId, requestBody.input, requestBody.encryptedInputHex))
        return fetchJsonAllowingNotFound(buildJsonPostRequest("/v1/accounts/${encodePathSegment(normalizedAccountId)}/identifiers/claim-receipt", body), IdentifierJsonParser::parseResolutionReceipt, "identifier claim receipt")
    }

    fun issueIdentifierClaimReceipt(accountId: String, policyId: String, input: String?, encryptedInputHex: String?): CompletableFuture<Optional<IdentifierResolutionReceipt>> = issueIdentifierClaimReceipt(accountId, buildIdentifierResolveRequest(policyId, input, encryptedInputHex))

    fun executeRamLfeProgram(programId: String, requestBody: RamLfeExecuteRequest): CompletableFuture<Optional<RamLfeExecuteResponse>> {
        val normalizedProgramId = normalizeNonBlank(programId, "programId")
        val body = encodeJsonBody(buildRamLfeExecutePayload(requestBody.inputHex, requestBody.encryptedInputHex))
        return fetchJsonAllowingNotFound(buildJsonPostRequest("/v1/ram-lfe/programs/${encodePathSegment(normalizedProgramId)}/execute", body), RamLfeJsonParser::parseExecuteResponse, "ram-lfe execute")
    }

    fun executeRamLfeProgram(programId: String, inputHex: String?, encryptedInputHex: String?): CompletableFuture<Optional<RamLfeExecuteResponse>> = executeRamLfeProgram(programId, buildRamLfeExecuteRequest(inputHex, encryptedInputHex))

    fun verifyRamLfeReceipt(requestBody: RamLfeReceiptVerifyRequest): CompletableFuture<RamLfeReceiptVerifyResponse> {
        val body = encodeJsonBody(buildRamLfeReceiptVerifyPayload(requestBody.receipt, requestBody.outputHex))
        return fetchJson(buildJsonPostRequest("/v1/ram-lfe/receipts/verify", body), RamLfeJsonParser::parseReceiptVerifyResponse, "ram-lfe receipt verify")
    }

    fun verifyRamLfeReceipt(receipt: Map<String, Any>, outputHex: String?): CompletableFuture<RamLfeReceiptVerifyResponse> = verifyRamLfeReceipt(RamLfeReceiptVerifyRequest(receipt, outputHex))

    fun getVpnProfile(): CompletableFuture<VpnProfile> =
        fetchJson(buildJsonGetRequest("/v1/vpn/profile", emptyMap()), VpnJsonParser::parseProfile, "vpn profile")

    fun registerPushDevice(requestBody: PushDeviceRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<ClientResponse> {
        val body = encodeJsonBody(buildPushDevicePayload(requestBody.accountId, requestBody.platform, requestBody.token, requestBody.topics))
        return executeAccepted(buildVpnRequest("POST", "/v1/notify/devices", body, canonicalAuth), "push device register", 202)
    }

    fun unregisterPushDevice(requestBody: PushDeviceRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<ClientResponse> {
        val body = encodeJsonBody(buildPushDevicePayload(requestBody.accountId, requestBody.platform, requestBody.token, requestBody.topics))
        return executeAccepted(buildVpnRequest("DELETE", "/v1/notify/devices", body, canonicalAuth), "push device unregister", 202)
    }

    fun createVpnQuote(requestBody: VpnQuoteCreateRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnQuote> {
        val body = encodeJsonBody(buildVpnQuoteCreatePayload(requestBody.exitClass, requestBody.meteringPublicKeyHex))
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/quotes", body, canonicalAuth), VpnJsonParser::parseQuote, "vpn quote create")
    }

    fun createVpnSession(requestBody: VpnSessionCreateRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnSession> {
        val body = encodeJsonBody(buildVpnSessionCreatePayload(requestBody.exitClass, requestBody.quoteId, requestBody.paymentTxHash, requestBody.meteringPublicKeyHex))
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/sessions", body, canonicalAuth), VpnJsonParser::parseSession, "vpn session create")
    }

    fun getVpnSession(sessionId: String, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<Optional<VpnSession>> {
        val normalizedSessionId = normalizeHex32(sessionId, "sessionId")
        return fetchJsonAllowingNotFound(
            buildVpnRequest("GET", "/v1/vpn/sessions/${encodePathSegment(normalizedSessionId)}", null, canonicalAuth),
            VpnJsonParser::parseSession,
            "vpn session lookup",
        )
    }

    fun deleteVpnSession(sessionId: String, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<Optional<VpnReceipt>> {
        val normalizedSessionId = normalizeHex32(sessionId, "sessionId")
        return fetchJsonAllowingNotFound(
            buildVpnRequest("DELETE", "/v1/vpn/sessions/${encodePathSegment(normalizedSessionId)}", null, canonicalAuth),
            VpnJsonParser::parseReceipt,
            "vpn session delete",
        )
    }

    fun submitVpnReceipt(requestBody: VpnReceiptSubmitRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnReceipt> {
        val body = encodeJsonBody(buildVpnReceiptSubmitPayload(requestBody.relayReceiptHex, requestBody.clientVoucherHex, requestBody.leaseIdHex))
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/receipts", body, canonicalAuth), VpnJsonParser::parseReceipt, "vpn receipt submit")
    }

    fun listVpnReceipts(canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnReceiptListResponse> =
        fetchJson(buildVpnRequest("GET", "/v1/vpn/receipts", null, canonicalAuth), VpnJsonParser::parseReceiptList, "vpn receipt list")

    fun deployContract(
        authority: String,
        privateKey: String,
        codeB64: String,
        contractAlias: String,
        leaseExpiryMs: Long? = null,
    ): CompletableFuture<Optional<ContractDeployResponse>> {
        val body = encodeJsonBody(buildDeployContractPayload(authority, privateKey, codeB64, contractAlias, leaseExpiryMs))
        return fetchOptionalJson(buildJsonPostRequest("/v1/contracts/deploy", body), ContractJsonParser::parseDeployResponse, "contract deploy")
    }

    fun callContract(
        authority: String,
        privateKey: String,
        gasLimit: Long,
        contractAddress: String? = null,
        contractAlias: String? = null,
        entrypoint: String? = null,
        payload: Any? = null,
        gasAssetId: String? = null,
    ): CompletableFuture<ContractCallResponse> {
        val body = encodeJsonBody(
            buildContractCallPayload(
                authority = authority,
                privateKey = privateKey,
                gasLimit = gasLimit,
                contractAddress = contractAddress,
                contractAlias = contractAlias,
                entrypoint = entrypoint,
                payload = payload,
                gasAssetId = gasAssetId,
            )
        )
        return fetchJson(buildJsonPostRequest("/v1/contracts/call", body), ContractJsonParser::parseCallResponse, "contract call")
    }

    fun getGovernanceContract(contractAddress: String): CompletableFuture<GovernanceContractResponse> {
        val normalizedAddress = normalizeNonBlank(contractAddress, "contractAddress")
        return fetchJson(
            buildJsonGetRequest("/v1/gov/contracts/${encodePathSegment(normalizedAddress)}", emptyMap()),
            ContractJsonParser::parseGovernanceContractResponse,
            "governance contract"
        )
    }

    fun offlineToriiClient(): OfflineToriiClient = config.toOfflineToriiClient(executor)
    fun subscriptionToriiClient(): SubscriptionToriiClient = config.toSubscriptionToriiClient(executor)

    private fun flushPendingQueue(): CompletableFuture<Void?> {
        val queue = config.pendingQueue() ?: return CompletableFuture.completedFuture(null)
        val pending: List<SignedTransaction>
        try { pending = queue.drain() } catch (ex: IOException) { return CompletableFuture<Void?>().also { it.completeExceptionally(RuntimeException("Failed to drain pending queue", ex)) } }
        recordPendingQueueDepth(queue)
        if (pending.isEmpty()) return CompletableFuture.completedFuture(null)
        var chain: CompletableFuture<Void?> = CompletableFuture.completedFuture(null)
        for (i in pending.indices) {
            val index = i; val queuedTx = pending[i]
            chain = chain.thenCompose {
                submitWithRetryInternal(queuedTx, SignedTransactionHasher.hashHex(queuedTx), 1, true)
                    .thenApply<Void?> { null }
                    .exceptionally { ex -> requeueRemaining(pending, index + 1); throw if (ex is CompletionException) ex else CompletionException(ex) }
            }
        }
        return chain
    }

    private fun submitWithRetryInternal(transaction: SignedTransaction, hashHex: String, attempt: Int, skipFlush: Boolean): CompletableFuture<ClientResponse> {
        if (!skipFlush) return flushPendingQueue().exceptionally { null }.thenCompose { submitWithRetryInternal(transaction, hashHex, attempt, true) }
        val request = ToriiRequestBuilder.buildSubmitRequest(config.baseUri(), transaction, config.requestTimeout(), config.defaultHeaders())
        notifyRequest(request)
        return executor.execute(request).handle { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!); return@handle scheduleRetry(transaction, hashHex, attempt, request, null, cause)
            }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, extractTransactionHash(response) ?: hashHex, extractRejectCode(response))
            if (clientResponse.statusCode < 200 || clientResponse.statusCode >= 300) {
                if (config.retryPolicy().shouldRetryResponse(attempt, clientResponse)) return@handle scheduleRetry(transaction, hashHex, attempt, request, clientResponse, null)
                if (config.retryPolicy().isRetryableStatus(clientResponse.statusCode)) {
                    val error = RuntimeException("Torii request failed with status ${clientResponse.statusCode}")
                    notifyFailure(request, error); enqueuePending(transaction)
                    return@handle CompletableFuture<ClientResponse>().also { it.completeExceptionally(error) }
                }
                notifyResponse(request, clientResponse); return@handle CompletableFuture.completedFuture(clientResponse)
            }
            notifyResponse(request, clientResponse); CompletableFuture.completedFuture(clientResponse)
        }.thenCompose { it }
    }

    private fun scheduleRetry(transaction: SignedTransaction, hashHex: String, attempt: Int, request: TransportRequest, lastResponse: ClientResponse?, lastError: Throwable?): CompletableFuture<ClientResponse> {
        val isNetworkFailure = lastError != null && lastResponse == null
        val hasAnotherAttempt = if (isNetworkFailure) config.retryPolicy().shouldRetryError(attempt) else config.retryPolicy().allowsRetry(attempt)
        if (!hasAnotherAttempt) {
            enqueuePending(transaction)
            if (lastResponse != null && lastError == null) notifyFailure(request, RuntimeException("Retry attempts exhausted"))
            val runtime = if (lastError is RuntimeException) lastError else RuntimeException(if (lastResponse != null) "Retry attempts exhausted after status code ${lastResponse.statusCode}" else "Retry attempts exhausted; transaction queued for later submission", lastError)
            return CompletableFuture<ClientResponse>().also { it.completeExceptionally(runtime) }
        }
        val delay = config.retryPolicy().delayForAttempt(attempt)
        val delayMillis = maxOf(0L, minOf(delay.toMillis(), Long.MAX_VALUE))
        emitRetryTelemetry(request, attempt, delayMillis, lastResponse, lastError)
        val retryFuture = CompletableFuture<ClientResponse>()
        scheduler.schedule({
            submitWithRetryInternal(transaction, hashHex, attempt + 1, true).whenComplete { result, error ->
                if (error != null) retryFuture.completeExceptionally(error) else retryFuture.complete(result)
            }
        }, delayMillis, TimeUnit.MILLISECONDS)
        return retryFuture
    }

    private fun enqueuePending(transaction: SignedTransaction) {
        val queue = config.pendingQueue() ?: return
        try { val enriched = maybeAttachExportBundle(transaction); queue.enqueue(enriched); recordPendingQueueDepth(queue) }
        catch (ex: IOException) { throw RuntimeException("Failed to persist pending transaction", ex) }
    }

    private fun maybeAttachExportBundle(transaction: SignedTransaction): SignedTransaction {
        val alias = transaction.keyAlias().orElse(null)
        if (alias == null || transaction.exportedKeyBundle().isPresent) return transaction
        val exportOptions = config.exportOptions() ?: return transaction
        val passphrase = exportOptions.passphraseForAlias(alias)
        if (passphrase.isEmpty()) return transaction
        try {
            val keyPair = exportOptions.keyManager().load(alias)
                ?: throw KeyManagementException("Key not found for alias: $alias")
            val bundle = org.hyperledger.iroha.sdk.crypto.export.DeterministicKeyExporter.exportKeyPair(
                keyPair.private, keyPair.public, alias, passphrase)
            return SignedTransaction(transaction.encodedPayload(), transaction.signature(), transaction.publicKey(), transaction.schemaName(), transaction.keyAlias().orElse(null), bundle.encode())
        } catch (ex: Exception) { if (ex is KeyExportException || ex is KeyManagementException) throw RuntimeException("Failed to export key for pending transaction", ex); throw ex }
        finally { Arrays.fill(passphrase, '\u0000') }
    }

    private fun requeueRemaining(pending: List<SignedTransaction>, startIndex: Int) { for (i in startIndex until pending.size) enqueuePending(pending[i]) }

    private fun recordPendingQueueDepth(queue: PendingTransactionQueue?) {
        if (queue == null || !config.telemetryOptions().enabled) return
        val sink = config.telemetrySink().orElse(null) ?: return
        val depth: Int = try { queue.size() } catch (_: IOException) { return }
        sink.emitSignal("android.pending_queue.depth", mapOf("queue" to queue.telemetryQueueName(), "depth" to Integer.toUnsignedLong(depth)))
    }

    private fun emitDeviceProfileTelemetry() {
        if (!config.telemetryOptions().enabled || !deviceProfileEmitted.compareAndSet(false, true)) return
        val sink = config.telemetrySink().orElse(null) ?: return
        val provider = config.deviceProfileProvider() ?: return
        val profile = provider.snapshot().orElse(null) ?: return
        sink.emitSignal("android.telemetry.device_profile", mapOf("profile_bucket" to profile.bucket))
    }

    private fun emitNetworkContextTelemetry() {
        if (!config.telemetryOptions().enabled) return
        val sink = config.telemetrySink().orElse(null) ?: return
        val context = config.networkContextProvider().snapshot().orElse(null) ?: return
        sink.emitSignal("android.telemetry.network_context", context.toTelemetryFields())
    }

    private fun emitRetryTelemetry(request: TransportRequest, attempt: Int, delayMillis: Long, lastResponse: ClientResponse?, lastError: Throwable?) {
        if (!config.telemetryOptions().enabled) return; val sink = config.telemetrySink().orElse(null) ?: return
        val fields = LinkedHashMap<String, Any>()
        maybePutAuthorityHash(fields, request, sink, RETRY_SIGNAL_ID)
        fields["route"] = resolveRoute(request); fields["retry_count"] = attempt; fields["error_code"] = buildRetryErrorCode(lastResponse, lastError); fields["backoff_ms"] = delayMillis
        sink.emitSignal(RETRY_SIGNAL_ID, fields)
    }

    private fun emitPipelineStatusTelemetry(request: TransportRequest, transactionHash: String?, statusKind: String?, isSuccess: Boolean, isFailure: Boolean, attempts: Int) {
        if (!config.telemetryOptions().enabled) return; val sink = config.telemetrySink().orElse(null) ?: return
        val fields = LinkedHashMap<String, Any>()
        maybePutAuthorityHash(fields, request, sink, PIPELINE_STATUS_SIGNAL)
        if (!transactionHash.isNullOrBlank()) fields["tx_hash"] = transactionHash
        fields["status_kind"] = statusKind ?: ""; fields["outcome"] = if (isSuccess) "success" else if (isFailure) "failure" else "pending"; fields["attempts"] = attempts
        sink.emitSignal(PIPELINE_STATUS_SIGNAL, fields)
    }

    private fun maybePutAuthorityHash(fields: MutableMap<String, Any>, request: TransportRequest, sink: TelemetrySink, signalId: String) {
        val redaction = config.telemetryOptions().redaction; if (!redaction.enabled) return
        val authority = resolveAuthority(request).trim(); if (authority.isEmpty()) { emitRedactionFailure(sink, signalId, "blank_authority"); return }
        val hashed = redaction.hashAuthority(authority); if (hashed.isPresent) fields["authority_hash"] = hashed.get() else emitRedactionFailure(sink, signalId, "hash_failed")
    }

    private fun pollPipelineStatus(hashHex: String, options: PipelineStatusOptions, deadline: Long, attemptsSoFar: Int, lastPayload: Map<String, Any>?, future: CompletableFuture<Map<String, Any>>) {
        if (future.isDone) return
        if (options.maxAttempts != null && attemptsSoFar >= options.maxAttempts!!) { future.completeExceptionally(TransactionTimeoutException("Transaction $hashHex did not reach a terminal status after $attemptsSoFar attempts", hashHex, attemptsSoFar, lastPayload)); return }
        val request = ToriiRequestBuilder.buildStatusRequest(config.baseUri(), hashHex, config.requestTimeout(), config.defaultHeaders())
        notifyRequest(request)
        executor.execute(request).whenComplete { response, throwable ->
            try {
                if (future.isDone) return@whenComplete
                if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(cause); return@whenComplete }
                val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
                notifyResponse(request, clientResponse)
                val statusCode = clientResponse.statusCode
                if (statusCode != 200 && statusCode != 202 && statusCode != 204 && statusCode != 404) { future.completeExceptionally(buildPipelineStatusHttpException(hashHex, clientResponse)); return@whenComplete }
                val payload = parsePipelineStatusPayload(clientResponse.body)
                val nextAttempts = attemptsSoFar + 1
                val statusKind = if (payload == null) Optional.empty() else PipelineStatusExtractor.extractStatusKind(payload)
                val statusLiteral = statusKind.orElse(null)
                val isSuccess = statusLiteral != null && options.successStatuses.contains(statusLiteral)
                val isFailure = statusLiteral != null && options.failureStatuses.contains(statusLiteral)
                emitPipelineStatusTelemetry(request, hashHex, statusLiteral, isSuccess, isFailure, nextAttempts)
                if (options.observer != null) { try { options.observer.onStatus(statusLiteral ?: "", payload ?: emptyMap(), nextAttempts) } catch (observerError: RuntimeException) { future.completeExceptionally(observerError); return@whenComplete } }
                if (isSuccess) { future.complete(payload ?: emptyMap()); return@whenComplete }
                if (isFailure) { val rejectionReason = PipelineStatusExtractor.extractRejectionReason(payload).orElse(null); future.completeExceptionally(TransactionStatusException(hashHex, statusLiteral, rejectionReason, payload)); return@whenComplete }
                if (options.maxAttempts != null && nextAttempts >= options.maxAttempts!!) { future.completeExceptionally(TransactionTimeoutException("Transaction $hashHex did not reach a terminal status after $nextAttempts attempts", hashHex, nextAttempts, payload)); return@whenComplete }
                if (deadline != Long.MAX_VALUE && System.currentTimeMillis() >= deadline) { future.completeExceptionally(TransactionTimeoutException("Transaction $hashHex did not reach a terminal status within the configured timeout", hashHex, nextAttempts, payload)); return@whenComplete }
                scheduleNextPoll(hashHex, options, deadline, nextAttempts, payload, future)
            } catch (e: Exception) { if (!future.isDone) future.completeExceptionally(e) }
        }
    }

    private fun scheduleNextPoll(hashHex: String, options: PipelineStatusOptions, deadline: Long, attemptsSoFar: Int, lastPayload: Map<String, Any>?, future: CompletableFuture<Map<String, Any>>) {
        if (future.isDone) return
        val interval = options.intervalMillis
        val task = Runnable { pollPipelineStatus(hashHex, options, deadline, attemptsSoFar, lastPayload, future) }
        if (interval <= 0L) { task.run(); return }
        scheduler.schedule({ task.run() }, minOf(interval, Long.MAX_VALUE), TimeUnit.MILLISECONDS)
    }

    @Suppress("UNCHECKED_CAST")
    private fun parsePipelineStatusPayload(body: ByteArray?): Map<String, Any>? {
        if (body == null || body.isEmpty()) return null
        if (body.size >= 4 && body[0] == 'N'.code.toByte() && body[1] == 'R'.code.toByte() && body[2] == 'T'.code.toByte() && body[3] == '0'.code.toByte()) return null
        val json = String(body, StandardCharsets.UTF_8).trim(); if (json.isEmpty()) return null
        val parsed = JsonParser.parse(json); check(parsed is Map<*, *>) { "Pipeline status response must be a JSON object" }
        return parsed as Map<String, Any>
    }

    private fun notifyRequest(request: TransportRequest) { emitDeviceProfileTelemetry(); emitNetworkContextTelemetry(); for (o in config.observers()) o.onRequest(request) }
    private fun notifyResponse(request: TransportRequest, response: ClientResponse) { for (o in config.observers()) o.onResponse(request, response) }
    private fun notifyFailure(request: TransportRequest, error: Throwable) { for (o in config.observers()) o.onFailure(request, error) }

    private fun buildJsonGetRequest(path: String, queryParams: Map<String, String>): TransportRequest {
        val target = appendQuery(resolvePath(path), queryParams)
        val builder = TransportRequest.builder().setUri(target).setMethod("GET").addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        return builder.build()
    }

    private fun buildJsonPostRequest(path: String, body: ByteArray): TransportRequest {
        val builder = TransportRequest.builder().setUri(resolvePath(path)).setMethod("POST").setBody(body).addHeader("Content-Type", "application/json").addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        return builder.build()
    }

    private fun buildVpnRequest(method: String, path: String, body: ByteArray?, canonicalAuth: ToriiCanonicalRequestAuth): TransportRequest {
        val target = resolvePath(path)
        val builder = TransportRequest.builder().setUri(target).setMethod(method).addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        if (body != null) {
            builder.setBody(body).addHeader("Content-Type", "application/json")
        }
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        val canonicalHeaders = buildCanonicalHeaders(method, target, body, canonicalAuth)
        for ((k, v) in canonicalHeaders) builder.addHeader(k, v)
        return builder.build()
    }

    private fun buildCanonicalHeaders(method: String, target: URI, body: ByteArray?, canonicalAuth: ToriiCanonicalRequestAuth): Map<String, String> {
        val timestampMs = canonicalAuth.timestampMs
        val nonce = canonicalAuth.nonce
        require((timestampMs == null) == (nonce == null)) { "timestampMs and nonce must be provided together" }
        return if (timestampMs == null) {
            CanonicalRequestSigner.buildHeaders(method, target, body, canonicalAuth.accountId, canonicalAuth.privateKey)
        } else {
            CanonicalRequestSigner.buildHeaders(method, target, body, canonicalAuth.accountId, canonicalAuth.privateKey, timestampMs, nonce!!)
        }
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return config.baseUri()
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = config.baseUri().toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun <T> fetchJson(request: TransportRequest, parser: Function<ByteArray, T>, errorContext: String): CompletableFuture<T> {
        notifyRequest(request); val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(RuntimeException("$errorContext request failed", cause)); return@whenComplete }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            if (response.statusCode < 200 || response.statusCode >= 300) { val error = RuntimeException("$errorContext request failed with status ${response.statusCode}"); notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete }
            try { val parsed = parser.apply(response.body); notifyResponse(request, clientResponse); future.complete(parsed) }
            catch (ex: RuntimeException) { notifyFailure(request, ex); future.completeExceptionally(ex) }
        }; return future
    }

    private fun executeAccepted(request: TransportRequest, errorContext: String, acceptedStatus: Int): CompletableFuture<ClientResponse> {
        notifyRequest(request); val future = CompletableFuture<ClientResponse>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!)
                future.completeExceptionally(RuntimeException("$errorContext request failed", cause))
                return@whenComplete
            }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            if (response.statusCode != acceptedStatus) {
                val error = RuntimeException("$errorContext request failed with status ${response.statusCode}")
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            notifyResponse(request, clientResponse)
            future.complete(clientResponse)
        }; return future
    }

    private fun <T : Any> fetchJsonAllowingNotFound(request: TransportRequest, parser: Function<ByteArray, T>, errorContext: String): CompletableFuture<Optional<T>> {
        notifyRequest(request); val future = CompletableFuture<Optional<T>>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(RuntimeException("$errorContext request failed", cause)); return@whenComplete }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            if (response.statusCode == 404) { notifyResponse(request, clientResponse); future.complete(Optional.empty<T>()); return@whenComplete }
            if (response.statusCode < 200 || response.statusCode >= 300) { val error = RuntimeException("$errorContext request failed with status ${response.statusCode}"); notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete }
            try { val parsed = parser.apply(response.body); notifyResponse(request, clientResponse); future.complete(Optional.of<T>(parsed)) }
            catch (ex: RuntimeException) { notifyFailure(request, ex); future.completeExceptionally(ex) }
        }; return future
    }

    private fun <T : Any> fetchOptionalJson(request: TransportRequest, parser: Function<ByteArray, T>, errorContext: String): CompletableFuture<Optional<T>> {
        notifyRequest(request); val future = CompletableFuture<Optional<T>>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(RuntimeException("$errorContext request failed", cause)); return@whenComplete }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            if (response.statusCode < 200 || response.statusCode >= 300) { val error = RuntimeException("$errorContext request failed with status ${response.statusCode}"); notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete }
            if (response.body.isEmpty()) { notifyResponse(request, clientResponse); future.complete(Optional.empty<T>()); return@whenComplete }
            try { val parsed = parser.apply(response.body); notifyResponse(request, clientResponse); future.complete(Optional.of(parsed)) }
            catch (ex: RuntimeException) { notifyFailure(request, ex); future.completeExceptionally(ex) }
        }; return future
    }

    companion object {
        private const val RETRY_SIGNAL_ID = "android.torii.http.retry"
        private const val PIPELINE_STATUS_SIGNAL = "android.torii.pipeline.status"
        private const val REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure"

        @JvmStatic fun createDefault(config: ClientConfig): HttpClientTransport = HttpClientTransport(PlatformHttpTransportExecutor.createDefault(), config)
        @JvmStatic fun withExecutor(executor: HttpTransportExecutor, config: ClientConfig): HttpClientTransport = HttpClientTransport(executor, config)
        @JvmStatic fun withDefaultExecutor(config: ClientConfig): HttpClientTransport = HttpClientTransport(PlatformHttpTransportExecutor.createDefault(), config)
        /**
         * Builds a transport whose underlying [UrlConnectionTransportExecutor] runs the synchronous
         * HTTP work on [asyncExecutor]; pass `null` for behavior equivalent to [withDefaultExecutor].
         * The injected executor changes scheduling only and leaves URLConnection timeout defaults
         * unchanged for requests that do not specify their own timeout.
         * See [UrlConnectionTransportExecutor] for the full rationale (Android `StrictMode` /
         * `TrafficStats` interaction).
         */
        @JvmStatic fun withDefaultExecutor(config: ClientConfig, asyncExecutor: Executor?): HttpClientTransport =
            if (asyncExecutor == null) withDefaultExecutor(config) else HttpClientTransport(
                org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor(
                    connectTimeout = null,
                    readTimeout = null,
                    asyncExecutor = asyncExecutor,
                ),
                config,
            )
        @JvmStatic fun withOfflineJournalQueue(config: ClientConfig, journalPath: Path, key: OfflineJournalKey): ClientConfig = config.toBuilder().enableOfflineJournalQueue(journalPath, key).build()
        @JvmStatic fun withOfflineJournalQueue(config: ClientConfig, journalPath: Path, seed: ByteArray): ClientConfig = config.toBuilder().enableOfflineJournalQueue(journalPath, seed).build()
        @JvmStatic fun withOfflineJournalQueue(config: ClientConfig, journalPath: Path, passphrase: CharArray): ClientConfig = config.toBuilder().enableOfflineJournalQueue(journalPath, passphrase).build()
        @JvmStatic fun withDirectoryPendingQueue(config: ClientConfig, queueDir: Path): ClientConfig = config.toBuilder().enableDirectoryPendingQueue(queueDir).build()
        @JvmStatic fun withFilePendingQueue(config: ClientConfig, queueFile: Path): ClientConfig = config.toBuilder().enableFilePendingQueue(queueFile).build()

        private fun buildRetryErrorCode(lastResponse: ClientResponse?, lastError: Throwable?): String = lastResponse?.statusCode?.toString() ?: lastError?.javaClass?.simpleName ?: "unknown"
        private fun resolveRoute(request: TransportRequest?): String = request?.uri?.rawPath ?: ""
        private fun extractRejectCode(response: TransportResponse?): String? =
            if (response == null) null else HttpErrorMessageExtractor.extractRejectCode(
                response.headers,
                "x-iroha-reject-code",
                response.body,
            )
        private fun extractTransactionHash(response: TransportResponse?): String? {
            if (response == null) return null
            for (headerName in listOf("x-iroha-transaction-hash", "x-iroha-tx-hash")) {
                val values = response.headers[headerName] ?: continue
                for (value in values) {
                    val normalized = normalizeTransactionHashHeader(value)
                    if (normalized != null) return normalized
                }
            }
            return null
        }
        private fun normalizeTransactionHashHeader(value: String?): String? {
            var normalized = value?.trim() ?: return null
            if (normalized.startsWith("0x") || normalized.startsWith("0X")) {
                normalized = normalized.substring(2)
            }
            if (normalized.length != 64) return null
            if (!normalized.all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) {
                return null
            }
            return normalized.lowercase(Locale.ROOT)
        }
        private fun resolveAuthority(request: TransportRequest?): String {
            if (request == null) return ""; val uri = request.uri; if (uri != null && uri.authority != null) return uri.authority
            val host = request.headers["Host"]; return if (host.isNullOrEmpty()) "" else host[0]
        }
        private fun emitRedactionFailure(sink: TelemetrySink, signalId: String, reason: String) { sink.emitSignal(REDACTION_FAILURE_SIGNAL, mapOf("signal_id" to signalId, "reason" to reason)) }
        private fun buildPipelineStatusHttpException(hashHex: String, response: ClientResponse): TransactionStatusHttpException = TransactionStatusHttpException(hashHex, response.statusCode, response.rejectCode(), HttpErrorMessageExtractor.extractMessage(response.body))
        private fun appendQuery(target: URI, params: Map<String, String>): URI { if (params.isEmpty()) return target; val sb = StringBuilder(target.toString()).append(if (target.toString().contains("?")) "&" else "?").append(encodeQuery(params)); return URI.create(sb.toString()) }
        private fun encodeQuery(params: Map<String, String>): String = params.entries.joinToString("&") { (k, v) -> "${urlEncode(k)}=${urlEncode(v)}" }
        private fun encodePathSegment(segment: String): String = urlEncode(segment).replace("+", "%20")
        private fun urlEncode(value: String): String = URLEncoder.encode(value, StandardCharsets.UTF_8.name())
        private fun encodeJsonBody(payload: Map<String, Any>): ByteArray = JsonEncoder.encode(payload).toByteArray(StandardCharsets.UTF_8)

        @JvmStatic internal fun buildIdentifierResolveRequest(policyId: String, input: String?, encryptedInputHex: String?): IdentifierResolveRequest {
            val normalizedInput = normalizeOptionalNonBlank(input, "input")
            val normalizedEncryptedInput = encryptedInputHex?.let { normalizeEvenLengthHex(it, "encryptedInputHex") }
            require((normalizedInput == null) != (normalizedEncryptedInput == null)) { "Exactly one of input or encryptedInputHex must be provided" }
            return if (normalizedInput != null) IdentifierResolveRequest.plaintext(policyId, normalizedInput) else IdentifierResolveRequest.encrypted(policyId, normalizedEncryptedInput!!)
        }

        @JvmStatic internal fun buildRamLfeExecuteRequest(inputHex: String?, encryptedInputHex: String?): RamLfeExecuteRequest {
            val normalizedInputHex = inputHex?.let { normalizeEvenLengthHex(it, "inputHex") }
            val normalizedEncryptedInput = encryptedInputHex?.let { normalizeEvenLengthHex(it, "encryptedInputHex") }
            require((normalizedInputHex == null) != (normalizedEncryptedInput == null)) { "Exactly one of inputHex or encryptedInputHex must be provided" }
            return if (normalizedInputHex != null) RamLfeExecuteRequest.plaintext(normalizedInputHex) else RamLfeExecuteRequest.encrypted(normalizedEncryptedInput!!)
        }

        @JvmStatic internal fun buildIdentifierResolvePayload(policyId: String, input: String?, encryptedInputHex: String?): Map<String, Any> {
            val normalizedPolicyId = normalizeNonBlank(policyId, "policyId")
            val normalizedInput = normalizeOptionalNonBlank(input, "input")
            val normalizedEncryptedInput = encryptedInputHex?.let { normalizeEvenLengthHex(it, "encryptedInputHex") }
            require((normalizedInput == null) != (normalizedEncryptedInput == null)) { "Exactly one of input or encryptedInputHex must be provided" }
            val payload = LinkedHashMap<String, Any>(); payload["policy_id"] = normalizedPolicyId
            if (normalizedInput != null) payload["input"] = normalizedInput else payload["encrypted_input"] = normalizedEncryptedInput!!
            return payload
        }

        @JvmStatic internal fun buildRamLfeExecutePayload(inputHex: String?, encryptedInputHex: String?): Map<String, Any> {
            val normalizedInputHex = inputHex?.let { normalizeEvenLengthHex(it, "inputHex") }
            val normalizedEncryptedInput = encryptedInputHex?.let { normalizeEvenLengthHex(it, "encryptedInputHex") }
            require((normalizedInputHex == null) != (normalizedEncryptedInput == null)) { "Exactly one of inputHex or encryptedInputHex must be provided" }
            val payload = LinkedHashMap<String, Any>()
            if (normalizedInputHex != null) payload["input_hex"] = normalizedInputHex else payload["encrypted_input"] = normalizedEncryptedInput!!
            return payload
        }

        @JvmStatic internal fun buildRamLfeReceiptVerifyPayload(receipt: Map<String, Any>, outputHex: String?): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>(); payload["receipt"] = LinkedHashMap(receipt)
            if (outputHex != null) payload["output_hex"] = normalizeEvenLengthHex(outputHex, "outputHex")
            return payload
        }

        @JvmStatic internal fun buildVpnQuoteCreatePayload(exitClass: String?, meteringPublicKeyHex: String): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["exit_class"] = normalizeOptionalNonBlank(exitClass, "exitClass") ?: ""
            payload["metering_public_key_hex"] = normalizeHex32(meteringPublicKeyHex, "meteringPublicKeyHex")
            return payload
        }

        @JvmStatic internal fun buildPushDevicePayload(accountId: String, platform: String, token: String, topics: List<String>?): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["account_id"] = normalizeNonBlank(accountId, "accountId")
            payload["platform"] = normalizeNonBlank(platform, "platform")
            payload["token"] = normalizeNonBlank(token, "token")
            if (topics != null) payload["topics"] = topics.map { normalizeNonBlank(it, "topics") }
            return payload
        }

        @JvmStatic internal fun buildVpnSessionCreatePayload(
            exitClass: String?,
            quoteId: String,
            paymentTxHash: String,
            meteringPublicKeyHex: String,
        ): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["exit_class"] = normalizeOptionalNonBlank(exitClass, "exitClass") ?: ""
            payload["quote_id"] = normalizeHex32(quoteId, "quoteId")
            payload["payment_tx_hash"] = normalizeHex32(paymentTxHash, "paymentTxHash")
            payload["metering_public_key_hex"] = normalizeHex32(meteringPublicKeyHex, "meteringPublicKeyHex")
            return payload
        }

        @JvmStatic internal fun buildVpnReceiptSubmitPayload(relayReceiptHex: String, clientVoucherHex: String, leaseIdHex: String?): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["relay_receipt_hex"] = normalizeEvenLengthHex(relayReceiptHex, "relayReceiptHex")
            payload["client_voucher_hex"] = normalizeEvenLengthHex(clientVoucherHex, "clientVoucherHex")
            if (leaseIdHex != null) payload["lease_id_hex"] = normalizeHex32(leaseIdHex, "leaseIdHex")
            return payload
        }

        @JvmStatic internal fun buildDeployContractPayload(
            authority: String,
            privateKey: String,
            codeB64: String,
            contractAlias: String,
            leaseExpiryMs: Long?,
        ): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["authority"] = normalizeNonBlank(authority, "authority")
            payload["private_key"] = normalizeNonBlank(privateKey, "privateKey")
            payload["code_b64"] = normalizeRequiredBase64Payload(codeB64, "codeB64")
            payload["contract_alias"] = normalizeNonBlank(contractAlias, "contractAlias")
            if (leaseExpiryMs != null) {
                require(leaseExpiryMs >= 0) { "leaseExpiryMs must be non-negative" }
                payload["lease_expiry_ms"] = leaseExpiryMs
            }
            return payload
        }

        @JvmStatic internal fun buildContractCallPayload(
            authority: String,
            privateKey: String,
            gasLimit: Long,
            contractAddress: String?,
            contractAlias: String?,
            entrypoint: String?,
            payload: Any?,
            gasAssetId: String?,
        ): Map<String, Any> {
            require(gasLimit >= 0) { "gasLimit must be non-negative" }
            val normalized = LinkedHashMap<String, Any>()
            normalized["authority"] = normalizeNonBlank(authority, "authority")
            normalized["private_key"] = normalizeNonBlank(privateKey, "privateKey")
            normalized.putAll(buildContractTargetSelector(contractAddress, contractAlias))
            if (entrypoint != null) normalized["entrypoint"] = normalizeNonBlank(entrypoint, "entrypoint")
            if (payload != null) normalized["payload"] = payload
            if (gasAssetId != null) normalized["gas_asset_id"] = normalizeNonBlank(gasAssetId, "gasAssetId")
            normalized["gas_limit"] = gasLimit
            return normalized
        }

        @JvmStatic internal fun buildContractTargetSelector(contractAddress: String?, contractAlias: String?): Map<String, String> {
            val hasContractAddress = contractAddress != null
            val hasContractAlias = contractAlias != null
            require(hasContractAddress != hasContractAlias) { "Exactly one of contractAddress or contractAlias must be provided" }
            return if (hasContractAddress) mapOf("contract_address" to normalizeNonBlank(contractAddress!!, "contractAddress"))
            else mapOf("contract_alias" to normalizeNonBlank(contractAlias!!, "contractAlias"))
        }

        @JvmStatic internal fun normalizeRequiredBase64Payload(value: String, field: String): String {
            val normalized = normalizeNonBlank(value, field)
            val decoded = try {
                Base64.getDecoder().decode(normalized)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$field must be valid base64", ex)
            }
            require(decoded.isNotEmpty()) { "$field must not decode to empty bytes" }
            return normalized
        }

        @JvmStatic internal fun normalizeOptionalNonBlank(value: String?, field: String): String? = if (value == null) null else normalizeNonBlank(value, field)
        @JvmStatic internal fun normalizeNonBlank(value: String, field: String): String { val trimmed = value.trim(); require(trimmed.isNotEmpty()) { "$field must not be blank" }; return trimmed }
        @JvmStatic internal fun normalizeEvenLengthHex(value: String, field: String): String {
            var trimmed = normalizeNonBlank(value, field)
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) trimmed = trimmed.substring(2)
            require(trimmed.length % 2 == 0 && trimmed.isNotEmpty()) { "$field must be an even-length hex string" }
            for (c in trimmed) require(c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F') { "$field must be an even-length hex string" }
            return trimmed.lowercase()
        }
        @JvmStatic internal fun normalizeHex32(value: String, field: String): String { val normalized = normalizeEvenLengthHex(value, field); require(normalized.length == 64) { "$field must contain 64 hex characters" }; return normalized }
    }
}
