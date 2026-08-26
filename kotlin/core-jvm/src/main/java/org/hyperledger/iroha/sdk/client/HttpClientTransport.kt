package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.nio.file.Path
import java.security.MessageDigest
import java.time.Duration
import java.util.LinkedHashMap
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
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission
import org.hyperledger.iroha.sdk.consensus.SumeragiDiagnosticsStatus
import org.hyperledger.iroha.sdk.consensus.SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES
import org.hyperledger.iroha.sdk.consensus.SUMERAGI_STATUS_JSON_MAX_BYTES
import org.hyperledger.iroha.sdk.consensus.SumeragiV2Status
import org.hyperledger.iroha.sdk.nexus.*
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityAdmissionV1
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityManifestV1
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityTupleAdmissionV1
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1
import org.hyperledger.iroha.sdk.sorafs.GatewayFetchRequest
import org.hyperledger.iroha.sdk.sorafs.GatewayFetchSummary
import org.hyperledger.iroha.sdk.sorafs.SorafsGatewayClient
import org.hyperledger.iroha.sdk.telemetry.*
import org.hyperledger.iroha.sdk.client.stream.ToriiEventStreamClient
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.alias.AliasSetupPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasAutoRenewPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasLeaseRenewPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AliasLifecycleTransactionPlanJsonParser
import org.hyperledger.iroha.sdk.alias.AliasLifecycleTransactionPlanV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingJsonParser
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanReceiptV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingProofRequiredPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetClaimV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPolicyV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPrepareRequestV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedVerifier
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedVerifier
import org.hyperledger.iroha.sdk.alias.AccountOnboardingReceiptVerifier
import org.hyperledger.iroha.sdk.alias.AliasSetupReportV1
import org.hyperledger.iroha.sdk.alias.PreparedTransactionSubmitResponseV1
import org.hyperledger.iroha.sdk.alias.TairaPublicResetMutationBindingV1
import org.hyperledger.iroha.sdk.alias.requireOnboardingCredential
import org.hyperledger.iroha.sdk.alias.AliasTransactionPlanJsonParser
import org.hyperledger.iroha.sdk.alias.AliasTransactionPlanV1
import org.hyperledger.iroha.sdk.alias.AccountAliasName

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

    private val sorafsGatewayClient: SorafsGatewayClient by lazy {
        SorafsGatewayClient(
            baseUri = config.sorafsGatewayUri(),
            executor = executor,
            timeout = config.requestTimeout(),
            defaultHeaders = config.defaultHeaders(),
            observers = config.observers(),
        )
    }
    private val deviceProfileEmitted = AtomicBoolean(false)
    private val lazyScheduler = lazy {
        Executors.newSingleThreadScheduledExecutor { r ->
            Thread(r, "iroha-http-pipeline-poll").apply { isDaemon = true }
        }
    }
    private val scheduler: ScheduledExecutorService by lazyScheduler

    override fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse> {
        val hashHex = SignedTransactionHasher.hashHex(transaction)
        return submitOnce(transaction, hashHex)
    }

    override fun submitTransactionJson(encodedVersionedTransactionJson: ByteArray): CompletableFuture<ClientResponse> {
        val request = ToriiRequestBuilder.buildSubmitJsonRequest(
            config.baseUri(),
            encodedVersionedTransactionJson,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader(),
        )
        return ensureTransactionSubmissionCompatibility()
            .thenCompose { executeAccepted(request, "transaction JSON submit", 202) }
    }

    override fun submitSccpDestinationProof(
        request: SccpDestinationProofSubmitRequest,
    ): CompletableFuture<ClientResponse> =
        executeSccpJsonAccepted(
            buildBridgeJsonPostRequest("/v1/bridge/proofs/submit", request.toJsonBytes()),
            "SCCP destination proof submit",
        )

    override fun submitSccpNativeMessage(
        request: SccpNativeMessageSubmitRequest,
    ): CompletableFuture<ClientResponse> =
        executeSccpJsonAccepted(
            buildBridgeJsonPostRequest("/v1/bridge/messages", request.toJsonBytes()),
            "SCCP native message submit",
        )

    override fun submitTransactionEntrypoint(encodedVersionedEntrypoint: ByteArray): CompletableFuture<ClientResponse> {
        val request = ToriiRequestBuilder.buildSubmitEntrypointRequest(
            config.baseUri(),
            encodedVersionedEntrypoint,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader(),
        )
        return ensureTransactionSubmissionCompatibility().thenCompose {
            notifyRequest(request)
            executor.execute(request).handle { response, throwable ->
                if (throwable != null) {
                    val cause = unwrapCompletion(throwable)
                    notifyFailure(request, cause)
                    val failed = CompletableFuture<ClientResponse>()
                    failed.completeExceptionally(cause)
                    return@handle failed
                }
                val statusCode = response.statusCode
                if (statusCode != 202) {
                    val error = RuntimeException(
                        "transaction entrypoint submit request failed with status $statusCode",
                    )
                    notifyFailure(request, error)
                    return@handle CompletableFuture<ClientResponse>().also {
                        it.completeExceptionally(error)
                    }
                }
                val clientResponse = ClientResponse(
                    statusCode,
                    response.body,
                    response.message,
                    extractEntrypointHash(response),
                    extractRejectCode(response),
                )
                notifyResponse(request, clientResponse)
                CompletableFuture.completedFuture(clientResponse)
            }.thenCompose { it }
        }
    }

    override fun submitTransactionEntrypointJson(encodedVersionedEntrypointJson: ByteArray): CompletableFuture<ClientResponse> {
        val request = ToriiRequestBuilder.buildSubmitEntrypointJsonRequest(
            config.baseUri(),
            encodedVersionedEntrypointJson,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader(),
        )
        return ensureTransactionSubmissionCompatibility()
            .thenCompose { executeAccepted(request, "transaction entrypoint JSON submit", 202) }
    }

    override fun waitForTransactionStatus(hashHex: String, options: PipelineStatusOptions?): CompletableFuture<Map<String, Any>> {
        val resolved = PipelineStatusOptions.resolve(options)
        val timeoutMillis = resolved.timeoutMillis
        val deadline = if (timeoutMillis == null) {
            Long.MAX_VALUE
        } else {
            val now = System.currentTimeMillis()
            if (timeoutMillis > Long.MAX_VALUE - now) Long.MAX_VALUE else now + timeoutMillis
        }
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
    fun newDaToriiClient(): DaToriiClient = DaToriiClient.builder()
        .executor(executor)
        .baseUri(config.baseUri())
        .timeout(config.requestTimeout())
        .defaultHeaders(config.defaultHeaders())
        .observers(config.observers())
        .build()
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

    /** Fetch the exact result-bearing `SignedBlockWire` committed at `height`. */
    fun getLedgerExecutedBlockWire(height: BigInteger): CompletableFuture<ByteArray> {
        require(height.signum() > 0 && height.bitLength() <= 64) {
            "height must be a positive u64"
        }
        val request = buildExactNoritoGetRequest(
            "/v1/ledger/block/$height",
            EXECUTED_BLOCK_WIRE_MAX_BYTES,
        )
        return fetchExactNoritoBytes(request, "executed block wire")
    }

    /** Convenience overload for positive signed heights. */
    fun getLedgerExecutedBlockWire(height: Long): CompletableFuture<ByteArray> =
        getLedgerExecutedBlockWire(BigInteger.valueOf(height))

    /** Fetch the exact committed Exact12 manifest with one-shot canonical account authentication. */
    fun getPrivacyCapabilities(
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<PrivacyExact12CapabilityManifestV1> =
        fetchExactNoritoBytes(
            buildExactNoritoGetRequest(
                "/v1/privacy/capabilities",
                PrivacyExact12CapabilityManifestV1.MAX_ARCHIVE_BYTES.toLong(),
                canonicalAuth,
            ),
            "privacy capabilities",
        ).thenApply(PrivacyNativeBridge::decodeExact12CapabilityManifestV1)

    /**
     * Obtain the token required immediately before constructing a retained privacy action.
     *
     * The token is issued only when committed readiness/activation and the complete native local
     * profile tuple agree. A legacy snapshot or local catalog cannot enter this path. Capability
     * discovery is authenticated against the exact locally configured network.
     */
    fun requirePrivacyExact12CapabilityAdmission(
        protocolId: PrivacyProtocolIdV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<PrivacyExact12CapabilityTupleAdmissionV1> =
        getPrivacyCapabilities(canonicalAuth).thenApply { manifest ->
            PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
                manifest,
                protocolId,
            )
        }

    /** Fetch and strictly decode exact-lane SCCP capability discovery. */
    fun getSccpCapabilities(): CompletableFuture<SccpCapabilities> =
        fetchSccpJson(
            buildJsonGetRequest(
                "/v1/sccp/capabilities",
                emptyMap(),
                SCCP_CAPABILITIES_RESPONSE_MAX_BYTES,
            ),
            SccpJsonParser::parseCapabilities,
            "SCCP capabilities",
        )

    /** Fetch and strictly decode the authoritative typed SCCP route registry. */
    fun getSccpRegistry(): CompletableFuture<SccpRegistryV1> =
        fetchSccpJson(
            buildJsonGetRequest(
                "/v1/sccp/registry",
                emptyMap(),
                SCCP_JSON_RESPONSE_MAX_BYTES,
            ),
            SccpJsonParser::parseRegistry,
            "SCCP registry",
        )

    /** Fetch one query-free finalized SCCP message bundle by canonical message id. */
    fun getSccpMessageBundle(messageIdHex: String): CompletableFuture<SccpMessageBundleV1> {
        val messageId = normalizeExactNonZeroEvenLengthHex(messageIdHex, "messageIdHex", 32)
        return fetchSccpJson(
            buildJsonGetRequest(
                "/v1/sccp/proofs/message/$messageId",
                emptyMap(),
                SCCP_JSON_RESPONSE_MAX_BYTES,
            ),
            Function { bytes ->
                SccpJsonParser.parseMessageBundle(bytes).also {
                    require(it.messageIdHex == messageId) {
                        "SCCP bundle message id does not match the requested id"
                    }
                }
            },
            "SCCP message bundle",
        )
    }

    /** Fetch one query-free state-derived Groth16 request by canonical message id. */
    fun getSccpProofRequest(messageIdHex: String): CompletableFuture<SccpGroth16ProofRequestV1> {
        val messageId = normalizeExactNonZeroEvenLengthHex(messageIdHex, "messageIdHex", 32)
        return fetchSccpJson(
            buildJsonGetRequest(
                "/v1/sccp/proof-requests/$messageId",
                emptyMap(),
                SCCP_JSON_RESPONSE_MAX_BYTES,
            ),
            Function { bytes ->
                SccpJsonParser.parseProofRequest(bytes).also {
                    require(it.messageIdHex == messageId) {
                        "SCCP proof request message id does not match the requested id"
                    }
                }
            },
            "SCCP proof request",
        )
    }

    /** Fetch newest-first exact-context SCCP outbound messages. */
    @JvmOverloads
    fun getSccpRecentMessages(
        from: BigInteger? = null,
        afterIndex: Int? = null,
        limit: Int? = null,
    ): CompletableFuture<SccpRecentMessages> {
        require(from == null || (from.signum() > 0 && from.bitLength() <= 64)) {
            "from must be a positive u64 height"
        }
        require(afterIndex == null || from != null) { "afterIndex requires the paired from height" }
        require(afterIndex == null || afterIndex in 0 until SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1) {
            "afterIndex must be between 0 and ${SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1}"
        }
        require(limit == null || limit in 1..50) { "limit must be between 1 and 50" }
        val query = linkedMapOf<String, String>()
        from?.let { query["from"] = it.toString() }
        afterIndex?.let { query["after_index"] = it.toString() }
        limit?.let { query["limit"] = it.toString() }
        return fetchSccpJson(
            buildJsonGetRequest(
                "/v1/sccp/messages/recent",
                query,
                SCCP_RECENT_RESPONSE_MAX_BYTES,
            ),
            SccpJsonParser::parseRecentMessages,
            "SCCP recent messages",
        )
    }

    /** Continue newest-first SCCP discovery from an exact server-issued cursor. */
    @JvmOverloads
    fun getSccpRecentMessages(
        cursor: SccpRecentCursor,
        limit: Int? = null,
    ): CompletableFuture<SccpRecentMessages> =
        getSccpRecentMessages(cursor.from, cursor.afterIndex, limit)

    fun getIdentifierClaimByReceiptHash(receiptHash: String): CompletableFuture<Optional<IdentifierClaimRecord>> {
        val normalizedReceiptHash = normalizeHex32(receiptHash, "receiptHash")
        return fetchJsonAllowingNotFound(buildJsonGetRequest("/v1/identifiers/receipts/${encodePathSegment(normalizedReceiptHash)}", emptyMap()), IdentifierJsonParser::parseClaimRecord, "identifier claim lookup")
    }

    fun resolveIdentifier(
        requestBody: IdentifierResolveRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<IdentifierResolutionReceipt>> {
        val body = encodeJsonBody(buildIdentifierResolvePayload(requestBody.policyId, requestBody.encryptedInputHex, requestBody.outputOpening))
        return fetchJsonAllowingNotFound(buildVpnRequest("POST", "/v1/identifiers/resolve", body, canonicalAuth), IdentifierJsonParser::parseResolutionReceipt, "identifier resolve")
    }

    fun resolveIdentifier(
        policyId: String,
        encryptedInputHex: String,
        outputOpening: RamLfeOutputOpening,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<IdentifierResolutionReceipt>> =
        resolveIdentifier(IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening), canonicalAuth)

    override fun resolveAccountAlias(alias: String): CompletableFuture<Optional<AccountAliasResolution>> {
        val normalizedAlias = AccountAliasName.parse(alias).canonicalText()
        val body = encodeJsonBody(linkedMapOf("alias" to normalizedAlias))
        return fetchJsonAllowingNotFound(
            buildJsonPostRequest("/v1/aliases/resolve", body),
            Function { response -> parsePinnedAliasResolution(response, normalizedAlias) },
            "account alias resolve",
        )
    }

    /** Resolves a restricted account alias with canonical account/signature/timestamp/nonce headers. */
    override fun resolveAccountAlias(
        alias: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasResolution>> {
        val normalizedAlias = AccountAliasName.parse(alias).canonicalText()
        val body = encodeJsonBody(linkedMapOf("alias" to normalizedAlias))
        val request = buildVpnRequest("POST", "/v1/aliases/resolve", body, canonicalAuth)
        return fetchJsonAllowingNotFound(
            request,
            Function { response -> parsePinnedAliasResolution(response, normalizedAlias) },
            "account alias resolve",
        )
    }

    override fun planAliasSetup(
        request: AliasSetupPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasTransactionPlanV1> {
        val body = JsonEncoder.encode(request.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        val httpRequest = buildVpnRequest("POST", "/v1/aliases/setup/plan", body, canonicalAuth)
        return fetchJson(
            httpRequest,
            Function { response ->
                AliasTransactionPlanJsonParser.parse(response).also { plan ->
                    require(plan.body.authority == canonicalAuth.accountId) {
                        "alias setup plan authority does not match the canonical request signer"
                    }
                }
            },
            "alias setup plan",
            200,
        )
    }

    override fun planAliasLeaseRenewal(
        request: AliasLeaseRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasLifecycleTransactionPlanV1> =
        planAliasLifecycle(
            "/v1/aliases/lease/renew/plan",
            request.toJsonMap(),
            canonicalAuth,
            "alias lease renewal plan",
        )

    override fun planAliasAutoRenew(
        request: AliasAutoRenewPlanRequestV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AliasLifecycleTransactionPlanV1> =
        planAliasLifecycle(
            "/v1/aliases/auto-renew/plan",
            request.toJsonMap(),
            canonicalAuth,
            "alias auto-renew plan",
        )

    private fun planAliasLifecycle(
        path: String,
        requestBody: Map<String, Any?>,
        canonicalAuth: ToriiCanonicalRequestAuth,
        context: String,
    ): CompletableFuture<AliasLifecycleTransactionPlanV1> {
        val body = JsonEncoder.encode(requestBody).toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildVpnRequest("POST", path, body, canonicalAuth),
            Function { response ->
                AliasLifecycleTransactionPlanJsonParser.parse(response).also { plan ->
                    require(plan.body.authority == canonicalAuth.accountId) {
                        "$context authority does not match the canonical request signer"
                    }
                }
            },
            context,
            200,
        )
    }

    override fun planSponsoredAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountOnboardingPlanReceiptV1> {
        val body = JsonEncoder.encode(request.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildOnboardingRequest("POST", "/v1/accounts/onboard/plan", body, onboardingToken),
            Function { response ->
                AccountOnboardingReceiptVerifier.requireValidForRequest(
                    request,
                    AccountOnboardingJsonParser.parseReceipt(response),
                    expectedNetworkId,
                    expectedAuthority,
                )
            },
            "sponsored account onboarding plan",
            200,
        )
    }

    override fun prepareSponsoredAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        feePayment: FeePaymentIntent,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountOnboardingPrepareResponseV1> {
        AccountOnboardingReceiptVerifier.requireValidForRequest(
            request,
            receipt,
            expectedNetworkId,
            expectedAuthority,
        )
        require(binding.kind == TairaPublicResetMutationBindingV1.ONBOARDING) {
            "onboarding prepare requires an onboarding binding"
        }
        require(binding.executionExpiresAtUnixMs > System.currentTimeMillis()) {
            "onboarding prepare binding is expired"
        }
        val body = JsonEncoder.encode(
            AccountOnboardingPrepareRequestV1(binding, receipt, feePayment).toJsonMap(),
        )
            .toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildOnboardingRequest("POST", "/v1/accounts/onboard/prepare", body, onboardingToken),
            Function { response ->
                when (val result = AccountOnboardingJsonParser.parsePrepareResponse(response)) {
                    is AccountOnboardingPreparedTransactionV1 -> {
                        AccountOnboardingPreparedVerifier.requireValidPrepared(
                            result,
                            request,
                            receipt,
                            binding,
                            feePayment,
                            expectedNetworkId,
                            expectedAuthority,
                        )
                        result
                    }
                    is AccountOnboardingProofRequiredPrepareResponseV1 ->
                        AccountOnboardingPreparedVerifier.requireValidProofRequired(
                            result,
                            request,
                            receipt,
                            binding,
                            expectedNetworkId,
                            expectedAuthority,
                        )
                }
            },
            "sponsored account onboarding prepare",
            200,
        )
    }

    override fun verifyAccountOnboardingCurrentState(
        proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
        request: AccountOnboardingPlanRequestV1,
        receipt: AccountOnboardingPlanReceiptV1,
        binding: TairaPublicResetMutationBindingV1,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<AccountOnboardingCurrentStateV1> {
        AccountOnboardingPreparedVerifier.requireValidProofRequired(
            proofRequired,
            request,
            receipt,
            binding,
            expectedNetworkId,
            expectedAuthority,
        )
        val atomicRequest = AccountOnboardingCurrentStateRequestV1(
            proofRequired.accountId,
            proofRequired.alias,
        )
        val body = JsonEncoder.encode(atomicRequest.toJsonMap())
            .toByteArray(StandardCharsets.UTF_8)
        require(config.requireLocalSigningContext().networkId() == expectedNetworkId) {
            "atomic onboarding current-state signing requires the expected network context"
        }
        return fetchExactJson(
            buildVpnRequest(
                "POST",
                "/v1/accounts/onboarding/current-state",
                body,
                canonicalAuth,
                ACCOUNT_ONBOARDING_CURRENT_STATE_RESPONSE_MAX_BYTES,
            ),
            Function { payload ->
                AccountOnboardingJsonParser.parseCurrentStateResponse(payload)
                    .classify(atomicRequest, expectedNetworkId)
            },
            "atomic account onboarding current-state",
        )
    }

    override fun submitPreparedAccountOnboarding(
        request: AccountOnboardingPlanRequestV1,
        prepared: AccountOnboardingPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        onboardingToken: String,
        expectedAuthority: String,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<PreparedTransactionSubmitResponseV1> {
        AccountOnboardingPreparedVerifier.requireValidPrepared(
            prepared,
            request,
            prepared.receipt,
            prepared.binding,
            expectedFeePayment,
            expectedNetworkId,
            expectedAuthority,
        )
        require(prepared.binding.executionExpiresAtUnixMs > System.currentTimeMillis()) {
            "prepared onboarding binding is expired"
        }
        val body = JsonEncoder.encode(prepared.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildOnboardingRequest("POST", "/v1/accounts/onboard", body, onboardingToken),
            AccountOnboardingJsonParser::parseSubmitResponse,
            "prepared account onboarding submit",
            responseValidator = { response, statusCode ->
                AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
                    response,
                    prepared,
                    expectedFeePayment,
                    statusCode,
                )
            },
        )
    }

    override fun prepareAccountFaucetTransaction(
        claim: AccountFaucetClaimV1,
        binding: TairaPublicResetMutationBindingV1,
        feePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<AccountFaucetPreparedTransactionV1> {
        require(binding.kind == TairaPublicResetMutationBindingV1.FAUCET) {
            "faucet prepare requires a faucet binding"
        }
        require(binding.executionExpiresAtUnixMs > System.currentTimeMillis()) {
            "faucet prepare binding is expired"
        }
        val body = JsonEncoder.encode(
            AccountFaucetPrepareRequestV1(binding, claim, feePayment).toJsonMap(),
        ).toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildJsonPostRequest("/v1/accounts/faucet/prepare", body),
            Function { response ->
                AccountOnboardingJsonParser.parseFaucetPrepareResponse(response).also { prepared ->
                    AccountFaucetPreparedVerifier.requireValidPrepared(
                        prepared,
                        claim,
                        binding,
                        feePayment,
                        policy,
                        expectedNetworkId,
                    )
                }
            },
            "account faucet prepare",
            200,
        )
    }

    override fun submitPreparedAccountFaucetTransaction(
        prepared: AccountFaucetPreparedTransactionV1,
        expectedFeePayment: FeePaymentIntent,
        policy: AccountFaucetPolicyV1,
        expectedNetworkId: NetworkId,
    ): CompletableFuture<PreparedTransactionSubmitResponseV1> {
        AccountFaucetPreparedVerifier.requireValidPrepared(
            prepared,
            prepared.claim,
            prepared.binding,
            expectedFeePayment,
            policy,
            expectedNetworkId,
        )
        require(prepared.binding.executionExpiresAtUnixMs > System.currentTimeMillis()) {
            "prepared faucet binding is expired"
        }
        val body = JsonEncoder.encode(prepared.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        return fetchJson(
            buildJsonPostRequest("/v1/accounts/faucet", body),
            AccountOnboardingJsonParser::parseSubmitResponse,
            "prepared account faucet submit",
            responseValidator = { response, statusCode ->
                AccountFaucetPreparedVerifier.requireValidSubmitResponse(
                    response,
                    prepared,
                    expectedFeePayment,
                    policy,
                    expectedNetworkId,
                    statusCode,
                )
            },
        )
    }

    override fun getAccountOnboardingReadiness(
        onboardingToken: String,
    ): CompletableFuture<AliasSetupReportV1> = fetchJson(
        buildOnboardingRequest("GET", "/v1/accounts/onboarding/readiness", null, onboardingToken),
        AccountOnboardingJsonParser::parseReadiness,
        "account onboarding readiness",
        200,
    )

    override fun getSumeragiStatus(): CompletableFuture<SumeragiV2Status> =
        fetchExactJson(
            buildExactOperatorJsonGetRequest(
                "/v1/sumeragi/status",
                SUMERAGI_STATUS_JSON_MAX_BYTES,
            ),
            Function { payload -> SumeragiV2Status.parseJson(payload) },
            "Sumeragi status",
        )

    override fun getSumeragiDiagnostics(): CompletableFuture<SumeragiDiagnosticsStatus> =
        fetchExactJson(
            buildExactOperatorJsonGetRequest(
                "/v1/sumeragi/diagnostics",
                SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES,
            ),
            Function { payload -> SumeragiDiagnosticsStatus.parseJson(payload) },
            "Sumeragi diagnostics",
        )

    override fun resolveAccountAliasIndex(
        index: BigInteger,
    ): CompletableFuture<Optional<AccountAliasIndexResolution>> {
        requireAliasU64(index, "index")
        val body = encodeJsonBody(linkedMapOf("index" to index))
        return fetchJsonAllowingNotFound(
            buildJsonPostRequest("/v1/aliases/resolve-index", body),
            Function { response -> parsePinnedAliasIndexResolution(response, index) },
            "account alias index resolve",
        )
    }

    override fun resolveAccountAliasIndex(
        index: BigInteger,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasIndexResolution>> {
        requireAliasU64(index, "index")
        val body = encodeJsonBody(linkedMapOf("index" to index))
        return fetchJsonAllowingNotFound(
            buildVpnRequest("POST", "/v1/aliases/resolve-index", body, canonicalAuth),
            Function { response -> parsePinnedAliasIndexResolution(response, index) },
            "account alias index resolve",
        )
    }

    override fun listAccountAliases(
        request: AccountAliasesByAccountRequest,
    ): CompletableFuture<Optional<AccountAliasesByAccount>> {
        val body = JsonEncoder.encode(request.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        return fetchJsonAllowingNotFound(
            buildJsonPostRequest("/v1/aliases/by-account", body),
            Function { response -> parsePinnedAliasesByAccount(response, request) },
            "account aliases lookup",
        )
    }

    override fun listAccountAliases(
        request: AccountAliasesByAccountRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<AccountAliasesByAccount>> {
        val body = JsonEncoder.encode(request.toJsonMap()).toByteArray(StandardCharsets.UTF_8)
        return fetchJsonAllowingNotFound(
            buildVpnRequest("POST", "/v1/aliases/by-account", body, canonicalAuth),
            Function { response -> parsePinnedAliasesByAccount(response, request) },
            "account aliases lookup",
        )
    }

    private fun parsePinnedAliasResolution(
        response: ByteArray,
        requestedAlias: String,
    ): AccountAliasResolution = AccountAliasJsonParser.parseResolution(response).also { resolution ->
        require(AccountAliasName.parse(resolution.alias).canonicalText() == requestedAlias) {
            "account alias response does not match the requested alias"
        }
    }

    private fun parsePinnedAliasIndexResolution(
        response: ByteArray,
        requestedIndex: BigInteger,
    ): AccountAliasIndexResolution = AccountAliasReadJsonParser.parseIndexResolution(response).also { resolution ->
        require(resolution.index == requestedIndex) {
            "account alias index response does not match the requested index"
        }
    }

    private fun parsePinnedAliasesByAccount(
        response: ByteArray,
        request: AccountAliasesByAccountRequest,
    ): AccountAliasesByAccount = AccountAliasReadJsonParser.parseByAccount(response).also { aliases ->
        require(aliases.accountId == request.accountId) {
            "account aliases response does not match the requested account"
        }
        require(
            aliases.items.all { item ->
                (request.dataspace == null || item.dataspace == request.dataspace) &&
                    (request.domain == null || item.domain == request.domain)
            },
        ) { "account aliases response contains entries outside the requested scope" }
    }

    fun issueIdentifierClaimReceipt(
        accountId: String,
        requestBody: IdentifierResolveRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<IdentifierResolutionReceipt>> {
        val normalizedAccountId = org.hyperledger.iroha.sdk.address.requireCanonicalI105Address(accountId, "accountId")
        require(normalizedAccountId == canonicalAuth.accountId) {
            "canonicalAuth.accountId must equal the claim-receipt path accountId"
        }
        val body = encodeJsonBody(buildIdentifierResolvePayload(requestBody.policyId, requestBody.encryptedInputHex, requestBody.outputOpening))
        return fetchJsonAllowingNotFound(buildVpnRequest("POST", "/v1/accounts/${encodePathSegment(normalizedAccountId)}/identifiers/claim-receipt", body, canonicalAuth), IdentifierJsonParser::parseResolutionReceipt, "identifier claim receipt")
    }

    fun issueIdentifierClaimReceipt(
        accountId: String,
        policyId: String,
        encryptedInputHex: String,
        outputOpening: RamLfeOutputOpening,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<IdentifierResolutionReceipt>> =
        issueIdentifierClaimReceipt(accountId, IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening), canonicalAuth)

    fun executeRamLfeProgram(
        programId: String,
        requestBody: RamLfeExecuteRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<Optional<RamLfeExecuteResponse>> {
        val normalizedProgramId = normalizeNonBlank(programId, "programId")
        val body = encodeJsonBody(buildRamLfeExecutePayload(requestBody.encryptedInputHex))
        return fetchJsonAllowingNotFound(buildVpnRequest("POST", "/v1/ram-lfe/programs/${encodePathSegment(normalizedProgramId)}/execute", body, canonicalAuth), RamLfeJsonParser::parseExecuteResponse, "ram-lfe execute")
    }

    fun verifyRamLfeReceipt(
        requestBody: RamLfeReceiptVerifyRequest,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<RamLfeReceiptVerifyResponse> {
        val body = encodeJsonBody(buildRamLfeReceiptVerifyPayload(requestBody.receipt, requestBody.outputHex))
        return fetchJson(buildVpnRequest("POST", "/v1/ram-lfe/receipts/verify", body, canonicalAuth), RamLfeJsonParser::parseReceiptVerifyResponse, "ram-lfe receipt verify")
    }

    fun verifyRamLfeReceipt(
        receipt: Map<String, Any>,
        outputHex: String?,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<RamLfeReceiptVerifyResponse> =
        verifyRamLfeReceipt(RamLfeReceiptVerifyRequest(receipt, outputHex), canonicalAuth)

    fun getVpnProfile(): CompletableFuture<VpnProfile> {
        requireSecureVpnBaseUri()
        return fetchJson(buildJsonGetRequest("/v1/vpn/profile", emptyMap()), VpnJsonParser::parseProfile, "vpn profile", 200)
    }

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
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/quotes", body, canonicalAuth), VpnJsonParser::parseQuote, "vpn quote create", 201)
    }

    fun createVpnSession(requestBody: VpnSessionCreateRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnSession> {
        val body = encodeJsonBody(buildVpnSessionCreatePayload(requestBody.exitClass, requestBody.quoteId, requestBody.paymentTxHash, requestBody.meteringPublicKeyHex))
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/sessions", body, canonicalAuth), VpnJsonParser::parseSession, "vpn session create", 201)
    }

    fun getVpnSession(sessionId: String, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<Optional<VpnSession>> {
        val normalizedSessionId = normalizeHex16(sessionId, "sessionId")
        return fetchJsonAllowingNotFound(
            buildVpnRequest("GET", "/v1/vpn/sessions/${encodePathSegment(normalizedSessionId)}", null, canonicalAuth),
            VpnJsonParser::parseSession,
            "vpn session lookup",
            200,
        )
    }

    fun submitVpnReceipt(requestBody: VpnReceiptSubmitRequest, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnReceipt> {
        val body = encodeJsonBody(buildVpnReceiptSubmitPayload(requestBody.relayReceiptHex, requestBody.clientVoucherHex, requestBody.leaseIdHex))
        return fetchJson(buildVpnRequest("POST", "/v1/vpn/receipts", body, canonicalAuth), VpnJsonParser::parseReceipt, "vpn receipt submit", 201)
    }

    fun listVpnReceipts(canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<VpnReceiptListResponse> =
        fetchJson(buildVpnRequest("GET", "/v1/vpn/receipts", null, canonicalAuth), VpnJsonParser::parseReceiptList, "vpn receipt list", 200)

    /**
     * Prepare an unsigned verifying-key registration transaction for local signing.
     *
     * Requires [ClientConfig.localSigningContext] and rejects any draft not bound to that exact
     * network, the requested authority, and the exact requested registry record.
     */
    fun registerVerifyingKey(
        requestBody: VerifyingKeyRegisterRequest,
    ): CompletableFuture<VerifyingKeyTransactionDraft> {
        val signingContext = config.requireLocalSigningContext()
        val payload = buildVerifyingKeyRegisterPayload(requestBody)
        val body = encodeJsonBody(payload)
        return fetchJson(
            buildJsonPostRequest("/v1/zk/vk/register", body),
            { bytes ->
                VerifyingKeyTransactionDraftParser.parseRegister(
                    bytes,
                    signingContext.networkId(),
                    payload,
                )
            },
            "verifying key register draft",
            200,
        )
    }

    /**
     * Prepare an unsigned verifying-key update transaction for local signing.
     *
     * Requires [ClientConfig.localSigningContext] and rejects any draft not bound to that exact
     * network, the requested authority, and the exact requested registry record.
     */
    fun updateVerifyingKey(
        requestBody: VerifyingKeyUpdateRequest,
    ): CompletableFuture<VerifyingKeyTransactionDraft> {
        val signingContext = config.requireLocalSigningContext()
        val payload = buildVerifyingKeyUpdatePayload(requestBody)
        val body = encodeJsonBody(payload)
        return fetchJson(
            buildJsonPostRequest("/v1/zk/vk/update", body),
            { bytes ->
                VerifyingKeyTransactionDraftParser.parseUpdate(
                    bytes,
                    signingContext.networkId(),
                    payload,
                )
            },
            "verifying key update draft",
            200,
        )
    }

    /** Quote the exact unsigned transaction payload before replacing only its fee maxima. */
    fun quoteFees(
        unsignedPayload: Map<String, Any?>,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<FeeQuoteResponse> {
        requireNetworkTransactionDomain(unsignedPayload)
        val authority = requireCanonicalI105Address(
            unsignedPayload["authority"] as? String
                ?: throw IllegalArgumentException("unsignedPayload.authority must be a string"),
            "unsignedPayload.authority",
        )
        require(
            CanonicalRequestSigner.isCanonicalAsciiAccountAlias(canonicalAuth.accountId) ||
                sameFeeQuoteAccountIdentity(authority, canonicalAuth.accountId),
        ) {
            "canonicalAuth.accountId must identify unsignedPayload.authority or be a canonical account alias"
        }
        val requestedIntent = FeePaymentJson.parse(
            unsignedPayload["fee_payment"],
            "unsignedPayload.fee_payment",
        )
        val body = encodeJsonBody(linkedMapOf("payload" to unsignedPayload))
        return fetchJson(
            buildVpnRequest(
                "POST",
                "/v1/fees/quote",
                body,
                canonicalAuth,
                FEE_QUOTE_RESPONSE_MAX_BYTES,
            ),
            { response ->
                require(response.size.toLong() <= FEE_QUOTE_RESPONSE_MAX_BYTES) {
                    "fee quote response exceeds the $FEE_QUOTE_RESPONSE_MAX_BYTES byte limit"
                }
                FeePaymentJson.parseQuote(response)
            },
            "fee quote",
            200,
            exactJsonMediaType = true,
        ).thenApply { quote ->
            quote.validateForDraft(requestedIntent, authority)
            quote
        }
    }

    private fun requireNetworkTransactionDomain(
        unsignedPayload: Map<String, Any?>,
    ): NetworkId {
        for (field in listOf("chain", "chainId", "chain_id")) {
            require(field !in unsignedPayload) {
                "unsignedPayload contains retired transaction identity field `$field`"
            }
        }
        val domain = unsignedPayload["domain"] as? Map<*, *>
            ?: throw IllegalArgumentException(
                "unsignedPayload.domain must be TransactionDomain::Network",
            )
        require(
            domain.keys == setOf("kind", "value") &&
                domain["kind"] == "network" &&
                domain["value"] is String,
        ) {
            "unsignedPayload.domain must contain exactly kind=network and a NetworkId value"
        }
        return NetworkId.parse(domain["value"] as String)
    }

    /** Fetch one exact on-chain fee sponsor program under canonical request authentication. */
    fun getFeeSponsorProgram(
        programId: FeeSponsorProgramId,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<FeeSponsorProgramResponse> {
        val body = encodeJsonBody(linkedMapOf("program_id" to programId.literal()))
        return fetchJson(
            buildVpnRequest(
                "POST",
                "/v1/fee-sponsor-programs/by-id",
                body,
                canonicalAuth,
                FEE_SPONSOR_PROGRAM_RESPONSE_MAX_BYTES,
            ),
            FeePaymentJson::parseProgram,
            "fee sponsor program lookup",
            200,
            exactJsonMediaType = true,
        ).thenApply { program ->
            require(program.id == programId) {
                "fee sponsor program response id does not match the requested program"
            }
            program
        }
    }

    /**
     * Prepares an unsigned contract-call transaction for local signing.
     *
     * Private signing material is never accepted by or sent to Torii. Sign the
     * returned canonical transaction payload locally and submit the resulting signed
     * transaction through [submitTransaction].
     */
    fun prepareContractCall(
        authority: String,
        feePayment: FeePaymentIntent,
        contractAddress: String? = null,
        contractAlias: String? = null,
        entrypoint: String,
        payload: Any? = null,
    ): CompletableFuture<ContractCallResponse> {
        val requestPayload = buildContractCallDraftPayload(
            authority = authority,
            feePayment = feePayment,
            contractAddress = contractAddress,
            contractAlias = contractAlias,
            entrypoint = entrypoint,
            payload = payload,
        )
        val body = encodeJsonBody(requestPayload)
        return fetchJson(
            buildJsonPostRequest("/v1/contracts/call", body),
            ContractJsonParser::parseCallResponse,
            "contract call draft",
        ).thenApply { response ->
            validateContractCallDraft(response, requestPayload)
        }
    }

    override fun proposeMultisig(request: MultisigProposeRequest): CompletableFuture<MultisigResponse> {
        val body = encodeJsonBody(buildMultisigProposePayload(request))
        return fetchJson(
            buildJsonPostRequest("/v1/multisig/propose", body),
            ContractJsonParser::parseMultisigResponse,
            "multisig propose",
        ).thenApply { response -> validateMultisigResponse(response, request) }
    }

    fun getGovernanceContract(contractAddress: String, canonicalAuth: ToriiCanonicalRequestAuth): CompletableFuture<GovernanceContractResponse> {
        val normalizedAddress = normalizeNonBlank(contractAddress, "contractAddress")
        return fetchJson(
            buildVpnRequest("GET", "/v1/gov/contracts/${encodePathSegment(normalizedAddress)}", null, canonicalAuth),
            ContractJsonParser::parseGovernanceContractResponse,
            "governance contract"
        )
    }

    /** Draft one typed Parliament attempt for local transaction signing. */
    fun draftParliamentAttemptV1(
        proposal: ParliamentApiV1.Proposal,
        attemptSequence: Long,
        expectedProposalContentId: String,
        expectedGovernanceAttemptId: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<ParliamentAttemptDraftResponseV1> {
        val body = ParliamentApiV1.attemptDraftRequestJson(proposal, attemptSequence)
        return fetchJson(
            buildVpnRequest(
                "POST",
                ParliamentApiV1.ATTEMPT_DRAFT_PATH,
                body,
                canonicalAuth,
                1024L * 1024L,
            ),
            Function { response ->
                ParliamentApiV1.parseAttemptDraftResponse(
                    response,
                    expectedProposalContentId,
                    expectedGovernanceAttemptId,
                )
            },
            "Parliament attempt draft",
            200,
        )
    }

    /** Read and strictly validate one authenticated typed Parliament attempt. */
    fun getParliamentAttemptV1(
        governanceAttemptId: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<ParliamentAttemptReadResponseV1> = fetchJson(
        buildVpnRequest(
            "GET",
            ParliamentApiV1.attemptReadPath(governanceAttemptId),
            null,
            canonicalAuth,
            2L * ParliamentApiV1.MAX_STATE_BYTES + 2L * 1024L * 1024L,
        ),
        Function { response ->
            ParliamentApiV1.parseAttemptReadResponse(response, governanceAttemptId)
        },
        "Parliament attempt read",
        200,
    )

    /** Draft one closed public Parliament transition for local transaction signing. */
    fun draftParliamentTransitionV1(
        governanceAttemptId: String,
        transitionJson: ByteArray,
        expectedTransitionKind: String,
        expectedTransitionDigest: ByteArray,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<ParliamentTransitionDraftResponseV1> {
        val body = ParliamentApiV1.transitionDraftRequestJson(governanceAttemptId, transitionJson)
        return fetchJson(
            buildVpnRequest(
                "POST",
                ParliamentApiV1.TRANSITION_DRAFT_PATH,
                body,
                canonicalAuth,
                1024L * 1024L,
            ),
            Function { response ->
                ParliamentApiV1.parseTransitionDraftResponse(
                    response,
                    governanceAttemptId,
                    expectedTransitionKind,
                    expectedTransitionDigest,
                )
            },
            "Parliament transition draft",
            200,
        )
    }

    /** Fetch the complete public transcript for one currently authorized TLE release. */
    fun getParliamentTleReleaseContextV1(
        ballotAttemptId: String,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<ParliamentTleReleaseContextResponseV1> = fetchJson(
        buildVpnRequest(
            "GET",
            ParliamentApiV1.tleReleaseContextReadPath(ballotAttemptId),
            null,
            canonicalAuth,
            1024L * 1024L,
        ),
        Function { response ->
            ParliamentApiV1.parseTleReleaseContextResponse(response, ballotAttemptId)
        },
        "Parliament TLE release context",
        200,
    )

    /** Request one node-local proof-carrying partial bound to an admitted release context. */
    fun requestParliamentTlePartialReleaseV1(
        ballotAttemptId: String,
        context: ParliamentTleReleaseContextResponseV1,
        canonicalAuth: ToriiCanonicalRequestAuth,
    ): CompletableFuture<ParliamentTlePartialReleaseShareV1> {
        require(context.ballotAttemptId == ballotAttemptId) {
            "release context ballot id differs from the partial-release request"
        }
        return fetchJson(
            buildVpnRequest(
                "POST",
                ParliamentApiV1.tlePartialReleasePath(ballotAttemptId),
                null,
                canonicalAuth,
                16L * 1024L,
            ),
            Function { response ->
                ParliamentApiV1.parseTlePartialReleaseResponse(
                    response,
                    context.keySession.keySessionId,
                    context.identityDigest,
                    context.keySession.committeeSize,
                )
            },
            "Parliament TLE partial release",
            200,
        )
    }

    override fun getContractManifest(codeHash: String): CompletableFuture<ContractManifestRecord> {
        require(codeHash.length == 64) { "codeHash must contain exactly 64 hex characters" }
        val normalizedCodeHash = normalizeExactEvenLengthHex(codeHash, "codeHash")
        return fetchJson(
            buildJsonGetRequest(
                "/v1/contracts/code/${encodePathSegment(normalizedCodeHash)}",
                emptyMap(),
            ),
            ContractJsonParser::parseManifestRecord,
            "contract manifest",
        )
    }

    fun subscriptionToriiClient(): SubscriptionToriiClient = config.toSubscriptionToriiClient(executor)

    private fun submitOnce(
        transaction: SignedTransaction,
        hashHex: String,
    ): CompletableFuture<ClientResponse> {
        val request = ToriiRequestBuilder.buildSubmitRequest(
            config.baseUri(),
            transaction,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader(),
        )

        return ensureTransactionSubmissionCompatibility().thenCompose {
            notifyRequest(request)
            executor.execute(request).handle { response, throwable ->
                if (throwable != null) {
                    val cause = unwrapCompletion(throwable)
                    val error = AmbiguousTransactionSubmissionException(
                        hashHex,
                        null,
                        null,
                        null,
                        cause,
                    )
                    notifyFailure(request, error)
                    return@handle CompletableFuture<ClientResponse>().also {
                        it.completeExceptionally(error)
                    }
                }
                val statusCode = response.statusCode
                val rejectCode = extractRejectCode(response)
                val responseBody = HttpErrorMessageExtractor.extractMessage(response.body)
                if (submissionOutcomeIsAmbiguous(statusCode)) {
                    val error = AmbiguousTransactionSubmissionException(
                        hashHex,
                        statusCode,
                        rejectCode,
                        responseBody,
                        null,
                    )
                    notifyFailure(request, error)
                    return@handle CompletableFuture<ClientResponse>().also {
                        it.completeExceptionally(error)
                    }
                }
                if (statusCode != 202) {
                    val error = TransactionSubmissionHttpException(
                        hashHex,
                        statusCode,
                        rejectCode,
                        responseBody,
                    )
                    notifyFailure(request, error)
                    return@handle CompletableFuture<ClientResponse>().also {
                        it.completeExceptionally(error)
                    }
                }
                val clientResponse = ClientResponse(
                    statusCode,
                    response.body,
                    response.message,
                    extractEntrypointHash(response) ?: hashHex,
                    rejectCode,
                )
                notifyResponse(request, clientResponse)
                CompletableFuture.completedFuture(clientResponse)
            }.thenCompose { it }
        }
    }

    private fun submissionOutcomeIsAmbiguous(statusCode: Int): Boolean =
        statusCode in 300..399 ||
            statusCode == 408 ||
            statusCode == 409 ||
            statusCode == 425 ||
            statusCode == 429 ||
            statusCode >= 500

    private fun emitDeviceProfileTelemetry() {
        if (!config.telemetryOptions().enabled || !deviceProfileEmitted.compareAndSet(false, true)) return
        val sink = config.telemetrySink().orElse(null) ?: return
        val provider = config.deviceProfileProvider()
        val profile = provider.snapshot().orElse(null) ?: return
        sink.emitSignal("android.telemetry.device_profile", mapOf("profile_bucket" to profile.bucket))
    }

    private fun emitNetworkContextTelemetry() {
        if (!config.telemetryOptions().enabled) return
        val sink = config.telemetrySink().orElse(null) ?: return
        val context = config.networkContextProvider().snapshot().orElse(null) ?: return
        sink.emitSignal("android.telemetry.network_context", context.toTelemetryFields())
    }

    private fun emitPipelineStatusTelemetry(request: TransportRequest, transactionHash: String?, statusKind: String?, isSuccess: Boolean, isFailure: Boolean, attempts: Int) {
        if (!config.telemetryOptions().enabled) return; val sink = config.telemetrySink().orElse(null) ?: return
        val fields = LinkedHashMap<String, Any>()
        maybePutAuthorityHash(fields, request, sink, PIPELINE_STATUS_SIGNAL)
        if (transactionHash != null) fields["tx_hash"] = transactionHash
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
        val configuredMaxAttempts = options.maxAttempts
        if (configuredMaxAttempts != null && attemptsSoFar >= configuredMaxAttempts) { future.completeExceptionally(TransactionTimeoutException("Transaction $hashHex did not reach a terminal status after $attemptsSoFar attempts", hashHex, attemptsSoFar, lastPayload)); return }
        val request = ToriiRequestBuilder.buildStatusRequest(config.baseUri(), hashHex, config.requestTimeout(), config.defaultHeaders())
        notifyRequest(request)
        executor.execute(request).whenComplete { response, throwable ->
            try {
                if (future.isDone) return@whenComplete
                if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause ?: throwable else throwable; notifyFailure(request, cause); future.completeExceptionally(cause); return@whenComplete }
                val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
                notifyResponse(request, clientResponse)
                val statusCode = clientResponse.statusCode
                if (statusCode != 200 && statusCode != 404) { future.completeExceptionally(buildPipelineStatusHttpException(hashHex, clientResponse)); return@whenComplete }
                val payload =
                    if (statusCode == 404) null
                    else parsePipelineStatusPayload(clientResponse.body)
                val nextAttempts = attemptsSoFar + 1
                val statusLiteral =
                    if (payload == null) null
                    else PipelineStatusExtractor.requireAuthoritativeStatus(payload, hashHex)
                val isStateResolved = payload?.get("resolved_from") == "state"
                val isSuccess = statusLiteral == "Applied" && isStateResolved
                val isFailure =
                    (statusLiteral == "Rejected" || statusLiteral == "Expired") &&
                        isStateResolved
                emitPipelineStatusTelemetry(request, hashHex, statusLiteral, isSuccess, isFailure, nextAttempts)
                if (options.observer != null) { try { options.observer.onStatus(statusLiteral ?: "", payload ?: emptyMap(), nextAttempts) } catch (observerError: RuntimeException) { future.completeExceptionally(observerError); return@whenComplete } }
                if (isSuccess) { future.complete(payload); return@whenComplete }
                if (isFailure) { future.completeExceptionally(TransactionStatusException(hashHex, statusLiteral, payload)); return@whenComplete }
                if (configuredMaxAttempts != null && nextAttempts >= configuredMaxAttempts) { future.completeExceptionally(TransactionTimeoutException("Transaction $hashHex did not reach a terminal status after $nextAttempts attempts", hashHex, nextAttempts, payload)); return@whenComplete }
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
    private fun parsePipelineStatusPayload(body: ByteArray?): Map<String, Any> {
        check(body != null && body.isNotEmpty()) {
            "Pipeline status response must not be empty"
        }
        val hasNoritoHeader =
            body.size >= 4 &&
                body[0] == 'N'.code.toByte() &&
                body[1] == 'R'.code.toByte() &&
                body[2] == 'T'.code.toByte() &&
                body[3] == '0'.code.toByte()
        check(!hasNoritoHeader) {
            "Pipeline status response violated the requested application/json contract"
        }
        val json = String(body, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "Pipeline status response must not be empty" }
        val parsed = JsonParser.parse(json); check(parsed is Map<*, *>) { "Pipeline status response must be a JSON object" }
        return PipelineStatusExtractor.normalizePublicStatus(parsed as Map<String, Any>)
    }

    private fun notifyRequest(request: TransportRequest) { emitDeviceProfileTelemetry(); emitNetworkContextTelemetry(); for (o in config.observers()) o.onRequest(request) }
    private fun notifyResponse(request: TransportRequest, response: ClientResponse) { for (o in config.observers()) o.onResponse(request, response) }
    private fun notifyFailure(request: TransportRequest, error: Throwable) { for (o in config.observers()) o.onFailure(request, error) }

    private fun buildJsonGetRequest(
        path: String,
        queryParams: Map<String, String>,
        maximumResponseBytes: Long? = null,
    ): TransportRequest {
        val target = appendQuery(resolvePath(path), queryParams)
        val builder = TransportRequest.builder().setUri(target).setMethod("GET").addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        if (maximumResponseBytes != null) builder.setMaximumResponseBytes(maximumResponseBytes)
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        return builder.build()
    }

    private fun buildExactJsonGetRequest(
        path: String,
        maximumResponseBytes: Long,
    ): TransportRequest {
        require(config.defaultHeaders().keys.none { it.equals("Accept", ignoreCase = true) }) {
            "Accept must not be overridden for exact JSON requests"
        }
        val builder = TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setMaximumResponseBytes(maximumResponseBytes)
            .setTimeout(config.requestTimeout())
        for ((key, value) in config.defaultHeaders()) builder.addHeader(key, value)
        return builder.build()
    }

    private fun buildExactOperatorJsonGetRequest(
        path: String,
        maximumResponseBytes: Long,
    ): TransportRequest {
        require(config.defaultHeaders().keys.none { it.equals("Accept", ignoreCase = true) }) {
            "Accept must not be overridden for exact JSON requests"
        }
        OperatorRequestSigner.requireGeneratedAuth(config.defaultHeaders())
        val target = resolvePath(path)
        val operatorHeaders = OperatorRequestSigner.buildHeaders(
            config.requireOperatorSigningContext(),
            "GET",
            target,
            ByteArray(0),
        )
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setMaximumResponseBytes(maximumResponseBytes)
            .setTimeout(config.requestTimeout())
        for ((key, value) in config.defaultHeaders()) builder.addHeader(key, value)
        for ((key, value) in operatorHeaders) builder.addHeader(key, value)
        TransportSecurity.requireHttpRequestAllowed(
            "HttpClientTransport operator GET",
            config.baseUri(),
            target,
            operatorHeaders,
            null,
        )
        return builder.build()
    }

    private fun buildJsonPostRequest(
        path: String,
        body: ByteArray,
        maximumResponseBytes: Long? = null,
    ): TransportRequest {
        val builder = TransportRequest.builder().setUri(resolvePath(path)).setMethod("POST").setBody(body).addHeader("Content-Type", "application/json").addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        if (maximumResponseBytes != null) builder.setMaximumResponseBytes(maximumResponseBytes)
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        return builder.build()
    }

    private fun buildExactNoritoGetRequest(
        path: String,
        maximumResponseBytes: Long,
        canonicalAuth: ToriiCanonicalRequestAuth? = null,
    ): TransportRequest {
        require(config.defaultHeaders().keys.none { it.equals("Accept", ignoreCase = true) }) {
            "Accept must not be overridden for exact Norito requests"
        }
        if (canonicalAuth != null) requireCanonicalHeadersUnset()
        val target = resolvePath(path)
        val builder = TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", APPLICATION_NORITO)
            .setMaximumResponseBytes(maximumResponseBytes)
            .setTimeout(config.requestTimeout())
        for ((key, value) in config.defaultHeaders()) builder.addHeader(key, value)
        if (canonicalAuth != null) {
            val canonicalHeaders = buildCanonicalHeaders("GET", target, null, canonicalAuth)
            for ((key, value) in canonicalHeaders) builder.addHeader(key, value)
            TransportSecurity.requireHttpRequestAllowed(
                "HttpClientTransport",
                config.baseUri(),
                target,
                canonicalHeaders,
                null,
            )
        }
        return builder.build()
    }

    private fun requireCanonicalHeadersUnset() {
        require(config.defaultHeaders().keys.none { candidate ->
            CANONICAL_AUTH_HEADERS.any { it.equals(candidate, ignoreCase = true) }
        }) { "canonical request headers must be supplied only through canonicalAuth" }
    }

    private fun buildBridgeJsonPostRequest(path: String, body: ByteArray): TransportRequest {
        preflightSccpBridgeSubmitJson(body, path)
        return buildJsonPostRequest(path, body, SCCP_JSON_RESPONSE_MAX_BYTES)
    }

    private fun buildVpnRequest(
        method: String,
        path: String,
        body: ByteArray?,
        canonicalAuth: ToriiCanonicalRequestAuth,
        maximumResponseBytes: Long? = null,
    ): TransportRequest {
        if (path.startsWith("/v1/vpn/")) requireSecureVpnBaseUri()
        requireCanonicalHeadersUnset()
        val target = resolvePath(path)
        val builder = TransportRequest.builder().setUri(target).setMethod(method).addHeader("Accept", "application/json").setTimeout(config.requestTimeout())
        if (maximumResponseBytes != null) builder.setMaximumResponseBytes(maximumResponseBytes)
        if (body != null) {
            builder.setBody(body).addHeader("Content-Type", "application/json")
        }
        if (maximumResponseBytes != null) builder.setMaximumResponseBytes(maximumResponseBytes)
        for ((k, v) in config.defaultHeaders()) builder.addHeader(k, v)
        val canonicalHeaders = buildCanonicalHeaders(method, target, body, canonicalAuth)
        for ((k, v) in canonicalHeaders) builder.addHeader(k, v)
        TransportSecurity.requireHttpRequestAllowed(
            "HttpClientTransport",
            config.baseUri(),
            target,
            canonicalHeaders,
            body,
        )
        return builder.build()
    }

    private fun requireSecureVpnBaseUri() {
        require(config.baseUri().scheme.equals("https", ignoreCase = true)) {
            "Sora VPN requests require an HTTPS Torii base URI"
        }
    }

    private fun buildOnboardingRequest(
        method: String,
        path: String,
        body: ByteArray?,
        onboardingToken: String,
    ): TransportRequest {
        val token = requireOnboardingCredential(onboardingToken)
        require(config.defaultHeaders().keys.none { it.equals(ONBOARDING_TOKEN_HEADER, ignoreCase = true) }) {
            "$ONBOARDING_TOKEN_HEADER must be supplied only through the sponsored onboarding API"
        }
        val builder = TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod(method)
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout())
        if (body != null) {
            builder.setBody(body).addHeader("Content-Type", "application/json")
        }
        for ((key, value) in config.defaultHeaders()) builder.addHeader(key, value)
        builder.addHeader(ONBOARDING_TOKEN_HEADER, token)
        return builder.build()
    }

    private fun buildCanonicalHeaders(method: String, target: URI, body: ByteArray?, canonicalAuth: ToriiCanonicalRequestAuth): Map<String, String> {
        val networkId = config.requireLocalSigningContext().networkId()
        val timestampMs = canonicalAuth.timestampMs
        val nonce = canonicalAuth.nonce
        require((timestampMs == null) == (nonce == null)) { "timestampMs and nonce must be provided together" }
        return if (timestampMs == null) {
            CanonicalRequestSigner.buildHeaders(networkId, method, target, body, canonicalAuth.accountId, canonicalAuth.privateKey)
        } else {
            CanonicalRequestSigner.buildHeaders(networkId, method, target, body, canonicalAuth.accountId, canonicalAuth.privateKey, timestampMs, nonce!!)
        }
    }

    private fun resolvePath(path: String?): URI {
        if (path.isNullOrBlank()) return config.baseUri()
        if (path.startsWith("http://") || path.startsWith("https://")) return URI.create(path)
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = config.baseUri().toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun ensureTransactionSubmissionCompatibility(): CompletableFuture<Unit> {
        val request = buildJsonGetRequest(
            "/v1/node/capabilities",
            emptyMap(),
            NODE_CAPABILITIES_RESPONSE_MAX_BYTES,
        )
        return fetchJson(
            request,
            Function { payload ->
                ToriiTransactionCompatibility.requireCompatible(payload)
                Unit
            },
            "transaction submission compatibility",
            200,
        ).handle { _, throwable ->
            if (throwable != null) {
                val cause = unwrapCompletion(throwable)
                if (cause is ToriiTransactionCompatibilityException) {
                    throw CompletionException(cause)
                }
                throw CompletionException(ToriiTransactionCompatibilityProbeException(cause))
            }
            Unit
        }
    }

    private fun unwrapCompletion(throwable: Throwable): Throwable {
        var current = throwable
        while (current is CompletionException && current.cause != null) {
            current = current.cause!!
        }
        return current
    }

    private fun <T> fetchJson(
        request: TransportRequest,
        parser: Function<ByteArray, T>,
        errorContext: String,
        acceptedStatus: Int? = null,
        responseValidator: ((T, Int) -> T)? = null,
        exactJsonMediaType: Boolean = false,
    ): CompletableFuture<T> {
        notifyRequest(request); val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(RuntimeException("$errorContext request failed", cause)); return@whenComplete }
            val maximumResponseBytes = request.maximumResponseBytes
            if (maximumResponseBytes != null && response.body.size.toLong() > maximumResponseBytes) {
                val error = IllegalArgumentException(
                    "$errorContext response exceeds the $maximumResponseBytes byte limit",
                )
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            val statusAccepted = acceptedStatus?.let { response.statusCode == it }
                ?: (response.statusCode in 200..299)
            if (!statusAccepted) { val error = RuntimeException("$errorContext request failed with status ${response.statusCode}"); notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete }
            try {
                if (exactJsonMediaType) {
                    requireExactJsonResponse(response, errorContext)
                }
                val parsed = parser.apply(response.body)
                val validated = responseValidator?.invoke(parsed, response.statusCode) ?: parsed
                notifyResponse(request, clientResponse)
                future.complete(validated)
            }
            catch (ex: RuntimeException) { notifyFailure(request, ex); future.completeExceptionally(ex) }
        }; return future
    }

    private fun <T> fetchSccpJson(
        request: TransportRequest,
        parser: Function<ByteArray, T>,
        errorContext: String,
    ): CompletableFuture<T> = fetchExactJson(request, parser, errorContext)

    private fun <T> fetchExactJson(
        request: TransportRequest,
        parser: Function<ByteArray, T>,
        errorContext: String,
    ): CompletableFuture<T> {
        notifyRequest(request)
        val future = CompletableFuture<T>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!)
                future.completeExceptionally(RuntimeException("$errorContext request failed", cause))
                return@whenComplete
            }
            val body = response.body
            val clientResponse = ClientResponse(
                response.statusCode,
                body,
                response.message,
                null,
                extractRejectCode(response),
            )
            try {
                requireExactJsonResponse(response, errorContext)
                val maximumResponseBytes = requireNotNull(request.maximumResponseBytes) {
                    "$errorContext request must declare a response-body limit"
                }
                require(body.isNotEmpty()) { "$errorContext response must not be empty" }
                require(body.size.toLong() <= maximumResponseBytes) {
                    "$errorContext response exceeds $maximumResponseBytes bytes"
                }
                requireExactOptionalContentLength(response.headers, body.size, errorContext)
                val parsed = parser.apply(body)
                notifyResponse(request, clientResponse)
                future.complete(parsed)
            } catch (error: RuntimeException) {
                notifyFailure(request, error)
                future.completeExceptionally(error)
            }
        }
        return future
    }

    private fun fetchExactNoritoBytes(
        request: TransportRequest,
        errorContext: String,
    ): CompletableFuture<ByteArray> {
        notifyRequest(request)
        val future = CompletableFuture<ByteArray>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!)
                future.completeExceptionally(RuntimeException("$errorContext request failed", cause))
                return@whenComplete
            }
            val body = response.body
            val clientResponse = ClientResponse(
                response.statusCode,
                body,
                response.message,
                null,
                extractRejectCode(response),
            )
            try {
                require(response.statusCode == 200) {
                    "$errorContext request failed with status ${response.statusCode}"
                }
                requireExactHeader(response.headers, "Content-Type", APPLICATION_NORITO, errorContext)
                require(body.isNotEmpty()) { "$errorContext response must not be empty" }
                val maximumResponseBytes = requireNotNull(request.maximumResponseBytes) {
                    "$errorContext request must declare a response-body limit"
                }
                require(body.size.toLong() <= maximumResponseBytes) {
                    "$errorContext response exceeds $maximumResponseBytes bytes"
                }
                requireExactOptionalContentLength(response.headers, body.size, errorContext)
                notifyResponse(request, clientResponse)
                future.complete(body.copyOf())
            } catch (error: RuntimeException) {
                notifyFailure(request, error)
                future.completeExceptionally(error)
            }
        }
        return future
    }

    private fun requireExactHeader(
        headers: Map<String, List<String>>,
        name: String,
        expected: String,
        errorContext: String,
    ) {
        val values = headers.entries
            .asSequence()
            .filter { (header, _) -> header.equals(name, ignoreCase = true) }
            .flatMap { (_, headerValues) -> headerValues.asSequence() }
            .toList()
        require(values.size == 1 && values[0] == expected) {
            "$errorContext response $name must be exactly $expected"
        }
    }

    private fun requireExactOptionalContentLength(
        headers: Map<String, List<String>>,
        actualBytes: Int,
        errorContext: String,
    ) {
        val matchingHeaders = headers.entries
            .filter { (name, _) -> name.equals("Content-Length", ignoreCase = true) }
        if (matchingHeaders.isEmpty()) return
        val values = matchingHeaders
            .asSequence()
            .flatMap { (_, headerValues) -> headerValues.asSequence() }
            .toList()
        require(values.size == 1) { "$errorContext response has ambiguous Content-Length" }
        val value = values.single()
        require(
            value == "0" ||
                (value.isNotEmpty() && value[0] in '1'..'9' &&
                    value.drop(1).all { it in '0'..'9' }),
        ) {
            "$errorContext response Content-Length must be one canonical decimal integer"
        }
        require(value.toLongOrNull() == actualBytes.toLong()) {
            "$errorContext response Content-Length does not match the body"
        }
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

    private fun executeSccpJsonAccepted(
        request: TransportRequest,
        errorContext: String,
    ): CompletableFuture<ClientResponse> {
        notifyRequest(request)
        val future = CompletableFuture<ClientResponse>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val cause = if (throwable is CompletionException) throwable.cause else throwable
                notifyFailure(request, cause!!)
                future.completeExceptionally(RuntimeException("$errorContext request failed", cause))
                return@whenComplete
            }
            val clientResponse = ClientResponse(
                response.statusCode,
                response.body,
                response.message,
                null,
                extractRejectCode(response),
            )
            try {
                requireExactSccpJsonResponse(response, errorContext)
                notifyResponse(request, clientResponse)
                future.complete(clientResponse)
            } catch (ex: RuntimeException) {
                notifyFailure(request, ex)
                future.completeExceptionally(ex)
            }
        }
        return future
    }

    private fun requireExactSccpJsonResponse(
        response: TransportResponse,
        errorContext: String,
    ) {
        requireExactJsonResponse(response, errorContext)
    }

    private fun requireExactJsonResponse(
        response: TransportResponse,
        errorContext: String,
    ) {
        if (response.statusCode != 200) {
            throw RuntimeException(
                "$errorContext request failed with status ${response.statusCode}",
            )
        }
        val contentTypes = response.headers.entries
            .asSequence()
            .filter { (name, _) -> name.equals("Content-Type", ignoreCase = true) }
            .flatMap { (_, values) -> values.asSequence() }
            .toList()
        if (contentTypes.size != 1 || !isUnambiguousApplicationJson(contentTypes[0])) {
            throw RuntimeException(
                "$errorContext response Content-Type must be exactly application/json",
            )
        }
    }

    private fun isUnambiguousApplicationJson(value: String): Boolean {
        if (',' in value) {
            return false
        }
        var index = skipHttpOws(value, 0)
        val mediaType = "application/json"
        if (index + mediaType.length > value.length) {
            return false
        }
        mediaType.indices.forEach { offset ->
            val actual = value[index + offset]
            val expected = mediaType[offset]
            if (actual != expected && !(expected in 'a'..'z' && actual == (expected.code - 32).toChar())) {
                return false
            }
        }
        index = skipHttpOws(value, index + mediaType.length)
        while (index < value.length) {
            if (value[index] != ';') {
                return false
            }
            index = skipHttpOws(value, index + 1)
            val nameStart = index
            while (index < value.length && isHttpTokenCharacter(value[index])) {
                index += 1
            }
            if (index == nameStart || index >= value.length || value[index] != '=') {
                return false
            }
            index += 1
            if (index >= value.length) {
                return false
            }
            if (value[index] == '"') {
                index += 1
                var closed = false
                while (index < value.length) {
                    val current = value[index]
                    if (current == '"') {
                        index += 1
                        closed = true
                        break
                    }
                    if (current == '\\') {
                        index += 1
                        if (index >= value.length || !isHttpQuotedPairCharacter(value[index])) {
                            return false
                        }
                    } else if (!isHttpQuotedTextCharacter(current)) {
                        return false
                    }
                    index += 1
                }
                if (!closed) {
                    return false
                }
            } else {
                val parameterValueStart = index
                while (index < value.length && isHttpTokenCharacter(value[index])) {
                    index += 1
                }
                if (index == parameterValueStart) {
                    return false
                }
            }
            index = skipHttpOws(value, index)
        }
        return true
    }

    private fun skipHttpOws(value: String, start: Int): Int {
        var index = start
        while (index < value.length && (value[index] == ' ' || value[index] == '\t')) {
            index += 1
        }
        return index
    }

    private fun isHttpTokenCharacter(value: Char): Boolean =
        value in '0'..'9' || value in 'A'..'Z' || value in 'a'..'z' ||
            value in "!#$%&'*+-.^_`|~"

    private fun isHttpQuotedTextCharacter(value: Char): Boolean {
        val code = value.code
        return code == 0x09 || code in 0x20..0x21 || code in 0x23..0x5B ||
            code in 0x5D..0x7E || code in 0x80..0xFF
    }

    private fun isHttpQuotedPairCharacter(value: Char): Boolean {
        val code = value.code
        return code == 0x09 || code in 0x20..0x7E || code in 0x80..0xFF
    }

    private fun <T : Any> fetchJsonAllowingNotFound(
        request: TransportRequest,
        parser: Function<ByteArray, T>,
        errorContext: String,
        acceptedStatus: Int? = null,
    ): CompletableFuture<Optional<T>> {
        notifyRequest(request); val future = CompletableFuture<Optional<T>>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) { val cause = if (throwable is CompletionException) throwable.cause else throwable; notifyFailure(request, cause!!); future.completeExceptionally(RuntimeException("$errorContext request failed", cause)); return@whenComplete }
            val clientResponse = ClientResponse(response.statusCode, response.body, response.message, null, extractRejectCode(response))
            if (response.statusCode == 404) { notifyResponse(request, clientResponse); future.complete(Optional.empty<T>()); return@whenComplete }
            val statusAccepted = acceptedStatus?.let { response.statusCode == it }
                ?: (response.statusCode in 200..299)
            if (!statusAccepted) { val error = RuntimeException("$errorContext request failed with status ${response.statusCode}"); notifyFailure(request, error); future.completeExceptionally(error); return@whenComplete }
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
        private const val ONBOARDING_TOKEN_HEADER = "X-Iroha-Onboarding-Token"
        private const val PIPELINE_STATUS_SIGNAL = "android.torii.pipeline.status"
        private const val REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure"
        private const val U32_MAX = 4_294_967_295L
        private const val FEE_QUOTE_RESPONSE_MAX_BYTES = 64L * 1024L
        private const val FEE_SPONSOR_PROGRAM_RESPONSE_MAX_BYTES = 64L * 1024L
        private const val SCCP_CAPABILITIES_RESPONSE_MAX_BYTES = 64L * 1024L
        private const val NODE_CAPABILITIES_RESPONSE_MAX_BYTES = 64L * 1024L
        private const val SCCP_RECENT_RESPONSE_MAX_BYTES = 8L * 1024L * 1024L
        private const val SCCP_JSON_RESPONSE_MAX_BYTES = 64L * 1024L * 1024L
        private const val EXECUTED_BLOCK_WIRE_MAX_BYTES = 32L * 1024L * 1024L
        private const val ACCOUNT_ONBOARDING_CURRENT_STATE_RESPONSE_MAX_BYTES = 4L * 1024L
        private const val APPLICATION_NORITO = "application/x-norito"
        private val CANONICAL_AUTH_HEADERS = setOf(
            CanonicalRequestSigner.HEADER_ACCOUNT,
            CanonicalRequestSigner.HEADER_SIGNATURE,
            CanonicalRequestSigner.HEADER_TIMESTAMP_MS,
            CanonicalRequestSigner.HEADER_NONCE,
        )

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
        /** Adds explicit local staging; transaction submission never drains or fills this queue. */
        @JvmStatic fun withDirectoryPendingQueue(config: ClientConfig, queueDir: Path): ClientConfig = config.toBuilder().enableDirectoryPendingQueue(queueDir).build()
        /** Adds explicit local staging; transaction submission never drains or fills this queue. */
        @JvmStatic fun withFilePendingQueue(config: ClientConfig, queueFile: Path): ClientConfig = config.toBuilder().enableFilePendingQueue(queueFile).build()

        private fun resolveRoute(request: TransportRequest?): String = request?.uri?.rawPath ?: ""
        private fun extractRejectCode(response: TransportResponse?): String? =
            if (response == null) null else HttpErrorMessageExtractor.extractRejectCode(
                response.headers,
                "x-iroha-reject-code",
                response.body,
            )
        private fun extractEntrypointHash(response: TransportResponse?): String? {
            if (response == null) return null
            val values = response.headers["x-iroha-entrypoint-hash"] ?: return null
            check(values.size == 1) {
                "Torii transaction hash header must contain exactly one value"
            }
            val value = values.single()
            check(value.matches(Regex("[0-9a-f]{63}[13579bdf]"))) {
                "Torii transaction hash header must be an exact lowercase marked 32-byte hash"
            }
            return value
        }
        private fun resolveAuthority(request: TransportRequest?): String {
            if (request == null) return ""; val authority = request.uri.authority; if (authority != null) return authority
            val host = request.headers["Host"]; return if (host.isNullOrEmpty()) "" else host[0]
        }
        private fun emitRedactionFailure(sink: TelemetrySink, signalId: String, reason: String) { sink.emitSignal(REDACTION_FAILURE_SIGNAL, mapOf("signal_id" to signalId, "reason" to reason)) }
        private fun buildPipelineStatusHttpException(hashHex: String, response: ClientResponse): TransactionStatusHttpException = TransactionStatusHttpException(hashHex, response.statusCode, response.rejectCode(), HttpErrorMessageExtractor.extractMessage(response.body))
        private fun appendQuery(target: URI, params: Map<String, String>): URI {
            if (params.isEmpty()) return target
            val targetText = target.toString()
            val fragmentIndex = targetText.indexOf('#').let { if (it >= 0) it else targetText.length }
            val builder = StringBuilder(targetText.length + 1)
                .append(targetText, 0, fragmentIndex)
            builder.append(if (builder.indexOf("?") >= 0) "&" else "?")
            builder.append(encodeQuery(params))
            builder.append(targetText, fragmentIndex, targetText.length)
            return URI.create(builder.toString())
        }
        private fun encodeQuery(params: Map<String, String>): String = params.entries.joinToString("&") { (k, v) -> "${urlEncode(k)}=${urlEncode(v)}" }
        private fun encodePathSegment(segment: String): String = urlEncode(segment).replace("+", "%20")
        private fun urlEncode(value: String): String = URLEncoder.encode(value, StandardCharsets.UTF_8.name())
        private fun encodeJsonBody(payload: Map<String, Any>): ByteArray = JsonEncoder.encode(payload).toByteArray(StandardCharsets.UTF_8)

        @JvmStatic internal fun buildIdentifierResolveRequest(
            policyId: String,
            encryptedInputHex: String,
            outputOpening: RamLfeOutputOpening,
        ): IdentifierResolveRequest {
            val normalizedPolicyId = normalizeNonBlank(policyId, "policyId")
            val normalizedEncryptedInput = normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            return IdentifierResolveRequest.encrypted(normalizedPolicyId, normalizedEncryptedInput, outputOpening)
        }

        @JvmStatic internal fun buildRamLfeExecuteRequest(encryptedInputHex: String): RamLfeExecuteRequest {
            val normalizedEncryptedInput = normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            return RamLfeExecuteRequest.encrypted(normalizedEncryptedInput)
        }

        @JvmStatic internal fun buildIdentifierResolvePayload(
            policyId: String,
            encryptedInputHex: String,
            outputOpening: RamLfeOutputOpening,
        ): Map<String, Any> {
            val normalizedPolicyId = normalizeNonBlank(policyId, "policyId")
            val normalizedEncryptedInput = normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            val payload = LinkedHashMap<String, Any>(); payload["policy_id"] = normalizedPolicyId
            payload["encrypted_input"] = normalizedEncryptedInput
            payload["output_opening"] = outputOpening.toJsonMap()
            return payload
        }

        @JvmStatic internal fun buildRamLfeExecutePayload(encryptedInputHex: String): Map<String, Any> {
            val normalizedEncryptedInput = normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex")
            val payload = LinkedHashMap<String, Any>()
            payload["encrypted_input"] = normalizedEncryptedInput
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
            payload["metering_public_key_hex"] =
                normalizeEd25519PublicKeyHex(meteringPublicKeyHex, "meteringPublicKeyHex")
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
            payload["metering_public_key_hex"] =
                normalizeEd25519PublicKeyHex(meteringPublicKeyHex, "meteringPublicKeyHex")
            return payload
        }

        @JvmStatic internal fun buildVpnReceiptSubmitPayload(relayReceiptHex: String, clientVoucherHex: String, leaseIdHex: String?): Map<String, Any> {
            val payload = LinkedHashMap<String, Any>()
            payload["relay_receipt_hex"] = normalizeEvenLengthHex(relayReceiptHex, "relayReceiptHex")
            payload["client_voucher_hex"] = normalizeEvenLengthHex(clientVoucherHex, "clientVoucherHex")
            if (leaseIdHex != null) payload["lease_id_hex"] = normalizeHex32(leaseIdHex, "leaseIdHex")
            return payload
        }

        /** Builds the secret-free request used to prepare a contract-call draft. */
        @JvmStatic internal fun buildContractCallDraftPayload(
            authority: String,
            feePayment: FeePaymentIntent,
            contractAddress: String?,
            contractAlias: String?,
            entrypoint: String,
            payload: Any?,
        ): Map<String, Any> {
            require(feePayment.gasLimit != null) { "contract feePayment must include gasLimit" }
            val normalized = LinkedHashMap<String, Any>()
            normalized["authority"] = normalizeNonBlank(authority, "authority")
            normalized.putAll(buildContractTargetSelector(contractAddress, contractAlias))
            normalized["entrypoint"] = normalizeNonBlank(entrypoint, "entrypoint")
            if (payload != null) normalized["payload"] = payload
            normalized["fee_payment"] = feePayment.toJsonMap()
            return normalized
        }

        /** Validates that Torii returned a secret-free draft bound to the requested call. */
        @JvmStatic internal fun validateContractCallDraft(
            response: ContractCallResponse,
            request: Map<String, Any>,
        ): ContractCallResponse {
            check(response.ok) { "contract call draft.ok must be true" }
            check(!response.submitted) { "contract call draft must not be submitted" }
            check(response.txHashHex == null && response.pipelineStatus == null) {
                "contract call draft must not contain submission state"
            }
            check(response.entrypoint == request["entrypoint"]) {
                "contract call draft entrypoint is not bound to the request"
            }
            val receipt = response.operationReceipt
            check(receipt.operationKind == "contract_call" && receipt.status == "pending_signature") {
                "contract call draft receipt must be pending_signature"
            }
            check(receipt.entrypoint == response.entrypoint && receipt.txHashHex == null) {
                "contract call draft receipt is inconsistent"
            }
            check(response.transactionPayloadB64 != null) {
                "contract call draft must contain one exact canonical transaction payload"
            }
            val signingMessageB64 = response.signingMessageB64
            check(signingMessageB64 != null) {
                "contract call draft must contain a signing message"
            }
            check(Base64.getDecoder().decode(signingMessageB64).size == 32) {
                "contract call draft signing message must be 32 bytes"
            }
            check(response.entrypointHashHex == null && receipt.entrypointHashHex == null) {
                "contract call draft must not claim a final entrypoint hash"
            }
            request["contract_address"]?.let { expected ->
                check(response.contractAddress == expected && receipt.contractAddress == expected) {
                    "contract call draft address is not bound to the request"
                }
            }
            request["contract_alias"]?.let { expected ->
                check(receipt.contractAlias == expected) {
                    "contract call draft alias is not bound to the request"
                }
            }
            return response
        }

        @JvmStatic internal fun buildMultisigProposePayload(request: MultisigProposeRequest): Map<String, Any> {
            val hasAccountId = request.multisigAccountId != null
            val hasAlias = request.multisigAccountAlias != null
            require(hasAccountId != hasAlias) { "Exactly one of multisigAccountId or multisigAccountAlias must be provided" }
            require(request.instructions.isNotEmpty()) { "instructions must not be empty" }

            val payload = LinkedHashMap<String, Any>()
            val accountId = request.multisigAccountId
            val accountAlias = request.multisigAccountAlias
            if (accountId != null) {
                payload["multisig_account_id"] = normalizeNonBlank(accountId, "multisigAccountId")
            } else {
                payload["multisig_account_alias"] = normalizeNonBlank(accountAlias!!, "multisigAccountAlias")
            }
            payload["signer_account_id"] = normalizeNonBlank(request.signerAccountId, "signerAccountId")
            if (request.publicKeyHex != null) {
                payload["public_key_hex"] =
                    normalizeEd25519PublicKeyHex(request.publicKeyHex, "publicKeyHex")
            }
            if (request.signatureB64 != null) payload["signature_b64"] = normalizeRequiredExactBase64Payload(request.signatureB64, "signatureB64")
            if (request.creationTimeMs != null) {
                require(request.creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
                payload["creation_time_ms"] = request.creationTimeMs
            }
            payload["fee_payment"] = request.feePayment.toJsonMap()
            if (request.memo != null) payload["memo"] = normalizeNonBlank(request.memo, "memo")
            putValidationFeePolicyMetadata(
                payload,
                request.validationFeePolicyVersion,
                request.validationFeePolicyHash,
                request.validationFeeInstructionIndex,
                request.validationFeeTransferEntryIndex,
            )
            payload["instructions"] = request.instructions.mapIndexed { index, instruction ->
                require(instruction.isNotEmpty()) { "instructions[$index] must not be empty" }
                Base64.getEncoder().encodeToString(instruction)
            }
            return payload
        }

        /** Reject a multisig response that changes a signature-bound request field. */
        @JvmStatic internal fun validateMultisigResponse(
            response: MultisigResponse,
            request: MultisigProposeRequest,
        ): MultisigResponse {
            check(request.feePayment.hasSamePayerAndGasBound(response.feePayment)) {
                "multisig response fee_payment changed the requested payer, sponsor revision, or gas bound"
            }
            request.creationTimeMs?.let { expected ->
                check(response.creationTimeMs == expected) {
                    "multisig response creation_time_ms is not bound to the request"
                }
            }
            return response
        }

        @JvmStatic internal fun putValidationFeePolicyMetadata(
            payload: MutableMap<String, Any>,
            validationFeePolicyVersion: Long?,
            validationFeePolicyHash: String?,
            validationFeeInstructionIndex: Long?,
            validationFeeTransferEntryIndex: Long?,
        ) {
            val hasPolicyVersion = validationFeePolicyVersion != null
            val hasPolicyHash = validationFeePolicyHash != null
            val hasInstructionIndex = validationFeeInstructionIndex != null
            val hasTransferEntryIndex = validationFeeTransferEntryIndex != null
            require(hasPolicyVersion == hasPolicyHash) {
                "validationFeePolicyVersion and validationFeePolicyHash must be provided together"
            }
            require(hasPolicyVersion || !hasInstructionIndex) {
                "validationFeeInstructionIndex requires validation fee policy metadata"
            }
            require(hasPolicyVersion || !hasTransferEntryIndex) {
                "validationFeeTransferEntryIndex requires validation fee policy metadata"
            }
            require(!hasTransferEntryIndex || hasInstructionIndex) {
                "validationFeeTransferEntryIndex requires validationFeeInstructionIndex"
            }
            val policyVersion = validationFeePolicyVersion ?: return
            val policyHash = requireNotNull(validationFeePolicyHash) {
                "validationFeePolicyVersion and validationFeePolicyHash must be provided together"
            }
            require(policyVersion >= 0L) { "validationFeePolicyVersion must be non-negative" }
            val instructionIndex = validationFeeInstructionIndex
            if (instructionIndex != null) {
                require(instructionIndex >= 0L) { "validationFeeInstructionIndex must be non-negative" }
            }
            val transferEntryIndex = validationFeeTransferEntryIndex
            if (transferEntryIndex != null) {
                require(transferEntryIndex >= 0L) { "validationFeeTransferEntryIndex must be non-negative" }
            }
            payload["validation_fee_policy_version"] = policyVersion.toString()
            payload["validation_fee_policy_hash"] = normalizeHex32(policyHash, "validationFeePolicyHash")
            if (instructionIndex != null) {
                payload["validation_fee_instruction_index"] = instructionIndex.toString()
            }
            if (transferEntryIndex != null) {
                payload["validation_fee_transfer_entry_index"] = transferEntryIndex.toString()
            }
        }

        @JvmStatic internal fun buildVerifyingKeyRegisterPayload(request: VerifyingKeyRegisterRequest): Map<String, Any> {
            val backend = VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(request.backend, "backend")
            val vkPayload = normalizeVerifierBytes(request.verifyingKeyBytes, request.verifyingKeyLength)
            val commitmentHex = normalizeOptionalHex32(request.commitmentHex, "commitmentHex")
            validateVerifyingKeyMaterial(vkPayload, commitmentHex)
            validateInlineVerifyingKeyCommitment(backend, vkPayload?.bytes, commitmentHex)
            validateVerifyingKeyHeightRange(request.activationHeight, request.withdrawHeight)

            val payload = LinkedHashMap<String, Any>()
            payload["authority"] = normalizeVerifyingKeyAuthority(request.authority)
            payload["backend"] = backend
            payload["name"] = normalizeVerifyingKeyName(request.name)
            payload["version"] = normalizePositiveU32(request.version, "version")
            payload["circuit_id"] = normalizeNonBlank(request.circuitId, "circuitId")
            payload["public_inputs_schema_hash_hex"] = normalizeHex32(request.publicInputsSchemaHashHex, "publicInputsSchemaHashHex")
            payload["gas_schedule_id"] = normalizeNonBlank(request.gasScheduleId, "gasScheduleId")
            putOptionalVerifierFields(
                payload,
                request.curve,
                request.maxProofBytes,
                request.metadataUriCid,
                request.verifyingKeyBytesCid,
                request.activationHeight,
                request.withdrawHeight,
                commitmentHex,
                vkPayload,
                request.status,
            )
            return payload
        }

        @JvmStatic internal fun buildVerifyingKeyUpdatePayload(request: VerifyingKeyUpdateRequest): Map<String, Any> {
            val backend = VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(request.backend, "backend")
            val vkPayload = normalizeVerifierBytes(request.verifyingKeyBytes, request.verifyingKeyLength)
            val commitmentHex = normalizeOptionalHex32(request.commitmentHex, "commitmentHex")
            validateVerifyingKeyMaterial(vkPayload, commitmentHex)
            validateInlineVerifyingKeyCommitment(backend, vkPayload?.bytes, commitmentHex)
            validateVerifyingKeyHeightRange(request.activationHeight, request.withdrawHeight)

            val payload = LinkedHashMap<String, Any>()
            payload["authority"] = normalizeVerifyingKeyAuthority(request.authority)
            payload["backend"] = backend
            payload["name"] = normalizeVerifyingKeyName(request.name)
            payload["version"] = normalizePositiveU32(request.version, "version")
            payload["circuit_id"] = normalizeNonBlank(request.circuitId, "circuitId")
            payload["public_inputs_schema_hash_hex"] = normalizeHex32(request.publicInputsSchemaHashHex, "publicInputsSchemaHashHex")
            request.gasScheduleId?.let { payload["gas_schedule_id"] = normalizeNonBlank(it, "gasScheduleId") }
            putOptionalVerifierFields(
                payload,
                request.curve,
                request.maxProofBytes,
                request.metadataUriCid,
                request.verifyingKeyBytesCid,
                request.activationHeight,
                request.withdrawHeight,
                commitmentHex,
                vkPayload,
                request.status,
            )
            return payload
        }

        @JvmStatic internal fun buildContractTargetSelector(contractAddress: String?, contractAlias: String?): Map<String, String> {
            val hasContractAddress = contractAddress != null
            val hasContractAlias = contractAlias != null
            require(hasContractAddress != hasContractAlias) { "Exactly one of contractAddress or contractAlias must be provided" }
            return if (contractAddress != null) {
                mapOf("contract_address" to normalizeNonBlank(contractAddress, "contractAddress"))
            } else {
                mapOf("contract_alias" to normalizeNonBlank(requireNotNull(contractAlias), "contractAlias"))
            }
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

        @JvmStatic internal fun normalizeRequiredExactBase64Payload(value: String, field: String): String {
            require(value.isNotEmpty() && value == value.trim()) { "$field must be exact standard-base64" }
            val decoded = try {
                Base64.getDecoder().decode(value)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("$field must be valid base64", ex)
            }
            require(decoded.isNotEmpty()) { "$field must not decode to empty bytes" }
            require(Base64.getEncoder().encodeToString(decoded) == value) {
                "$field must be exact standard-base64"
            }
            return value
        }

        @JvmStatic internal fun normalizeOptionalNonBlank(value: String?, field: String): String? = if (value == null) null else normalizeNonBlank(value, field)
        @JvmStatic internal fun normalizeNonBlank(value: String, field: String): String { val trimmed = value.trim(); require(trimmed.isNotEmpty()) { "$field must not be blank" }; return trimmed }
        @JvmStatic internal fun normalizeVerifyingKeyName(value: String): String {
            val normalized = normalizeNonBlank(value, "name")
            require(!normalized.contains(':')) { "name must not contain ':' characters" }
            return normalized
        }
        @JvmStatic internal fun normalizeVerifyingKeyAuthority(value: String): String {
            val normalized = normalizeNonBlank(value, "authority")
            org.hyperledger.iroha.sdk.address.requireCanonicalI105Address(
                normalized,
                "authority",
            )
            return normalized
        }
        @JvmStatic internal fun normalizeEvenLengthHex(value: String, field: String): String {
            var trimmed = normalizeNonBlank(value, field)
            if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) trimmed = trimmed.substring(2)
            require(trimmed.length % 2 == 0 && trimmed.isNotEmpty()) { "$field must be an even-length hex string" }
            for (c in trimmed) require(c in '0'..'9' || c in 'a'..'f' || c in 'A'..'F') { "$field must be an even-length hex string" }
            return trimmed.lowercase()
        }
        @JvmStatic internal fun normalizeExactEvenLengthHex(value: String, field: String): String {
            require(value.trim() == value) { "$field must be a canonical hex string" }
            return normalizeEvenLengthHex(value, field)
        }
        @JvmStatic internal fun normalizeNonZeroEvenLengthHex(value: String, field: String, expectedByteLength: Int? = null): String {
            val normalized = normalizeEvenLengthHex(value, field)
            require(normalized.any { it != '0' }) { "$field must not be all zero" }
            if (expectedByteLength != null) {
                require(normalized.length == expectedByteLength * 2) { "$field must be a $expectedByteLength-byte hex string" }
            }
            return normalized
        }
        @JvmStatic internal fun normalizeExactNonZeroEvenLengthHex(value: String, field: String, expectedByteLength: Int? = null): String {
            val normalized = normalizeExactEvenLengthHex(value, field)
            require(normalized.any { it != '0' }) { "$field must not be all zero" }
            if (expectedByteLength != null) {
                require(normalized.length == expectedByteLength * 2) { "$field must be a $expectedByteLength-byte hex string" }
            }
            return normalized
        }
        @JvmStatic internal fun normalizeHexBytes(value: String, field: String, expectedByteLength: Int): String {
            val normalized = normalizeEvenLengthHex(value, field)
            require(normalized.length == expectedByteLength * 2) { "$field must be a $expectedByteLength-byte hex string" }
            return normalized
        }
        @JvmStatic internal fun preflightSccpBridgeSubmitJson(body: ByteArray, path: String) {
            require(String(body, StandardCharsets.UTF_8).toByteArray(StandardCharsets.UTF_8).contentEquals(body)) {
                "SCCP bridge submit payload must be UTF-8 JSON"
            }
            val parsed = try {
                JsonParser.parse(String(body, StandardCharsets.UTF_8))
            } catch (ex: RuntimeException) {
                throw IllegalArgumentException("bridge submit payload must be valid JSON", ex)
            }
            val fields = parsed as? Map<*, *>
                ?: throw IllegalArgumentException("bridge submit payload must be a JSON object")
            val allowed = when (path) {
                "/v1/bridge/proofs/submit" -> SCCP_PROOF_SUBMIT_FIELDS
                "/v1/bridge/messages" -> SCCP_MESSAGE_SUBMIT_FIELDS
                else -> throw IllegalArgumentException("unsupported SCCP bridge submit path")
            }
            val unknown = fields.keys.firstOrNull { it !is String || it !in allowed }
            require(unknown == null) { "unknown or retired bridge submit field `$unknown`" }
            val authority = fields["authority"] as? String
                ?: throw IllegalArgumentException("authority is required and must be canonical")
            requireCanonicalSccpAuthority(authority)
            val feePayment = FeePaymentJson.parse(
                fields["fee_payment"],
                "bridge submit payload.fee_payment",
            )
            val hasSignature = fields.containsKey("signature_b64")
            val signature = fields["signature_b64"]
            if (hasSignature) {
                require(signature is String) { "signature_b64 must be canonical padded base64" }
                normalizeOptionalSignature(signature)
            }
            val hasTransactionPayload = fields.containsKey("transaction_payload_b64")
            val transactionPayload = fields["transaction_payload_b64"]
            if (hasTransactionPayload) {
                require(transactionPayload is String) {
                    "transaction_payload_b64 must be canonical padded base64"
                }
            }
            var creationTimeMs: Long? = null
            if (fields.containsKey("creation_time_ms")) {
                val value = fields["creation_time_ms"]
                require(value is Number && value.toLong() > 0 && value.toString() == value.toLong().toString()) {
                    "creation_time_ms must be a positive integer"
                }
                creationTimeMs = value.toLong()
            }
            validateSccpDetachedSigningState(
                signature as? String,
                transactionPayload as? String,
                creationTimeMs,
            )
            if (transactionPayload is String) {
                normalizeOptionalTransactionPayload(
                    transactionPayload,
                    creationTimeMs,
                    authority,
                    feePayment,
                )
            }
            val artifactField = if (path == "/v1/bridge/messages") {
                "native_proof_b64"
            } else {
                "destination_proof_b64"
            }
            val artifact = optionalSccpArtifact(fields, artifactField)
                ?: throw IllegalArgumentException("$artifactField is required")
            validateCanonicalSccpNoritoBase64(
                artifact,
                artifactField,
                if (artifactField == "destination_proof_b64") {
                    SCCP_MAX_DESTINATION_ARTIFACT_BYTES
                } else {
                    SCCP_MAX_NATIVE_PROOF_BYTES
                },
                if (artifactField == "destination_proof_b64") {
                    SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME
                } else {
                    SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME
                },
            )
        }
        @JvmStatic internal fun normalizeHex16(value: String, field: String): String { val normalized = normalizeEvenLengthHex(value, field); require(normalized.length == 32) { "$field must contain 32 hex characters" }; return normalized }
        @JvmStatic internal fun normalizeHex32(value: String, field: String): String { val normalized = normalizeEvenLengthHex(value, field); require(normalized.length == 64) { "$field must contain 64 hex characters" }; return normalized }
        @JvmStatic internal fun normalizeEd25519PublicKeyHex(value: String, field: String): String {
            val normalized = normalizeHex32(value, field)
            val publicKey = ByteArray(Ed25519PublicKeyAdmission.PUBLIC_KEY_LENGTH) { index ->
                val offset = index * 2
                ((Character.digit(normalized[offset], 16) shl 4) or
                    Character.digit(normalized[offset + 1], 16)).toByte()
            }
            require(Ed25519PublicKeyAdmission.isValid(publicKey)) {
                "$field must encode a canonical prime-order Ed25519 public key"
            }
            return normalized
        }
        @JvmStatic internal fun normalizeOptionalHex32(value: String?, field: String): String? = if (value == null || value.trim().isEmpty()) null else normalizeHex32(value, field)

        @JvmStatic internal fun normalizePositiveU32(value: Long, field: String): Long {
            require(value > 0L && value <= U32_MAX) { "$field must be a positive u32" }
            return value
        }

        @JvmStatic internal fun normalizeOptionalU32(value: Long?, field: String): Long? {
            if (value == null) return null
            require(value >= 0L && value <= U32_MAX) { "$field must be a u32" }
            return value
        }

        @JvmStatic internal fun normalizeOptionalNonNegative(value: Long?, field: String): Long? {
            if (value == null) return null
            require(value >= 0L) { "$field must be non-negative" }
            return value
        }

        @JvmStatic internal fun normalizeVerifyingKeyStatus(value: String?): String? {
            val normalized = normalizeOptionalNonBlank(value, "status")?.lowercase() ?: return null
            return when (normalized) {
                "proposed" -> "Proposed"
                "active" -> "Active"
                "withdrawn" -> "Withdrawn"
                else -> throw IllegalArgumentException("status must be Proposed, Active, or Withdrawn")
            }
        }

        private data class VerifyingKeyPayload(val bytes: ByteArray?, val length: Long?)

        private fun normalizeVerifierBytes(bytes: ByteArray?, explicitLength: Long?): VerifyingKeyPayload? {
            if (bytes == null) {
                val length = explicitLength?.let { normalizePositiveU32(it, "vkLen") }
                return if (length == null) null else VerifyingKeyPayload(null, length)
            }
            require(bytes.isNotEmpty()) { "vkBytes must not be empty" }
            val actualLength = bytes.size.toLong()
            require(actualLength <= U32_MAX) { "vkBytes length must fit in a u32" }
            if (explicitLength != null) {
                val expected = normalizePositiveU32(explicitLength, "vkLen")
                require(expected == actualLength) { "vkLen must match vkBytes length" }
            }
            return VerifyingKeyPayload(bytes.copyOf(), actualLength)
        }

        @JvmStatic internal fun validateVerifyingKeyHeightRange(activationHeight: Long?, withdrawHeight: Long?) {
            val activation = normalizeOptionalNonNegative(activationHeight, "activationHeight")
            val withdraw = normalizeOptionalNonNegative(withdrawHeight, "withdrawHeight")
            require(activation == null || withdraw == null || withdraw >= activation) {
                "withdrawHeight must be greater than or equal to activationHeight"
            }
        }

        private fun validateVerifyingKeyMaterial(
            vkPayload: VerifyingKeyPayload?,
            commitmentHex: String?,
        ) {
            if (vkPayload?.bytes == null) {
                require(commitmentHex != null) { "commitmentHex is required when vkBytes is omitted" }
                require(vkPayload?.length != null) { "vkLen is required when vkBytes is omitted" }
            }
        }

        @JvmStatic internal fun validateInlineVerifyingKeyCommitment(backend: String, bytes: ByteArray?, commitmentHex: String?) {
            if (bytes == null || commitmentHex == null) return
            val expected = verifyingKeyCommitmentHex(backend, bytes)
            require(expected == commitmentHex) {
                "commitmentHex must match domain-separated SHA-256 of backend and vkBytes"
            }
        }

        @JvmStatic internal fun verifyingKeyCommitmentHex(
            backend: String,
            bytes: ByteArray,
        ): String {
            val digest = MessageDigest.getInstance("SHA-256")
            val backendBytes = backend.toByteArray(StandardCharsets.UTF_8)
            digest.update("iroha:zk:v1:vk".toByteArray(StandardCharsets.UTF_8))
            digest.update(u64Be(backendBytes.size.toLong()))
            digest.update(backendBytes)
            digest.update(u64Be(bytes.size.toLong()))
            digest.update(bytes)
            return hexLower(digest.digest())
        }

        private fun u64Be(value: Long): ByteArray {
            var remaining = value
            val out = ByteArray(8)
            for (index in 7 downTo 0) {
                out[index] = (remaining and 0xffL).toByte()
                remaining = remaining ushr 8
            }
            return out
        }

        private fun putOptionalVerifierFields(
            payload: MutableMap<String, Any>,
            curve: String?,
            maxProofBytes: Long?,
            metadataUriCid: String?,
            verifyingKeyBytesCid: String?,
            activationHeight: Long?,
            withdrawHeight: Long?,
            commitmentHex: String?,
            vkPayload: VerifyingKeyPayload?,
            status: String?,
        ) {
            curve?.let { payload["curve"] = normalizeNonBlank(it, "curve") }
            normalizeOptionalU32(maxProofBytes, "maxProofBytes")?.let { payload["max_proof_bytes"] = it }
            metadataUriCid?.let { payload["metadata_uri_cid"] = normalizeNonBlank(it, "metadataUriCid") }
            verifyingKeyBytesCid?.let { payload["vk_bytes_cid"] = normalizeNonBlank(it, "verifyingKeyBytesCid") }
            normalizeOptionalNonNegative(activationHeight, "activationHeight")?.let { payload["activation_height"] = it }
            normalizeOptionalNonNegative(withdrawHeight, "withdrawHeight")?.let { payload["withdraw_height"] = it }
            commitmentHex?.let { payload["commitment_hex"] = it }
            vkPayload?.bytes?.let { payload["vk_bytes"] = Base64.getEncoder().encodeToString(it) }
            vkPayload?.length?.let { payload["vk_len"] = it }
            normalizeVerifyingKeyStatus(status)?.let { payload["status"] = it }
        }

        private fun hexLower(bytes: ByteArray): String {
            val out = StringBuilder(bytes.size * 2)
            for (byte in bytes) {
                val value = byte.toInt() and 0xff
                if (value < 16) out.append('0')
                out.append(value.toString(16))
            }
            return out.toString()
        }

        private val SCCP_PROOF_SUBMIT_FIELDS = setOf(
            "authority", "fee_payment", "signature_b64", "transaction_payload_b64",
            "destination_proof_b64", "creation_time_ms",
        )
        private val SCCP_MESSAGE_SUBMIT_FIELDS = setOf(
            "authority", "fee_payment", "signature_b64", "transaction_payload_b64",
            "native_proof_b64", "creation_time_ms",
        )
        private const val SCCP_MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024
        private const val SCCP_MAX_DESTINATION_ARTIFACT_BYTES =
            SCCP_MAX_NATIVE_PROOF_BYTES + 64 * 1024

        private fun optionalSccpArtifact(fields: Map<*, *>, field: String): String? =
            when (val value = fields[field]) {
                null -> null
                is String -> value
                else -> throw IllegalArgumentException(
                    "$field must be a canonical padded base64 string"
                )
            }
    }
}
