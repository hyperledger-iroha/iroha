package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.io.UnsupportedEncodingException;
import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Function;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.crypto.export.KeyExportBundle;
import org.hyperledger.iroha.android.crypto.export.KeyExportException;
import org.hyperledger.iroha.android.nexus.UaidBindingsQuery;
import org.hyperledger.iroha.android.nexus.UaidBindingsResponse;
import org.hyperledger.iroha.android.nexus.UaidJsonParser;
import org.hyperledger.iroha.android.nexus.UaidLiteral;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse;
import org.hyperledger.iroha.android.offline.OfflineJournalKey;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchSummary;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/**
 * HTTP-based client implementation that will forward transactions to an Iroha Torii endpoint.
 *
 * <p>Serialization and endpoint construction follow the `/v1/pipeline/transactions` Torii route.
 * Network execution is delegated to {@link HttpTransportExecutor} so tests can run without making
 * outbound calls.
 */
public final class HttpClientTransport implements IrohaClient {

  private static final String RETRY_SIGNAL_ID = "android.torii.http.retry";
  private static final String PIPELINE_STATUS_SIGNAL = "android.torii.pipeline.status";
  private static final String REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure";

  private final HttpTransportExecutor executor;
  private final ClientConfig config;
  private final SorafsGatewayClient sorafsGatewayClient;
  private final AtomicBoolean deviceProfileEmitted = new AtomicBoolean(false);

  public HttpClientTransport(final HttpTransportExecutor executor, final ClientConfig config) {
    this.executor = Objects.requireNonNull(executor, "executor");
    this.config = Objects.requireNonNull(config, "config");
    this.sorafsGatewayClient =
        SorafsGatewayClient.builder()
            .setExecutor(this.executor)
            .setBaseUri(config.sorafsGatewayUri())
            .setTimeout(config.requestTimeout())
            .setDefaultHeaders(config.defaultHeaders())
            .setObservers(config.observers())
            .build();
  }

  @Override
  public CompletableFuture<ClientResponse> submitTransaction(final SignedTransaction transaction) {
    Objects.requireNonNull(transaction, "transaction");
    final String hashHex = SignedTransactionHasher.hashHex(transaction);
    return flushPendingQueue()
        .exceptionally(ex -> null)
        .thenCompose(ignored -> submitWithRetryInternal(transaction, hashHex, 1, true));
  }

  @Override
  public CompletableFuture<ClientResponse> submitTransactionJson(
      final byte[] encodedVersionedTransactionJson) {
    final TransportRequest request =
        ToriiRequestBuilder.buildSubmitJsonRequest(
            config.baseUri(),
            encodedVersionedTransactionJson,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader());
    return executeAccepted(request, "transaction JSON submit", 202);
  }

  @Override
  public CompletableFuture<ClientResponse> submitTransactionEntrypoint(
      final byte[] encodedVersionedEntrypoint) {
    final TransportRequest request =
        ToriiRequestBuilder.buildSubmitEntrypointRequest(
            config.baseUri(),
            encodedVersionedEntrypoint,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader());
    notifyRequest(request);
    return executor
        .execute(request)
        .handle(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                notifyFailure(request, cause);
                final CompletableFuture<ClientResponse> failed = new CompletableFuture<>();
                failed.completeExceptionally(cause);
                return failed;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      extractTransactionHash(response).orElse(null),
                      extractRejectCode(response));
              if (clientResponse.statusCode() < 200 || clientResponse.statusCode() >= 300) {
                notifyFailure(
                    request,
                    new RuntimeException(
                        "Torii request failed with status " + clientResponse.statusCode()));
              } else {
                notifyResponse(request, clientResponse);
              }
              return CompletableFuture.completedFuture(clientResponse);
            })
        .thenCompose(future -> future);
  }

  @Override
  public CompletableFuture<ClientResponse> submitTransactionEntrypointJson(
      final byte[] encodedVersionedEntrypointJson) {
    final TransportRequest request =
        ToriiRequestBuilder.buildSubmitEntrypointJsonRequest(
            config.baseUri(),
            encodedVersionedEntrypointJson,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader());
    return executeAccepted(request, "transaction entrypoint JSON submit", 202);
  }

  private CompletableFuture<ClientResponse> executeAccepted(
      final TransportRequest request, final String errorContext, final int acceptedStatus) {
    notifyRequest(request);
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                notifyFailure(request, cause);
                future.completeExceptionally(
                    new RuntimeException(errorContext + " request failed", cause));
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              if (response.statusCode() != acceptedStatus) {
                final RuntimeException error =
                    new RuntimeException(
                        errorContext + " request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              notifyResponse(request, clientResponse);
              future.complete(clientResponse);
            });
    return future;
  }

  @Override
  public CompletableFuture<Map<String, Object>> waitForTransactionStatus(
      final String hashHex, final PipelineStatusOptions options) {
    Objects.requireNonNull(hashHex, "hashHex");
    final PipelineStatusOptions resolved = PipelineStatusOptions.resolve(options);
    final long deadline =
        resolved.timeoutMillis() == null
            ? Long.MAX_VALUE
            : System.currentTimeMillis() + Math.max(0L, resolved.timeoutMillis());
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    pollPipelineStatus(hashHex, resolved, deadline, 0, null, future);
    return future;
  }

  @Override
  public CompletableFuture<Map<String, Object>> waitForTransactionStatusStream(
      final String hashHex, final PipelineStatusOptions options) {
    Objects.requireNonNull(hashHex, "hashHex");
    final PipelineStatusOptions resolved = PipelineStatusOptions.resolve(options);
    final long deadline =
        resolved.timeoutMillis() == null
            ? Long.MAX_VALUE
            : System.currentTimeMillis() + Math.max(0L, resolved.timeoutMillis());
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    fetchPipelineStatusSnapshot(hashHex)
        .whenComplete(
            (payload, throwable) -> {
              if (future.isDone()) {
                return;
              }
              if (throwable != null) {
                final Throwable cause = unwrapCompletion(throwable);
                future.completeExceptionally(cause);
                return;
              }

              final AtomicInteger attempts = new AtomicInteger(payload == null ? 0 : 1);
              final AtomicReference<Map<String, Object>> lastPayload = new AtomicReference<>(payload);
              if (payload != null
                  && processPipelineTerminalPayload(
                      hashHex, resolved, payload, attempts.get(), future)) {
                return;
              }

              if (deadline != Long.MAX_VALUE && System.currentTimeMillis() >= deadline) {
                future.completeExceptionally(
                    new TransactionTimeoutException(
                        "Transaction " + hashHex + " did not reach a terminal status "
                            + "within the configured timeout",
                        hashHex,
                        attempts.get(),
                        lastPayload.get()));
                return;
              }

              openPipelineStatusStream(hashHex, resolved, deadline, attempts, lastPayload, future);
            });
    return future;
  }

  public ClientConfig config() {
    return config;
  }

  /**
   * Cancels in-flight HTTP requests and releases any underlying resources held by the executor.
   *
   * <p>This is a no-op for executors that do not expose lifecycle hooks.</p>
   */
  public void invalidateAndCancel() {
    executor.invalidateAndCancel();
  }

  /** Creates a Norito RPC client that reuses this transport's configuration (and HTTP client when possible). */
  public NoritoRpcClient newNoritoRpcClient() {
    return config.toNoritoRpcClient(executor);
  }

  /** Creates a streaming client wired to this transport's configuration. */
  public ToriiEventStreamClient newEventStreamClient() {
    return ToriiEventStreamClient.builder()
        .setBaseUri(config.baseUri())
        .setTransportExecutor(executor)
        .defaultHeaders(config.defaultHeaders())
        .observers(config.observers())
        .build();
  }

  /** Creates a SoraFS gateway client that reuses this transport's HTTP executor and configuration. */
  public SorafsGatewayClient newSorafsGatewayClient() {
    return newSorafsGatewayClient(config.sorafsGatewayUri());
  }

  /**
   * Creates a SoraFS gateway client targeting {@code baseUri} while reusing this transport's
   * executor, timeout, headers, and observers.
   */
  public SorafsGatewayClient newSorafsGatewayClient(final URI baseUri) {
    final SorafsGatewayClient.Builder builder =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(Objects.requireNonNull(baseUri, "baseUri"))
            .setTimeout(config.requestTimeout())
            .setDefaultHeaders(config.defaultHeaders())
            .setObservers(config.observers());
    return builder.build();
  }

  /** Returns the SoraFS gateway client wired to this transport's configuration. */
  public SorafsGatewayClient sorafsGatewayClient() {
    return sorafsGatewayClient;
  }

  /** Post a gateway fetch request and return the raw response. */
  public CompletableFuture<ClientResponse> sorafsGatewayFetch(final GatewayFetchRequest request) {
    return sorafsGatewayClient.fetch(request);
  }

  /** Post a gateway fetch request and parse the response summary. */
  public CompletableFuture<GatewayFetchSummary> sorafsGatewayFetchSummary(
      final GatewayFetchRequest request) {
    return sorafsGatewayClient.fetchSummary(request);
  }

  /** Fetches `/v1/accounts/{uaid}/portfolio`. */
  public CompletableFuture<UaidPortfolioResponse> getUaidPortfolio(final String uaid) {
    return getUaidPortfolio(uaid, null);
  }

  /** Fetches `/v1/accounts/{uaid}/portfolio` with optional query parameters. */
  public CompletableFuture<UaidPortfolioResponse> getUaidPortfolio(
      final String uaid, final UaidPortfolioQuery query) {
    final String canonical = UaidLiteral.canonicalize(uaid, "uaid portfolio");
    final Map<String, String> params = query == null ? Map.of() : query.toQueryParameters();
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/accounts/" + encodePathSegment(canonical) + "/portfolio", params);
    return fetchJson(request, UaidJsonParser::parsePortfolio, "UAID portfolio");
  }

  /** Fetches `/v1/space-directory/uaids/{uaid}` bindings. */
  public CompletableFuture<UaidBindingsResponse> getUaidBindings(final String uaid) {
    return getUaidBindings(uaid, null);
  }

  /** Fetches `/v1/space-directory/uaids/{uaid}` bindings with query parameters. */
  public CompletableFuture<UaidBindingsResponse> getUaidBindings(
      final String uaid, final UaidBindingsQuery query) {
    final String canonical = UaidLiteral.canonicalize(uaid, "uaid bindings");
    final Map<String, String> params =
        query == null ? Map.of() : query.toQueryParameters();
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/space-directory/uaids/" + encodePathSegment(canonical), params);
    return fetchJson(request, UaidJsonParser::parseBindings, "UAID bindings");
  }

  /** Fetches `/v1/space-directory/uaids/{uaid}/manifests`. */
  public CompletableFuture<UaidManifestsResponse> getUaidManifests(
      final String uaid, final UaidManifestQuery query) {
    final String canonical = UaidLiteral.canonicalize(uaid, "uaid manifests");
    final Map<String, String> params =
        query == null ? Map.of() : query.toQueryParameters();
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/space-directory/uaids/" + encodePathSegment(canonical) + "/manifests", params);
    return fetchJson(request, UaidJsonParser::parseManifests, "UAID manifests");
  }

  /** Fetches globally registered identifier policies from `/v1/identifier-policies`. */
  public CompletableFuture<IdentifierPolicyListResponse> listIdentifierPolicies() {
    final TransportRequest request = buildJsonGetRequest("/v1/identifier-policies", Map.of());
    return fetchJson(request, IdentifierJsonParser::parsePolicyList, "identifier policy list");
  }

  /** Fetches globally registered RAM-LFE program policies from `/v1/ram-lfe/program-policies`. */
  public CompletableFuture<RamLfeProgramPolicyListResponse> listRamLfeProgramPolicies() {
    final TransportRequest request =
        buildJsonGetRequest("/v1/ram-lfe/program-policies", Map.of());
    return fetchJson(request, RamLfeJsonParser::parsePolicyList, "ram-lfe program policy list");
  }

  /** Fetches a persisted identifier claim by its deterministic receipt hash. */
  public CompletableFuture<Optional<IdentifierClaimRecord>> getIdentifierClaimByReceiptHash(
      final String receiptHash) {
    final String normalizedReceiptHash = normalizeHex32(receiptHash, "receiptHash");
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/identifiers/receipts/" + encodePathSegment(normalizedReceiptHash), Map.of());
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseClaimRecord, "identifier claim lookup");
  }

  /** Resolves an identifier using a typed request wrapper. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> resolveIdentifier(
      final IdentifierResolveRequest requestBody) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildIdentifierResolvePayload(
                requestBody.policyId(),
                requestBody.encryptedInputHex(),
                requestBody.outputOpening()));
    final TransportRequest request = buildJsonPostRequest("/v1/identifiers/resolve", body);
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseResolutionReceipt, "identifier resolve");
  }

  /** Resolves a hidden identifier by posting encrypted input and a verified output opening. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> resolveIdentifier(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    return resolveIdentifier(
        IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening));
  }

  /** Issues a claim receipt using a typed request wrapper. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> issueIdentifierClaimReceipt(
      final String accountId, final IdentifierResolveRequest requestBody) {
    Objects.requireNonNull(requestBody, "requestBody");
    final String normalizedAccountId = normalizeNonBlank(accountId, "accountId");
    final byte[] body =
        encodeJsonBody(
            buildIdentifierResolvePayload(
                requestBody.policyId(),
                requestBody.encryptedInputHex(),
                requestBody.outputOpening()));
    final TransportRequest request =
        buildJsonPostRequest(
            "/v1/accounts/"
                + encodePathSegment(normalizedAccountId)
                + "/identifiers/claim-receipt",
            body);
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseResolutionReceipt, "identifier claim receipt");
  }

  /** Issues a claim receipt by posting encrypted input and a verified output opening. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> issueIdentifierClaimReceipt(
      final String accountId,
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    return issueIdentifierClaimReceipt(
        accountId, IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening));
  }

  /** Executes a RAM-LFE program using a typed request wrapper. */
  public CompletableFuture<Optional<RamLfeExecuteResponse>> executeRamLfeProgram(
      final String programId, final RamLfeExecuteRequest requestBody) {
    Objects.requireNonNull(requestBody, "requestBody");
    final String normalizedProgramId = normalizeNonBlank(programId, "programId");
    final byte[] body =
        encodeJsonBody(buildRamLfeExecutePayload(requestBody.encryptedInputHex()));
    final TransportRequest request =
        buildJsonPostRequest(
            "/v1/ram-lfe/programs/" + encodePathSegment(normalizedProgramId) + "/execute", body);
    return fetchJsonAllowingNotFound(
        request, RamLfeJsonParser::parseExecuteResponse, "ram-lfe execute");
  }

  /**
   * Executes a RAM-LFE program by posting BFV ciphertext hex to
   * `/v1/ram-lfe/programs/{program_id}/execute`.
   */
  public CompletableFuture<Optional<RamLfeExecuteResponse>> executeRamLfeProgram(
      final String programId, final String encryptedInputHex) {
    return executeRamLfeProgram(programId, buildRamLfeExecuteRequest(encryptedInputHex));
  }

  /** Verifies a RAM-LFE execution receipt against the node's registered program policy. */
  public CompletableFuture<RamLfeReceiptVerifyResponse> verifyRamLfeReceipt(
      final RamLfeReceiptVerifyRequest requestBody) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildRamLfeReceiptVerifyPayload(requestBody.receipt(), requestBody.outputHex()));
    final TransportRequest request = buildJsonPostRequest("/v1/ram-lfe/receipts/verify", body);
    return fetchJson(
        request, RamLfeJsonParser::parseReceiptVerifyResponse, "ram-lfe receipt verify");
  }

  /** Verifies a RAM-LFE execution receipt against the node's registered program policy. */
  public CompletableFuture<RamLfeReceiptVerifyResponse> verifyRamLfeReceipt(
      final Map<String, Object> receipt, final String outputHex) {
    return verifyRamLfeReceipt(new RamLfeReceiptVerifyRequest(receipt, outputHex));
  }

  /** Fetches the public Sora VPN profile. */
  public CompletableFuture<VpnProfile> getVpnProfile() {
    final TransportRequest request = buildJsonGetRequest("/v1/vpn/profile", Map.of());
    return fetchJson(request, VpnJsonParser::parseProfile, "vpn profile");
  }

  /** Creates a signed quote for a native XOR VPN lease escrow. */
  public CompletableFuture<VpnQuote> createVpnQuote(
      final VpnQuoteCreateRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildVpnQuoteCreatePayload(
                requestBody.exitClass(), requestBody.meteringPublicKeyHex()));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/vpn/quotes", body, canonicalAuth);
    return fetchJson(request, VpnJsonParser::parseQuote, "vpn quote create");
  }

  /** Opens a VPN session after the exact quote-bound native lease transaction commits. */
  public CompletableFuture<VpnSession> createVpnSession(
      final VpnSessionCreateRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildVpnSessionCreatePayload(
                requestBody.exitClass(),
                requestBody.quoteId(),
                requestBody.paymentTxHash(),
                requestBody.meteringPublicKeyHex()));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/vpn/sessions", body, canonicalAuth);
    return fetchJson(request, VpnJsonParser::parseSession, "vpn session create");
  }

  /** Fetches an active VPN session owned by the signed account. */
  public CompletableFuture<Optional<VpnSession>> getVpnSession(
      final String sessionId,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final String normalizedSessionId = normalizeHex32(sessionId, "sessionId");
    final TransportRequest request =
        buildVpnRequest(
            "GET",
            "/v1/vpn/sessions/" + encodePathSegment(normalizedSessionId),
            null,
            canonicalAuth);
    return fetchJsonAllowingNotFound(request, VpnJsonParser::parseSession, "vpn session lookup");
  }

  /** Deletes an active VPN session and returns Torii's disconnect receipt when present. */
  public CompletableFuture<Optional<VpnReceipt>> deleteVpnSession(
      final String sessionId,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final String normalizedSessionId = normalizeHex32(sessionId, "sessionId");
    final TransportRequest request =
        buildVpnRequest(
            "DELETE",
            "/v1/vpn/sessions/" + encodePathSegment(normalizedSessionId),
            null,
            canonicalAuth);
    return fetchJsonAllowingNotFound(request, VpnJsonParser::parseReceipt, "vpn session delete");
  }

  /** Submits an operator receipt and returns the native lease settlement instruction. */
  public CompletableFuture<VpnReceipt> submitVpnReceipt(
      final VpnReceiptSubmitRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildVpnReceiptSubmitPayload(
                requestBody.relayReceiptHex(),
                requestBody.clientVoucherHex(),
                requestBody.leaseIdHex()));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/vpn/receipts", body, canonicalAuth);
    return fetchJson(request, VpnJsonParser::parseReceipt, "vpn receipt submit");
  }

  /** Lists VPN receipts for the signed account. */
  public CompletableFuture<VpnReceiptListResponse> listVpnReceipts(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final TransportRequest request = buildVpnRequest("GET", "/v1/vpn/receipts", null, canonicalAuth);
    return fetchJson(request, VpnJsonParser::parseReceiptList, "vpn receipt list");
  }

  /** Deploys contract bytecode via `POST /v1/contracts/deploy`. */
  public CompletableFuture<Optional<ContractDeployResponse>> deployContract(
      final String authority,
      final String privateKey,
      final String codeB64,
      final String contractAlias,
      final Long leaseExpiryMs) {
    final byte[] body =
        encodeJsonBody(
            buildDeployContractPayload(authority, privateKey, codeB64, contractAlias, leaseExpiryMs));
    final TransportRequest request = buildJsonPostRequest("/v1/contracts/deploy", body);
    return fetchOptionalJson(request, ContractJsonParser::parseDeployResponse, "contract deploy");
  }

  /** Deploys contract bytecode via `POST /v1/contracts/deploy`. */
  public CompletableFuture<Optional<ContractDeployResponse>> deployContract(
      final String authority,
      final String privateKey,
      final String codeB64,
      final String contractAlias) {
    return deployContract(authority, privateKey, codeB64, contractAlias, null);
  }

  /** Calls a deployed contract via `POST /v1/contracts/call`. */
  public CompletableFuture<ContractCallResponse> callContract(
      final String authority,
      final String privateKey,
      final long gasLimit,
      final String contractAddress,
      final String contractAlias,
      final String entrypoint,
      final Object payload,
      final String gasAssetId) {
    final byte[] body =
        encodeJsonBody(
            buildContractCallPayload(
                authority,
                privateKey,
                gasLimit,
                contractAddress,
                contractAlias,
                entrypoint,
                payload,
                gasAssetId));
    final TransportRequest request = buildJsonPostRequest("/v1/contracts/call", body);
    return fetchJson(request, ContractJsonParser::parseCallResponse, "contract call");
  }

  /** Proposes a generic multisig instruction batch via `POST /v1/multisig/propose`. */
  @Override
  public CompletableFuture<MultisigResponse> proposeMultisig(
      final MultisigProposeRequest requestBody) {
    final byte[] body = encodeJsonBody(buildMultisigProposePayload(requestBody));
    final TransportRequest request = buildJsonPostRequest("/v1/multisig/propose", body);
    return fetchJson(request, ContractJsonParser::parseMultisigResponse, "multisig propose");
  }

  /** Fetches one governance binding via `GET /v1/gov/contracts/{contract_address}`. */
  public CompletableFuture<GovernanceContractResponse> getGovernanceContract(
      final String contractAddress) {
    final String normalizedAddress = normalizeNonBlank(contractAddress, "contractAddress");
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/gov/contracts/" + encodePathSegment(normalizedAddress), Map.of());
    return fetchJson(
        request,
        ContractJsonParser::parseGovernanceContractResponse,
        "governance contract");
  }

  /** Resolves an account alias via `POST /v1/aliases/resolve`. */
  @Override
  public CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias) {
    final String normalizedAlias = normalizeNonBlank(alias, "alias");
    final byte[] body = encodeJsonBody(Map.of("alias", normalizedAlias));
    final TransportRequest request = buildJsonPostRequest("/v1/aliases/resolve", body);
    return fetchJsonAllowingNotFound(
        request, AccountAliasJsonParser::parseResolution, "account alias resolve");
  }

  /** Creates a transport backed by the platform HTTP executor (OkHttp on Android). */
  public static HttpClientTransport createDefault(final ClientConfig config) {
    return new HttpClientTransport(PlatformHttpTransportExecutor.createDefault(), config);
  }

  public static HttpClientTransport withExecutor(
      final HttpTransportExecutor executor, final ClientConfig config) {
    return new HttpClientTransport(executor, config);
  }

  /**
   * Creates a transport using the platform-default executor (OkHttp on Android, JDK client on JVM).
   */
  public static HttpClientTransport withDefaultExecutor(final ClientConfig config) {
    return new HttpClientTransport(PlatformHttpTransportExecutor.createDefault(), config);
  }

  public static ClientConfig withOfflineJournalQueue(
      final ClientConfig config, final Path journalPath, final OfflineJournalKey key) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableOfflineJournalQueue(journalPath, key).build();
  }

  public static ClientConfig withOfflineJournalQueue(
      final ClientConfig config, final Path journalPath, final byte[] seed) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableOfflineJournalQueue(journalPath, seed).build();
  }

  public static ClientConfig withOfflineJournalQueue(
      final ClientConfig config, final Path journalPath, final char[] passphrase) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableOfflineJournalQueue(journalPath, passphrase).build();
  }

  /**
   * Returns a copy of {@code config} with a directory-backed pending queue rooted at {@code
   * queueDir}. Each queued transaction is persisted as its own envelope file to satisfy OEM or
   * managed-device storage policies.
   */
  public static ClientConfig withDirectoryPendingQueue(
      final ClientConfig config, final Path queueDir) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableDirectoryPendingQueue(queueDir).build();
  }

  /** Returns a copy of {@link ClientConfig} with a file-backed pending queue enabled. */
  public static ClientConfig withFilePendingQueue(
      final ClientConfig config, final Path queueFile) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableFilePendingQueue(queueFile).build();
  }

  /**
   * Creates an {@link OfflineToriiClient} that reuses this transport's HTTP executor, base URI,
   * headers, and observers.
   */
  public OfflineToriiClient offlineToriiClient() {
    return config.toOfflineToriiClient(executor);
  }

  /**
   * Creates a {@link SubscriptionToriiClient} that reuses this transport's HTTP executor, base
   * URI, headers, and observers.
   */
  public SubscriptionToriiClient subscriptionToriiClient() {
    return config.toSubscriptionToriiClient(executor);
  }

  private CompletableFuture<Void> flushPendingQueue() {
    final PendingTransactionQueue queue = config.pendingQueue();
    if (queue == null) {
      return CompletableFuture.completedFuture(null);
    }
    final List<SignedTransaction> pending;
    try {
      pending = queue.drain();
    } catch (final IOException ex) {
      final CompletableFuture<Void> failed = new CompletableFuture<>();
      failed.completeExceptionally(new RuntimeException("Failed to drain pending queue", ex));
      return failed;
    }
    recordPendingQueueDepth(queue);
    if (pending.isEmpty()) {
      return CompletableFuture.completedFuture(null);
    }
    CompletableFuture<Void> chain = CompletableFuture.completedFuture(null);
    for (int i = 0; i < pending.size(); i++) {
      final int index = i;
      final SignedTransaction queuedTx = pending.get(i);
      chain = chain.thenCompose(
          ignored -> {
            final CompletableFuture<ClientResponse> submission =
                submitWithRetryInternal(queuedTx, SignedTransactionHasher.hashHex(queuedTx), 1, true);
            CompletableFuture<Void> stage = submission.thenApply(response -> null);
            stage = stage.exceptionally(ex -> {
              requeueRemaining(pending, index + 1);
              throw ex instanceof CompletionException
                  ? (CompletionException) ex
                  : new CompletionException(ex);
            });
            return stage;
          });
    }
    return chain;
  }

  private CompletableFuture<ClientResponse> submitWithRetryInternal(
      final SignedTransaction transaction,
      final String hashHex,
      final int attempt,
      final boolean skipFlush) {
    if (!skipFlush) {
      return flushPendingQueue()
          .exceptionally(ex -> null)
          .thenCompose(ignored -> submitWithRetryInternal(transaction, hashHex, attempt, true));
    }

    final TransportRequest request =
        ToriiRequestBuilder.buildSubmitRequest(
            config.baseUri(),
            transaction,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader());

    notifyRequest(request);
    return executor
        .execute(request)
        .handle((response, throwable) -> {
          if (throwable != null) {
            final Throwable cause =
                throwable instanceof CompletionException ? throwable.getCause() : throwable;
            notifyFailure(request, cause);
            return scheduleRetry(transaction, hashHex, attempt, request, null, cause);
          }
          final ClientResponse clientResponse =
              new ClientResponse(
                  response.statusCode(),
                  response.body(),
                  response.message(),
                  extractTransactionHash(response).orElse(hashHex),
                  extractRejectCode(response));
          if (clientResponse.statusCode() < 200 || clientResponse.statusCode() >= 300) {
            if (config.retryPolicy().shouldRetryResponse(attempt, clientResponse)) {
              return scheduleRetry(transaction, hashHex, attempt, request, clientResponse, null);
            }
            if (config.retryPolicy().isRetryableStatus(clientResponse.statusCode())) {
              final RuntimeException error =
                  new RuntimeException(
                      "Torii request failed with status " + clientResponse.statusCode());
              notifyFailure(request, error);
              enqueuePending(transaction);
              final CompletableFuture<ClientResponse> failed = new CompletableFuture<>();
              failed.completeExceptionally(error);
              return failed;
            }
            notifyResponse(request, clientResponse);
            return CompletableFuture.completedFuture(clientResponse);
          }
          notifyResponse(request, clientResponse);
          return CompletableFuture.completedFuture(clientResponse);
        })
        .thenCompose(future -> future);
  }

  private CompletableFuture<ClientResponse> scheduleRetry(
      final SignedTransaction transaction,
      final String hashHex,
      final int attempt,
      final TransportRequest request,
      final ClientResponse lastResponse,
      final Throwable lastError) {
    final boolean isNetworkFailure = lastError != null && lastResponse == null;
    final boolean hasAnotherAttempt =
        isNetworkFailure
            ? config.retryPolicy().shouldRetryError(attempt)
            : config.retryPolicy().allowsRetry(attempt);
    if (!hasAnotherAttempt) {
      enqueuePending(transaction);
      if (lastResponse != null && lastError == null) {
        notifyFailure(request, new RuntimeException("Retry attempts exhausted"));
      }
      final RuntimeException runtime =
          lastError instanceof RuntimeException
              ? (RuntimeException) lastError
              : new RuntimeException(
                  lastResponse != null
                      ? "Retry attempts exhausted after status code " + lastResponse.statusCode()
                      : "Retry attempts exhausted; transaction queued for later submission",
                  lastError);
      final CompletableFuture<ClientResponse> failed = new CompletableFuture<>();
      failed.completeExceptionally(runtime);
      return failed;
    }

    final Duration delay = config.retryPolicy().delayForAttempt(attempt);
    final long delayMillis = Math.max(0L, Math.min(delay.toMillis(), Long.MAX_VALUE));
    emitRetryTelemetry(request, attempt, delayMillis, lastResponse, lastError);
    return CompletableFuture
        .supplyAsync(
            () -> null, CompletableFuture.delayedExecutor(delayMillis, TimeUnit.MILLISECONDS))
        .thenCompose(ignored -> submitWithRetryInternal(transaction, hashHex, attempt + 1, true));
  }

  private void enqueuePending(final SignedTransaction transaction) {
    final PendingTransactionQueue queue = config.pendingQueue();
    if (queue == null) {
      return;
    }
    try {
      final SignedTransaction enriched = maybeAttachExportBundle(transaction);
      queue.enqueue(enriched);
      recordPendingQueueDepth(queue);
    } catch (final IOException ex) {
      throw new RuntimeException("Failed to persist pending transaction", ex);
    }
  }

  private SignedTransaction maybeAttachExportBundle(final SignedTransaction transaction) {
    if (transaction.keyAlias().isEmpty() || transaction.exportedKeyBundle().isPresent()) {
      return transaction;
    }
    final ClientConfig.ExportOptions exportOptions = config.exportOptions();
    if (exportOptions == null) {
      return transaction;
    }
    final char[] passphrase = exportOptions.passphraseForAlias(transaction.keyAlias().get());
    if (passphrase.length == 0) {
      return transaction;
    }
    try {
      final KeyExportBundle bundle =
          exportOptions.keyManager().exportDeterministicKey(transaction.keyAlias().get(), passphrase);
      final byte[] encoded = bundle.encode();
      return new SignedTransaction(
          transaction.encodedPayload(),
          transaction.signature(),
          transaction.publicKey(),
          transaction.schemaName(),
          transaction.keyAlias().orElse(null),
          encoded);
    } catch (final KeyExportException | org.hyperledger.iroha.android.KeyManagementException ex) {
      throw new RuntimeException("Failed to export key for pending transaction", ex);
    } finally {
      Arrays.fill(passphrase, '\0');
    }
  }

  private void requeueRemaining(final List<SignedTransaction> pending, final int startIndex) {
    final PendingTransactionQueue queue = config.pendingQueue();
    if (queue == null) {
      return;
    }
    for (int i = startIndex; i < pending.size(); i++) {
      enqueuePending(pending.get(i));
    }
  }

  private void recordPendingQueueDepth(final PendingTransactionQueue queue) {
    if (queue == null) {
      return;
    }
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final int depth;
    try {
      depth = queue.size();
    } catch (final IOException ex) {
      return;
    }
    final long depthValue = Integer.toUnsignedLong(depth);
    sink.get()
        .emitSignal(
            "android.pending_queue.depth",
            Map.of(
                "queue", queue.telemetryQueueName(),
                "depth", depthValue));
  }

  private void emitDeviceProfileTelemetry() {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    if (!deviceProfileEmitted.compareAndSet(false, true)) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final DeviceProfileProvider provider = config.deviceProfileProvider();
    if (provider == null) {
      return;
    }
    final Optional<DeviceProfile> profile = provider.snapshot();
    if (profile.isEmpty()) {
      return;
    }
    sink
        .get()
        .emitSignal(
            "android.telemetry.device_profile",
            Map.of("profile_bucket", profile.get().bucket()));
  }

  private void emitNetworkContextTelemetry() {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final Optional<NetworkContext> context = config.networkContextProvider().snapshot();
    if (context.isEmpty()) {
      return;
    }
    sink
        .get()
        .emitSignal("android.telemetry.network_context", context.get().toTelemetryFields());
  }

  private void emitRetryTelemetry(
      final TransportRequest request,
      final int attempt,
      final long delayMillis,
      final ClientResponse lastResponse,
      final Throwable lastError) {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final Map<String, Object> fields = new LinkedHashMap<>();
    maybePutAuthorityHash(fields, request, sink.get(), RETRY_SIGNAL_ID);
    fields.put("route", resolveRoute(request));
    fields.put("retry_count", attempt);
    fields.put("error_code", buildRetryErrorCode(lastResponse, lastError));
    fields.put("backoff_ms", delayMillis);
    sink.get().emitSignal(RETRY_SIGNAL_ID, fields);
  }

  private void emitPipelineStatusTelemetry(
      final TransportRequest request,
      final String transactionHash,
      final String statusKind,
      final boolean isSuccess,
      final boolean isFailure,
      final int attempts) {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (sink.isEmpty()) {
      return;
    }
    final Map<String, Object> fields = new LinkedHashMap<>();
    maybePutAuthorityHash(fields, request, sink.get(), PIPELINE_STATUS_SIGNAL);
    if (transactionHash != null && !transactionHash.isBlank()) {
      fields.put("tx_hash", transactionHash);
    }
    fields.put("status_kind", statusKind == null ? "" : statusKind);
    fields.put("outcome", isSuccess ? "success" : (isFailure ? "failure" : "pending"));
    fields.put("attempts", attempts);
    sink.get().emitSignal(PIPELINE_STATUS_SIGNAL, fields);
  }

  private void maybePutAuthorityHash(
      final Map<String, Object> fields,
      final TransportRequest request,
      final TelemetrySink sink,
      final String signalId) {
    final TelemetryOptions.Redaction redaction = config.telemetryOptions().redaction();
    if (!redaction.enabled()) {
      return;
    }
    final String authority = resolveAuthority(request).trim();
    if (authority.isEmpty()) {
      emitRedactionFailure(sink, signalId, "blank_authority");
      return;
    }
    final Optional<String> hashed = redaction.hashAuthority(authority);
    if (hashed.isPresent()) {
      fields.put("authority_hash", hashed.get());
    } else {
      emitRedactionFailure(sink, signalId, "hash_failed");
    }
  }

  private static String buildRetryErrorCode(
      final ClientResponse lastResponse, final Throwable lastError) {
    if (lastResponse != null) {
      return Integer.toString(lastResponse.statusCode());
    }
    if (lastError != null) {
      return lastError.getClass().getSimpleName();
    }
    return "unknown";
  }

  private static String resolveRoute(final TransportRequest request) {
    final URI uri = request == null ? null : request.uri();
    if (uri == null) {
      return "";
    }
    final String path = uri.getRawPath();
    return path == null ? "" : path;
  }

  private static String extractRejectCode(final TransportResponse response) {
    if (response == null) {
      return null;
    }
    return HttpErrorMessageExtractor.extractRejectCode(
        response.headers(), "x-iroha-reject-code", response.body());
  }

  private static Optional<String> extractTransactionHash(final TransportResponse response) {
    if (response == null) {
      return Optional.empty();
    }
    for (final String headerName : List.of("x-iroha-transaction-hash", "x-iroha-tx-hash")) {
      final List<String> values = response.headers().get(headerName);
      if (values == null) {
        continue;
      }
      for (final String value : values) {
        final String normalized = normalizeTransactionHashHeader(value);
        if (normalized != null) {
          return Optional.of(normalized);
        }
      }
    }
    return Optional.empty();
  }

  private static String normalizeTransactionHashHeader(final String value) {
    if (value == null) {
      return null;
    }
    String normalized = value.trim();
    if (normalized.startsWith("0x") || normalized.startsWith("0X")) {
      normalized = normalized.substring(2);
    }
    if (normalized.length() != 64) {
      return null;
    }
    for (int index = 0; index < normalized.length(); index++) {
      final char ch = normalized.charAt(index);
      final boolean hex =
          (ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F');
      if (!hex) {
        return null;
      }
    }
    return normalized.toLowerCase(java.util.Locale.ROOT);
  }

  private static String resolveAuthority(final TransportRequest request) {
    if (request == null) {
      return "";
    }
    final URI uri = request.uri();
    if (uri != null && uri.getAuthority() != null) {
      return uri.getAuthority();
    }
    final List<String> host = request.headers().get("Host");
    return host == null || host.isEmpty() ? "" : host.get(0);
  }

  private static void emitRedactionFailure(
      final TelemetrySink sink, final String signalId, final String reason) {
    sink.emitSignal(
        REDACTION_FAILURE_SIGNAL,
        Map.of(
            "signal_id", signalId,
            "reason", reason));
  }

  private void pollPipelineStatus(
      final String hashHex,
      final PipelineStatusOptions options,
      final long deadline,
      final int attemptsSoFar,
      final Map<String, Object> lastPayload,
      final CompletableFuture<Map<String, Object>> future) {
    if (future.isDone()) {
      return;
    }
    if (options.maxAttempts() != null && attemptsSoFar >= options.maxAttempts()) {
      future.completeExceptionally(
          new TransactionTimeoutException(
              "Transaction " + hashHex + " did not reach a terminal status "
                  + "after " + attemptsSoFar + " attempts",
              hashHex,
              attemptsSoFar,
              lastPayload));
      return;
    }

    final TransportRequest request =
        ToriiRequestBuilder.buildStatusRequest(
            config.baseUri(), hashHex, config.requestTimeout(), config.defaultHeaders());
    notifyRequest(request);
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              try {
                if (future.isDone()) {
                  return;
                }
                if (throwable != null) {
                  final Throwable cause =
                      throwable instanceof CompletionException ? throwable.getCause() : throwable;
                  notifyFailure(request, cause);
                  future.completeExceptionally(cause);
                  return;
                }

                final ClientResponse clientResponse =
                    new ClientResponse(
                        response.statusCode(),
                        response.body(),
                        response.message(),
                        null,
                        extractRejectCode(response));
                notifyResponse(request, clientResponse);

                final int statusCode = clientResponse.statusCode();
                if (statusCode != 200
                    && statusCode != 202
                    && statusCode != 204
                    && statusCode != 404) {
                  future.completeExceptionally(
                      buildPipelineStatusHttpException(hashHex, clientResponse));
                  return;
                }

                final Map<String, Object> payload =
                    parsePipelineStatusPayload(clientResponse.body());
                final int nextAttempts = attemptsSoFar + 1;
                final Optional<String> statusKind =
                    payload == null
                        ? Optional.empty()
                        : PipelineStatusExtractor.extractStatusKind(payload);
                final String statusLiteral = statusKind.orElse(null);
                final boolean isSuccess =
                    statusLiteral != null && options.successStatuses().contains(statusLiteral);
                final boolean isFailure =
                    statusLiteral != null && options.failureStatuses().contains(statusLiteral);
                emitPipelineStatusTelemetry(
                    request, hashHex, statusLiteral, isSuccess, isFailure, nextAttempts);

                if (options.observer() != null) {
                  try {
                    options.observer().onStatus(statusLiteral, payload, nextAttempts);
                  } catch (final RuntimeException observerError) {
                    future.completeExceptionally(observerError);
                    return;
                  }
                }

                if (isSuccess) {
                  future.complete(payload != null ? payload : Map.of());
                  return;
                }
                if (isFailure) {
                  final String rejectionReason =
                      PipelineStatusExtractor.extractRejectionReason(payload).orElse(null);
                  future.completeExceptionally(
                      new TransactionStatusException(
                          hashHex,
                          statusLiteral,
                          rejectionReason,
                          payload));
                  return;
                }

                if (options.maxAttempts() != null && nextAttempts >= options.maxAttempts()) {
                  future.completeExceptionally(
                      new TransactionTimeoutException(
                          "Transaction " + hashHex + " did not reach a terminal status "
                              + "after " + nextAttempts + " attempts",
                          hashHex,
                          nextAttempts,
                          payload));
                  return;
                }

                if (deadline != Long.MAX_VALUE && System.currentTimeMillis() >= deadline) {
                  future.completeExceptionally(
                      new TransactionTimeoutException(
                          "Transaction " + hashHex + " did not reach a terminal status "
                              + "within the configured timeout",
                          hashHex,
                          nextAttempts,
                          payload));
                  return;
                }

                scheduleNextPoll(
                    hashHex, options, deadline, nextAttempts, payload, future);
              } catch (final Exception e) {
                if (!future.isDone()) {
                  future.completeExceptionally(e);
                }
              }
            });
  }

  private CompletableFuture<Map<String, Object>> fetchPipelineStatusSnapshot(final String hashHex) {
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    final TransportRequest request =
        ToriiRequestBuilder.buildStatusRequest(
            config.baseUri(), hashHex, config.requestTimeout(), config.defaultHeaders());
    notifyRequest(request);
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause = unwrapCompletion(throwable);
                notifyFailure(request, cause);
                future.completeExceptionally(cause);
                return;
              }

              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              notifyResponse(request, clientResponse);

              final int statusCode = clientResponse.statusCode();
              if (statusCode != 200
                  && statusCode != 202
                  && statusCode != 204
                  && statusCode != 404) {
                future.completeExceptionally(
                    buildPipelineStatusHttpException(hashHex, clientResponse));
                return;
              }

              try {
                future.complete(parsePipelineStatusPayload(clientResponse.body()));
              } catch (final Exception error) {
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private void openPipelineStatusStream(
      final String hashHex,
      final PipelineStatusOptions options,
      final long deadline,
      final AtomicInteger attempts,
      final AtomicReference<Map<String, Object>> lastPayload,
      final CompletableFuture<Map<String, Object>> future) {
    final ToriiEventStreamOptions.Builder streamOptions = ToriiEventStreamOptions.builder();
    streamOptions.putQueryParameter("filter", transactionHashFilter(hashHex));
    if (options.timeoutMillis() != null) {
      streamOptions.setTimeout(Duration.ofMillis(Math.max(0L, options.timeoutMillis())));
    }

    final AtomicReference<ToriiEventStream> streamRef = new AtomicReference<>();
    future.whenComplete(
        (ignored, throwable) -> {
          final ToriiEventStream active = streamRef.getAndSet(null);
          if (active != null) {
            active.close();
          }
        });

    if (deadline != Long.MAX_VALUE) {
      final long remainingMs = Math.max(0L, deadline - System.currentTimeMillis());
      CompletableFuture
          .runAsync(
              () -> {},
              CompletableFuture.delayedExecutor(remainingMs, TimeUnit.MILLISECONDS))
          .whenComplete(
              (ignored, throwable) -> {
                if (future.isDone()) {
                  return;
                }
                if (throwable != null) {
                  future.completeExceptionally(unwrapCompletion(throwable));
                  return;
                }
                future.completeExceptionally(
                    new TransactionTimeoutException(
                        "Transaction " + hashHex + " did not reach a terminal status "
                            + "within the configured timeout",
                        hashHex,
                        attempts.get(),
                        lastPayload.get()));
              });
    }

    final ToriiEventStream stream =
        newEventStreamClient()
            .openSseStream(
                "/v1/events/sse",
                streamOptions.build(),
                new ToriiEventStreamListener() {
                  @Override
                  public void onEvent(
                      final org.hyperledger.iroha.android.client.stream.ServerSentEvent event) {
                    if (future.isDone()) {
                      return;
                    }
                    try {
                      final Map<String, Object> payload = parsePipelineEventPayload(event.data());
                      if (!isTransactionPipelineEvent(payload)) {
                        return;
                      }
                      lastPayload.set(payload);
                      final int attempt = attempts.incrementAndGet();
                      processPipelineTerminalPayload(hashHex, options, payload, attempt, future);
                    } catch (final Throwable error) {
                      if (!future.isDone()) {
                        future.completeExceptionally(error);
                      }
                    }
                  }

                  @Override
                  public void onClosed() {
                    if (future.isDone()) {
                      return;
                    }
                    future.completeExceptionally(
                        new IOException(
                            "Torii SSE stream closed before transaction "
                                + hashHex
                                + " reached a terminal status"));
                  }

                  @Override
                  public void onError(final Throwable error) {
                    if (future.isDone()) {
                      return;
                    }
                    future.completeExceptionally(error);
                  }
                });
    streamRef.set(stream);
  }

  private boolean processPipelineTerminalPayload(
      final String hashHex,
      final PipelineStatusOptions options,
      final Map<String, Object> payload,
      final int attempt,
      final CompletableFuture<Map<String, Object>> future) {
    final Optional<String> statusKind = PipelineStatusExtractor.extractStatusKind(payload);
    final String statusLiteral = statusKind.orElse(null);
    final boolean isSuccess =
        statusLiteral != null && options.successStatuses().contains(statusLiteral);
    final boolean isFailure =
        statusLiteral != null && options.failureStatuses().contains(statusLiteral);

    if (options.observer() != null) {
      options.observer().onStatus(statusLiteral, payload, attempt);
    }

    if (isSuccess) {
      future.complete(payload != null ? payload : Map.of());
      return true;
    }
    if (isFailure) {
      final String rejectionReason =
          PipelineStatusExtractor.extractRejectionReason(payload).orElse(null);
      future.completeExceptionally(
          new TransactionStatusException(hashHex, statusLiteral, rejectionReason, payload));
      return true;
    }
    return false;
  }

  private void scheduleNextPoll(
      final String hashHex,
      final PipelineStatusOptions options,
      final long deadline,
      final int attemptsSoFar,
      final Map<String, Object> lastPayload,
      final CompletableFuture<Map<String, Object>> future) {
    if (future.isDone()) {
      return;
    }
    final long interval = options.intervalMillis();
    final Runnable task =
        () -> pollPipelineStatus(hashHex, options, deadline, attemptsSoFar, lastPayload, future);
    if (interval <= 0L) {
      task.run();
      return;
    }
    CompletableFuture
        .runAsync(
            () -> {},
            CompletableFuture.delayedExecutor(
                Math.min(interval, Long.MAX_VALUE), TimeUnit.MILLISECONDS))
        .whenComplete(
            (ignored, delayError) -> {
              if (delayError != null) {
                future.completeExceptionally(
                    delayError instanceof CompletionException ? delayError.getCause() : delayError);
              } else {
                task.run();
              }
            });
  }

  @SuppressWarnings("unchecked")
  private Map<String, Object> parsePipelineStatusPayload(final byte[] body) {
    if (body == null || body.length == 0) {
      return null;
    }
    if (body.length >= 4
        && body[0] == 'N'
        && body[1] == 'R'
        && body[2] == 'T'
        && body[3] == '0') {
      return null;
    }
    final String json = new String(body, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      return null;
    }
    final Object parsed = JsonParser.parse(json);
    if (parsed instanceof Map) {
      return (Map<String, Object>) parsed;
    }
    throw new IllegalStateException("Pipeline status response must be a JSON object");
  }

  @SuppressWarnings("unchecked")
  private Map<String, Object> parsePipelineEventPayload(final String json) {
    if (json == null || json.isBlank()) {
      throw new IllegalStateException("Pipeline event payload must be a JSON object");
    }
    final Object parsed = JsonParser.parse(json.trim());
    if (parsed instanceof Map) {
      return (Map<String, Object>) parsed;
    }
    throw new IllegalStateException("Pipeline event payload must be a JSON object");
  }

  private boolean isTransactionPipelineEvent(final Map<String, Object> payload) {
    if (payload == null) {
      return false;
    }
    final Object event = payload.get("event");
    if (event == null) {
      return true;
    }
    return "Transaction".equalsIgnoreCase(String.valueOf(event).trim());
  }

  private static String transactionHashFilter(final String hashHex) {
    return JsonEncoder.encode(
        Map.of(
            "op", "eq",
            "args", List.of("tx_hash", hashHex)));
  }

  private static TransactionStatusHttpException buildPipelineStatusHttpException(
      final String hashHex, final ClientResponse response) {
    final String bodyPreview = HttpErrorMessageExtractor.extractMessage(response.body());
    return new TransactionStatusHttpException(
        hashHex,
        response.statusCode(),
        response.rejectCode().orElse(null),
        bodyPreview);
  }

  private void notifyRequest(final TransportRequest request) {
    emitDeviceProfileTelemetry();
    emitNetworkContextTelemetry();
    for (final ClientObserver observer : config.observers()) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : config.observers()) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : config.observers()) {
      observer.onFailure(request, error);
    }
  }

  private TransportRequest buildJsonGetRequest(
      final String path, final Map<String, String> queryParams) {
    final URI target = appendQuery(resolvePath(path), queryParams);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildJsonPostRequest(final String path, final byte[] body) {
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod("POST")
            .setBody(Objects.requireNonNull(body, "body"))
            .addHeader("Content-Type", "application/json")
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildVpnRequest(
      final String method,
      final String path,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final URI target = resolvePath(path);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    if (body != null) {
      builder.setBody(body).addHeader("Content-Type", "application/json");
    }
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    for (final Map.Entry<String, String> entry :
        buildCanonicalHeaders(method, target, body, canonicalAuth).entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private static Map<String, String> buildCanonicalHeaders(
      final String method,
      final URI target,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final Long timestampMs = canonicalAuth.timestampMs();
    final String nonce = canonicalAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    if (timestampMs == null) {
      return CanonicalRequestSigner.buildHeaders(
          method, target, body, canonicalAuth);
    }
    return CanonicalRequestSigner.buildHeaders(
        method, target, body, canonicalAuth, timestampMs, nonce);
  }

  private URI resolvePath(final String path) {
    if (path == null || path.isBlank()) {
      return config.baseUri();
    }
    if (path.startsWith("http://") || path.startsWith("https://")) {
      return URI.create(path);
    }
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = config.baseUri().toString();
    final String joined = base.endsWith("/") ? base + normalized : base + "/" + normalized;
    return URI.create(joined);
  }

  private static URI appendQuery(final URI target, final Map<String, String> params) {
    if (params.isEmpty()) {
      return target;
    }
    final StringBuilder builder = new StringBuilder(target.toString());
    builder.append(target.toString().contains("?") ? "&" : "?");
    builder.append(encodeQuery(params));
    return URI.create(builder.toString());
  }

  private static String encodeQuery(final Map<String, String> params) {
    final StringBuilder builder = new StringBuilder();
    boolean first = true;
    for (final Map.Entry<String, String> entry : params.entrySet()) {
      if (!first) {
        builder.append('&');
      } else {
        first = false;
      }
      builder
          .append(urlEncode(entry.getKey()))
          .append('=')
          .append(urlEncode(entry.getValue()));
    }
    return builder.toString();
  }

  private static String encodePathSegment(final String segment) {
    final String encoded = urlEncode(segment);
    return encoded.replace("+", "%20");
  }

  private static String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.name());
    } catch (final UnsupportedEncodingException ex) {
      throw new IllegalStateException("UTF-8 not supported", ex);
    }
  }

  private static Throwable unwrapCompletion(final Throwable throwable) {
    Throwable current = throwable;
    while (current instanceof CompletionException && current.getCause() != null) {
      current = current.getCause();
    }
    return current;
  }

  private <T> CompletableFuture<T> fetchJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    notifyRequest(request);
    final CompletableFuture<T> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final RuntimeException error =
                    new RuntimeException(errorContext + " request failed", cause);
                notifyFailure(request, cause);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final RuntimeException error =
                    new RuntimeException(
                        errorContext + " request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.apply(response.body());
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (final RuntimeException ex) {
                notifyFailure(request, ex);
                future.completeExceptionally(ex);
              }
            });
    return future;
  }

  private <T> CompletableFuture<Optional<T>> fetchJsonAllowingNotFound(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    notifyRequest(request);
    final CompletableFuture<Optional<T>> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final RuntimeException error =
                    new RuntimeException(errorContext + " request failed", cause);
                notifyFailure(request, cause);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              if (response.statusCode() == 404) {
                notifyResponse(request, clientResponse);
                future.complete(Optional.empty());
                return;
              }
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final RuntimeException error =
                    new RuntimeException(
                        errorContext + " request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.apply(response.body());
                notifyResponse(request, clientResponse);
                future.complete(Optional.of(parsed));
              } catch (final RuntimeException ex) {
                notifyFailure(request, ex);
                future.completeExceptionally(ex);
              }
            });
    return future;
  }

  private <T> CompletableFuture<Optional<T>> fetchOptionalJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    notifyRequest(request);
    final CompletableFuture<Optional<T>> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final RuntimeException error =
                    new RuntimeException(errorContext + " request failed", cause);
                notifyFailure(request, cause);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final RuntimeException error =
                    new RuntimeException(
                        errorContext + " request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              if (response.body().length == 0) {
                notifyResponse(request, clientResponse);
                future.complete(Optional.empty());
                return;
              }
              try {
                final T parsed = parser.apply(response.body());
                notifyResponse(request, clientResponse);
                future.complete(Optional.of(parsed));
              } catch (final RuntimeException ex) {
                notifyFailure(request, ex);
                future.completeExceptionally(ex);
              }
            });
    return future;
  }

  private static byte[] encodeJsonBody(final Map<String, Object> payload) {
    return JsonEncoder.encode(Objects.requireNonNull(payload, "payload"))
        .getBytes(StandardCharsets.UTF_8);
  }

  static IdentifierResolveRequest buildIdentifierResolveRequest(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    final String normalizedPolicyId = normalizeNonBlank(policyId, "policyId");
    final String normalizedEncryptedInput =
        normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    return IdentifierResolveRequest.encrypted(
        normalizedPolicyId, normalizedEncryptedInput, outputOpening);
  }

  static RamLfeExecuteRequest buildRamLfeExecuteRequest(final String encryptedInputHex) {
    final String normalizedEncryptedInput =
        normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    return RamLfeExecuteRequest.encrypted(normalizedEncryptedInput);
  }

  static Map<String, Object> buildIdentifierResolvePayload(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening) {
    final String normalizedPolicyId = normalizeNonBlank(policyId, "policyId");
    final String normalizedEncryptedInput =
        normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("policy_id", normalizedPolicyId);
    payload.put("encrypted_input", normalizedEncryptedInput);
    payload.put("output_opening", Objects.requireNonNull(outputOpening, "outputOpening").toJsonMap());
    return payload;
  }

  static Map<String, Object> buildRamLfeExecutePayload(final String encryptedInputHex) {
    final String normalizedEncryptedInput =
        normalizeEvenLengthHex(encryptedInputHex, "encryptedInputHex");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("encrypted_input", normalizedEncryptedInput);
    return payload;
  }

  static Map<String, Object> buildRamLfeReceiptVerifyPayload(
      final Map<String, Object> receipt, final String outputHex) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("receipt", new LinkedHashMap<>(Objects.requireNonNull(receipt, "receipt")));
    if (outputHex != null) {
      payload.put("output_hex", normalizeEvenLengthHex(outputHex, "outputHex"));
    }
    return payload;
  }

  static Map<String, Object> buildVpnQuoteCreatePayload(
      final String exitClass,
      final String meteringPublicKeyHex) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    final String normalizedExitClass = normalizeOptionalNonBlank(exitClass, "exitClass");
    payload.put("exit_class", normalizedExitClass == null ? "" : normalizedExitClass);
    payload.put("metering_public_key_hex", normalizeHex32(meteringPublicKeyHex, "meteringPublicKeyHex"));
    return payload;
  }

  static Map<String, Object> buildVpnSessionCreatePayload(
      final String exitClass,
      final String quoteId,
      final String paymentTxHash,
      final String meteringPublicKeyHex) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    final String normalizedExitClass = normalizeOptionalNonBlank(exitClass, "exitClass");
    payload.put("exit_class", normalizedExitClass == null ? "" : normalizedExitClass);
    payload.put("quote_id", normalizeHex32(quoteId, "quoteId"));
    payload.put("payment_tx_hash", normalizeHex32(paymentTxHash, "paymentTxHash"));
    payload.put("metering_public_key_hex", normalizeHex32(meteringPublicKeyHex, "meteringPublicKeyHex"));
    return payload;
  }

  static Map<String, Object> buildVpnReceiptSubmitPayload(
      final String relayReceiptHex,
      final String clientVoucherHex,
      final String leaseIdHex) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("relay_receipt_hex", normalizeEvenLengthHex(relayReceiptHex, "relayReceiptHex"));
    payload.put("client_voucher_hex", normalizeEvenLengthHex(clientVoucherHex, "clientVoucherHex"));
    if (leaseIdHex != null) {
      payload.put("lease_id_hex", normalizeHex32(leaseIdHex, "leaseIdHex"));
    }
    return payload;
  }

  static Map<String, Object> buildDeployContractPayload(
      final String authority,
      final String privateKey,
      final String codeB64,
      final String contractAlias,
      final Long leaseExpiryMs) {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("authority", normalizeNonBlank(authority, "authority"));
    payload.put("private_key", normalizeNonBlank(privateKey, "privateKey"));
    payload.put("code_b64", normalizeRequiredBase64Payload(codeB64, "codeB64"));
    payload.put("contract_alias", normalizeNonBlank(contractAlias, "contractAlias"));
    if (leaseExpiryMs != null) {
      if (leaseExpiryMs.longValue() < 0L) {
        throw new IllegalArgumentException("leaseExpiryMs must be non-negative");
      }
      payload.put("lease_expiry_ms", leaseExpiryMs);
    }
    return payload;
  }

  static Map<String, Object> buildContractCallPayload(
      final String authority,
      final String privateKey,
      final long gasLimit,
      final String contractAddress,
      final String contractAlias,
      final String entrypoint,
      final Object payloadValue,
      final String gasAssetId) {
    if (gasLimit < 0L) {
      throw new IllegalArgumentException("gasLimit must be non-negative");
    }
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("authority", normalizeNonBlank(authority, "authority"));
    payload.put("private_key", normalizeNonBlank(privateKey, "privateKey"));
    payload.putAll(buildContractTargetSelector(contractAddress, contractAlias));
    if (entrypoint != null) {
      payload.put("entrypoint", normalizeNonBlank(entrypoint, "entrypoint"));
    }
    if (payloadValue != null) {
      payload.put("payload", payloadValue);
    }
    if (gasAssetId != null) {
      payload.put("gas_asset_id", normalizeNonBlank(gasAssetId, "gasAssetId"));
    }
    payload.put("gas_limit", gasLimit);
    return payload;
  }

  static Map<String, Object> buildMultisigProposePayload(
      final MultisigProposeRequest request) {
    Objects.requireNonNull(request, "request");
    final boolean hasAccountId = request.multisigAccountId() != null;
    final boolean hasAlias = request.multisigAccountAlias() != null;
    if (hasAccountId == hasAlias) {
      throw new IllegalArgumentException(
          "Exactly one of multisigAccountId or multisigAccountAlias must be provided");
    }
    if (request.instructions().isEmpty()) {
      throw new IllegalArgumentException("instructions must not be empty");
    }

    final Map<String, Object> payload = new LinkedHashMap<>();
    if (hasAccountId) {
      payload.put(
          "multisig_account_id",
          normalizeNonBlank(request.multisigAccountId(), "multisigAccountId"));
    } else {
      payload.put(
          "multisig_account_alias",
          normalizeNonBlank(request.multisigAccountAlias(), "multisigAccountAlias"));
    }
    payload.put("signer_account_id", normalizeNonBlank(request.signerAccountId(), "signerAccountId"));
    if (request.publicKeyHex() != null) {
      payload.put("public_key_hex", normalizeHex32(request.publicKeyHex(), "publicKeyHex"));
    }
    if (request.signatureB64() != null) {
      payload.put(
          "signature_b64",
          normalizeRequiredBase64Payload(request.signatureB64(), "signatureB64"));
    }
    if (request.creationTimeMs() != null) {
      if (request.creationTimeMs().longValue() < 0L) {
        throw new IllegalArgumentException("creationTimeMs must be non-negative");
      }
      payload.put("creation_time_ms", request.creationTimeMs());
    }
    if (request.feeSponsor() != null) {
      payload.put("fee_sponsor", normalizeNonBlank(request.feeSponsor(), "feeSponsor"));
    }
    final List<String> instructions = new ArrayList<>();
    int index = 0;
    for (final byte[] instruction : request.instructions()) {
      if (instruction == null || instruction.length == 0) {
        throw new IllegalArgumentException("instructions[" + index + "] must not be empty");
      }
      instructions.add(Base64.getEncoder().encodeToString(instruction));
      index++;
    }
    payload.put("instructions", instructions);
    return payload;
  }

  static Map<String, String> buildContractTargetSelector(
      final String contractAddress, final String contractAlias) {
    final boolean hasContractAddress = contractAddress != null;
    final boolean hasContractAlias = contractAlias != null;
    if (hasContractAddress == hasContractAlias) {
      throw new IllegalArgumentException(
          "Exactly one of contractAddress or contractAlias must be provided");
    }
    final Map<String, String> selector = new LinkedHashMap<>();
    if (hasContractAddress) {
      selector.put(
          "contract_address",
          normalizeNonBlank(contractAddress, "contractAddress"));
      return selector;
    }
    selector.put(
        "contract_alias",
        normalizeNonBlank(contractAlias, "contractAlias"));
    return selector;
  }

  static String normalizeRequiredBase64Payload(final String value, final String field) {
    final String normalized = normalizeNonBlank(value, field);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(normalized);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(field + " must not decode to empty bytes");
    }
    return normalized;
  }

  static String normalizeOptionalNonBlank(final String value, final String field) {
    return value == null ? null : normalizeNonBlank(value, field);
  }

  static String normalizeNonBlank(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return trimmed;
  }

  static String normalizeEvenLengthHex(final String value, final String field) {
    String trimmed = normalizeNonBlank(value, field);
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    if ((trimmed.length() & 1) != 0 || trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must be an even-length hex string");
    }
    for (int i = 0; i < trimmed.length(); i++) {
      final char c = trimmed.charAt(i);
      final boolean isHex =
          (c >= '0' && c <= '9')
              || (c >= 'a' && c <= 'f')
              || (c >= 'A' && c <= 'F');
      if (!isHex) {
        throw new IllegalArgumentException(field + " must be an even-length hex string");
      }
    }
    return trimmed.toLowerCase();
  }

  static String normalizeHex32(final String value, final String field) {
    final String normalized = normalizeEvenLengthHex(value, field);
    if (normalized.length() != 64) {
      throw new IllegalArgumentException(field + " must contain 64 hex characters");
    }
    return normalized;
  }
}
