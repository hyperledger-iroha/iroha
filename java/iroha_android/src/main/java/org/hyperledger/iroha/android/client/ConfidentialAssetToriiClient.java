package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.offline.OfflineToriiException;

/** Lightweight Torii client for confidential-asset query endpoints. */
public final class ConfidentialAssetToriiClient {
  private static final String ZK_ROOTS_PATH = "/v1/zk/roots";
  private static final String ZK_MERKLE_PATH_PATH = "/v1/zk/merkle-path";

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private ConfidentialAssetToriiClient(final Builder builder) {
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

  public CompletableFuture<ZkRootsResponse> getZkAssetRoots(final ZkRootsRequest request) {
    return executePost(ZK_ROOTS_PATH, Objects.requireNonNull(request, "request").toJsonBytes(), ZkRootsResponse::parse);
  }

  public CompletableFuture<ZkMerklePathResponse> getZkAssetMerklePaths(
      final ZkMerklePathRequest request) {
    return executePost(
        ZK_MERKLE_PATH_PATH,
        Objects.requireNonNull(request, "request").toJsonBytes(),
        ZkMerklePathResponse::parse);
  }

  public CompletableFuture<byte[]> getLatestZkAssetRoot(final String assetId) {
    return getZkAssetRoots(new ZkRootsRequest(assetId, 1)).thenApply(ZkRootsResponse::latestRootBytes);
  }

  public HttpTransportExecutor executor() {
    return executor;
  }

  private <T> CompletableFuture<T> executePost(
      final String path, final byte[] body, final ResponseParser<T> parser) {
    final TransportRequest request = buildPostRequest(path, body);
    notifyRequest(request);
    return executeHttpRequest(request, parser);
  }

  private TransportRequest buildPostRequest(final String path, final byte[] body) {
    final URI target = resolvePath(path);
    final Map<String, String> headers = mergeHeaders();
    TransportSecurity.requireHttpRequestAllowed(
        "ConfidentialAssetToriiClient", baseUri, target, headers, body);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .setBody(body)
            .setTimeout(timeout);
    headers.forEach(builder::addHeader);
    return builder.build();
  }

  private Map<String, String> mergeHeaders() {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Accept", "application/json");
    ensureHeader(headers, "Content-Type", "application/json");
    return headers;
  }

  private static void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    final String existing = findHeader(headers, name);
    headers.put(existing == null ? name : existing, value);
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
    return URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
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
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Confidential asset request failed: " + summarizeCauseMessage(cause),
                        cause,
                        null,
                        null,
                        null);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final String rejectCode =
                  HttpErrorMessageExtractor.extractRejectCode(
                      response.headers(), "x-iroha-reject-code", response.body());
              final String bodyPreview = HttpErrorMessageExtractor.extractMessage(response.body());
              final ClientResponse clientResponse =
                  new ClientResponse(
                      response.statusCode(), response.body(), response.message(), null, rejectCode);
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        buildHttpFailureMessage(
                            request,
                            response.statusCode(),
                            response.message(),
                            rejectCode,
                            bodyPreview),
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

  private static String summarizeCauseMessage(final Throwable cause) {
    if (cause == null) {
      return "unknown transport error";
    }
    final String detail = cause.getMessage();
    return detail == null || detail.isBlank() ? cause.getClass().getSimpleName() : detail;
  }

  private static String buildHttpFailureMessage(
      final TransportRequest request,
      final int statusCode,
      final String statusMessage,
      final String rejectCode,
      final String bodyPreview) {
    final StringBuilder message =
        new StringBuilder("Confidential asset request failed with HTTP ").append(statusCode);
    if (statusMessage != null && !statusMessage.isBlank()) {
      message.append(" (").append(statusMessage).append(")");
    }
    if (request != null) {
      message.append(" on ").append(request.uri().getPath());
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
        new StringBuilder("Failed to parse confidential asset response (HTTP ")
            .append(statusCode)
            .append(")");
    if (request != null) {
      message.append(" for ").append(request.uri().getPath());
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

    public ConfidentialAssetToriiClient build() {
      return new ConfidentialAssetToriiClient(this);
    }
  }
}
