package org.hyperledger.iroha.android.client;

import java.io.UnsupportedEncodingException;
import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.offline.OfflineAllowanceList;
import org.hyperledger.iroha.android.offline.OfflineAllowanceRegisterResponse;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueRequest;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineBundleProofStatus;
import org.hyperledger.iroha.android.offline.OfflineCertificateIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineJsonParser;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.hyperledger.iroha.android.offline.OfflineQueryEnvelope;
import org.hyperledger.iroha.android.offline.OfflineProofRequestParams;
import org.hyperledger.iroha.android.offline.OfflineProofRequestResult;
import org.hyperledger.iroha.android.offline.OfflineRevocationList;
import org.hyperledger.iroha.android.offline.OfflineSettlementBuildClaimOverride;
import org.hyperledger.iroha.android.offline.OfflineSettlementSubmitResponse;
import org.hyperledger.iroha.android.offline.OfflineSummaryList;
import org.hyperledger.iroha.android.offline.OfflineTopUpResponse;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineTransferList;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificate;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificateDraft;

/**
 * Lightweight HTTP client for Torii offline inspection endpoints (`/v1/offline/*`).
 *
 * <p>The client reuses the shared {@link HttpTransportExecutor} abstraction so telemetry hooks and
 * custom HTTP stacks can be injected by SDK consumers. Responses are parsed into immutable model
 * types under {@code org.hyperledger.iroha.android.offline}.
 */
public final class OfflineToriiClient {

  private static final String ALLOWANCES_PATH = "/v1/offline/allowances";
  private static final String TRANSFERS_PATH = "/v1/offline/transfers";
  private static final String SETTLEMENTS_PATH = "/v1/offline/settlements";
  private static final String SUMMARIES_PATH = "/v1/offline/summaries";
  private static final String REVOCATIONS_PATH = "/v1/offline/revocations";
  private static final String ALLOWANCES_QUERY_PATH = "/v1/offline/allowances/query";
  private static final String TRANSFERS_QUERY_PATH = "/v1/offline/transfers/query";
  private static final String SUMMARIES_QUERY_PATH = "/v1/offline/summaries/query";
  private static final String REVOCATIONS_QUERY_PATH = "/v1/offline/revocations/query";
  private static final String TRANSFER_PROOF_PATH = "/v1/offline/transfers/proof";
  private static final String BUNDLE_PROOF_STATUS_PATH = "/v1/offline/bundle/proof_status";
  private static final String CERTIFICATE_ISSUE_PATH = "/v1/offline/certificates/issue";
  private static final String BUILD_CLAIM_ISSUE_PATH = "/v1/offline/build-claims/issue";
  private static final String CERTIFICATE_RENEW_ISSUE_PATH = "/v1/offline/certificates";

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private OfflineToriiClient(final Builder builder) {
    this.executor = Objects.requireNonNull(builder.executor, "executor");
    this.baseUri = Objects.requireNonNull(builder.baseUri, "baseUri");
    this.timeout = builder.timeout;
    this.defaultHeaders =
        java.util.Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.observers = List.copyOf(builder.observers);
  }

  public static Builder builder() {
    return new Builder();
  }

  public CompletableFuture<OfflineAllowanceList> listAllowances(
      final OfflineListParams params) {
    return executeRequest(ALLOWANCES_PATH, params, OfflineJsonParser::parseAllowances);
  }

  public CompletableFuture<OfflineTransferList> listTransfers(
      final OfflineListParams params) {
    return executeRequest(TRANSFERS_PATH, params, OfflineJsonParser::parseTransfers);
  }

  public CompletableFuture<OfflineSummaryList> listSummaries(
      final OfflineListParams params) {
    return executeRequest(SUMMARIES_PATH, params, OfflineJsonParser::parseSummaries);
  }

  public CompletableFuture<OfflineRevocationList> listRevocations(
      final OfflineListParams params) {
    return executeRequest(REVOCATIONS_PATH, params, OfflineJsonParser::parseRevocations);
  }

  public CompletableFuture<OfflineAllowanceList> queryAllowances(
      final OfflineQueryEnvelope envelope) {
    return executeQuery(ALLOWANCES_QUERY_PATH, envelope, OfflineJsonParser::parseAllowances);
  }

  public CompletableFuture<OfflineTransferList> queryTransfers(
      final OfflineQueryEnvelope envelope) {
    return executeQuery(TRANSFERS_QUERY_PATH, envelope, OfflineJsonParser::parseTransfers);
  }

  public CompletableFuture<OfflineSummaryList> querySummaries(
      final OfflineQueryEnvelope envelope) {
    return executeQuery(SUMMARIES_QUERY_PATH, envelope, OfflineJsonParser::parseSummaries);
  }

  public CompletableFuture<OfflineRevocationList> queryRevocations(
      final OfflineQueryEnvelope envelope) {
    return executeQuery(REVOCATIONS_QUERY_PATH, envelope, OfflineJsonParser::parseRevocations);
  }

  /** Submit an offline settlement bundle for on-ledger settlement. */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlement(
      final Map<String, Object> transferPayload, final String authority, final String privateKeyHex) {
    return submitSettlement(transferPayload, authority, privateKeyHex, List.of(), false);
  }

  /**
   * Submit an offline settlement bundle and wait for the corresponding transaction to reach a
   * terminal pipeline status.
   *
   * <p>This helper reduces boilerplate for clients that always want submit + status polling in one
   * call. Any terminal rejection is surfaced via {@link TransactionStatusException} (or {@link
   * TransactionStatusHttpException} when the polling endpoint itself fails).
   */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlementAndWait(
      final Map<String, Object> transferPayload,
      final String authority,
      final String privateKeyHex,
      final IrohaClient statusClient) {
    return submitSettlementAndWait(
        transferPayload,
        authority,
        privateKeyHex,
        List.of(),
        false,
        statusClient,
        null);
  }

  /**
   * Submit an offline settlement bundle and wait for the corresponding transaction to reach a
   * terminal pipeline status.
   */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlementAndWait(
      final Map<String, Object> transferPayload,
      final String authority,
      final String privateKeyHex,
      final IrohaClient statusClient,
      final PipelineStatusOptions statusOptions) {
    return submitSettlementAndWait(
        transferPayload,
        authority,
        privateKeyHex,
        List.of(),
        false,
        statusClient,
        statusOptions);
  }

  /**
   * Submit an offline settlement bundle with optional per-receipt build-claim overrides.
   *
   * <p>Set {@code repairExistingBuildClaims} when Torii should re-issue receipts that already have
   * a build claim.
   */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlement(
      final Map<String, Object> transferPayload,
      final String authority,
      final String privateKeyHex,
      final List<OfflineSettlementBuildClaimOverride> buildClaimOverrides,
      final boolean repairExistingBuildClaims) {
    Objects.requireNonNull(transferPayload, "transferPayload");
    Objects.requireNonNull(authority, "authority");
    Objects.requireNonNull(privateKeyHex, "privateKeyHex");
    final List<OfflineSettlementBuildClaimOverride> overrides =
        buildClaimOverrides == null ? List.of() : List.copyOf(buildClaimOverrides);
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("authority", authority);
    body.put("private_key", privateKeyHex);
    body.put("transfer", transferPayload);
    if (!overrides.isEmpty()) {
      final List<Map<String, Object>> overridePayloads = new ArrayList<>(overrides.size());
      for (final OfflineSettlementBuildClaimOverride override : overrides) {
        overridePayloads.add(
            Objects.requireNonNull(
                    override,
                    "buildClaimOverrides entries must be non-null")
                .toJsonMap());
      }
      body.put("build_claim_overrides", overridePayloads);
    }
    if (repairExistingBuildClaims) {
      body.put("repair_existing_build_claims", true);
    }
    final TransportRequest request =
        buildPostRequest(SETTLEMENTS_PATH, JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8));
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseSettlementSubmitResponse);
  }

  /**
   * Submit an offline settlement bundle with optional build-claim overrides and wait for terminal
   * transaction status.
   */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlementAndWait(
      final Map<String, Object> transferPayload,
      final String authority,
      final String privateKeyHex,
      final List<OfflineSettlementBuildClaimOverride> buildClaimOverrides,
      final boolean repairExistingBuildClaims,
      final IrohaClient statusClient) {
    return submitSettlementAndWait(
        transferPayload,
        authority,
        privateKeyHex,
        buildClaimOverrides,
        repairExistingBuildClaims,
        statusClient,
        null);
  }

  /**
   * Submit an offline settlement bundle with optional build-claim overrides and wait for terminal
   * transaction status.
   */
  public CompletableFuture<OfflineSettlementSubmitResponse> submitSettlementAndWait(
      final Map<String, Object> transferPayload,
      final String authority,
      final String privateKeyHex,
      final List<OfflineSettlementBuildClaimOverride> buildClaimOverrides,
      final boolean repairExistingBuildClaims,
      final IrohaClient statusClient,
      final PipelineStatusOptions statusOptions) {
    Objects.requireNonNull(statusClient, "statusClient");
    return submitSettlement(
            transferPayload,
            authority,
            privateKeyHex,
            buildClaimOverrides,
            repairExistingBuildClaims)
        .thenCompose(
            settlement ->
                statusClient
                    .waitForTransactionStatus(settlement.transactionHashHex(), statusOptions)
                    .thenApply(ignored -> settlement));
  }

  /** Fetch one offline settlement bundle detail (alias for offline transfer detail). */
  public CompletableFuture<OfflineTransferList.OfflineTransferItem> getSettlement(
      final String bundleIdHex) {
    Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    final String path = SETTLEMENTS_PATH + "/" + urlEncode(bundleIdHex.trim());
    return executeGet(path, OfflineJsonParser::parseTransferItem);
  }

  /** Fetch one offline transfer bundle detail. */
  public CompletableFuture<OfflineTransferList.OfflineTransferItem> getTransfer(
      final String bundleIdHex) {
    Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    final String path = TRANSFERS_PATH + "/" + urlEncode(bundleIdHex.trim());
    return executeGet(path, OfflineJsonParser::parseTransferItem);
  }

  /** Fetch proof integrity status for an offline bundle. */
  public CompletableFuture<OfflineBundleProofStatus> getBundleProofStatus(final String bundleIdHex) {
    Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    final Map<String, String> query = Map.of("bundle_id_hex", bundleIdHex.trim());
    final TransportRequest request = buildGetRequest(BUNDLE_PROOF_STATUS_PATH, query);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseBundleProofStatus);
  }

  /**
   * Build a FASTPQ witness payload by calling Torii `/v1/offline/transfers/proof`.
   *
   * <p>The request must include the transfer payload; the returned JSON string matches the
   * `OfflineProofRequest*` structs and can be forwarded directly to the prover.
   */
  public CompletableFuture<OfflineProofRequestResult> buildProofRequest(
      final OfflineProofRequestParams params) {
    Objects.requireNonNull(params, "params");
    final TransportRequest request = buildPostRequest(TRANSFER_PROOF_PATH, params.toJsonBytes());
    notifyRequest(request);
    return executeHttpRequest(
        request,
        payload ->
            new OfflineProofRequestResult(
                params.kind(), OfflineJsonParser.canonicalJson(payload)));
  }

  /** Issue a signed offline wallet certificate. */
  public CompletableFuture<OfflineCertificateIssueResponse> issueCertificate(
      final OfflineWalletCertificateDraft draft) {
    Objects.requireNonNull(draft, "draft");
    final byte[] body =
        org.hyperledger.iroha.android.client.JsonEncoder.encode(
            Map.of("certificate", draft.toJsonMap()))
            .getBytes(StandardCharsets.UTF_8);
    final TransportRequest request = buildPostRequest(CERTIFICATE_ISSUE_PATH, body);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseCertificateIssueResponse);
  }

  /** Issue an operator-signed build claim for a receipt transaction id. */
  public CompletableFuture<OfflineBuildClaimIssueResponse> issueBuildClaim(
      final OfflineBuildClaimIssueRequest requestBody) {
    Objects.requireNonNull(requestBody, "requestBody");
    final byte[] body =
        org.hyperledger.iroha.android.client.JsonEncoder.encode(requestBody.toJsonMap())
            .getBytes(StandardCharsets.UTF_8);
    final TransportRequest request = buildPostRequest(BUILD_CLAIM_ISSUE_PATH, body);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseBuildClaimIssueResponse);
  }

  /** Register a signed certificate on-ledger as an offline allowance. */
  public CompletableFuture<Void> registerAllowance(
      final OfflineWalletCertificate certificate,
      final String authority,
      final String privateKeyHex) {
    return registerAllowanceDetailed(certificate, authority, privateKeyHex)
        .thenApply(response -> null);
  }

  /** Register a signed certificate on-ledger as an offline allowance and return the response. */
  public CompletableFuture<OfflineAllowanceRegisterResponse> registerAllowanceDetailed(
      final OfflineWalletCertificate certificate,
      final String authority,
      final String privateKeyHex) {
    Objects.requireNonNull(certificate, "certificate");
    Objects.requireNonNull(authority, "authority");
    Objects.requireNonNull(privateKeyHex, "privateKeyHex");
    final byte[] bodyBytes = buildAllowanceRegisterBody(certificate, authority, privateKeyHex);
    final TransportRequest request = buildPostRequest(ALLOWANCES_PATH, bodyBytes);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseAllowanceRegisterResponse);
  }

  /** Renew a signed certificate on-ledger as an offline allowance. */
  public CompletableFuture<OfflineAllowanceRegisterResponse> renewAllowance(
      final String certificateIdHex,
      final OfflineWalletCertificate certificate,
      final String authority,
      final String privateKeyHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    Objects.requireNonNull(certificate, "certificate");
    Objects.requireNonNull(authority, "authority");
    Objects.requireNonNull(privateKeyHex, "privateKeyHex");
    final String encodedId = urlEncode(certificateIdHex.trim());
    final String path = ALLOWANCES_PATH + "/" + encodedId + "/renew";
    final byte[] bodyBytes = buildAllowanceRegisterBody(certificate, authority, privateKeyHex);
    final TransportRequest request = buildPostRequest(path, bodyBytes);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseAllowanceRegisterResponse);
  }

  /**
   * Issue and register an offline allowance certificate in one call (issue + register).
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowance(
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex) {
    Objects.requireNonNull(draft, "draft");
    Objects.requireNonNull(authority, "authority");
    Objects.requireNonNull(privateKeyHex, "privateKeyHex");
    return issueCertificate(draft)
        .thenCompose(
            issued ->
                registerAllowanceDetailed(issued.certificate(), authority, privateKeyHex)
                    .thenApply(
                        registered -> {
                          ensureTopUpCertificateIdsMatch(
                              issued.certificateIdHex(), registered.certificateIdHex());
                          return new OfflineTopUpResponse(issued, registered);
                        }));
  }

  /**
   * Issue and register a renewed offline allowance certificate in one call.
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowanceRenewal(
      final String certificateIdHex,
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    Objects.requireNonNull(draft, "draft");
    Objects.requireNonNull(authority, "authority");
    Objects.requireNonNull(privateKeyHex, "privateKeyHex");
    return issueCertificateRenewal(certificateIdHex, draft)
        .thenCompose(
            issued ->
                renewAllowance(certificateIdHex, issued.certificate(), authority, privateKeyHex)
                    .thenApply(
                        registered -> {
                          ensureTopUpCertificateIdsMatch(
                              issued.certificateIdHex(), registered.certificateIdHex());
                          return new OfflineTopUpResponse(issued, registered);
                        }));
  }

  /** Issue a signed renewal certificate for an existing allowance. */
  public CompletableFuture<OfflineCertificateIssueResponse> issueCertificateRenewal(
      final String certificateIdHex, final OfflineWalletCertificateDraft draft) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    Objects.requireNonNull(draft, "draft");
    final String encodedId = urlEncode(certificateIdHex.trim());
    final String path = CERTIFICATE_RENEW_ISSUE_PATH + "/" + encodedId + "/renew/issue";
    final byte[] body =
        org.hyperledger.iroha.android.client.JsonEncoder.encode(
            Map.of("certificate", draft.toJsonMap()))
            .getBytes(StandardCharsets.UTF_8);
    final TransportRequest request = buildPostRequest(path, body);
    notifyRequest(request);
    return executeHttpRequest(request, OfflineJsonParser::parseCertificateIssueResponse);
  }

  private static byte[] buildAllowanceRegisterBody(
      final OfflineWalletCertificate certificate,
      final String authority,
      final String privateKeyHex) {
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("authority", authority);
    body.put("private_key", privateKeyHex);
    body.put("certificate", certificate.toJsonMap());
    return JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8);
  }

  private static void ensureTopUpCertificateIdsMatch(
      final String issuedId, final String registeredId) {
    if (issuedId == null || registeredId == null) {
      throw new IllegalStateException("Missing certificate id in top-up responses");
    }
    if (!issuedId.equalsIgnoreCase(registeredId)) {
      throw new IllegalStateException(
          "Top-up certificate id mismatch: issued=" + issuedId + " registered=" + registeredId);
    }
  }

  /** Exposes the underlying executor so auxiliary clients can share the same HTTP transport. */
  public HttpTransportExecutor executor() {
    return executor;
  }

  private <T> CompletableFuture<T> executeRequest(
      final String path, final OfflineListParams params, final ResponseParser<T> parser) {
    final TransportRequest request = buildGetRequest(path, params);
    notifyRequest(request);
    return executeHttpRequest(request, parser);
  }

  private <T> CompletableFuture<T> executeGet(final String path, final ResponseParser<T> parser) {
    final TransportRequest request = buildGetRequest(path, Map.of());
    notifyRequest(request);
    return executeHttpRequest(request, parser);
  }

  private TransportRequest buildGetRequest(final String path, final OfflineListParams params) {
    final Map<String, String> query = params != null ? params.toQueryParameters() : Map.of();
    return buildGetRequest(path, query);
  }

  private TransportRequest buildGetRequest(final String path, final Map<String, String> query) {
    final URI target = appendQuery(resolvePath(path), query);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .setTimeout(timeout);
    mergeHeaders().forEach(builder::addHeader);
    return builder.build();
  }

  private <T> CompletableFuture<T> executeQuery(
      final String path, final OfflineQueryEnvelope envelope, final ResponseParser<T> parser) {
    final TransportRequest request = buildPostRequest(path, envelope);
    notifyRequest(request);
    return executeHttpRequest(request, parser);
  }

  private TransportRequest buildPostRequest(
      final String path, final OfflineQueryEnvelope envelope) {
    final OfflineQueryEnvelope resolved =
        envelope != null ? envelope : OfflineQueryEnvelope.builder().build();
    return buildPostRequest(path, resolved.toJsonBytes());
  }

  private TransportRequest buildPostRequest(final String path, final byte[] body) {
    final URI target = resolvePath(path);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setTimeout(timeout)
            .setBody(body);
    mergeHeaders().forEach(builder::addHeader);
    builder.addHeader("Content-Type", "application/json");
    return builder.build();
  }

  private Map<String, String> mergeHeaders() {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Accept", "application/json");
    return headers;
  }

  private void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    final String existing = findHeader(headers, name);
    if (existing != null) {
      headers.put(existing, value);
    } else {
      headers.put(name, value);
    }
  }

  private static String findHeader(final Map<String, String> headers, final String name) {
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(name)) {
        return key;
      }
    }
    return null;
  }

  private URI resolvePath(final String path) {
    if (path == null || path.isBlank()) {
      return baseUri;
    }
    if (path.startsWith("http://") || path.startsWith("https://")) {
      return URI.create(path);
    }
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
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

  private static String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.name());
    } catch (final UnsupportedEncodingException ex) {
      throw new IllegalStateException("UTF-8 not supported", ex);
    }
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : observers) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private <T> CompletableFuture<T> executeHttpRequest(
      final TransportRequest request, final ResponseParser<T> parser) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException
                        ? throwable.getCause()
                        : throwable;
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Offline request failed: " + summarizeCauseMessage(cause),
                        cause,
                        null,
                        null,
                        null);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final String rejectCode = extractRejectCode(response.headers());
              final String bodyPreview = decodeBodyPreview(response.body());
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      rejectCode);
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        buildHttpFailureMessage(
                            request, response.statusCode(), response.message(), rejectCode, bodyPreview),
                        response.statusCode(),
                        rejectCode,
                        bodyPreview);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.parse(response.body());
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (final RuntimeException ex) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        buildParseFailureMessage(request, response.statusCode(), bodyPreview),
                        ex,
                        response.statusCode(),
                        rejectCode,
                        bodyPreview);
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private static String extractRejectCode(final Map<String, List<String>> headers) {
    if (headers == null || headers.isEmpty()) {
      return null;
    }
    final List<String> values = headers.get("x-iroha-reject-code");
    if (values == null || values.isEmpty()) {
      return null;
    }
    for (final String value : values) {
      if (value != null && !value.isBlank()) {
        return value.trim();
      }
    }
    return null;
  }

  private static String decodeBodyPreview(final byte[] payload) {
    if (payload == null || payload.length == 0) {
      return null;
    }
    final String text = new String(payload, StandardCharsets.UTF_8).trim();
    if (text.isEmpty()) {
      return null;
    }
    final int maxLength = 512;
    if (text.length() <= maxLength) {
      return text;
    }
    return text.substring(0, maxLength) + "...";
  }

  private static String summarizeCauseMessage(final Throwable cause) {
    if (cause == null) {
      return "unknown transport error";
    }
    final String detail = cause.getMessage();
    if (detail == null || detail.isBlank()) {
      return cause.getClass().getSimpleName();
    }
    return detail;
  }

  private static String buildHttpFailureMessage(
      final TransportRequest request,
      final int statusCode,
      final String statusMessage,
      final String rejectCode,
      final String bodyPreview) {
    final StringBuilder message = new StringBuilder("Offline request failed with HTTP ")
        .append(statusCode);
    if (statusMessage != null && !statusMessage.isBlank()) {
      message.append(" (").append(statusMessage).append(")");
    }
    final URI uri = request == null ? null : request.uri();
    if (uri != null) {
      message.append(" on ").append(uri.getPath());
    }
    if (rejectCode != null && !rejectCode.isBlank()) {
      message.append(". reject_code=").append(rejectCode);
    }
    if (bodyPreview != null && !bodyPreview.isBlank()) {
      message.append(". body=").append(bodyPreview);
    }
    return message.toString();
  }

  private static String buildParseFailureMessage(
      final TransportRequest request, final int statusCode, final String bodyPreview) {
    final StringBuilder message =
        new StringBuilder("Failed to parse offline response (HTTP ")
            .append(statusCode)
            .append(")");
    final URI uri = request == null ? null : request.uri();
    if (uri != null) {
      message.append(" for ").append(uri.getPath());
    }
    if (bodyPreview != null && !bodyPreview.isBlank()) {
      message.append(". body=").append(bodyPreview);
    }
    return message.toString();
  }

  @FunctionalInterface
  private interface ResponseParser<T> {
    T parse(byte[] payload);
  }

  public static final class Builder {
    private HttpTransportExecutor executor = PlatformHttpTransportExecutor.createDefault();
    private URI baseUri = URI.create("http://localhost:8080");
    private Duration timeout = Duration.ofSeconds(15);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();

    private Builder() {}

    public Builder executor(final HttpTransportExecutor executor) {
      this.executor = executor;
      return this;
    }

    public Builder baseUri(final URI baseUri) {
      this.baseUri = baseUri;
      return this;
    }

    public Builder timeout(final Duration timeout) {
      this.timeout = timeout;
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      if (name != null && value != null) {
        defaultHeaders.put(name, value);
      }
      return this;
    }

    public Builder defaultHeaders(final Map<String, String> headers) {
      defaultHeaders.clear();
      if (headers != null) {
        headers.forEach((k, v) -> {
          if (k != null && v != null) {
            defaultHeaders.put(k, v);
          }
        });
      }
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      if (observer != null) {
        observers.add(observer);
      }
      return this;
    }

    public Builder observers(final List<ClientObserver> observers) {
      this.observers.clear();
      if (observers != null) {
        observers.forEach(this::addObserver);
      }
      return this;
    }

    public OfflineToriiClient build() {
      if (executor == null) {
        throw new IllegalStateException("executor is required");
      }
      if (baseUri == null) {
        throw new IllegalStateException("baseUri is required");
      }
      return new OfflineToriiClient(this);
    }
  }
}
