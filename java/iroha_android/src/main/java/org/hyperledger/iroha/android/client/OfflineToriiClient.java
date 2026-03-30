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
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueRequest;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineJsonParser;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.hyperledger.iroha.android.offline.OfflineQueryEnvelope;
import org.hyperledger.iroha.android.offline.OfflineRevocationList;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineTransferList;

/**
 * Lightweight HTTP client for Torii offline inspection endpoints (`/v1/offline/*`).
 *
 * <p>The client reuses the shared {@link HttpTransportExecutor} abstraction so telemetry hooks and
 * custom HTTP stacks can be injected by SDK consumers. Responses are parsed into immutable model
 * types under {@code org.hyperledger.iroha.android.offline}.
 */
public final class OfflineToriiClient {

  private static final String TRANSFERS_PATH = "/v1/offline/transfers";
  private static final String REVOCATIONS_PATH = "/v1/offline/revocations";
  private static final String REVOCATIONS_BUNDLE_PATH = "/v1/offline/revocations/bundle";
  private static final String CASH_SETUP_PATH = "/v1/offline/cash/setup";
  private static final String CASH_LOAD_PATH = "/v1/offline/cash/load";
  private static final String CASH_REFRESH_PATH = "/v1/offline/cash/refresh";
  private static final String CASH_SYNC_PATH = "/v1/offline/cash/sync";
  private static final String CASH_REDEEM_PATH = "/v1/offline/cash/redeem";
  private static final String TRANSFERS_QUERY_PATH = "/v1/offline/transfers/query";
  private static final String BUILD_CLAIM_ISSUE_PATH = "/v1/offline/build-claims/issue";

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

  public CompletableFuture<OfflineTransferList> listTransfers(
      final OfflineListParams params) {
    return executeRequest(TRANSFERS_PATH, params, OfflineJsonParser::parseTransfers);
  }

  public CompletableFuture<OfflineRevocationList> listRevocations(
      final OfflineListParams params) {
    return executeRequest(REVOCATIONS_PATH, params, OfflineJsonParser::parseRevocations);
  }

  public CompletableFuture<OfflineTransferList> queryTransfers(
      final OfflineQueryEnvelope envelope) {
    return executeQuery(TRANSFERS_QUERY_PATH, envelope, OfflineJsonParser::parseTransfers);
  }
  /** Fetch one offline transfer bundle detail. */
  public CompletableFuture<OfflineTransferList.OfflineTransferItem> getTransfer(
      final String bundleIdHex) {
    Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    final String path = TRANSFERS_PATH + "/" + urlEncode(bundleIdHex.trim());
    return executeGet(path, OfflineJsonParser::parseTransferItem);
  }

  /** Fetch the signed offline revocation bundle as raw JSON. */
  public CompletableFuture<String> getRevocationBundleJson() {
    return executeGet(REVOCATIONS_BUNDLE_PATH, OfflineToriiClient::decodeJsonPayload);
  }

  /** Post a JSON-encoded offline cash setup request and return the raw JSON response. */
  public CompletableFuture<String> setupCash(final String requestJson) {
    return executeJsonPost(CASH_SETUP_PATH, requestJson);
  }

  /** Post a JSON-encoded offline cash load request and return the raw JSON response. */
  public CompletableFuture<String> loadCash(final String requestJson) {
    return executeJsonPost(CASH_LOAD_PATH, requestJson);
  }

  /** Post a JSON-encoded offline cash refresh request and return the raw JSON response. */
  public CompletableFuture<String> refreshCash(final String requestJson) {
    return executeJsonPost(CASH_REFRESH_PATH, requestJson);
  }

  /** Post a JSON-encoded offline cash sync request and return the raw JSON response. */
  public CompletableFuture<String> syncCash(final String requestJson) {
    return executeJsonPost(CASH_SYNC_PATH, requestJson);
  }

  /** Post a JSON-encoded offline cash redeem request and return the raw JSON response. */
  public CompletableFuture<String> redeemCash(final String requestJson) {
    return executeJsonPost(CASH_REDEEM_PATH, requestJson);
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

  private CompletableFuture<String> executeJsonPost(
      final String path, final String requestJson) {
    Objects.requireNonNull(requestJson, "requestJson");
    final String trimmed = requestJson.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException("requestJson must not be blank");
    }
    final TransportRequest request =
        buildPostRequest(path, trimmed.getBytes(StandardCharsets.UTF_8));
    notifyRequest(request);
    return executeHttpRequest(request, OfflineToriiClient::decodeJsonPayload);
  }

  private TransportRequest buildGetRequest(final String path, final OfflineListParams params) {
    final Map<String, String> query = params != null ? params.toQueryParameters() : Map.of();
    return buildGetRequest(path, query);
  }

  private TransportRequest buildGetRequest(final String path, final Map<String, String> query) {
    final URI target = appendQuery(resolvePath(path), query);
    final Map<String, String> headers = mergeHeaders();
    TransportSecurity.requireHttpRequestAllowed(
        "OfflineToriiClient", baseUri, target, headers, null);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("GET")
            .setTimeout(timeout);
    headers.forEach(builder::addHeader);
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
    final Map<String, String> headers = mergeHeaders();
    TransportSecurity.requireHttpRequestAllowed(
        "OfflineToriiClient", baseUri, target, headers, body);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setTimeout(timeout)
            .setBody(body);
    headers.forEach(builder::addHeader);
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
    return HttpErrorMessageExtractor.extractRejectCode(headers, "x-iroha-reject-code");
  }

  private static String decodeBodyPreview(final byte[] payload) {
    return HttpErrorMessageExtractor.extractMessage(payload);
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

  private static String decodeJsonPayload(final byte[] payload) {
    return new String(payload, StandardCharsets.UTF_8);
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
