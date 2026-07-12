package org.hyperledger.iroha.android.client;

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
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.OfflineJsonParser;
import org.hyperledger.iroha.android.offline.OfflineOperationCodec;
import org.hyperledger.iroha.android.offline.OfflineOperationKind;
import org.hyperledger.iroha.android.offline.OfflineOperationReference;
import org.hyperledger.iroha.android.offline.OfflineOperationStatus;
import org.hyperledger.iroha.android.offline.OfflineReadiness;
import org.hyperledger.iroha.android.offline.OfflineRedeemRequest;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineTopUpRequest;

/**
 * Lightweight HTTP client for the maintained Torii Offline endpoint.
 *
 * <p>The retired offline HTTP routes have been
 * removed from Torii. This client exposes the first-release Offline readiness,
 * top-up, redemption, and operation resources.
 */
public final class OfflineToriiClient {

  private static final String OFFLINE_READINESS_PATH = "/v1/offline/readiness";
  private static final String OFFLINE_TOP_UP_PATH = "/v1/offline/top-up";
  private static final String OFFLINE_REDEEM_PATH = "/v1/offline/redeem";
  private static final String OFFLINE_OPERATIONS_PATH = "/v1/offline/operations";
  private static final String NORITO_MEDIA_TYPE = "application/x-norito";
  private static final Pattern OFFLINE_ASSET_ALIAS_PATTERN =
      Pattern.compile(
          "^[a-z0-9]+(?:[._-][a-z0-9]+)*#[a-z0-9]+(?:-[a-z0-9]+)*(?:\\.[a-z0-9]+(?:-[a-z0-9]+)*)?$");

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
    this.observers = java.util.Collections.unmodifiableList(new ArrayList<>(builder.observers));
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Fetch Torii's Offline readiness flags. */
  public CompletableFuture<OfflineReadiness> getOfflineReadiness(
      final String assetDefinitionId) {
    requireOfflineAssetSelector(assetDefinitionId);
    final String canonicalRequestedId =
        AssetDefinitionIdEncoder.isCanonicalAddress(assetDefinitionId)
            ? assetDefinitionId
            : null;
    return executeGet(
        OFFLINE_READINESS_PATH
            + "?asset_definition_id="
            + urlEncode(assetDefinitionId),
        body -> {
          final OfflineReadiness readiness = OfflineJsonParser.parseOfflineReadiness(body);
          if (canonicalRequestedId != null
              && !canonicalRequestedId.equals(readiness.assetDefinitionId())) {
            throw new IllegalArgumentException(
                "Offline readiness response assetDefinitionId does not match the requested asset definition");
          }
          return readiness;
        });
  }

  /** Submit the final first-release Offline top-up request. */
  public CompletableFuture<OfflineOperationReference> submitOfflineTopUp(
      final OfflineTopUpRequest request) {
    Objects.requireNonNull(request, "request");
    return executeNoritoPost(
        OFFLINE_TOP_UP_PATH,
        request.operationId(),
        request.noritoArchive(),
        OfflineOperationKind.TOP_UP);
  }

  /** Submit the final first-release Offline redemption request. */
  public CompletableFuture<OfflineOperationReference> submitOfflineRedeem(
      final OfflineRedeemRequest request) {
    Objects.requireNonNull(request, "request");
    return executeNoritoPost(
        OFFLINE_REDEEM_PATH,
        request.operationId(),
        request.noritoArchive(),
        OfflineOperationKind.REDEEM);
  }

  /** Fetch the final first-release Offline operation status resource. */
  public CompletableFuture<OfflineOperationStatus> getOfflineOperationStatus(
      final String operationId) {
    final String canonicalId =
        org.hyperledger.iroha.android.offline.OfflineOperationCodec.requireOperationId(
            operationId);
    return executeNoritoGet(OFFLINE_OPERATIONS_PATH + "/" + canonicalId, canonicalId);
  }

  /** Exposes the underlying executor so auxiliary clients can share the same HTTP transport. */
  public HttpTransportExecutor executor() {
    return executor;
  }

  private <T> CompletableFuture<T> executeGet(final String path, final PayloadParser<T> parser) {
    final TransportRequest request = buildGetRequest(path);
    notifyRequest(request);
    return executeHttpRequest(
        request,
        200,
        response -> {
          requireResponseMediaType(response, "application/json");
          return parser.parse(response.body());
        });
  }

  private TransportRequest buildGetRequest(final String path) {
    final URI target = resolvePath(path);
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

  private CompletableFuture<OfflineOperationReference> executeNoritoPost(
      final String path,
      final String idempotencyKey,
      final byte[] body,
      final OfflineOperationKind expectedKind) {
    final TransportRequest request =
        buildNoritoRequest(path, "POST", body, idempotencyKey);
    notifyRequest(request);
    return executeHttpRequest(
        request,
        202,
        response -> {
          requireResponseMediaType(response, NORITO_MEDIA_TYPE);
          final OfflineOperationReference reference =
              OfflineOperationCodec.decodeReference(response.body());
          if (!reference.operationId().equals(idempotencyKey)
              || reference.kind() != expectedKind) {
            throw new IllegalArgumentException(
                "Offline operation reference does not match the submitted command");
          }
          final String location = requireSingleResponseHeader(response.headers(), "Location");
          if (!location.equals(reference.statusUri())) {
            throw new IllegalArgumentException(
                "Offline operation response Location does not match its typed statusUri");
          }
          return reference;
        });
  }

  private CompletableFuture<OfflineOperationStatus> executeNoritoGet(
      final String path, final String expectedOperationId) {
    final TransportRequest request = buildNoritoRequest(path, "GET", null, null);
    notifyRequest(request);
    return executeHttpRequest(
        request,
        200,
        response -> {
          requireResponseMediaType(response, NORITO_MEDIA_TYPE);
          final OfflineOperationStatus status = OfflineOperationCodec.decodeStatus(response.body());
          if (!status.operationId().equals(expectedOperationId)) {
            throw new IllegalArgumentException(
                "Offline operation status operationId does not match the requested resource");
          }
          return status;
        });
  }

  private TransportRequest buildNoritoRequest(
      final String path,
      final String method,
      final byte[] body,
      final String idempotencyKey) {
    final URI target = resolvePath(path);
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Accept", NORITO_MEDIA_TYPE);
    if (body != null) {
      ensureHeader(headers, "Content-Type", NORITO_MEDIA_TYPE);
    }
    if (idempotencyKey != null) {
      ensureHeader(headers, "Idempotency-Key", idempotencyKey);
    }
    TransportSecurity.requireHttpRequestAllowed(
        "OfflineToriiClient", baseUri, target, headers, body);
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod(method).setTimeout(timeout);
    if (body != null) {
      builder.setBody(java.util.Arrays.copyOf(body, body.length));
    }
    headers.forEach(builder::addHeader);
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

  private static String requireSingleResponseHeader(
      final Map<String, List<String>> headers, final String name) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name) && entry.getValue() != null) {
        values.addAll(entry.getValue());
      }
    }
    if (values.size() != 1 || values.get(0) == null) {
      throw new IllegalArgumentException(
          name + " response header must occur exactly once");
    }
    return values.get(0);
  }

  private static void requireResponseMediaType(
      final TransportResponse response, final String expected) {
    final String raw = requireSingleResponseHeader(response.headers(), "Content-Type");
    final int parameterStart = raw.indexOf(';');
    final String mediaType =
        (parameterStart < 0 ? raw : raw.substring(0, parameterStart)).trim();
    if (!mediaType.equalsIgnoreCase(expected)) {
      throw new IllegalArgumentException(
          "Offline response Content-Type must be " + expected);
    }
  }

  private URI resolvePath(final String path) {
    if (path == null || path.trim().isEmpty()) {
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

  private static void requireExactNonEmptyText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
  }

  private static void requireOfflineAssetSelector(final String value) {
    requireExactNonEmptyText(value, "assetDefinitionId");
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(value)
        && !OFFLINE_ASSET_ALIAS_PATTERN.matcher(value).matches()) {
      throw new IllegalArgumentException(
          "assetDefinitionId must be a canonical Base58 id or lowercase scoped asset alias");
    }
  }

  private static String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.name());
    } catch (final java.io.UnsupportedEncodingException ex) {
      throw new IllegalStateException("UTF-8 unavailable", ex);
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
      final TransportRequest request,
      final int expectedStatus,
      final ResponseParser<T> parser) {
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
              final String rejectCode = extractRejectCode(response.headers(), response.body());
              final String bodyPreview = decodeBodyPreview(response.body());
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(),
                      response.body(),
                      response.message(),
                      null,
                      rejectCode);
              if (response.statusCode() != expectedStatus) {
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
                final T parsed = parser.parse(response);
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

  private static String extractRejectCode(final Map<String, List<String>> headers, final byte[] body) {
    return HttpErrorMessageExtractor.extractRejectCode(headers, "x-iroha-reject-code", body);
  }

  private static String decodeBodyPreview(final byte[] payload) {
    return HttpErrorMessageExtractor.extractMessage(payload);
  }

  private static String summarizeCauseMessage(final Throwable cause) {
    if (cause == null) {
      return "unknown transport error";
    }
    final String detail = cause.getMessage();
    if (detail == null || detail.trim().isEmpty()) {
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
    if (statusMessage != null && !statusMessage.trim().isEmpty()) {
      message.append(" (").append(statusMessage).append(")");
    }
    final URI uri = request == null ? null : request.uri();
    if (uri != null) {
      message.append(" on ").append(uri.getPath());
    }
    if (rejectCode != null && !rejectCode.trim().isEmpty()) {
      message.append(". reject_code=").append(rejectCode);
    }
    if (bodyPreview != null && !bodyPreview.trim().isEmpty()) {
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
    if (bodyPreview != null && !bodyPreview.trim().isEmpty()) {
      message.append(". body=").append(bodyPreview);
    }
    return message.toString();
  }

  @FunctionalInterface
  private interface ResponseParser<T> {
    T parse(TransportResponse response);
  }

  @FunctionalInterface
  private interface PayloadParser<T> {
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
      this.executor = Objects.requireNonNull(executor, "executor");
      return this;
    }

    public Builder baseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder timeout(final Duration timeout) {
      this.timeout = timeout;
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      this.defaultHeaders.put(Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder defaultHeaders(final Map<String, String> headers) {
      this.defaultHeaders.clear();
      if (headers != null) {
        headers.forEach(this::addHeader);
      }
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      if (observer != null) {
        this.observers.add(observer);
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
      return new OfflineToriiClient(this);
    }
  }
}
