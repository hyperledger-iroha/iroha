package org.hyperledger.iroha.android.client;

import java.io.UnsupportedEncodingException;
import java.lang.ref.WeakReference;
import java.math.BigInteger;
import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.BiFunction;
import java.util.function.Function;
import org.hyperledger.iroha.android.alias.AccountAliasName;
import org.hyperledger.iroha.android.alias.AliasSetupPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasAutoRenewPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLeaseRenewPlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLifecyclePlanRequestV1;
import org.hyperledger.iroha.android.alias.AliasLifecycleTransactionPlanJsonParser;
import org.hyperledger.iroha.android.alias.AliasLifecycleTransactionPlanV1;
import org.hyperledger.iroha.android.alias.AliasSetupModels;
import org.hyperledger.iroha.android.alias.AccountOnboardingApplyRequestV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingJsonParser;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanReceiptV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingPlanRequestV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingReceiptVerifier;
import org.hyperledger.iroha.android.alias.AccountOnboardingResponseV1;
import org.hyperledger.iroha.android.alias.AccountOnboardingResponseVerifier;
import org.hyperledger.iroha.android.alias.AliasTransactionPlanJsonParser;
import org.hyperledger.iroha.android.alias.AliasTransactionPlanV1;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.SumeragiDiagnosticsStatus;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.SumeragiV2Status;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.android.nexus.UaidBindingsQuery;
import org.hyperledger.iroha.android.nexus.UaidBindingsResponse;
import org.hyperledger.iroha.android.nexus.UaidJsonParser;
import org.hyperledger.iroha.android.nexus.UaidLiteral;
import org.hyperledger.iroha.android.nexus.UaidManifestQuery;
import org.hyperledger.iroha.android.nexus.UaidManifestsResponse;
import org.hyperledger.iroha.android.nexus.UaidPortfolioQuery;
import org.hyperledger.iroha.android.nexus.UaidPortfolioResponse;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.FeeSponsorProgramId;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchSummary;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.privacy.PrivacyNativeBridge;
import org.hyperledger.iroha.android.privacy.PrivacyExact12ActionInspectionV1;
import org.hyperledger.iroha.android.privacy.PrivacyProtocolIdV1;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityAdmissionV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityManifestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12CapabilityTupleAdmissionV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyActionLocalStateV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyActionOperationViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyActionTerminalChainStateV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12ActionContractV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12ActionRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyFinalizedStateViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAceReplayNullifierRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAceReplayNullifierProvenanceV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProofManagedPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProofManagedPoolStateViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardPoolStateViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardNullifierRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOrchardNullifierProvenanceV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyAnonymousPgcPoolStateRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyAnonymousPgcPoolStateViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsAdmissionRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsAdmissionViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsProvisionRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkAmsProvisionViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkX509CertificateNullifierRequestV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyZkX509CertificateNullifierProvenanceV1;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
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
  private static final String ONBOARDING_TOKEN_HEADER = "X-Iroha-Onboarding-Token";

  private static final String PIPELINE_STATUS_SIGNAL = "android.torii.pipeline.status";
  private static final String REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure";
  private static final long U32_MAX = 4_294_967_295L;
  private static final String TRON_BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  private static final String ENTRYPOINT_HASH_HEADER = "x-iroha-entrypoint-hash";

  private final HttpTransportExecutor executor;
  private final ClientConfig config;
  private volatile SorafsGatewayClient sorafsGatewayClient;
  private final AtomicBoolean deviceProfileEmitted = new AtomicBoolean(false);
  private final Object privacyActionProvenanceLockV1 = new Object();
  private final List<WeakReference<PrivacyActionOperationViewV1>>
      authenticatedPrivacyActionViewsV1 = new ArrayList<>();

  public HttpClientTransport(final HttpTransportExecutor executor, final ClientConfig config) {
    this.executor = Objects.requireNonNull(executor, "executor");
    this.config = Objects.requireNonNull(config, "config");
  }

  @Override
  public CompletableFuture<ClientResponse> submitTransaction(final SignedTransaction transaction) {
    Objects.requireNonNull(transaction, "transaction");
    final String hashHex = SignedTransactionHasher.hashHex(transaction);
    return submitOnce(transaction, hashHex);
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
    return ensureTransactionSubmissionCompatibility()
        .thenCompose(
            ignored -> executeAccepted(request, "transaction JSON submit", 202));
  }

  @Override
  public CompletableFuture<ClientResponse> submitSccpDestinationProof(
      final SccpDestinationProofSubmitRequest request) {
    Objects.requireNonNull(request, "request");
    return executeSccpJsonAccepted(
        buildBridgeJsonPostRequest("/v1/bridge/proofs/submit", request.toJsonBytes()),
        "SCCP destination proof submit");
  }

  @Override
  public CompletableFuture<ClientResponse> submitSccpNativeMessage(
      final SccpNativeMessageSubmitRequest request) {
    Objects.requireNonNull(request, "request");
    return executeSccpJsonAccepted(
        buildBridgeJsonPostRequest("/v1/bridge/messages", request.toJsonBytes()),
        "SCCP native message submit");
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
    return ensureTransactionSubmissionCompatibility()
        .thenCompose(
            ignored -> {
              notifyRequest(request);
              return executor
                  .execute(request)
                  .handle(
                      (response, throwable) -> {
                        if (throwable != null) {
                          final Throwable cause = unwrapCompletion(throwable);
                          notifyFailure(request, cause);
                          final CompletableFuture<ClientResponse> failed =
                              new CompletableFuture<>();
                          failed.completeExceptionally(cause);
                          return failed;
                        }
                        final ClientResponse clientResponse =
                            new ClientResponse(
                                response.statusCode(),
                                response.body(),
                                response.message(),
                                extractEntrypointHash(response).orElse(null),
                                extractRejectCode(response));
                        if (clientResponse.statusCode() < 200
                            || clientResponse.statusCode() >= 300) {
                          notifyFailure(
                              request,
                              new RuntimeException(
                                  "Torii request failed with status "
                                      + clientResponse.statusCode()));
                        } else {
                          notifyResponse(request, clientResponse);
                        }
                        return CompletableFuture.completedFuture(clientResponse);
                      })
                  .thenCompose(future -> future);
            });
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
    return ensureTransactionSubmissionCompatibility()
        .thenCompose(
            ignored ->
                executeAccepted(
                    request, "transaction entrypoint JSON submit", 202));
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

  private CompletableFuture<ClientResponse> executeSccpJsonAccepted(
      final TransportRequest request, final String errorContext) {
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
              try {
                requireExactSccpJsonResponse(response, errorContext);
                notifyResponse(request, clientResponse);
                future.complete(clientResponse);
              } catch (final RuntimeException ex) {
                notifyFailure(request, ex);
                future.completeExceptionally(ex);
              }
            });
    return future;
  }

  @Override
  public CompletableFuture<Map<String, Object>> waitForTransactionStatus(
      final String hashHex, final PipelineStatusOptions options) {
    Objects.requireNonNull(hashHex, "hashHex");
    final PipelineStatusOptions resolved = PipelineStatusOptions.resolve(options);
    final long deadline = saturatedDeadline(resolved.timeoutMillis());
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    pollPipelineStatus(hashHex, resolved, deadline, 0, null, future);
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

  /** Creates a typed DA proof client that reuses this transport's configuration. */
  public DaToriiClient newDaToriiClient() {
    return DaToriiClient.builder()
        .setExecutor(executor)
        .setBaseUri(config.baseUri())
        .setTimeout(config.requestTimeout())
        .setDefaultHeaders(config.defaultHeaders())
        .setObservers(config.observers())
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
    SorafsGatewayClient client = sorafsGatewayClient;
    if (client == null) {
      synchronized (this) {
        client = sorafsGatewayClient;
        if (client == null) {
          client = newSorafsGatewayClient(config.sorafsGatewayUri());
          sorafsGatewayClient = client;
        }
      }
    }
    return client;
  }

  /** Post a gateway fetch request and return the raw response. */
  public CompletableFuture<ClientResponse> sorafsGatewayFetch(final GatewayFetchRequest request) {
    return sorafsGatewayClient().fetch(request);
  }

  /** Post a gateway fetch request and parse the response summary. */
  public CompletableFuture<GatewayFetchSummary> sorafsGatewayFetchSummary(
      final GatewayFetchRequest request) {
    return sorafsGatewayClient().fetchSummary(request);
  }

  /** Fetches `/v1/accounts/{uaid}/portfolio`. */
  public CompletableFuture<UaidPortfolioResponse> getUaidPortfolio(final String uaid) {
    return getUaidPortfolio(uaid, null);
  }

  /** Fetches `/v1/accounts/{uaid}/portfolio` with optional query parameters. */
  public CompletableFuture<UaidPortfolioResponse> getUaidPortfolio(
      final String uaid, final UaidPortfolioQuery query) {
    final String canonical = UaidLiteral.canonicalize(uaid, "uaid portfolio");
    final Map<String, String> params =
        query == null ? Collections.emptyMap() : query.toQueryParameters();
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
        query == null ? Collections.emptyMap() : query.toQueryParameters();
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
        query == null ? Collections.emptyMap() : query.toQueryParameters();
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/space-directory/uaids/" + encodePathSegment(canonical) + "/manifests", params);
    return fetchJson(request, UaidJsonParser::parseManifests, "UAID manifests");
  }

  /** Fetches globally registered identifier policies from `/v1/identifier-policies`. */
  public CompletableFuture<IdentifierPolicyListResponse> listIdentifierPolicies() {
    final TransportRequest request =
        buildJsonGetRequest("/v1/identifier-policies", Collections.emptyMap());
    return fetchJson(request, IdentifierJsonParser::parsePolicyList, "identifier policy list");
  }

  /** Fetches globally registered RAM-LFE program policies from `/v1/ram-lfe/program-policies`. */
  public CompletableFuture<RamLfeProgramPolicyListResponse> listRamLfeProgramPolicies() {
    final TransportRequest request =
        buildJsonGetRequest("/v1/ram-lfe/program-policies", Collections.emptyMap());
    return fetchJson(request, RamLfeJsonParser::parsePolicyList, "ram-lfe program policy list");
  }

  /** Fetches the authoritative protocol-v4 Sumeragi status snapshot. */
  @Override
  public CompletableFuture<SumeragiV2Status> getSumeragiStatus() {
    return fetchExactJson(
        buildExactOperatorJsonGetRequest(
            "/v1/sumeragi/status", SumeragiStatusModels.STATUS_JSON_MAX_BYTES),
        SumeragiStatusModels::parseStatus,
        "Sumeragi status");
  }

  /** Fetches operational Sumeragi evidence from its separate diagnostics route. */
  @Override
  public CompletableFuture<SumeragiDiagnosticsStatus> getSumeragiDiagnostics() {
    return fetchExactJson(
        buildExactOperatorJsonGetRequest(
            "/v1/sumeragi/diagnostics", SumeragiStatusModels.DIAGNOSTICS_JSON_MAX_BYTES),
        SumeragiDiagnosticsModels::parseDiagnostics,
        "Sumeragi diagnostics");
  }

  /** Fetch the exact result-bearing {@code SignedBlockWire} committed at {@code height}. */
  public CompletableFuture<byte[]> getLedgerExecutedBlockWire(final BigInteger height) {
    if (height == null || height.signum() <= 0 || height.bitLength() > 64) {
      throw new IllegalArgumentException("height must be a positive u64");
    }
    final TransportRequest request =
        buildExactNoritoGetRequest(
            "/v1/ledger/block/" + height.toString(), EXECUTED_BLOCK_WIRE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "executed block wire");
  }

  /** Convenience overload for positive signed heights. */
  public CompletableFuture<byte[]> getLedgerExecutedBlockWire(final long height) {
    return getLedgerExecutedBlockWire(BigInteger.valueOf(height));
  }

  /** Fetch the exact self-contained bridge finality proof for {@code height}. */
  public CompletableFuture<byte[]> getBridgeFinalityProofV1(final BigInteger height) {
    if (height == null || height.signum() <= 0 || height.bitLength() > 64) {
      throw new IllegalArgumentException("height must be a positive u64");
    }
    final TransportRequest request =
        buildExactNoritoGetRequest(
            "/v1/bridge/finality/" + height.toString(),
            AuthenticatedTransactionDetailsNativeBridge.FINALITY_PROOF_MAX_BYTES);
    return fetchExactNoritoBytes(request, "bridge finality proof");
  }

  /** Convenience overload for positive signed heights. */
  public CompletableFuture<byte[]> getBridgeFinalityProofV1(final long height) {
    return getBridgeFinalityProofV1(BigInteger.valueOf(height));
  }

  /**
   * Submit one fresh signed {@code FindTransactions} query and return its native-verified terminal
   * rejection.
   *
   * <p>The request is one-shot and is never retried by the SDK. {@code signer} receives only the
   * native 32-byte query digest; a KeyMint/Keystore implementation need not export private bytes.
   */
  public CompletableFuture<AuthenticatedCommittedRejectionV1>
      getAuthenticatedCommittedRejectionV1(
          final String transactionHashHex,
          final NetworkId networkId,
          final String authorityAccountId,
          final IrohaQuerySignatureProvider signer) {
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedRejectedTransactionQueryV1(
            transactionHashHex,
            Objects.requireNonNull(networkId, "networkId"),
            authorityAccountId,
            Objects.requireNonNull(signer, "signer"));
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "authenticated committed transaction rejection")
        .thenApply(
            response ->
                AuthenticatedTransactionDetailsNativeBridge.projectCommittedRejectionV1(
                    signedQuery, response));
  }

  /** Query with one account while pinning a separately trusted transaction authority. */
  public CompletableFuture<AuthenticatedCommittedRejectionV2>
      getAuthenticatedCommittedRejectionV2(
          final String transactionHashHex,
          final NetworkId networkId,
          final String queryAuthorityAccountId,
          final String expectedTransactionAuthorityAccountId,
          final IrohaQuerySignatureProvider signer) {
    return getAuthenticatedCommittedRejectionV2Internal(
        transactionHashHex,
        networkId,
        queryAuthorityAccountId,
        expectedTransactionAuthorityAccountId,
        signer,
        null,
        null,
        null);
  }

  /**
   * Authority-split lookup which additionally binds the signed transaction to one exact retained
   * Kagemusha request. HTTP acceptance and public operation status are never rejection evidence.
   */
  public CompletableFuture<AuthenticatedCommittedRejectionV2>
      getAuthenticatedKagemushaCommittedRejectionV2(
          final String transactionHashHex,
          final NetworkId networkId,
          final String queryAuthorityAccountId,
          final String expectedTransactionAuthorityAccountId,
          final IrohaQuerySignatureProvider signer,
          final byte[] expectedOperationId,
          final String expectedKind,
          final byte[] expectedRequestNorito) {
    return getAuthenticatedCommittedRejectionV2Internal(
        transactionHashHex,
        networkId,
        queryAuthorityAccountId,
        expectedTransactionAuthorityAccountId,
        signer,
        Objects.requireNonNull(expectedOperationId, "expectedOperationId").clone(),
        Objects.requireNonNull(expectedKind, "expectedKind"),
        Objects.requireNonNull(expectedRequestNorito, "expectedRequestNorito").clone());
  }

  private CompletableFuture<AuthenticatedCommittedRejectionV2>
      getAuthenticatedCommittedRejectionV2Internal(
          final String transactionHashHex,
          final NetworkId networkId,
          final String queryAuthorityAccountId,
          final String expectedTransactionAuthorityAccountId,
          final IrohaQuerySignatureProvider signer,
          final byte[] expectedOperationId,
          final String expectedKind,
          final byte[] expectedRequestNorito) {
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV2(
            transactionHashHex,
            Objects.requireNonNull(networkId, "networkId"),
            queryAuthorityAccountId,
            expectedTransactionAuthorityAccountId,
            Objects.requireNonNull(signer, "signer"));
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "authenticated committed transaction rejection V2")
        .thenApply(
            response ->
                expectedOperationId == null
                    ? AuthenticatedTransactionDetailsNativeBridge.projectCommittedRejectionV2(
                        signedQuery, response)
                    : AuthenticatedTransactionDetailsNativeBridge
                        .projectKagemushaCommittedRejectionV2(
                            signedQuery,
                            response,
                            expectedOperationId,
                            expectedKind,
                            expectedRequestNorito));
  }

  /** Fetch the exact committed Exact12 manifest with one-shot canonical account authentication. */
  public CompletableFuture<PrivacyExact12CapabilityManifestV1> getPrivacyCapabilities(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return fetchExactNoritoBytes(
            buildExactNoritoGetRequest(
                "/v1/privacy/capabilities",
                (long) PrivacyExact12CapabilityManifestV1.MAX_ARCHIVE_BYTES,
                Objects.requireNonNull(canonicalAuth, "canonicalAuth")),
            "privacy capabilities")
        .thenApply(PrivacyNativeBridge::decodeExact12CapabilityManifestV1);
  }

  /**
   * Authenticates, fresh-gates, and submits one member of the closed 13-operation Exact12 union.
   *
   * <p>Native inspection verifies the exact signed transaction before any network call. HTTP 202
   * is dispatch admission only; when {@code wait} is true, the returned terminal view is resolved
   * through authenticated global status and committed transaction-details queries; Applied
   * additionally requires the native-verified finalized ID105 execution receipt.
   */
  public CompletableFuture<PrivacyActionOperationViewV1> submitSignedPrivacyActionV1(
      final PrivacyExact12ActionRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final boolean wait,
      final PipelineStatusOptions options) {
    Objects.requireNonNull(request, "request");
    requireAutomaticCanonicalFreshness(canonicalAuth, "Exact12 action submission");
    final NetworkId networkId = config.requireLocalSigningContext().networkId();
    final PrivacyExact12ActionInspectionV1 inspection =
        PrivacyNativeBridge.inspectSignedExact12ActionV1(
            request, networkId, canonicalAuth.accountId());
    final String transactionHashHex = lowerHex(inspection.transactionHash());

    return getPrivacyCapabilities(canonicalAuth)
        .thenCompose(
            manifest -> {
              final byte[] manifestDigest = manifest.manifestDigest.bytes();
              final byte[] expectedDigest = request.expectedManifestDigestBytes();
              if (expectedDigest != null
                  && !MessageDigest.isEqual(expectedDigest, manifestDigest)) {
                throw new IllegalStateException(
                    "fresh Exact12 capability manifest does not match expectedManifestDigest");
              }
              final org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1 protocolId =
                  PrivacyExact12ActionContractV1.protocolId(request.operation);
              final PrivacyExact12CapabilityTupleAdmissionV1 admission =
                  PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
                      manifest, protocolId);
              if (!admission.operationSchemas.equals(manifest.rowFor(protocolId).operationSchemas)
                  || !admission.operationSchemas.contains(request.operation)) {
                throw new IllegalStateException(
                    "Exact12 capability admission does not contain the requested operation schema");
              }
              if (!MessageDigest.isEqual(admission.manifestDigest.bytes(), manifestDigest)) {
                throw new IllegalStateException(
                    "Exact12 capability admission does not bind the fresh manifest digest");
              }
              if (admission.committedHeight.signum() <= 0) {
                throw new IllegalStateException(
                    "Exact12 capability admission must be committed at a positive height");
              }
              final PrivacyActionOperationViewV1 submitted =
                  new PrivacyActionOperationViewV1(
                      protocolId,
                      request.operation,
                      inspection.transactionHash(),
                      inspection.transactionIntentDigest(),
                      inspection.statementDigest(),
                      inspection.proofEnvelopeHash(),
                      PrivacyActionLocalStateV1.SUBMITTED,
                      null,
                      null,
                      null,
                      PrivacyExact12ActionContractV1.ledgerEffectKind(request.operation),
                      manifestDigest,
                      admission.committedHeight);
              return submitExactPrivacyActionWire(
                      request.signedTransactionVersionedBytes(),
                      canonicalAuth,
                      transactionHashHex)
                  .thenCompose(
                      ignored -> {
                        bindAuthenticatedPrivacyActionViewV1(submitted);
                        return wait
                            ? waitForPrivacyActionTerminalV1(
                                submitted, canonicalAuth, options)
                            : CompletableFuture.completedFuture(submitted);
                      });
            });
  }

  /** Convenience overload using the standard bounded terminal wait. */
  public CompletableFuture<PrivacyActionOperationViewV1> submitSignedPrivacyActionV1(
      final PrivacyExact12ActionRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return submitSignedPrivacyActionV1(request, canonicalAuth, true, null);
  }

  /** Convenience overload selecting whether to wait with the standard bounded options. */
  public CompletableFuture<PrivacyActionOperationViewV1> submitSignedPrivacyActionV1(
      final PrivacyExact12ActionRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final boolean wait) {
    return submitSignedPrivacyActionV1(request, canonicalAuth, wait, null);
  }

  /** Refreshes one typed Exact12 action without allowing terminal regression or reason elision. */
  public CompletableFuture<PrivacyActionOperationViewV1> getPrivacyActionStatusV1(
      final PrivacyActionOperationViewV1 operation,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return fetchPrivacyActionStatusSnapshotV1(operation, canonicalAuth)
        .thenApply(snapshot -> snapshot.view);
  }

  /** Returns native-verified committed success or rejection for one exact transaction hash. */
  public CompletableFuture<AuthenticatedCommittedTransactionResultV1>
      getAuthenticatedCommittedTransactionResultV1(
          final String transactionHashHex,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV1(
            transactionHashHex,
            config.requireLocalSigningContext().networkId(),
            canonicalAuth.accountId(),
            canonicalAuth::sign);
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "authenticated committed transaction result")
        .thenApply(
            response ->
                AuthenticatedTransactionDetailsNativeBridge.projectCommittedTransactionResultV1(
                    signedQuery, response));
  }

  /** Returns a committed result with independent query and transaction-authority bindings. */
  public CompletableFuture<AuthenticatedCommittedTransactionResultV2>
      getAuthenticatedCommittedTransactionResultV2(
          final String transactionHashHex,
          final NetworkId networkId,
          final String queryAuthorityAccountId,
          final String expectedTransactionAuthorityAccountId,
          final IrohaQuerySignatureProvider signer) {
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV2(
            transactionHashHex,
            Objects.requireNonNull(networkId, "networkId"),
            queryAuthorityAccountId,
            expectedTransactionAuthorityAccountId,
            Objects.requireNonNull(signer, "signer"));
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "authenticated committed transaction result V2")
        .thenApply(
            response ->
                AuthenticatedTransactionDetailsNativeBridge.projectCommittedTransactionResultV2(
                    signedQuery, response));
  }

  /**
   * Return the exact authenticated transaction-details carrier, or {@code null} only for HTTP 404.
   * Its height/result fields remain routing hints until finalized outcome projection succeeds.
   */
  public CompletableFuture<AuthenticatedTransactionDetailsCarrierV2>
      getAuthenticatedTransactionDetailsCarrierV2(
          final String transactionHashHex,
          final NetworkId networkId,
          final String queryAuthorityAccountId,
          final String expectedTransactionAuthorityAccountId,
          final IrohaQuerySignatureProvider signer) {
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV2 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV2(
            transactionHashHex,
            Objects.requireNonNull(networkId, "networkId"),
            queryAuthorityAccountId,
            expectedTransactionAuthorityAccountId,
            Objects.requireNonNull(signer, "signer"));
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytesAllowingNotFound(
            request, "authenticated transaction-details carrier V2")
        .thenApply(
            response ->
                response == null
                    ? null
                    : AuthenticatedTransactionDetailsNativeBridge
                        .bindTransactionDetailsCarrierV2(signedQuery, response));
  }

  /**
   * Returns the native-verified committed result for exactly one offline-device registration.
   *
   * <p>The authenticated lookup is identical to the generic transaction-details flow, while the
   * native projector additionally requires exactly one {@code RegisterOfflineDeviceAttestation}
   * instruction and preserves a closed typed eligibility rejection.
   */
  public CompletableFuture<AuthenticatedOfflineDeviceRegistrationResultV1>
      getAuthenticatedOfflineDeviceRegistrationResultV1(
          final String transactionHashHex,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV1(
            transactionHashHex,
            config.requireLocalSigningContext().networkId(),
            canonicalAuth.accountId(),
            canonicalAuth::sign);
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytes(request, "authenticated offline-device registration result")
        .thenApply(
            response ->
                AuthenticatedTransactionDetailsNativeBridge
                    .projectCommittedOfflineDeviceRegistrationResultV1(signedQuery, response));
  }

  /**
   * Returns the native-verified finalized ID105 receipt for one authenticated Exact12 action.
   * Only an exact HTTP 404 means that the typed receipt is not available yet.
   */
  public CompletableFuture<
          AuthenticatedPrivacyActionReceiptNativeBridge.AuthenticatedActionExecutionReceiptV1>
      getAuthenticatedPrivacyActionExecutionReceiptV1(
          final PrivacyActionOperationViewV1 operation,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(operation, "operation");
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    requireAuthenticatedPrivacyActionViewV1(operation);
    final AuthenticatedPrivacyActionReceiptNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedPrivacyActionReceiptNativeBridge.buildSignedPrivacyActionReceiptQueryV1(
            operation,
            config.requireLocalSigningContext().networkId(),
            canonicalAuth.accountId(),
            canonicalAuth::sign);
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/query",
            signedQuery.requestBody(),
            AuthenticatedPrivacyActionReceiptNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytesAllowingNotFound(
            request, "authenticated Exact12 action receipt")
        .thenApply(
            response ->
                response == null
                    ? null
                    : AuthenticatedPrivacyActionReceiptNativeBridge
                        .projectPrivacyActionReceiptV1(signedQuery, response));
  }

  /** Returns finalized ZK-ACE replay provenance, or null only for an exact HTTP 404. */
  public CompletableFuture<PrivacyZkAceReplayNullifierProvenanceV1>
      getPrivacyZkAceReplayNullifierV1(
          final PrivacyZkAceReplayNullifierRequestV1 request,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request, canonicalAuth, PrivacyZkAceReplayNullifierProvenanceV1.class);
  }

  /** Returns finalized FCMP++, private-IVM, or PQ-MASP pool state, or null only on 404. */
  public CompletableFuture<PrivacyProofManagedPoolStateViewV1>
      getPrivacyProofManagedPoolStateV1(
          final PrivacyProofManagedPoolStateRequestV1 request,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request, canonicalAuth, PrivacyProofManagedPoolStateViewV1.class);
  }

  /** Returns finalized governed Orchard pool state, or null only on 404. */
  public CompletableFuture<PrivacyOrchardPoolStateViewV1> getPrivacyOrchardPoolStateV1(
      final PrivacyOrchardPoolStateRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request, canonicalAuth, PrivacyOrchardPoolStateViewV1.class);
  }

  /** Returns finalized consumed Orchard-nullifier provenance, or null only on 404. */
  public CompletableFuture<PrivacyOrchardNullifierProvenanceV1> getPrivacyOrchardNullifierV1(
      final PrivacyOrchardNullifierRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request, canonicalAuth, PrivacyOrchardNullifierProvenanceV1.class);
  }

  /** Returns finalized Anonymous PGC pool state, or null only on 404. */
  public CompletableFuture<PrivacyAnonymousPgcPoolStateViewV1>
      getPrivacyAnonymousPgcPoolStateV1(
          final PrivacyAnonymousPgcPoolStateRequestV1 request,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request, canonicalAuth, PrivacyAnonymousPgcPoolStateViewV1.class);
  }

  /** Returns finalized ZK-AMS admission provenance, or null only on 404. */
  public CompletableFuture<PrivacyZkAmsAdmissionViewV1> getPrivacyZkAmsAdmissionV1(
      final PrivacyZkAmsAdmissionRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(request, canonicalAuth, PrivacyZkAmsAdmissionViewV1.class);
  }

  /** Returns finalized ZK-AMS provisioning provenance, or null only on 404. */
  public CompletableFuture<PrivacyZkAmsProvisionViewV1> getPrivacyZkAmsProvisionV1(
      final PrivacyZkAmsProvisionRequestV1 request,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(request, canonicalAuth, PrivacyZkAmsProvisionViewV1.class);
  }

  /** Returns finalized ZK-X509 certificate-nullifier provenance, or null only on 404. */
  public CompletableFuture<PrivacyZkX509CertificateNullifierProvenanceV1>
      getPrivacyZkX509CertificateNullifierV1(
          final PrivacyZkX509CertificateNullifierRequestV1 request,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyFinalizedStateV1(
        request,
        canonicalAuth,
        PrivacyZkX509CertificateNullifierProvenanceV1.class);
  }

  private <T extends PrivacyFinalizedStateViewV1> CompletableFuture<T>
      getPrivacyFinalizedStateV1(
          final PrivacyFinalizedStateRequestV1 request,
          final ToriiCanonicalRequestAuth canonicalAuth,
          final Class<T> expectedType) {
    final PrivacyFinalizedStateRequestV1 exactRequest =
        Objects.requireNonNull(request, "request");
    final ToriiCanonicalRequestAuth exactAuth =
        Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final AuthenticatedPrivacyStateQueryNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedPrivacyStateQueryNativeBridge.buildSignedPrivacyStateQueryV1(
            exactRequest,
            config.requireLocalSigningContext().networkId(),
            exactAuth.accountId(),
            exactAuth::sign);
    final TransportRequest transportRequest =
        buildExactSignedQueryPostRequest(
            "/v1/query",
            signedQuery.requestBody(),
            AuthenticatedPrivacyStateQueryNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytesAllowingNotFound(
            transportRequest,
            "authenticated finalized privacy-state query " + exactRequest.getQueryId())
        .thenApply(
            response -> {
              if (response == null) {
                return null;
              }
              final PrivacyFinalizedStateViewV1 view =
                  AuthenticatedPrivacyStateQueryNativeBridge.projectPrivacyStateQueryV1(
                      signedQuery, response);
              if (!expectedType.isInstance(view)) {
                throw new IllegalStateException(
                    "authenticated finalized privacy-state query returned a different typed view");
              }
              return expectedType.cast(view);
            });
  }

  private CompletableFuture<AuthenticatedCommittedTransactionResultV1>
      getOptionalAuthenticatedCommittedTransactionResultV1(
          final String transactionHashHex,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    final AuthenticatedTransactionDetailsNativeBridge.SignedQueryV1 signedQuery =
        AuthenticatedTransactionDetailsNativeBridge.buildSignedTransactionDetailsQueryV1(
            transactionHashHex,
            config.requireLocalSigningContext().networkId(),
            canonicalAuth.accountId(),
            canonicalAuth::sign);
    final TransportRequest request =
        buildExactSignedQueryPostRequest(
            "/v1/pipeline/transactions/details",
            signedQuery.requestBody(),
            AuthenticatedTransactionDetailsNativeBridge.RESPONSE_MAX_BYTES);
    return fetchExactNoritoBytesAllowingNotFound(
            request, "authenticated committed transaction result")
        .thenApply(
            response ->
                response == null
                    ? null
                    : AuthenticatedTransactionDetailsNativeBridge
                        .projectCommittedTransactionResultV1(signedQuery, response));
  }

  /**
   * Require committed/native tuple agreement before retained privacy construction, authenticated
   * against the exact locally configured network.
   */
  public CompletableFuture<PrivacyExact12CapabilityTupleAdmissionV1>
      requirePrivacyExact12CapabilityAdmission(
          final PrivacyProtocolIdV1 protocolId,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    return getPrivacyCapabilities(canonicalAuth)
        .thenApply(
            manifest ->
                PrivacyExact12CapabilityAdmissionV1.requireExact12CapabilityTupleV1(
                    manifest,
                    org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1.fromCanonicalLabel(
                        protocolId.canonicalLabel())));
  }

  /** Fetch and strictly decode exact-lane SCCP capability discovery. */
  public CompletableFuture<SccpModels.Capabilities> getSccpCapabilities() {
    return fetchSccpJson(
        buildJsonGetRequest(
            "/v1/sccp/capabilities",
            Collections.emptyMap(),
            SCCP_CAPABILITIES_RESPONSE_MAX_BYTES),
        SccpJsonParser::parseCapabilities,
        "SCCP capabilities");
  }

  /** Fetch and strictly decode the authoritative typed SCCP route registry. */
  public CompletableFuture<SccpModels.RegistryV1> getSccpRegistry() {
    return fetchSccpJson(
        buildJsonGetRequest(
            "/v1/sccp/registry",
            Collections.emptyMap(),
            SCCP_JSON_RESPONSE_MAX_BYTES),
        SccpJsonParser::parseRegistry,
        "SCCP registry");
  }

  /** Fetch one query-free finalized SCCP message bundle by canonical message id. */
  public CompletableFuture<SccpModels.MessageBundleV1> getSccpMessageBundle(
      final String messageIdHex) {
    final String messageId =
        normalizeExactNonZeroEvenLengthHex(messageIdHex, "messageIdHex", 32);
    return fetchSccpJson(
        buildJsonGetRequest(
            "/v1/sccp/proofs/message/" + encodePathSegment(messageId),
            Collections.emptyMap(),
            SCCP_JSON_RESPONSE_MAX_BYTES),
        bytes -> {
          final SccpModels.MessageBundleV1 result = SccpJsonParser.parseMessageBundle(bytes);
          if (!messageId.equals(result.messageIdHex)) {
            throw new IllegalArgumentException(
                "SCCP bundle message id does not match the requested id");
          }
          return result;
        },
        "SCCP message bundle");
  }

  /** Fetch one query-free state-derived Groth16 request by canonical message id. */
  public CompletableFuture<SccpModels.Groth16ProofRequestV1> getSccpProofRequest(
      final String messageIdHex) {
    final String messageId =
        normalizeExactNonZeroEvenLengthHex(messageIdHex, "messageIdHex", 32);
    return fetchSccpJson(
        buildJsonGetRequest(
            "/v1/sccp/proof-requests/" + encodePathSegment(messageId),
            Collections.emptyMap(),
            SCCP_JSON_RESPONSE_MAX_BYTES),
        bytes -> {
          final SccpModels.Groth16ProofRequestV1 result =
              SccpJsonParser.parseProofRequest(bytes);
          if (!messageId.equals(result.messageIdHex)) {
            throw new IllegalArgumentException(
                "SCCP proof request message id does not match the requested id");
          }
          return result;
        },
        "SCCP proof request");
  }

  /** Fetch newest-first exact-context SCCP outbound messages. */
  public CompletableFuture<SccpModels.RecentMessages> getSccpRecentMessages() {
    return getSccpRecentMessages(null, null, null);
  }

  /** Fetch newest-first exact-context SCCP outbound messages using an explicit compound window. */
  public CompletableFuture<SccpModels.RecentMessages> getSccpRecentMessages(
      final BigInteger from, final Integer afterIndex, final Integer limit) {
    if (from != null && (from.signum() <= 0 || from.bitLength() > 64)) {
      throw new IllegalArgumentException("from must be a positive u64 height");
    }
    if (afterIndex != null && from == null) {
      throw new IllegalArgumentException("afterIndex requires the paired from height");
    }
    if (afterIndex != null
        && (afterIndex.intValue() < 0
            || afterIndex.intValue()
                >= SccpModels.SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1)) {
      throw new IllegalArgumentException(
          "afterIndex must be between 0 and "
              + (SccpModels.SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1 - 1));
    }
    if (limit != null && (limit.intValue() < 1 || limit.intValue() > 50)) {
      throw new IllegalArgumentException("limit must be between 1 and 50");
    }
    final Map<String, String> query = new LinkedHashMap<>();
    if (from != null) query.put("from", from.toString());
    if (afterIndex != null) {
      query.put("after_index", Integer.toString(afterIndex.intValue()));
    }
    if (limit != null) query.put("limit", Integer.toString(limit.intValue()));
    return fetchSccpJson(
        buildJsonGetRequest(
            "/v1/sccp/messages/recent", query, SCCP_RECENT_RESPONSE_MAX_BYTES),
        SccpJsonParser::parseRecentMessages,
        "SCCP recent messages");
  }

  /** Continue newest-first SCCP discovery from an exact server-issued cursor. */
  public CompletableFuture<SccpModels.RecentMessages> getSccpRecentMessages(
      final SccpModels.RecentCursor cursor) {
    return getSccpRecentMessages(cursor, null);
  }

  /** Continue newest-first SCCP discovery from a cursor with an optional page limit. */
  public CompletableFuture<SccpModels.RecentMessages> getSccpRecentMessages(
      final SccpModels.RecentCursor cursor, final Integer limit) {
    if (cursor == null) {
      throw new IllegalArgumentException("cursor must not be null");
    }
    return getSccpRecentMessages(
        cursor.from, Integer.valueOf(cursor.afterIndex), limit);
  }

  /** Fetches a persisted identifier claim by its deterministic receipt hash. */
  public CompletableFuture<Optional<IdentifierClaimRecord>> getIdentifierClaimByReceiptHash(
      final String receiptHash) {
    final String normalizedReceiptHash = normalizeHex32(receiptHash, "receiptHash");
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/identifiers/receipts/" + encodePathSegment(normalizedReceiptHash),
            Collections.emptyMap());
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseClaimRecord, "identifier claim lookup");
  }

  /** Resolves an identifier using a typed request wrapper. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> resolveIdentifier(
      final IdentifierResolveRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildIdentifierResolvePayload(
                requestBody.policyId(),
                requestBody.encryptedInputHex(),
                requestBody.outputOpening()));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/identifiers/resolve", body, canonicalAuth);
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseResolutionReceipt, "identifier resolve");
  }

  /** Resolves a hidden identifier by posting encrypted input and a verified output opening. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> resolveIdentifier(
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return resolveIdentifier(
        IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening),
        canonicalAuth);
  }

  /** Issues a claim receipt using a typed request wrapper. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> issueIdentifierClaimReceipt(
      final String accountId,
      final IdentifierResolveRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final String normalizedAccountId =
        org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
            accountId, "accountId");
    if (!normalizedAccountId.equals(canonicalAuth.accountId())) {
      throw new IllegalArgumentException(
          "canonicalAuth.accountId must equal the claim-receipt path accountId");
    }
    final byte[] body =
        encodeJsonBody(
            buildIdentifierResolvePayload(
                requestBody.policyId(),
                requestBody.encryptedInputHex(),
                requestBody.outputOpening()));
    final TransportRequest request =
        buildVpnRequest(
            "POST",
            "/v1/accounts/"
                + encodePathSegment(normalizedAccountId)
                + "/identifiers/claim-receipt",
            body,
            canonicalAuth);
    return fetchJsonAllowingNotFound(
        request, IdentifierJsonParser::parseResolutionReceipt, "identifier claim receipt");
  }

  /** Issues a claim receipt by posting encrypted input and a verified output opening. */
  public CompletableFuture<Optional<IdentifierResolutionReceipt>> issueIdentifierClaimReceipt(
      final String accountId,
      final String policyId,
      final String encryptedInputHex,
      final RamLfeOutputOpening outputOpening,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return issueIdentifierClaimReceipt(
        accountId,
        IdentifierResolveRequest.encrypted(policyId, encryptedInputHex, outputOpening),
        canonicalAuth);
  }

  /** Executes a RAM-LFE program using a typed request wrapper. */
  public CompletableFuture<Optional<RamLfeExecuteResponse>> executeRamLfeProgram(
      final String programId,
      final RamLfeExecuteRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final String normalizedProgramId = normalizeNonBlank(programId, "programId");
    final byte[] body =
        encodeJsonBody(buildRamLfeExecutePayload(requestBody.encryptedInputHex()));
    final TransportRequest request =
        buildVpnRequest(
            "POST",
            "/v1/ram-lfe/programs/" + encodePathSegment(normalizedProgramId) + "/execute",
            body,
            canonicalAuth);
    return fetchJsonAllowingNotFound(
        request, RamLfeJsonParser::parseExecuteResponse, "ram-lfe execute");
  }

  /**
   * Executes a RAM-LFE program by posting BFV ciphertext hex to
   * `/v1/ram-lfe/programs/{program_id}/execute`.
   */
  public CompletableFuture<Optional<RamLfeExecuteResponse>> executeRamLfeProgram(
      final String programId,
      final String encryptedInputHex,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return executeRamLfeProgram(
        programId, buildRamLfeExecuteRequest(encryptedInputHex), canonicalAuth);
  }

  /** Verifies a RAM-LFE execution receipt against the node's registered program policy. */
  public CompletableFuture<RamLfeReceiptVerifyResponse> verifyRamLfeReceipt(
      final RamLfeReceiptVerifyRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        encodeJsonBody(
            buildRamLfeReceiptVerifyPayload(requestBody.receipt(), requestBody.outputHex()));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/ram-lfe/receipts/verify", body, canonicalAuth);
    return fetchJson(
        request, RamLfeJsonParser::parseReceiptVerifyResponse, "ram-lfe receipt verify");
  }

  /** Verifies a RAM-LFE execution receipt against the node's registered program policy. */
  public CompletableFuture<RamLfeReceiptVerifyResponse> verifyRamLfeReceipt(
      final Map<String, Object> receipt,
      final String outputHex,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return verifyRamLfeReceipt(
        new RamLfeReceiptVerifyRequest(receipt, outputHex), canonicalAuth);
  }

  /** Fetches the public Sora VPN profile. */
  public CompletableFuture<VpnProfile> getVpnProfile() {
    requireSecureVpnBaseUri();
    final TransportRequest request =
        buildJsonGetRequest("/v1/vpn/profile", Collections.emptyMap());
    return fetchJson(request, VpnJsonParser::parseProfile, "vpn profile", 200);
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
    return fetchJson(request, VpnJsonParser::parseQuote, "vpn quote create", 201);
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
    return fetchJson(request, VpnJsonParser::parseSession, "vpn session create", 201);
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
    return fetchJsonAllowingNotFound(
        request, VpnJsonParser::parseSession, "vpn session lookup", 200);
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
    return fetchJsonAllowingNotFound(
        request, VpnJsonParser::parseReceipt, "vpn session delete", 200);
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
    return fetchJson(request, VpnJsonParser::parseReceipt, "vpn receipt submit", 201);
  }

  /** Lists VPN receipts for the signed account. */
  public CompletableFuture<VpnReceiptListResponse> listVpnReceipts(
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final TransportRequest request = buildVpnRequest("GET", "/v1/vpn/receipts", null, canonicalAuth);
    return fetchJson(request, VpnJsonParser::parseReceiptList, "vpn receipt list", 200);
  }

  /**
   * Prepares an unsigned verifier registration transaction for local signing.
   *
   * <p>Requires {@link ClientConfig#localSigningContext()} and rejects any draft not bound to that
   * exact network, the requested authority, and the exact requested registry record.
   */
  public CompletableFuture<VerifyingKeyTransactionDraft> registerVerifyingKey(
      final VerifyingKeyRegisterRequest requestBody) {
    final LocalSigningContext signingContext = config.requireLocalSigningContext();
    final Map<String, Object> payload = buildVerifyingKeyRegisterPayload(requestBody);
    final byte[] body = encodeJsonBody(payload);
    final TransportRequest request = buildJsonPostRequest("/v1/zk/vk/register", body);
    return fetchJson(
        request,
        bytes -> VerifyingKeyTransactionDraft.parseRegister(
            bytes, signingContext.networkId(), payload),
        "verifying key register draft",
        200);
  }

  /**
   * Prepares an unsigned verifier update transaction for local signing.
   *
   * <p>Requires {@link ClientConfig#localSigningContext()} and rejects any draft not bound to that
   * exact network, the requested authority, and the exact requested registry record.
   */
  public CompletableFuture<VerifyingKeyTransactionDraft> updateVerifyingKey(
      final VerifyingKeyUpdateRequest requestBody) {
    final LocalSigningContext signingContext = config.requireLocalSigningContext();
    final Map<String, Object> payload = buildVerifyingKeyUpdatePayload(requestBody);
    final byte[] body = encodeJsonBody(payload);
    final TransportRequest request = buildJsonPostRequest("/v1/zk/vk/update", body);
    return fetchJson(
        request,
        bytes -> VerifyingKeyTransactionDraft.parseUpdate(
            bytes, signingContext.networkId(), payload),
        "verifying key update draft",
        200);
  }

  /** Quotes the exact unsigned transaction payload before signing. */
  public CompletableFuture<FeeQuoteResponse> quoteFees(
      final Map<String, Object> unsignedPayload,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(unsignedPayload, "unsignedPayload");
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    requireNetworkTransactionDomain(unsignedPayload);
    final Object authority = unsignedPayload.get("authority");
    if (!(authority instanceof String) || !authority.equals(canonicalAuth.accountId())) {
      throw new IllegalArgumentException(
          "canonicalAuth.accountId must equal unsignedPayload.authority");
    }
    final FeePaymentIntent requestedIntent =
        FeePaymentJson.parse(unsignedPayload.get("fee_payment"), "unsignedPayload.fee_payment");
    final Map<String, Object> requestBody = new LinkedHashMap<>();
    requestBody.put("payload", unsignedPayload);
    final byte[] body = encodeJsonBody(requestBody);
    return fetchJson(
            buildVpnRequest("POST", "/v1/fees/quote", body, canonicalAuth),
            FeePaymentJson::parseQuote,
            "fee quote",
            200)
        .thenApply(
            quote -> {
              if (!requestedIntent.hasSamePayerAndGasBound(quote.intent())) {
                throw new IllegalArgumentException(
                    "fee quote response changed the requested payer, sponsor revision, or gas bound");
              }
              return quote;
            });
  }

  private static NetworkId requireNetworkTransactionDomain(
      final Map<String, Object> unsignedPayload) {
    for (final String field : Arrays.asList("chain", "chainId", "chain_id")) {
      if (unsignedPayload.containsKey(field)) {
        throw new IllegalArgumentException(
            "unsignedPayload contains retired transaction identity field `" + field + "`");
      }
    }
    final Object rawDomain = unsignedPayload.get("domain");
    if (!(rawDomain instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(
          "unsignedPayload.domain must be TransactionDomain::Network");
    }
    final Map<?, ?> domain = (Map<?, ?>) rawDomain;
    if (!domain.keySet().equals(Set.of("kind", "value"))
        || !"network".equals(domain.get("kind"))
        || !(domain.get("value") instanceof String)) {
      throw new IllegalArgumentException(
          "unsignedPayload.domain must contain exactly kind=network and a NetworkId value");
    }
    return NetworkId.parse((String) domain.get("value"));
  }

  /** Fetches one exact on-chain fee sponsor program under canonical request authentication. */
  public CompletableFuture<FeeSponsorProgramResponse> getFeeSponsorProgram(
      final FeeSponsorProgramId programId,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(programId, "programId");
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final Map<String, Object> requestBody = new LinkedHashMap<>();
    requestBody.put("program_id", programId.literal());
    final byte[] body = encodeJsonBody(requestBody);
    return fetchJson(
            buildVpnRequest("POST", "/v1/fee-sponsor-programs/by-id", body, canonicalAuth),
            FeePaymentJson::parseProgram,
            "fee sponsor program lookup",
            200)
        .thenApply(
            program -> {
              if (!programId.equals(program.id())) {
                throw new IllegalArgumentException(
                    "fee sponsor program response id does not match the requested program");
              }
              return program;
            });
  }

  /**
   * Prepares an unsigned contract-call transaction for local signing.
   *
   * <p>Private signing material is never accepted by or sent to Torii. Sign the returned
   * canonical transaction payload locally and submit the resulting signed transaction through {@link
   * #submitTransaction(SignedTransaction)}.
   */
  public CompletableFuture<ContractCallResponse> prepareContractCall(
      final String authority,
      final FeePaymentIntent feePayment,
      final String contractAddress,
      final String contractAlias,
      final String entrypoint,
      final Object payload) {
    final Map<String, Object> requestPayload =
        buildContractCallDraftPayload(
            authority,
            feePayment,
            contractAddress,
            contractAlias,
            entrypoint,
            payload);
    final byte[] body = encodeJsonBody(requestPayload);
    final TransportRequest request = buildJsonPostRequest("/v1/contracts/call", body);
    return fetchJson(request, ContractJsonParser::parseCallResponse, "contract call draft")
        .thenApply(response -> validateContractCallDraft(response, requestPayload));
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
      final String contractAddress, final ToriiCanonicalRequestAuth canonicalAuth) {
    final String normalizedAddress = normalizeNonBlank(contractAddress, "contractAddress");
    final TransportRequest request =
        buildVpnRequest(
            "GET",
            "/v1/gov/contracts/" + encodePathSegment(normalizedAddress),
            null,
            canonicalAuth);
    return fetchJson(
        request,
        ContractJsonParser::parseGovernanceContractResponse,
        "governance contract");
  }

  /** Fetches the complete manifest via `GET /v1/contracts/code/{code_hash}`. */
  @Override
  public CompletableFuture<ContractManifestRecord> getContractManifest(final String codeHash) {
    if (codeHash == null || codeHash.length() != 64) {
      throw new IllegalArgumentException("codeHash must contain exactly 64 hex characters");
    }
    final String normalizedCodeHash = normalizeExactEvenLengthHex(codeHash, "codeHash");
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/contracts/code/" + encodePathSegment(normalizedCodeHash),
            Collections.emptyMap());
    return fetchJson(request, ContractJsonParser::parseManifestRecord, "contract manifest");
  }

  /** Resolves an account alias via `POST /v1/aliases/resolve`. */
  @Override
  public CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias) {
    final String normalizedAlias = AccountAliasName.parse(alias).canonicalText();
    final byte[] body = encodeJsonBody(objectMapOf("alias", normalizedAlias));
    final TransportRequest request = buildJsonPostRequest("/v1/aliases/resolve", body);
    return fetchJsonAllowingNotFound(
        request,
        response -> parsePinnedAliasResolution(response, normalizedAlias),
        "account alias resolve");
  }

  /** Plans one atomic alias setup transaction without invoking a mutation route. */
  @Override
  public CompletableFuture<AliasTransactionPlanV1> planAliasSetup(
      final AliasSetupPlanRequestV1 requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(requestBody, "requestBody");
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    final byte[] body = encodeJsonBody(requestBody.toJsonMap());
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/aliases/setup/plan", body, canonicalAuth);
    return fetchJson(
        request,
        response -> {
          final AliasTransactionPlanV1 plan = AliasTransactionPlanJsonParser.parse(response);
          if (!plan.body().authority().equals(canonicalAuth.accountId())) {
            throw new IllegalArgumentException(
                "alias setup plan authority does not match the canonical request signer");
          }
          return plan;
        },
        "alias setup plan",
        200);
  }

  @Override
  public CompletableFuture<AliasLifecycleTransactionPlanV1> planAliasLeaseRenewal(
      final AliasLeaseRenewPlanRequestV1 requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return planAliasLifecycle(
        "/v1/aliases/lease/renew/plan",
        requestBody,
        canonicalAuth,
        "alias lease renewal plan");
  }

  @Override
  public CompletableFuture<AliasLifecycleTransactionPlanV1> planAliasAutoRenew(
      final AliasAutoRenewPlanRequestV1 requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return planAliasLifecycle(
        "/v1/aliases/auto-renew/plan",
        requestBody,
        canonicalAuth,
        "alias auto-renew plan");
  }

  private CompletableFuture<AliasLifecycleTransactionPlanV1> planAliasLifecycle(
      final String path,
      final AliasLifecyclePlanRequestV1 requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final String context) {
    final byte[] body =
        JsonEncoder.encode(requestBody.toJsonMap()).getBytes(StandardCharsets.UTF_8);
    final TransportRequest request = buildVpnRequest("POST", path, body, canonicalAuth);
    return fetchJson(
        request,
        response -> {
          final AliasLifecycleTransactionPlanV1 plan =
              AliasLifecycleTransactionPlanJsonParser.parse(response);
          if (!plan.body().authority().equals(canonicalAuth.accountId())) {
            throw new IllegalArgumentException(
                context + " authority does not match the canonical request signer");
          }
          return plan;
        },
        context,
        200);
  }

  @Override
  public CompletableFuture<AccountOnboardingPlanReceiptV1> planSponsoredAccountOnboarding(
      final AccountOnboardingPlanRequestV1 requestBody,
      final String onboardingToken,
      final NetworkId expectedNetworkId) {
    return planSponsoredAccountOnboarding(requestBody, onboardingToken, null, expectedNetworkId);
  }

  @Override
  public CompletableFuture<AccountOnboardingPlanReceiptV1> planSponsoredAccountOnboarding(
      final AccountOnboardingPlanRequestV1 requestBody,
      final String onboardingToken,
      final String expectedAuthority,
      final NetworkId expectedNetworkId) {
    final byte[] body =
        JsonEncoder.encode(requestBody.toJsonMap()).getBytes(StandardCharsets.UTF_8);
    return fetchJson(
        buildOnboardingRequest("POST", "/v1/accounts/onboard/plan", body, onboardingToken),
        response ->
            AccountOnboardingReceiptVerifier.requireValidForRequest(
                requestBody,
                AccountOnboardingJsonParser.parseReceipt(response),
                expectedNetworkId,
                expectedAuthority),
        "sponsored account onboarding plan",
        200);
  }

  @Override
  public CompletableFuture<AccountOnboardingResponseV1> applySponsoredAccountOnboarding(
      final AccountOnboardingPlanReceiptV1 receipt,
      final String onboardingToken,
      final NetworkId expectedNetworkId) {
    return applySponsoredAccountOnboarding(receipt, onboardingToken, null, expectedNetworkId);
  }

  @Override
  public CompletableFuture<AccountOnboardingResponseV1> applySponsoredAccountOnboarding(
      final AccountOnboardingPlanReceiptV1 receipt,
      final String onboardingToken,
      final String expectedAuthority,
      final NetworkId expectedNetworkId) {
    AccountOnboardingReceiptVerifier.requireValid(receipt, expectedNetworkId, expectedAuthority);
    final byte[] body =
        JsonEncoder.encode(new AccountOnboardingApplyRequestV1(receipt).toJsonMap())
            .getBytes(StandardCharsets.UTF_8);
    return fetchJson(
        buildOnboardingRequest("POST", "/v1/accounts/onboard", body, onboardingToken),
        AccountOnboardingJsonParser::parseResponse,
        "sponsored account onboarding apply",
        null,
        (response, statusCode) ->
            AccountOnboardingResponseVerifier.requireValidForReceipt(
                receipt, response, statusCode.intValue()));
  }

  @Override
  public CompletableFuture<AliasSetupModels.AliasSetupReportV1> getAccountOnboardingReadiness(
      final String onboardingToken) {
    return fetchJson(
        buildOnboardingRequest(
            "GET", "/v1/accounts/onboarding/readiness", null, onboardingToken),
        AccountOnboardingJsonParser::parseReadiness,
        "account onboarding readiness",
        200);
  }

  @Override
  public CompletableFuture<Optional<AccountAliasIndexResolution>> resolveAccountAliasIndex(
      final BigInteger index) {
    AccountAliasUInt64.require(index, "index");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("index", index);
    final byte[] body = JsonEncoder.encode(payload).getBytes(StandardCharsets.UTF_8);
    return fetchJsonAllowingNotFound(
        buildJsonPostRequest("/v1/aliases/resolve-index", body),
        response -> parsePinnedAliasIndexResolution(response, index),
        "account alias index resolve");
  }

  @Override
  public CompletableFuture<Optional<AccountAliasIndexResolution>> resolveAccountAliasIndex(
      final BigInteger index, final ToriiCanonicalRequestAuth canonicalAuth) {
    AccountAliasUInt64.require(index, "index");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("index", index);
    final byte[] body = JsonEncoder.encode(payload).getBytes(StandardCharsets.UTF_8);
    return fetchJsonAllowingNotFound(
        buildVpnRequest("POST", "/v1/aliases/resolve-index", body, canonicalAuth),
        response -> parsePinnedAliasIndexResolution(response, index),
        "account alias index resolve");
  }

  @Override
  public CompletableFuture<Optional<AccountAliasesByAccount>> listAccountAliases(
      final AccountAliasesByAccountRequest requestBody) {
    final byte[] body =
        JsonEncoder.encode(requestBody.toJsonMap()).getBytes(StandardCharsets.UTF_8);
    return fetchJsonAllowingNotFound(
        buildJsonPostRequest("/v1/aliases/by-account", body),
        response -> parsePinnedAliasesByAccount(response, requestBody),
        "account aliases lookup");
  }

  @Override
  public CompletableFuture<Optional<AccountAliasesByAccount>> listAccountAliases(
      final AccountAliasesByAccountRequest requestBody,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final byte[] body =
        JsonEncoder.encode(requestBody.toJsonMap()).getBytes(StandardCharsets.UTF_8);
    return fetchJsonAllowingNotFound(
        buildVpnRequest("POST", "/v1/aliases/by-account", body, canonicalAuth),
        response -> parsePinnedAliasesByAccount(response, requestBody),
        "account aliases lookup");
  }

  /** Resolves a restricted account alias with canonical Iroha request headers. */
  @Override
  public CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias, final ToriiCanonicalRequestAuth canonicalAuth) {
    final String normalizedAlias = AccountAliasName.parse(alias).canonicalText();
    final byte[] body = encodeJsonBody(objectMapOf("alias", normalizedAlias));
    final TransportRequest request =
        buildVpnRequest("POST", "/v1/aliases/resolve", body, canonicalAuth);
    return fetchJsonAllowingNotFound(
        request,
        response -> parsePinnedAliasResolution(response, normalizedAlias),
        "account alias resolve");
  }

  private static AccountAliasResolution parsePinnedAliasResolution(
      final byte[] response, final String requestedAlias) {
    final AccountAliasResolution resolution = AccountAliasJsonParser.parseResolution(response);
    if (!AccountAliasName.parse(resolution.alias()).canonicalText().equals(requestedAlias)) {
      throw new IllegalArgumentException(
          "account alias response does not match the requested alias");
    }
    return resolution;
  }

  private static AccountAliasIndexResolution parsePinnedAliasIndexResolution(
      final byte[] response, final BigInteger requestedIndex) {
    final AccountAliasIndexResolution resolution =
        AccountAliasReadJsonParser.parseIndexResolution(response);
    if (!resolution.index().equals(requestedIndex)) {
      throw new IllegalArgumentException(
          "account alias index response does not match the requested index");
    }
    return resolution;
  }

  private static AccountAliasesByAccount parsePinnedAliasesByAccount(
      final byte[] response, final AccountAliasesByAccountRequest request) {
    final AccountAliasesByAccount aliases = AccountAliasReadJsonParser.parseByAccount(response);
    if (!aliases.accountId().equals(request.accountId())) {
      throw new IllegalArgumentException(
          "account aliases response does not match the requested account");
    }
    for (final AccountAliasListItem item : aliases.items()) {
      if ((request.dataspace() != null && !request.dataspace().equals(item.dataspace()))
          || (request.domain() != null && !request.domain().equals(item.domain()))) {
        throw new IllegalArgumentException(
            "account aliases response contains entries outside the requested scope");
      }
    }
    return aliases;
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

  /**
   * Returns a copy of {@code config} with a directory-backed pending queue rooted at {@code
   * queueDir}. Each queued transaction is persisted as its own envelope file to satisfy OEM or
   * managed-device storage policies. Transaction submission never drains or fills this queue.
   */
  public static ClientConfig withDirectoryPendingQueue(
      final ClientConfig config, final Path queueDir) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableDirectoryPendingQueue(queueDir).build();
  }

  /** Returns a copy with explicit local staging; transaction submission never drains or fills it. */
  public static ClientConfig withFilePendingQueue(
      final ClientConfig config, final Path queueFile) {
    Objects.requireNonNull(config, "config");
    return config.toBuilder().enableFilePendingQueue(queueFile).build();
  }

  /**
   * Creates a {@link SubscriptionToriiClient} that reuses this transport's HTTP executor, base
   * URI, headers, and observers.
   */
  public SubscriptionToriiClient subscriptionToriiClient() {
    return config.toSubscriptionToriiClient(executor);
  }

  private CompletableFuture<ClientResponse> submitOnce(
      final SignedTransaction transaction, final String hashHex) {
    final TransportRequest request =
        ToriiRequestBuilder.buildSubmitRequest(
            config.baseUri(),
            transaction,
            config.requestTimeout(),
            config.defaultHeaders(),
            config.wireFormatPreference().acceptHeader());

    return ensureTransactionSubmissionCompatibility()
        .thenCompose(
            ignored -> {
              notifyRequest(request);
              return executor
                  .execute(request)
                  .handle(
                      (response, throwable) -> {
                        if (throwable != null) {
                          final Throwable cause = unwrapCompletion(throwable);
                          final AmbiguousTransactionSubmissionException error =
                              new AmbiguousTransactionSubmissionException(hashHex, null, cause);
                          notifyFailure(request, error);
                          final CompletableFuture<ClientResponse> failed =
                              new CompletableFuture<>();
                          failed.completeExceptionally(error);
                          return failed;
                        }
                        final ClientResponse clientResponse =
                            new ClientResponse(
                                response.statusCode(),
                                response.body(),
                                response.message(),
                                extractEntrypointHash(response).orElse(hashHex),
                                extractRejectCode(response));
                        if (submissionOutcomeIsAmbiguous(clientResponse.statusCode())) {
                          final AmbiguousTransactionSubmissionException error =
                              new AmbiguousTransactionSubmissionException(
                                  hashHex, Integer.valueOf(clientResponse.statusCode()), null);
                          notifyFailure(request, error);
                          final CompletableFuture<ClientResponse> failed =
                              new CompletableFuture<>();
                          failed.completeExceptionally(error);
                          return failed;
                        }
                        notifyResponse(request, clientResponse);
                        return CompletableFuture.completedFuture(clientResponse);
                      })
                  .thenCompose(future -> future);
            });
  }

  private static boolean submissionOutcomeIsAmbiguous(final int statusCode) {
    return (statusCode >= 300 && statusCode <= 399)
        || statusCode == 408
        || statusCode == 425
        || statusCode == 429
        || statusCode >= 500;
  }

  private void emitDeviceProfileTelemetry() {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    if (!deviceProfileEmitted.compareAndSet(false, true)) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (!sink.isPresent()) {
      return;
    }
    final DeviceProfileProvider provider = config.deviceProfileProvider();
    if (provider == null) {
      return;
    }
    final Optional<DeviceProfile> profile = provider.snapshot();
    if (!profile.isPresent()) {
      return;
    }
    sink
        .get()
        .emitSignal(
            "android.telemetry.device_profile",
            objectMapOf("profile_bucket", profile.get().bucket()));
  }

  private void emitNetworkContextTelemetry() {
    if (!config.telemetryOptions().enabled()) {
      return;
    }
    final Optional<TelemetrySink> sink = config.telemetrySink();
    if (!sink.isPresent()) {
      return;
    }
    final Optional<NetworkContext> context = config.networkContextProvider().snapshot();
    if (!context.isPresent()) {
      return;
    }
    sink
        .get()
        .emitSignal("android.telemetry.network_context", context.get().toTelemetryFields());
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
    if (!sink.isPresent()) {
      return;
    }
    final Map<String, Object> fields = new LinkedHashMap<>();
    maybePutAuthorityHash(fields, request, sink.get(), PIPELINE_STATUS_SIGNAL);
    if (transactionHash != null && !transactionHash.trim().isEmpty()) {
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

  private static Optional<String> extractEntrypointHash(final TransportResponse response) {
    if (response == null) {
      return Optional.empty();
    }
    final List<String> values = response.headers().get(ENTRYPOINT_HASH_HEADER);
    if (values == null) {
      return Optional.empty();
    }
    for (final String value : values) {
      final String normalized = normalizeEntrypointHashHeader(value);
      if (normalized != null) {
        return Optional.of(normalized);
      }
    }
    return Optional.empty();
  }

  private static String normalizeEntrypointHashHeader(final String value) {
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
        objectMapOf(
            "signal_id", signalId,
            "reason", reason));
  }

  private static final class PrivacyActionStatusSnapshotV1 {
    private final PrivacyActionOperationViewV1 view;
    private final Map<String, Object> payload;
    private final String statusKind;

    private PrivacyActionStatusSnapshotV1(
        final PrivacyActionOperationViewV1 view,
        final Map<String, Object> payload,
        final String statusKind) {
      this.view = view;
      this.payload = payload;
      this.statusKind = statusKind;
    }
  }

  private CompletableFuture<PrivacyActionStatusSnapshotV1>
      fetchPrivacyActionStatusSnapshotV1(
          final PrivacyActionOperationViewV1 operation,
          final ToriiCanonicalRequestAuth canonicalAuth) {
    Objects.requireNonNull(operation, "operation");
    requireAuthenticatedPrivacyActionViewV1(operation);
    final String transactionHashHex = lowerHex(operation.transactionHashBytes());
    return fetchAuthenticatedPrivacyPublicStatusV1(transactionHashHex, canonicalAuth)
        .thenCompose(
            payload -> {
              if (payload == null) {
                if (operation.localState == PrivacyActionLocalStateV1.TERMINAL) {
                  throw new IllegalStateException(
                      "terminal Exact12 action disappeared from pipeline status");
                }
                return CompletableFuture.completedFuture(
                    new PrivacyActionStatusSnapshotV1(operation, null, null));
              }
              final String kind =
                  PipelineStatusExtractor.requireAuthoritativeStatus(
                      payload, transactionHashHex);
              if ("Queued".equals(kind)
                  || "Approved".equals(kind)
                  || "Committed".equals(kind)) {
                if (operation.localState == PrivacyActionLocalStateV1.TERMINAL) {
                  throw new IllegalStateException("terminal Exact12 action status regressed");
                }
                return CompletableFuture.completedFuture(
                    new PrivacyActionStatusSnapshotV1(operation, payload, kind));
              }
              if ("Expired".equals(kind)) {
                if ("cache".equals(payload.get("resolved_from"))) {
                  if (operation.localState == PrivacyActionLocalStateV1.TERMINAL) {
                    throw new IllegalStateException(
                        "terminal Exact12 action status regressed to cache-only expiry");
                  }
                  return CompletableFuture.completedFuture(
                      new PrivacyActionStatusSnapshotV1(operation, payload, kind));
                }
                if (!"state".equals(payload.get("resolved_from"))
                    || privacyStatusBlockHeightV1(payload) != null) {
                  throw new IllegalStateException(
                      "expired Exact12 action requires state-resolved evidence without a committed height");
                }
                final PrivacyActionOperationViewV1 refreshed =
                    withTerminalPrivacyStatusV1(
                        operation, PrivacyActionTerminalChainStateV1.EXPIRED, null, null);
                requireStableTerminalPrivacyView(operation, refreshed);
                bindAuthenticatedPrivacyActionViewV1(refreshed);
                return CompletableFuture.completedFuture(
                    new PrivacyActionStatusSnapshotV1(refreshed, payload, kind));
              }
              if (!"Applied".equals(kind) && !"Rejected".equals(kind)) {
                throw new IllegalStateException("Exact12 action status kind is unsupported");
              }
              if (!"state".equals(payload.get("resolved_from"))
                  && !"cache".equals(payload.get("resolved_from"))) {
                throw new IllegalStateException(
                    "committed Exact12 action status must be resolved from state or cache");
              }
              final BigInteger publicHeight = privacyStatusBlockHeightV1(payload);
              final CompletableFuture<AuthenticatedCommittedTransactionResultV1> detailsFuture =
                  getOptionalAuthenticatedCommittedTransactionResultV1(
                      transactionHashHex, canonicalAuth);
              final CompletableFuture<
                      AuthenticatedPrivacyActionReceiptNativeBridge
                          .AuthenticatedActionExecutionReceiptV1>
                  receiptFuture =
                      getAuthenticatedPrivacyActionExecutionReceiptV1(
                          operation, canonicalAuth);
              return detailsFuture.thenCombine(
                  receiptFuture,
                  (details, receipt) -> {
                    if (publicHeight != null
                        && receipt != null
                        && !publicHeight.equals(receipt.admittedAtHeight())) {
                      throw new IllegalStateException(
                          "Exact12 status height differs from authenticated receipt admission");
                    }
                    if ("Rejected".equals(kind)) {
                      if (receipt != null) {
                        throw new IllegalStateException(
                            "rejected Exact12 action contradicts an authenticated execution receipt");
                      }
                      if (details == null) {
                        return new PrivacyActionStatusSnapshotV1(operation, payload, kind);
                      }
                      if (!transactionHashHex.equals(details.transactionHashHex())) {
                        throw new IllegalStateException(
                            "authenticated Exact12 transaction result changed the transaction hash");
                      }
                      if (publicHeight != null
                          && !publicHeight.equals(details.committedBlockHeight())) {
                        throw new IllegalStateException(
                            "Exact12 status height differs from authenticated committed height");
                      }
                      if (details.resultOk() || details.rejectionMessage() == null) {
                        throw new IllegalStateException(
                            "rejected Exact12 action omitted its authenticated committed rejection");
                      }
                      final PrivacyActionOperationViewV1 refreshed =
                          withTerminalPrivacyStatusV1(
                              operation,
                              PrivacyActionTerminalChainStateV1.REJECTED,
                              details.committedBlockHeight(),
                              details.rejectionMessage());
                      requireStableTerminalPrivacyView(operation, refreshed);
                      bindAuthenticatedPrivacyActionViewV1(refreshed);
                      return new PrivacyActionStatusSnapshotV1(refreshed, payload, kind);
                    }

                    if (details != null) {
                      if (!transactionHashHex.equals(details.transactionHashHex())) {
                        throw new IllegalStateException(
                            "authenticated Exact12 transaction result changed the transaction hash");
                      }
                      if (publicHeight != null
                          && !publicHeight.equals(details.committedBlockHeight())) {
                        throw new IllegalStateException(
                            "Exact12 status height differs from authenticated committed height");
                      }
                      if (!details.resultOk()) {
                        throw new IllegalStateException(
                            "Applied Exact12 action resolved to a committed rejection");
                      }
                    }
                    if (details == null || receipt == null) {
                      return new PrivacyActionStatusSnapshotV1(operation, payload, kind);
                    }
                    if (!receipt.admittedAtHeight().equals(details.committedBlockHeight())) {
                      throw new IllegalStateException(
                          "authenticated Exact12 receipt differs from the committed transaction height");
                    }
                    final PrivacyActionOperationViewV1 refreshed =
                        withTerminalPrivacyStatusV1(
                            operation,
                            PrivacyActionTerminalChainStateV1.APPLIED,
                            details.committedBlockHeight(),
                            null,
                            receipt.capabilityManifestDigest(),
                            receipt.capabilityCommittedHeight(),
                            receipt.finalizedHeight(),
                            receipt.finalizedBlockHash());
                    requireStableTerminalPrivacyView(operation, refreshed);
                    bindAuthenticatedPrivacyActionViewV1(refreshed);
                    return new PrivacyActionStatusSnapshotV1(refreshed, payload, kind);
                  });
            });
  }

  private void bindAuthenticatedPrivacyActionViewV1(
      final PrivacyActionOperationViewV1 operation) {
    synchronized (privacyActionProvenanceLockV1) {
      boolean alreadyBound = false;
      for (int index = authenticatedPrivacyActionViewsV1.size() - 1; index >= 0; index--) {
        final PrivacyActionOperationViewV1 candidate =
            authenticatedPrivacyActionViewsV1.get(index).get();
        if (candidate == null) {
          authenticatedPrivacyActionViewsV1.remove(index);
        } else if (candidate == operation) {
          alreadyBound = true;
        }
      }
      if (!alreadyBound) {
        authenticatedPrivacyActionViewsV1.add(new WeakReference<>(operation));
      }
    }
  }

  private void requireAuthenticatedPrivacyActionViewV1(
      final PrivacyActionOperationViewV1 operation) {
    synchronized (privacyActionProvenanceLockV1) {
      for (int index = authenticatedPrivacyActionViewsV1.size() - 1; index >= 0; index--) {
        final PrivacyActionOperationViewV1 candidate =
            authenticatedPrivacyActionViewsV1.get(index).get();
        if (candidate == null) {
          authenticatedPrivacyActionViewsV1.remove(index);
        } else if (candidate == operation) {
          return;
        }
      }
    }
    throw new IllegalStateException(
        "Exact12 status requires a view returned by this client's authenticated submission");
  }

  private CompletableFuture<Map<String, Object>> fetchAuthenticatedPrivacyPublicStatusV1(
      final String transactionHashHex,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    if (transactionHashHex == null || !transactionHashHex.matches("[0-9a-f]{64}")) {
      throw new IllegalArgumentException(
          "transactionHashHex must be an exact lowercase 32-byte hash");
    }
    final LinkedHashMap<String, String> query = new LinkedHashMap<>();
    query.put("hash", transactionHashHex);
    query.put("scope", "global");
    final TransportRequest request =
        buildCanonicalJsonGetRequest(
            "/v1/pipeline/transactions/status",
            query,
            PRIVACY_STATUS_RESPONSE_MAX_BYTES,
            canonicalAuth);
    notifyRequest(request);
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause = unwrapCompletion(throwable);
                notifyFailure(request, cause);
                future.completeExceptionally(
                    new RuntimeException(
                        "authenticated Exact12 status request failed", cause));
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      extractRejectCode(response));
              try {
                if (response.statusCode() == 204 || response.statusCode() == 404) {
                  notifyResponse(request, clientResponse);
                  future.complete(null);
                } else if (response.statusCode() == 200 || response.statusCode() == 202) {
                  requireExactHeader(
                      response.headers(),
                      "Content-Type",
                      APPLICATION_JSON,
                      "authenticated Exact12 status");
                  if (response.body().length == 0) {
                    throw new IllegalStateException(
                        "authenticated Exact12 status response must not be empty");
                  }
                  if ((long) response.body().length > PRIVACY_STATUS_RESPONSE_MAX_BYTES) {
                    throw new IllegalStateException(
                        "authenticated Exact12 status response exceeds its byte bound");
                  }
                  requireExactOptionalContentLength(
                      response.headers(),
                      response.body().length,
                      "authenticated Exact12 status");
                  final Map<String, Object> payload =
                      parsePipelineStatusPayload(response.body());
                  notifyResponse(request, clientResponse);
                  future.complete(payload);
                } else {
                  throw new IllegalStateException(
                      "authenticated Exact12 status request failed with status "
                          + response.statusCode());
                }
              } catch (final RuntimeException error) {
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private CompletableFuture<PrivacyActionOperationViewV1> waitForPrivacyActionTerminalV1(
      final PrivacyActionOperationViewV1 submitted,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final PipelineStatusOptions options) {
    final PipelineStatusOptions resolved = PipelineStatusOptions.resolve(options);
    final long deadline = saturatedDeadline(resolved.timeoutMillis());
    final CompletableFuture<PrivacyActionOperationViewV1> result = new CompletableFuture<>();
    pollPrivacyActionStatusV1(
        submitted, canonicalAuth, resolved, deadline, 0, null, result);
    return result;
  }

  private void pollPrivacyActionStatusV1(
      final PrivacyActionOperationViewV1 submitted,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final PipelineStatusOptions options,
      final long deadline,
      final int attempts,
      final Map<String, Object> lastPayload,
      final CompletableFuture<PrivacyActionOperationViewV1> result) {
    if (result.isDone()) {
      return;
    }
    if (options.maxAttempts() != null && attempts >= options.maxAttempts()) {
      result.completeExceptionally(
          new TransactionTimeoutException(
              "Exact12 transaction "
                  + lowerHex(submitted.transactionHashBytes())
                  + " did not reach terminal state after "
                  + attempts
                  + " attempts",
              lowerHex(submitted.transactionHashBytes()),
              attempts,
              lastPayload));
      return;
    }
    fetchPrivacyActionStatusSnapshotV1(submitted, canonicalAuth)
        .whenComplete(
            (snapshot, throwable) -> {
              if (throwable != null) {
                result.completeExceptionally(unwrapCompletion(throwable));
                return;
              }
              final int nextAttempts = attempts + 1;
              if (options.observer() != null) {
                try {
                  options
                      .observer()
                      .onStatus(
                          snapshot.statusKind == null ? "" : snapshot.statusKind,
                          snapshot.payload == null ? Collections.emptyMap() : snapshot.payload,
                          nextAttempts);
                } catch (final RuntimeException observerError) {
                  result.completeExceptionally(observerError);
                  return;
                }
              }
              if (snapshot.view.localState == PrivacyActionLocalStateV1.TERMINAL) {
                result.complete(snapshot.view);
                return;
              }
              if ((options.maxAttempts() != null
                      && nextAttempts >= options.maxAttempts())
                  || (deadline != Long.MAX_VALUE
                      && System.currentTimeMillis() >= deadline)) {
                result.completeExceptionally(
                    new TransactionTimeoutException(
                        "Exact12 transaction "
                            + lowerHex(submitted.transactionHashBytes())
                            + " did not reach terminal state within the configured bounds",
                        lowerHex(submitted.transactionHashBytes()),
                        nextAttempts,
                        snapshot.payload));
                return;
              }
              final Runnable task =
                  () ->
                      pollPrivacyActionStatusV1(
                          submitted,
                          canonicalAuth,
                          options,
                          deadline,
                          nextAttempts,
                          snapshot.payload,
                          result);
              if (options.intervalMillis() <= 0L) {
                task.run();
              } else {
                CompletableFuture.runAsync(
                    task,
                    CompletableFuture.delayedExecutor(
                        options.intervalMillis(), TimeUnit.MILLISECONDS));
              }
            });
  }

  private CompletableFuture<Void> submitExactPrivacyActionWire(
      final byte[] wire,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final String transactionHashHex) {
    final TransportRequest request =
        buildExactNoritoPostRequest(
            "/v1/pipeline/transactions",
            wire,
            PRIVACY_SUBMIT_RESPONSE_MAX_BYTES,
            canonicalAuth);
    notifyRequest(request);
    final CompletableFuture<Void> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final AmbiguousTransactionSubmissionException error =
                    new AmbiguousTransactionSubmissionException(
                        transactionHashHex, null, unwrapCompletion(throwable));
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      transactionHashHex,
                      extractRejectCode(response));
              if (response.statusCode() == 202) {
                notifyResponse(request, clientResponse);
                future.complete(null);
                return;
              }
              final RuntimeException error =
                  submissionOutcomeIsAmbiguous(response.statusCode())
                      ? new AmbiguousTransactionSubmissionException(
                          transactionHashHex, response.statusCode(), null)
                      : new IllegalStateException(
                          "Exact12 transaction dispatch failed with status "
                              + response.statusCode());
              notifyFailure(request, error);
              future.completeExceptionally(error);
            });
    return future;
  }

  private static PrivacyActionOperationViewV1 withTerminalPrivacyStatusV1(
      final PrivacyActionOperationViewV1 operation,
      final PrivacyActionTerminalChainStateV1 terminal,
      final BigInteger height,
      final String rejection) {
    return withTerminalPrivacyStatusV1(
        operation, terminal, height, rejection, null, null, null, null);
  }

  private static PrivacyActionOperationViewV1 withTerminalPrivacyStatusV1(
      final PrivacyActionOperationViewV1 operation,
      final PrivacyActionTerminalChainStateV1 terminal,
      final BigInteger height,
      final String rejection,
      final byte[] executionCapabilityManifestDigest,
      final BigInteger executionCapabilityCommittedHeight,
      final BigInteger executionReceiptFinalizedHeight,
      final byte[] executionReceiptFinalizedBlockHash) {
    return new PrivacyActionOperationViewV1(
        operation.protocolId,
        operation.operationSchema,
        operation.transactionHashBytes(),
        operation.transactionIntentDigestBytes(),
        operation.statementDigestBytes(),
        operation.proofEnvelopeHashBytes(),
        PrivacyActionLocalStateV1.TERMINAL,
        terminal,
        height,
        rejection,
        operation.ledgerEffectKind,
        operation.capabilityManifestDigestBytes(),
        operation.capabilityCommittedHeight,
        executionCapabilityManifestDigest,
        executionCapabilityCommittedHeight,
        executionReceiptFinalizedHeight,
        executionReceiptFinalizedBlockHash);
  }

  private static void requireStableTerminalPrivacyView(
      final PrivacyActionOperationViewV1 previous,
      final PrivacyActionOperationViewV1 refreshed) {
    if (previous.localState == PrivacyActionLocalStateV1.TERMINAL
        && !previous.equals(refreshed)) {
      throw new IllegalStateException("terminal Exact12 action status changed");
    }
  }

  private static BigInteger privacyStatusBlockHeightV1(
      final Map<String, Object> payload) {
    final Object rawStatus = payload.get("status");
    if (!(rawStatus instanceof Map)) {
      throw new IllegalStateException("Exact12 pipeline status omitted its typed status");
    }
    final Object raw = ((Map<?, ?>) rawStatus).get("block_height");
    if (raw == null) {
      return null;
    }
    final BigInteger height;
    if (raw instanceof BigInteger) {
      height = (BigInteger) raw;
    } else if (raw instanceof Byte
        || raw instanceof Short
        || raw instanceof Integer
        || raw instanceof Long) {
      height = BigInteger.valueOf(((Number) raw).longValue());
    } else if (raw instanceof java.math.BigDecimal) {
      try {
        height = ((java.math.BigDecimal) raw).toBigIntegerExact();
      } catch (final ArithmeticException error) {
        throw new IllegalStateException(
            "Exact12 status block height is not an integer", error);
      }
    } else {
      throw new IllegalStateException(
          "Exact12 status block height has an unsupported numeric type");
    }
    if (height.signum() <= 0 || height.bitLength() > 64) {
      throw new IllegalStateException(
          "Exact12 status block height must be a positive u64");
    }
    return height;
  }

  private static String lowerHex(final byte[] value) {
    final StringBuilder output = new StringBuilder(value.length * 2);
    for (final byte current : value) {
      output.append(String.format(java.util.Locale.ROOT, "%02x", current & 0xff));
    }
    return output.toString();
  }

  private static void requireAutomaticCanonicalFreshness(
      final ToriiCanonicalRequestAuth canonicalAuth, final String context) {
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    if (canonicalAuth.timestampMs() != null || canonicalAuth.nonce() != null) {
      throw new IllegalArgumentException(
          context
              + " performs multiple authenticated requests and requires generated freshness");
    }
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
                      throwable instanceof CompletionException && throwable.getCause() != null
                          ? throwable.getCause()
                          : throwable;
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
                // Torii returns 202 while the transaction is still queued and
                // 200 once an authoritative terminal status is available.
                // Both responses carry the same validated status envelope.
                if (statusCode != 200
                    && statusCode != 202
                    && statusCode != 204
                    && statusCode != 404) {
                  future.completeExceptionally(
                      buildPipelineStatusHttpException(hashHex, clientResponse));
                  return;
                }

                final Map<String, Object> payload =
                    statusCode == 204 || statusCode == 404
                        ? null
                        : parsePipelineStatusPayload(clientResponse.body());
                final int nextAttempts = attemptsSoFar + 1;
                final String statusLiteral =
                    payload == null
                        ? null
                        : PipelineStatusExtractor.requireAuthoritativeStatus(payload, hashHex);
                final boolean isSuccess = "Applied".equals(statusLiteral);
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
                  future.complete(payload != null ? payload : Collections.emptyMap());
                  return;
                }
                if (isFailure) {
                  future.completeExceptionally(
                      new TransactionStatusException(hashHex, statusLiteral, payload));
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
      throw new IllegalStateException("Pipeline status response must not be empty");
    }
    if (body.length >= 4
        && body[0] == 'N'
        && body[1] == 'R'
        && body[2] == 'T'
        && body[3] == '0') {
      throw new IllegalStateException(
          "Pipeline status response violated the requested application/json contract");
    }
    final String json = new String(body, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Pipeline status response must not be empty");
    }
    final Object parsed = JsonParser.parse(json);
    if (parsed instanceof Map) {
      return PipelineStatusExtractor.normalizePublicStatus((Map<String, Object>) parsed);
    }
    throw new IllegalStateException("Pipeline status response must be a JSON object");
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
    return buildJsonGetRequest(path, queryParams, null);
  }

  private TransportRequest buildJsonGetRequest(
      final String path,
      final Map<String, String> queryParams,
      final Long maximumResponseBytes) {
    final URI target = appendQuery(resolvePath(path), queryParams);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    if (maximumResponseBytes != null) {
      builder.setMaximumResponseBytes(maximumResponseBytes);
    }
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildCanonicalJsonGetRequest(
      final String path,
      final Map<String, String> queryParams,
      final long maximumResponseBytes,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept")) {
        throw new IllegalArgumentException(
            "Accept must not be overridden for canonical JSON requests");
      }
    }
    requireCanonicalHeadersUnset();
    final URI target = appendQuery(resolvePath(path), queryParams);
    final Map<String, String> canonicalHeaders =
        buildCanonicalHeaders("GET", target, null, canonicalAuth);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", APPLICATION_JSON)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    for (final Map.Entry<String, String> entry : canonicalHeaders.entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    TransportSecurity.requireHttpRequestAllowed(
        "HttpClientTransport canonical JSON GET",
        config.baseUri(),
        target,
        canonicalHeaders,
        null);
    return builder.build();
  }

  private TransportRequest buildJsonPostRequest(final String path, final byte[] body) {
    return buildJsonPostRequest(path, body, null);
  }

  private TransportRequest buildExactJsonGetRequest(
      final String path, final long maximumResponseBytes) {
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept")) {
        throw new IllegalArgumentException(
            "Accept must not be overridden for exact JSON requests");
      }
    }
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod("GET")
            .addHeader("Accept", APPLICATION_JSON)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildExactOperatorJsonGetRequest(
      final String path, final long maximumResponseBytes) {
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept")) {
        throw new IllegalArgumentException(
            "Accept must not be overridden for exact JSON requests");
      }
    }
    OperatorRequestSigner.requireGeneratedAuth(config.defaultHeaders());
    final URI target = resolvePath(path);
    final Map<String, String> operatorHeaders =
        OperatorRequestSigner.buildHeaders(
            config.requireOperatorSigningContext(), "GET", target, new byte[0]);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", APPLICATION_JSON)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    for (final Map.Entry<String, String> entry : operatorHeaders.entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    TransportSecurity.requireHttpRequestAllowed(
        "HttpClientTransport operator GET",
        config.baseUri(),
        target,
        operatorHeaders,
        null);
    return builder.build();
  }

  private TransportRequest buildExactNoritoGetRequest(
      final String path, final long maximumResponseBytes) {
    return buildExactNoritoGetRequest(path, maximumResponseBytes, null);
  }

  private TransportRequest buildExactNoritoGetRequest(
      final String path,
      final long maximumResponseBytes,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept")) {
        throw new IllegalArgumentException(
            "Accept must not be overridden for exact Norito requests");
      }
    }
    if (canonicalAuth != null) {
      requireCanonicalHeadersUnset();
    }
    final URI target = resolvePath(path);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .addHeader("Accept", APPLICATION_NORITO)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    if (canonicalAuth != null) {
      final Map<String, String> canonicalHeaders =
          buildCanonicalHeaders("GET", target, null, canonicalAuth);
      for (final Map.Entry<String, String> entry : canonicalHeaders.entrySet()) {
        builder.addHeader(entry.getKey(), entry.getValue());
      }
      TransportSecurity.requireHttpRequestAllowed(
          "HttpClientTransport", config.baseUri(), target, canonicalHeaders, null);
    }
    return builder.build();
  }

  private TransportRequest buildExactSignedQueryPostRequest(
      final String path, final byte[] body, final long maximumResponseBytes) {
    if (body == null || body.length == 0) {
      throw new IllegalArgumentException("signed query POST body must not be empty");
    }
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept") || name.equalsIgnoreCase("Content-Type")) {
        throw new IllegalArgumentException(
            "Accept and Content-Type must not be overridden for exact signed queries");
      }
    }
    requireCanonicalHeadersUnset();
    final URI target = resolvePath(path);
    TransportSecurity.requireAuthenticatedHttpRequestAllowed(
        "HttpClientTransport authenticated signed query", config.baseUri(), target);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .addHeader("Accept", APPLICATION_NORITO)
            .addHeader("Content-Type", APPLICATION_NORITO)
            .setBody(body)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildExactNoritoPostRequest(
      final String path,
      final byte[] body,
      final long maximumResponseBytes,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    if (body == null || body.length == 0) {
      throw new IllegalArgumentException("exact Norito POST body must not be empty");
    }
    for (final String name : config.defaultHeaders().keySet()) {
      if (name.equalsIgnoreCase("Accept") || name.equalsIgnoreCase("Content-Type")) {
        throw new IllegalArgumentException(
            "Accept and Content-Type must not be overridden for exact Norito requests");
      }
    }
    requireCanonicalHeadersUnset();
    final URI target = resolvePath(path);
    final Map<String, String> canonicalHeaders =
        buildCanonicalHeaders("POST", target, body, canonicalAuth);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .addHeader("Accept", APPLICATION_NORITO)
            .addHeader("Content-Type", APPLICATION_NORITO)
            .setBody(body)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes))
            .setTimeout(config.requestTimeout());
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    for (final Map.Entry<String, String> entry : canonicalHeaders.entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    TransportSecurity.requireHttpRequestAllowed(
        "HttpClientTransport exact Norito POST",
        config.baseUri(),
        target,
        canonicalHeaders,
        body);
    return builder.build();
  }

  private void requireCanonicalHeadersUnset() {
    for (final String candidate : config.defaultHeaders().keySet()) {
      if (candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_ACCOUNT)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_SIGNATURE)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_TIMESTAMP_MS)
          || candidate.equalsIgnoreCase(CanonicalRequestSigner.HEADER_NONCE)) {
        throw new IllegalArgumentException(
            "canonical request headers must be supplied only through canonicalAuth");
      }
    }
  }

  private TransportRequest buildJsonPostRequest(
      final String path, final byte[] body, final Long maximumResponseBytes) {
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod("POST")
            .setBody(Objects.requireNonNull(body, "body"))
            .addHeader("Content-Type", "application/json")
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    if (maximumResponseBytes != null) {
      builder.setMaximumResponseBytes(maximumResponseBytes);
    }
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    return builder.build();
  }

  private TransportRequest buildBridgeJsonPostRequest(final String path, final byte[] body) {
    preflightSccpBridgeSubmitJson(body, path);
    return buildJsonPostRequest(path, body, SCCP_JSON_RESPONSE_MAX_BYTES);
  }

  private TransportRequest buildVpnRequest(
      final String method,
      final String path,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    if (path.startsWith("/v1/vpn/")) {
      requireSecureVpnBaseUri();
    }
    Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    requireCanonicalHeadersUnset();
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
    final Map<String, String> canonicalHeaders =
        buildCanonicalHeaders(method, target, body, canonicalAuth);
    for (final Map.Entry<String, String> entry : canonicalHeaders.entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    TransportSecurity.requireHttpRequestAllowed(
        "HttpClientTransport", config.baseUri(), target, canonicalHeaders, body);
    return builder.build();
  }

  private void requireSecureVpnBaseUri() {
    if (!"https".equalsIgnoreCase(config.baseUri().getScheme())) {
      throw new IllegalArgumentException("Sora VPN requests require an HTTPS Torii base URI");
    }
  }

  private TransportRequest buildOnboardingRequest(
      final String method,
      final String path,
      final byte[] body,
      final String onboardingToken) {
    final String token = requireOnboardingCredential(onboardingToken);
    for (final String key : config.defaultHeaders().keySet()) {
      if (ONBOARDING_TOKEN_HEADER.equalsIgnoreCase(key)) {
        throw new IllegalArgumentException(
            ONBOARDING_TOKEN_HEADER
                + " must be supplied only through the sponsored onboarding API");
      }
    }
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(resolvePath(path))
            .setMethod(method)
            .addHeader("Accept", "application/json")
            .setTimeout(config.requestTimeout());
    if (body != null) {
      builder.setBody(body).addHeader("Content-Type", "application/json");
    }
    for (final Map.Entry<String, String> entry : config.defaultHeaders().entrySet()) {
      builder.addHeader(entry.getKey(), entry.getValue());
    }
    builder.addHeader(ONBOARDING_TOKEN_HEADER, token);
    return builder.build();
  }

  private static String requireOnboardingCredential(final String value) {
    if (value == null || value.length() < 32 || value.length() > 256) {
      throw new IllegalArgumentException(
          "onboarding token must contain 32..256 printable non-whitespace ASCII bytes");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '!' || character > '~') {
        throw new IllegalArgumentException(
            "onboarding token must contain 32..256 printable non-whitespace ASCII bytes");
      }
    }
    return value;
  }

  private Map<String, String> buildCanonicalHeaders(
      final String method,
      final URI target,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    final NetworkId networkId = config.requireLocalSigningContext().networkId();
    final Long timestampMs = canonicalAuth.timestampMs();
    final String nonce = canonicalAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    if (timestampMs == null) {
      return CanonicalRequestSigner.buildHeaders(
          networkId, method, target, body, canonicalAuth);
    }
    return CanonicalRequestSigner.buildHeaders(
        networkId, method, target, body, canonicalAuth, timestampMs, nonce);
  }

  private URI resolvePath(final String path) {
    if (path == null || path.trim().isEmpty()) {
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
    final String targetText = target.toString();
    final int rawFragmentIndex = targetText.indexOf('#');
    final int fragmentIndex = rawFragmentIndex >= 0 ? rawFragmentIndex : targetText.length();
    final StringBuilder builder = new StringBuilder(targetText.length() + 1);
    builder.append(targetText, 0, fragmentIndex);
    builder.append(builder.indexOf("?") >= 0 ? "&" : "?");
    builder.append(encodeQuery(params));
    builder.append(targetText, fragmentIndex, targetText.length());
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

  private static long saturatedDeadline(final Long timeoutMillis) {
    if (timeoutMillis == null) {
      return Long.MAX_VALUE;
    }
    final long now = System.currentTimeMillis();
    final long delay = Math.max(0L, timeoutMillis);
    try {
      return Math.addExact(now, delay);
    } catch (final ArithmeticException ignored) {
      return Long.MAX_VALUE;
    }
  }

  private static Throwable unwrapCompletion(final Throwable throwable) {
    Throwable current = throwable;
    while (current instanceof CompletionException && current.getCause() != null) {
      current = current.getCause();
    }
    return current;
  }

  private CompletableFuture<Void> ensureTransactionSubmissionCompatibility() {
    final TransportRequest request =
        buildJsonGetRequest(
            "/v1/node/capabilities",
            Collections.emptyMap(),
            NODE_CAPABILITIES_RESPONSE_MAX_BYTES);
    return fetchJson(
            request,
            payload -> {
              ToriiTransactionCompatibility.requireCompatible(payload);
              return Boolean.TRUE;
            },
            "transaction submission compatibility",
            200)
        .handle(
            (ignored, throwable) -> {
              if (throwable != null) {
                final Throwable cause = unwrapCompletion(throwable);
                if (cause instanceof ToriiTransactionCompatibilityException) {
                  throw new CompletionException(cause);
                }
                throw new CompletionException(
                    new ToriiTransactionCompatibilityProbeException(cause));
              }
              return null;
            });
  }

  private <T> CompletableFuture<T> fetchJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    return fetchJson(request, parser, errorContext, null, null);
  }

  private <T> CompletableFuture<T> fetchJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext,
      final Integer acceptedStatus) {
    return fetchJson(request, parser, errorContext, acceptedStatus, null);
  }

  private <T> CompletableFuture<T> fetchJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext,
      final Integer acceptedStatus,
      final BiFunction<T, Integer, T> responseValidator) {
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
              final boolean statusAccepted =
                  acceptedStatus == null
                      ? response.statusCode() >= 200 && response.statusCode() < 300
                      : response.statusCode() == acceptedStatus;
              if (!statusAccepted) {
                final RuntimeException error =
                    new RuntimeException(
                        errorContext + " request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.apply(response.body());
                final T validated =
                    responseValidator == null
                        ? parsed
                        : responseValidator.apply(parsed, response.statusCode());
                notifyResponse(request, clientResponse);
                future.complete(validated);
              } catch (final RuntimeException ex) {
                notifyFailure(request, ex);
                future.completeExceptionally(ex);
              }
            });
    return future;
  }

  private <T> CompletableFuture<T> fetchSccpJson(
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
              try {
                requireExactSccpJsonResponse(response, errorContext);
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

  private <T> CompletableFuture<T> fetchExactJson(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    final Long maximumResponseBytes =
        Objects.requireNonNull(
            request.maximumResponseBytes(),
            errorContext + " request must declare a response-body limit");
    notifyRequest(request);
    final CompletableFuture<T> future = new CompletableFuture<>();
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
              final byte[] body = response.body();
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      body,
                      response.message(),
                      null,
                      extractRejectCode(response));
              try {
                requireExactJsonResponse(
                    response, body, maximumResponseBytes.longValue(), errorContext);
                final T parsed = parser.apply(body);
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (final RuntimeException error) {
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private CompletableFuture<byte[]> fetchExactNoritoBytes(
      final TransportRequest request, final String errorContext) {
    notifyRequest(request);
    final CompletableFuture<byte[]> future = new CompletableFuture<>();
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
              final byte[] body = response.body();
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      body,
                      response.message(),
                      null,
                      extractRejectCode(response));
              try {
                if (response.statusCode() != 200) {
                  throw new IllegalStateException(
                      errorContext + " request failed with status " + response.statusCode());
                }
                requireExactHeader(
                    response.headers(),
                    "Content-Type",
                    APPLICATION_NORITO,
                    errorContext);
                if (body.length == 0) {
                  throw new IllegalStateException(errorContext + " response must not be empty");
                }
                final Long maximumResponseBytes = request.maximumResponseBytes();
                if (maximumResponseBytes == null) {
                  throw new IllegalStateException(
                      errorContext + " request must declare a response-body limit");
                }
                if ((long) body.length > maximumResponseBytes.longValue()) {
                  throw new IllegalStateException(
                      errorContext
                          + " response exceeds "
                          + maximumResponseBytes
                          + " bytes");
                }
                requireExactOptionalContentLength(response.headers(), body.length, errorContext);
                notifyResponse(request, clientResponse);
                future.complete(body.clone());
              } catch (final RuntimeException error) {
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  /** Executes an exact Norito request where, and only where, HTTP 404 means typed absence. */
  CompletableFuture<byte[]> fetchExactNoritoBytesAllowingNotFound(
      final TransportRequest request, final String errorContext) {
    notifyRequest(request);
    final CompletableFuture<byte[]> future = new CompletableFuture<>();
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
              final byte[] body = response.body();
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      body,
                      response.message(),
                      null,
                      extractRejectCode(response));
              try {
                if (response.statusCode() == 404) {
                  notifyResponse(request, clientResponse);
                  future.complete(null);
                  return;
                }
                if (response.statusCode() != 200) {
                  throw new IllegalStateException(
                      errorContext + " request failed with status " + response.statusCode());
                }
                requireExactHeader(
                    response.headers(), "Content-Type", APPLICATION_NORITO, errorContext);
                if (body.length == 0) {
                  throw new IllegalStateException(errorContext + " response must not be empty");
                }
                final Long maximumResponseBytes = request.maximumResponseBytes();
                if (maximumResponseBytes == null) {
                  throw new IllegalStateException(
                      errorContext + " request must declare a response-body limit");
                }
                if ((long) body.length > maximumResponseBytes.longValue()) {
                  throw new IllegalStateException(
                      errorContext + " response exceeds " + maximumResponseBytes + " bytes");
                }
                requireExactOptionalContentLength(response.headers(), body.length, errorContext);
                notifyResponse(request, clientResponse);
                future.complete(body.clone());
              } catch (final RuntimeException error) {
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private static void requireExactHeader(
      final Map<String, List<String>> headers,
      final String name,
      final String expected,
      final String errorContext) {
    final List<String> values = headerValues(headers, name);
    if (values.size() != 1 || !expected.equals(values.get(0))) {
      throw new IllegalStateException(
          errorContext + " response " + name + " must be exactly " + expected);
    }
  }

  private static void requireExactOptionalContentLength(
      final Map<String, List<String>> headers,
      final int actualBytes,
      final String errorContext) {
    final List<String> values = new ArrayList<>();
    boolean matchingHeaderPresent = false;
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      if (entry.getKey() != null && entry.getKey().equalsIgnoreCase("Content-Length")) {
        matchingHeaderPresent = true;
        if (entry.getValue() != null) {
          values.addAll(entry.getValue());
        }
      }
    }
    if (!matchingHeaderPresent) {
      return;
    }
    if (values.size() != 1) {
      throw new IllegalStateException(errorContext + " response has ambiguous Content-Length");
    }
    final String value = values.get(0);
    if (!isCanonicalUnsignedDecimal(value)) {
      throw new IllegalStateException(
          errorContext + " response Content-Length must be one canonical decimal integer");
    }
    if (!Integer.toString(actualBytes).equals(value)) {
      throw new IllegalStateException(
          errorContext + " response Content-Length does not match the body");
    }
  }

  private static List<String> headerValues(
      final Map<String, List<String>> headers, final String name) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      if (entry.getKey() != null
          && entry.getKey().equalsIgnoreCase(name)
          && entry.getValue() != null) {
        values.addAll(entry.getValue());
      }
    }
    return values;
  }

  private static boolean isCanonicalUnsignedDecimal(final String value) {
    if (value == null || value.isEmpty()) {
      return false;
    }
    if (value.equals("0")) {
      return true;
    }
    if (value.charAt(0) < '1' || value.charAt(0) > '9') {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < '0' || character > '9') {
        return false;
      }
    }
    return true;
  }

  private static void requireExactJsonResponse(
      final TransportResponse response,
      final byte[] body,
      final long maximumResponseBytes,
      final String errorContext) {
    if (response.statusCode() != 200) {
      throw new IllegalStateException(
          errorContext + " request failed with status " + response.statusCode());
    }
    requireExactHeader(response.headers(), "Content-Type", APPLICATION_JSON, errorContext);
    if (body.length == 0) {
      throw new IllegalStateException(errorContext + " response must not be empty");
    }
    if ((long) body.length > maximumResponseBytes) {
      throw new IllegalStateException(
          errorContext + " response exceeds " + maximumResponseBytes + " bytes");
    }
    requireExactOptionalContentLength(response.headers(), body.length, errorContext);
  }

  private static void requireExactSccpJsonResponse(
      final TransportResponse response, final String errorContext) {
    if (response.statusCode() != 200) {
      throw new RuntimeException(
          errorContext + " request failed with status " + response.statusCode());
    }
    final List<String> contentTypes = new ArrayList<>();
    for (final Map.Entry<String, List<String>> entry : response.headers().entrySet()) {
      if (entry.getKey() != null
          && entry.getKey().equalsIgnoreCase("Content-Type")
          && entry.getValue() != null) {
        contentTypes.addAll(entry.getValue());
      }
    }
    if (contentTypes.size() != 1 || !"application/json".equals(contentTypes.get(0))) {
      throw new RuntimeException(
          errorContext + " response Content-Type must be exactly application/json");
    }
  }

  private <T> CompletableFuture<Optional<T>> fetchJsonAllowingNotFound(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext) {
    return fetchJsonAllowingNotFound(request, parser, errorContext, null);
  }

  private <T> CompletableFuture<Optional<T>> fetchJsonAllowingNotFound(
      final TransportRequest request,
      final Function<byte[], T> parser,
      final String errorContext,
      final Integer acceptedStatus) {
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
              final boolean statusAccepted =
                  acceptedStatus == null
                      ? response.statusCode() >= 200 && response.statusCode() < 300
                      : response.statusCode() == acceptedStatus;
              if (!statusAccepted) {
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

  private static Map<String, Object> objectMapOf(final String key, final Object value) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put(key, value);
    return Collections.unmodifiableMap(map);
  }

  private static Map<String, Object> objectMapOf(
      final String key1,
      final Object value1,
      final String key2,
      final Object value2) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put(key1, value1);
    map.put(key2, value2);
    return Collections.unmodifiableMap(map);
  }

  private static Map<String, Object> objectMapOf(
      final String key1,
      final Object value1,
      final String key2,
      final Object value2,
      final String key3,
      final Object value3) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put(key1, value1);
    map.put(key2, value2);
    map.put(key3, value3);
    return Collections.unmodifiableMap(map);
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
    payload.put(
        "metering_public_key_hex",
        normalizeEd25519PublicKeyHex(meteringPublicKeyHex, "meteringPublicKeyHex"));
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
    payload.put(
        "metering_public_key_hex",
        normalizeEd25519PublicKeyHex(meteringPublicKeyHex, "meteringPublicKeyHex"));
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

  /** Builds the secret-free request used to prepare a contract-call draft. */
  static Map<String, Object> buildContractCallDraftPayload(
      final String authority,
      final FeePaymentIntent feePayment,
      final String contractAddress,
      final String contractAlias,
      final String entrypoint,
      final Object payloadValue) {
    if (feePayment == null || feePayment.gasLimit() == null) {
      throw new IllegalArgumentException("contract feePayment must include gasLimit");
    }
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("authority", normalizeNonBlank(authority, "authority"));
    payload.putAll(buildContractTargetSelector(contractAddress, contractAlias));
    payload.put("entrypoint", normalizeNonBlank(entrypoint, "entrypoint"));
    if (payloadValue != null) {
      payload.put("payload", payloadValue);
    }
    payload.put("fee_payment", feePayment.toJsonMap());
    return payload;
  }

  /** Validates that Torii returned a secret-free draft bound to the requested call. */
  static ContractCallResponse validateContractCallDraft(
      final ContractCallResponse response, final Map<String, Object> request) {
    if (!response.ok()) {
      throw new IllegalStateException("contract call draft.ok must be true");
    }
    if (response.submitted()) {
      throw new IllegalStateException("contract call draft must not be submitted");
    }
    if (response.txHashHex() != null || response.pipelineStatus() != null) {
      throw new IllegalStateException("contract call draft must not contain submission state");
    }
    if (!Objects.equals(response.entrypoint(), request.get("entrypoint"))) {
      throw new IllegalStateException(
          "contract call draft entrypoint is not bound to the request");
    }
    final ContractOperationReceipt receipt = response.operationReceipt();
    if (!"contract_call".equals(receipt.operationKind())
        || !"pending_signature".equals(receipt.status())) {
      throw new IllegalStateException(
          "contract call draft receipt must be pending_signature");
    }
    if (!Objects.equals(receipt.entrypoint(), response.entrypoint())
        || receipt.txHashHex() != null) {
      throw new IllegalStateException("contract call draft receipt is inconsistent");
    }
    if (response.transactionPayloadB64() == null) {
      throw new IllegalStateException(
          "contract call draft must contain one exact canonical transaction payload");
    }
    if (response.signingMessageB64() == null) {
      throw new IllegalStateException("contract call draft must contain a signing message");
    }
    if (Base64.getDecoder().decode(response.signingMessageB64()).length != 32) {
      throw new IllegalStateException("contract call draft signing message must be 32 bytes");
    }
    if (response.entrypointHashHex() != null || receipt.entrypointHashHex() != null) {
      throw new IllegalStateException(
          "contract call draft must not claim a final entrypoint hash");
    }
    final Object expectedAddress = request.get("contract_address");
    if (expectedAddress != null
        && (!expectedAddress.equals(response.contractAddress())
            || !expectedAddress.equals(receipt.contractAddress()))) {
      throw new IllegalStateException("contract call draft address is not bound to the request");
    }
    final Object expectedAlias = request.get("contract_alias");
    if (expectedAlias != null && !expectedAlias.equals(receipt.contractAlias())) {
      throw new IllegalStateException("contract call draft alias is not bound to the request");
    }
    return response;
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
      payload.put(
          "public_key_hex",
          normalizeEd25519PublicKeyHex(request.publicKeyHex(), "publicKeyHex"));
    }
    if (request.signatureB64() != null) {
      payload.put(
          "signature_b64",
          normalizeRequiredExactBase64Payload(request.signatureB64(), "signatureB64"));
    }
    if (request.creationTimeMs() != null) {
      if (request.creationTimeMs().longValue() < 0L) {
        throw new IllegalArgumentException("creationTimeMs must be non-negative");
      }
      payload.put("creation_time_ms", request.creationTimeMs());
    }
    payload.put("fee_payment", request.feePayment().toJsonMap());
    if (request.memo() != null) {
      payload.put("memo", normalizeNonBlank(request.memo(), "memo"));
    }
    putValidationFeePolicyMetadata(
        payload,
        request.validationFeePolicyVersion(),
        request.validationFeePolicyHash(),
        request.validationFeeInstructionIndex(),
        request.validationFeeTransferEntryIndex());
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

  static void putValidationFeePolicyMetadata(
      final Map<String, Object> payload,
      final Long validationFeePolicyVersion,
      final String validationFeePolicyHash,
      final Long validationFeeInstructionIndex,
      final Long validationFeeTransferEntryIndex) {
    final boolean hasPolicyVersion = validationFeePolicyVersion != null;
    final boolean hasPolicyHash = validationFeePolicyHash != null;
    final boolean hasInstructionIndex = validationFeeInstructionIndex != null;
    final boolean hasTransferEntryIndex = validationFeeTransferEntryIndex != null;
    if (hasPolicyVersion != hasPolicyHash) {
      throw new IllegalArgumentException(
          "validationFeePolicyVersion and validationFeePolicyHash must be provided together");
    }
    if (!hasPolicyVersion && hasInstructionIndex) {
      throw new IllegalArgumentException(
          "validationFeeInstructionIndex requires validation fee policy metadata");
    }
    if (!hasPolicyVersion && hasTransferEntryIndex) {
      throw new IllegalArgumentException(
          "validationFeeTransferEntryIndex requires validation fee policy metadata");
    }
    if (hasTransferEntryIndex && !hasInstructionIndex) {
      throw new IllegalArgumentException(
          "validationFeeTransferEntryIndex requires validationFeeInstructionIndex");
    }
    if (!hasPolicyVersion) {
      return;
    }
    if (validationFeePolicyVersion.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeePolicyVersion must be non-negative");
    }
    if (hasInstructionIndex && validationFeeInstructionIndex.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeeInstructionIndex must be non-negative");
    }
    if (hasTransferEntryIndex && validationFeeTransferEntryIndex.longValue() < 0L) {
      throw new IllegalArgumentException("validationFeeTransferEntryIndex must be non-negative");
    }
    payload.put("validation_fee_policy_version", validationFeePolicyVersion.toString());
    payload.put(
        "validation_fee_policy_hash",
        normalizeHex32(validationFeePolicyHash, "validationFeePolicyHash"));
    if (hasInstructionIndex) {
      payload.put("validation_fee_instruction_index", validationFeeInstructionIndex.toString());
    }
    if (hasTransferEntryIndex) {
      payload.put("validation_fee_transfer_entry_index", validationFeeTransferEntryIndex.toString());
    }
  }

  static Map<String, Object> buildVerifyingKeyRegisterPayload(
      final VerifyingKeyRegisterRequest request) {
    Objects.requireNonNull(request, "request");
    final String backend =
        VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(request.backend(), "backend");
    final VerifyingKeyPayload vkPayload =
        normalizeVerifierBytes(request.verifyingKeyBytes(), request.verifyingKeyLength());
    final String commitmentHex = normalizeOptionalHex32(request.commitmentHex(), "commitmentHex");
    validateVerifyingKeyMaterial(vkPayload, commitmentHex);
    validateInlineVerifyingKeyCommitment(
        backend, vkPayload == null ? null : vkPayload.bytes(), commitmentHex);
    validateVerifyingKeyHeightRange(request.activationHeight(), request.withdrawHeight());

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("authority", normalizeVerifyingKeyAuthority(request.authority()));
    payload.put("backend", backend);
    payload.put("name", normalizeVerifyingKeyName(request.name()));
    payload.put("version", normalizePositiveU32(request.version(), "version"));
    payload.put("circuit_id", normalizeNonBlank(request.circuitId(), "circuitId"));
    payload.put(
        "public_inputs_schema_hash_hex",
        normalizeHex32(request.publicInputsSchemaHashHex(), "publicInputsSchemaHashHex"));
    payload.put("gas_schedule_id", normalizeNonBlank(request.gasScheduleId(), "gasScheduleId"));
    putOptionalVerifierFields(
        payload,
        request.curve(),
        request.maxProofBytes(),
        request.metadataUriCid(),
        request.verifyingKeyBytesCid(),
        request.activationHeight(),
        request.withdrawHeight(),
        commitmentHex,
        vkPayload,
        request.status());
    return payload;
  }

  static Map<String, Object> buildVerifyingKeyUpdatePayload(
      final VerifyingKeyUpdateRequest request) {
    Objects.requireNonNull(request, "request");
    final String backend =
        VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(request.backend(), "backend");
    final VerifyingKeyPayload vkPayload =
        normalizeVerifierBytes(request.verifyingKeyBytes(), request.verifyingKeyLength());
    final String commitmentHex = normalizeOptionalHex32(request.commitmentHex(), "commitmentHex");
    validateVerifyingKeyMaterial(vkPayload, commitmentHex);
    validateInlineVerifyingKeyCommitment(
        backend, vkPayload == null ? null : vkPayload.bytes(), commitmentHex);
    validateVerifyingKeyHeightRange(request.activationHeight(), request.withdrawHeight());

    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("authority", normalizeVerifyingKeyAuthority(request.authority()));
    payload.put("backend", backend);
    payload.put("name", normalizeVerifyingKeyName(request.name()));
    payload.put("version", normalizePositiveU32(request.version(), "version"));
    payload.put("circuit_id", normalizeNonBlank(request.circuitId(), "circuitId"));
    payload.put(
        "public_inputs_schema_hash_hex",
        normalizeHex32(request.publicInputsSchemaHashHex(), "publicInputsSchemaHashHex"));
    if (request.gasScheduleId() != null) {
      payload.put("gas_schedule_id", normalizeNonBlank(request.gasScheduleId(), "gasScheduleId"));
    }
    putOptionalVerifierFields(
        payload,
        request.curve(),
        request.maxProofBytes(),
        request.metadataUriCid(),
        request.verifyingKeyBytesCid(),
        request.activationHeight(),
        request.withdrawHeight(),
        commitmentHex,
        vkPayload,
        request.status());
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

  static String normalizeRequiredExactBase64Payload(final String value, final String field) {
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact standard-base64");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(field + " must not decode to empty bytes");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be exact standard-base64");
    }
    return value;
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

  static String normalizeVerifyingKeyName(final String value) {
    final String normalized = normalizeNonBlank(value, "name");
    if (normalized.indexOf(':') >= 0) {
      throw new IllegalArgumentException("name must not contain ':' characters");
    }
    return normalized;
  }

  static String normalizeVerifyingKeyAuthority(final String value) {
    final String normalized = normalizeNonBlank(value, "authority");
    final Integer discriminant = AccountAddress.detectI105Discriminant(normalized);
    if (discriminant == null) {
      throw new IllegalArgumentException(
          "authority must be a canonical I105 account literal");
    }
    try {
      final AccountAddress address = AccountAddress.fromI105(normalized, discriminant);
      if (!normalized.equals(address.toI105(discriminant))) {
        throw new IllegalArgumentException(
            "authority must be a canonical I105 account literal");
      }
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalArgumentException(
          "authority must be a canonical I105 account literal", ex);
    }
    return normalized;
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

  static String normalizeExactEvenLengthHex(final String value, final String field) {
    if (!Objects.requireNonNull(value, field + " must not be null").trim().equals(value)) {
      throw new IllegalArgumentException(field + " must be a canonical hex string");
    }
    return normalizeEvenLengthHex(value, field);
  }

  static String normalizeNonZeroEvenLengthHex(final String value, final String field) {
    return normalizeNonZeroEvenLengthHex(value, field, -1);
  }

  static String normalizeNonZeroEvenLengthHex(
      final String value, final String field, final int expectedByteLength) {
    final String normalized = normalizeEvenLengthHex(value, field);
    for (int i = 0; i < normalized.length(); i++) {
      if (normalized.charAt(i) != '0') {
        if (expectedByteLength >= 0 && normalized.length() != expectedByteLength * 2) {
          throw new IllegalArgumentException(
              field + " must be a " + expectedByteLength + "-byte hex string");
        }
        return normalized;
      }
    }
    throw new IllegalArgumentException(field + " must not be all zero");
  }

  static String normalizeExactNonZeroEvenLengthHex(
      final String value, final String field, final int expectedByteLength) {
    final String normalized = normalizeExactEvenLengthHex(value, field);
    for (int i = 0; i < normalized.length(); i++) {
      if (normalized.charAt(i) != '0') {
        if (expectedByteLength >= 0 && normalized.length() != expectedByteLength * 2) {
          throw new IllegalArgumentException(
              field + " must be a " + expectedByteLength + "-byte hex string");
        }
        return normalized;
      }
    }
    throw new IllegalArgumentException(field + " must not be all zero");
  }

  static String normalizeHexBytes(
      final String value, final String field, final int expectedByteLength) {
    final String normalized = normalizeEvenLengthHex(value, field);
    if (normalized.length() != expectedByteLength * 2) {
      throw new IllegalArgumentException(
          field + " must be a " + expectedByteLength + "-byte hex string");
    }
    return normalized;
  }

  static String normalizeTronBase58CheckAddress(final String value, final String field) {
    final String normalized = normalizeNonBlank(value, field);
    if (!normalized.equals(value)) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    final byte[] decoded = decodeBase58(normalized, field);
    if (decoded.length != 25) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    final byte[] payload = Arrays.copyOfRange(decoded, 0, 21);
    final byte[] checksum = Arrays.copyOfRange(decoded, 21, 25);
    final byte[] expectedChecksum = Arrays.copyOfRange(sha256(sha256(payload)), 0, 4);
    if (!Arrays.equals(checksum, expectedChecksum)) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    boolean nonZeroAddress = false;
    for (int i = 1; i < payload.length; i++) {
      if (payload[i] != 0) {
        nonZeroAddress = true;
        break;
      }
    }
    if ((payload[0] & 0xff) != 0x41 || !nonZeroAddress) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    return normalized;
  }

  private static byte[] decodeBase58(final String value, final String field) {
    final ArrayList<Integer> decoded = new ArrayList<>();
    for (int i = 0; i < value.length(); i++) {
      final int digit = TRON_BASE58_ALPHABET.indexOf(value.charAt(i));
      if (digit < 0) {
        throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
      }
      int carry = digit;
      for (int outputIndex = decoded.size() - 1; outputIndex >= 0; outputIndex--) {
        final int next = decoded.get(outputIndex) * 58 + carry;
        decoded.set(outputIndex, next & 0xff);
        carry = next >>> 8;
      }
      while (carry > 0) {
        decoded.add(0, carry & 0xff);
        carry >>>= 8;
      }
    }
    int leadingZeroes = 0;
    while (leadingZeroes < value.length() && value.charAt(leadingZeroes) == '1') {
      leadingZeroes++;
    }
    final byte[] result = new byte[leadingZeroes + decoded.size()];
    for (int i = 0; i < decoded.size(); i++) {
      result[leadingZeroes + i] = (byte) (int) decoded.get(i);
    }
    return result;
  }

  private static byte[] sha256(final byte[] value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value);
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  static void preflightSccpBridgeSubmitJson(final byte[] body, final String path) {
    final byte[] exactBody = Objects.requireNonNull(body, "body");
    final String bodyText = new String(exactBody, StandardCharsets.UTF_8);
    if (!java.util.Arrays.equals(exactBody, bodyText.getBytes(StandardCharsets.UTF_8))) {
      throw new IllegalArgumentException("SCCP bridge submit payload must be UTF-8 JSON");
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(bodyText);
    } catch (final RuntimeException ex) {
      throw new IllegalArgumentException("bridge submit payload must be valid JSON", ex);
    }
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("bridge submit payload must be a JSON object");
    }
    final Map<?, ?> fields = (Map<?, ?>) parsed;
    final java.util.Set<String> allowed;
    if ("/v1/bridge/proofs/submit".equals(path)) {
      allowed = SCCP_PROOF_SUBMIT_FIELDS;
    } else if ("/v1/bridge/messages".equals(path)) {
      allowed = SCCP_MESSAGE_SUBMIT_FIELDS;
    } else {
      throw new IllegalArgumentException("unsupported SCCP bridge submit path");
    }
    for (final Object key : fields.keySet()) {
      if (!(key instanceof String) || !allowed.contains((String) key)) {
        throw new IllegalArgumentException("unknown or retired bridge submit field `" + key + "`");
      }
    }
    if (!(fields.get("authority") instanceof String)) {
      throw new IllegalArgumentException("authority is required and must be canonical");
    }
    SccpSubmitEncoding.requireCanonicalAuthority((String) fields.get("authority"), "authority");
    final FeePaymentIntent feePayment =
        FeePaymentJson.parse(fields.get("fee_payment"), "bridge submit payload.fee_payment");
    final boolean hasSignature = fields.containsKey("signature_b64");
    final Object signature = fields.get("signature_b64");
    if (hasSignature) {
      if (!(signature instanceof String)) {
        throw new IllegalArgumentException("signature_b64 must be canonical base64");
      }
      SccpSubmitEncoding.normalizeOptionalSignature((String) signature);
    }
    final boolean hasTransactionPayload = fields.containsKey("transaction_payload_b64");
    final Object transactionPayload = fields.get("transaction_payload_b64");
    if (hasTransactionPayload && !(transactionPayload instanceof String)) {
      throw new IllegalArgumentException(
          "transaction_payload_b64 must be canonical padded base64");
    }
    final Object creationTime = fields.get("creation_time_ms");
    Long normalizedCreationTime = null;
    if (fields.containsKey("creation_time_ms")) {
      if (!(creationTime instanceof Number)) {
        throw new IllegalArgumentException("creation_time_ms must be a positive integer");
      }
      final Number number = (Number) creationTime;
      final long value = number.longValue();
      if (value <= 0 || !number.toString().equals(Long.toString(value))) {
        throw new IllegalArgumentException("creation_time_ms must be a positive integer");
      }
      normalizedCreationTime = value;
    }
    SccpSubmitEncoding.validateDetachedSigningState(
        signature instanceof String ? (String) signature : null,
        transactionPayload instanceof String ? (String) transactionPayload : null,
        normalizedCreationTime);
    if (transactionPayload instanceof String) {
      SccpSubmitEncoding.normalizeOptionalTransactionPayload(
          (String) transactionPayload,
          normalizedCreationTime,
          (String) fields.get("authority"),
          feePayment);
    }
    if ("/v1/bridge/messages".equals(path)) {
      final String nativeProof = requiredSccpArtifact(fields, "native_proof_b64");
      SccpSubmitEncoding.validateCanonicalNoritoBase64(
          nativeProof,
          "native_proof_b64",
          SccpSubmitEncoding.MAX_NATIVE_PROOF_BYTES,
          SccpSubmitEncoding.NATIVE_INBOUND_PROOF_SCHEMA_NAME);
      return;
    }
    final String destinationProof = requiredSccpArtifact(fields, "destination_proof_b64");
    SccpSubmitEncoding.validateCanonicalNoritoBase64(
        destinationProof,
        "destination_proof_b64",
        SccpSubmitEncoding.MAX_DESTINATION_ARTIFACT_BYTES,
        SccpSubmitEncoding.DESTINATION_ARTIFACT_SCHEMA_NAME);
  }

  static String normalizeHex32(final String value, final String field) {
    final String normalized = normalizeEvenLengthHex(value, field);
    if (normalized.length() != 64) {
      throw new IllegalArgumentException(field + " must contain 64 hex characters");
    }
    return normalized;
  }

  static String normalizeEd25519PublicKeyHex(final String value, final String field) {
    final String normalized = normalizeHex32(value, field);
    final byte[] publicKey = new byte[Ed25519PublicKeyAdmission.PUBLIC_KEY_LENGTH];
    for (int index = 0; index < publicKey.length; index++) {
      final int offset = index * 2;
      publicKey[index] =
          (byte)
              ((Character.digit(normalized.charAt(offset), 16) << 4)
                  | Character.digit(normalized.charAt(offset + 1), 16));
    }
    if (!Ed25519PublicKeyAdmission.isValid(publicKey)) {
      throw new IllegalArgumentException(
          field + " must encode a canonical prime-order Ed25519 public key");
    }
    return normalized;
  }

  static String normalizeOptionalHex32(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      return null;
    }
    return normalizeHex32(value, field);
  }

  static long normalizePositiveU32(final long value, final String field) {
    if (value <= 0L || value > U32_MAX) {
      throw new IllegalArgumentException(field + " must be a positive u32");
    }
    return value;
  }

  static Long normalizeOptionalU32(final Long value, final String field) {
    if (value == null) {
      return null;
    }
    if (value.longValue() < 0L || value.longValue() > U32_MAX) {
      throw new IllegalArgumentException(field + " must be a u32");
    }
    return value;
  }

  static Long normalizeOptionalNonNegative(final Long value, final String field) {
    if (value == null) {
      return null;
    }
    if (value.longValue() < 0L) {
      throw new IllegalArgumentException(field + " must be non-negative");
    }
    return value;
  }

  static String normalizeVerifyingKeyStatus(final String value) {
    final String normalized = normalizeOptionalNonBlank(value, "status");
    if (normalized == null) {
      return null;
    }
    if ("proposed".equalsIgnoreCase(normalized)) {
      return "Proposed";
    }
    if ("active".equalsIgnoreCase(normalized)) {
      return "Active";
    }
    if ("withdrawn".equalsIgnoreCase(normalized)) {
      return "Withdrawn";
    }
    throw new IllegalArgumentException("status must be Proposed, Active, or Withdrawn");
  }

  private static VerifyingKeyPayload normalizeVerifierBytes(
      final byte[] bytes, final Long explicitLength) {
    if (bytes == null) {
      final Long length =
          explicitLength == null ? null : normalizePositiveU32(explicitLength, "vkLen");
      return length == null ? null : new VerifyingKeyPayload(null, length);
    }
    if (bytes.length == 0) {
      throw new IllegalArgumentException("vkBytes must not be empty");
    }
    final long actualLength = bytes.length;
    if (actualLength > U32_MAX) {
      throw new IllegalArgumentException("vkBytes length must fit in a u32");
    }
    if (explicitLength != null) {
      final long expected = normalizePositiveU32(explicitLength, "vkLen");
      if (expected != actualLength) {
        throw new IllegalArgumentException("vkLen must match vkBytes length");
      }
    }
    return new VerifyingKeyPayload(bytes, actualLength);
  }

  static void validateVerifyingKeyHeightRange(
      final Long activationHeight, final Long withdrawHeight) {
    final Long activation = normalizeOptionalNonNegative(activationHeight, "activationHeight");
    final Long withdraw = normalizeOptionalNonNegative(withdrawHeight, "withdrawHeight");
    if (activation != null && withdraw != null && withdraw.longValue() < activation.longValue()) {
      throw new IllegalArgumentException(
          "withdrawHeight must be greater than or equal to activationHeight");
    }
  }

  static void validateVerifyingKeyMaterial(
      final VerifyingKeyPayload vkPayload, final String commitmentHex) {
    if (vkPayload == null || vkPayload.bytes() == null) {
      if (commitmentHex == null) {
        throw new IllegalArgumentException("commitmentHex is required when vkBytes is omitted");
      }
      if (vkPayload == null || vkPayload.length() == null) {
        throw new IllegalArgumentException("vkLen is required when vkBytes is omitted");
      }
    }
  }

  static void validateInlineVerifyingKeyCommitment(
      final String backend, final byte[] bytes, final String commitmentHex) {
    if (bytes == null || commitmentHex == null) {
      return;
    }
    final String expected = verifyingKeyCommitmentHex(backend, bytes);
    if (!expected.equals(commitmentHex)) {
      throw new IllegalArgumentException(
          "commitmentHex must match domain-separated SHA-256 of backend and vkBytes");
    }
  }

  static String verifyingKeyCommitmentHex(final String backend, final byte[] bytes) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] backendBytes = backend.getBytes(StandardCharsets.UTF_8);
      digest.update("iroha:zk:v1:vk".getBytes(StandardCharsets.UTF_8));
      updateU64Be(digest, backendBytes.length);
      digest.update(backendBytes);
      updateU64Be(digest, bytes.length);
      digest.update(bytes);
      return hexLower(digest.digest());
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  private static void updateU64Be(final MessageDigest digest, final long value) {
    for (int shift = 56; shift >= 0; shift -= 8) {
      digest.update((byte) (value >>> shift));
    }
  }

  private static void putOptionalVerifierFields(
      final Map<String, Object> payload,
      final String curve,
      final Long maxProofBytes,
      final String metadataUriCid,
      final String verifyingKeyBytesCid,
      final Long activationHeight,
      final Long withdrawHeight,
      final String commitmentHex,
      final VerifyingKeyPayload vkPayload,
      final String status) {
    if (curve != null) {
      payload.put("curve", normalizeNonBlank(curve, "curve"));
    }
    final Long normalizedMaxProofBytes = normalizeOptionalU32(maxProofBytes, "maxProofBytes");
    if (normalizedMaxProofBytes != null) {
      payload.put("max_proof_bytes", normalizedMaxProofBytes);
    }
    if (metadataUriCid != null) {
      payload.put("metadata_uri_cid", normalizeNonBlank(metadataUriCid, "metadataUriCid"));
    }
    if (verifyingKeyBytesCid != null) {
      payload.put(
          "vk_bytes_cid", normalizeNonBlank(verifyingKeyBytesCid, "verifyingKeyBytesCid"));
    }
    final Long normalizedActivationHeight =
        normalizeOptionalNonNegative(activationHeight, "activationHeight");
    if (normalizedActivationHeight != null) {
      payload.put("activation_height", normalizedActivationHeight);
    }
    final Long normalizedWithdrawHeight =
        normalizeOptionalNonNegative(withdrawHeight, "withdrawHeight");
    if (normalizedWithdrawHeight != null) {
      payload.put("withdraw_height", normalizedWithdrawHeight);
    }
    if (commitmentHex != null) {
      payload.put("commitment_hex", commitmentHex);
    }
    if (vkPayload != null) {
      final byte[] bytes = vkPayload.bytes();
      if (bytes != null) {
        payload.put("vk_bytes", Base64.getEncoder().encodeToString(bytes));
      }
      if (vkPayload.length() != null) {
        payload.put("vk_len", vkPayload.length());
      }
    }
    final String normalizedStatus = normalizeVerifyingKeyStatus(status);
    if (normalizedStatus != null) {
      payload.put("status", normalizedStatus);
    }
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      final int value = b & 0xff;
      if (value < 16) {
        out.append('0');
      }
      out.append(Integer.toString(value, 16));
    }
    return out.toString();
  }

  private static final class VerifyingKeyPayload {
    private final byte[] bytes;
    private final Long length;

    VerifyingKeyPayload(final byte[] bytes, final Long length) {
      this.bytes = bytes == null ? null : bytes.clone();
      this.length = length;
    }

    byte[] bytes() {
      return bytes == null ? null : bytes.clone();
    }

    Long length() {
      return length;
    }
  }

  private static final java.util.Set<String> SCCP_PROOF_SUBMIT_FIELDS =
      java.util.Set.of(
          "authority",
          "fee_payment",
          "signature_b64",
          "transaction_payload_b64",
          "destination_proof_b64",
          "creation_time_ms");
  private static final java.util.Set<String> SCCP_MESSAGE_SUBMIT_FIELDS =
      java.util.Set.of(
          "authority",
          "fee_payment",
          "signature_b64",
          "transaction_payload_b64",
          "native_proof_b64",
          "creation_time_ms");
  private static final long SCCP_CAPABILITIES_RESPONSE_MAX_BYTES = 64L * 1024L;
  private static final long NODE_CAPABILITIES_RESPONSE_MAX_BYTES = 64L * 1024L;
  private static final long SCCP_RECENT_RESPONSE_MAX_BYTES = 8L * 1024L * 1024L;
  private static final long SCCP_JSON_RESPONSE_MAX_BYTES = 64L * 1024L * 1024L;
  private static final long EXECUTED_BLOCK_WIRE_MAX_BYTES = 32L * 1024L * 1024L;
  private static final long PRIVACY_STATUS_RESPONSE_MAX_BYTES = 64L * 1024L;
  private static final long PRIVACY_SUBMIT_RESPONSE_MAX_BYTES = 64L * 1024L;
  private static final String APPLICATION_JSON = "application/json";
  private static final String APPLICATION_NORITO = "application/x-norito";

  private static String requiredSccpArtifact(final Map<?, ?> fields, final String field) {
    final Object value = fields.get(field);
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(field + " must be a canonical padded base64 string");
    }
    return (String) value;
  }
}
