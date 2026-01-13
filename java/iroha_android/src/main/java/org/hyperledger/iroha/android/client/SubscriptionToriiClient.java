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
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionActionRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionActionResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionJsonParser;
import org.hyperledger.iroha.android.subscriptions.SubscriptionListParams;
import org.hyperledger.iroha.android.subscriptions.SubscriptionListResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateRequest;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanCreateResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanListParams;
import org.hyperledger.iroha.android.subscriptions.SubscriptionPlanListResponse;
import org.hyperledger.iroha.android.subscriptions.SubscriptionToriiException;
import org.hyperledger.iroha.android.subscriptions.SubscriptionUsageRequest;

/** Lightweight HTTP client for Torii subscription endpoints (`/v1/subscriptions/*`). */
public final class SubscriptionToriiClient {

  private static final String PLANS_PATH = "/v1/subscriptions/plans";
  private static final String SUBSCRIPTIONS_PATH = "/v1/subscriptions";

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private SubscriptionToriiClient(final Builder builder) {
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

  public CompletableFuture<SubscriptionPlanListResponse> listSubscriptionPlans(
      final SubscriptionPlanListParams params) {
    final TransportRequest request =
        buildGetRequest(PLANS_PATH, params == null ? Map.of() : params.toQueryParameters());
    notifyRequest(request);
    return executeHttpRequest(request, SubscriptionJsonParser::parsePlanList);
  }

  public CompletableFuture<SubscriptionPlanCreateResponse> createSubscriptionPlan(
      final SubscriptionPlanCreateRequest request) {
    Objects.requireNonNull(request, "request");
    final TransportRequest transport = buildPostRequest(PLANS_PATH, request.toJsonBytes());
    notifyRequest(transport);
    return executeHttpRequest(transport, SubscriptionJsonParser::parsePlanCreateResponse);
  }

  public CompletableFuture<SubscriptionListResponse> listSubscriptions(
      final SubscriptionListParams params) {
    final TransportRequest request =
        buildGetRequest(SUBSCRIPTIONS_PATH, params == null ? Map.of() : params.toQueryParameters());
    notifyRequest(request);
    return executeHttpRequest(request, SubscriptionJsonParser::parseSubscriptionList);
  }

  public CompletableFuture<SubscriptionCreateResponse> createSubscription(
      final SubscriptionCreateRequest request) {
    Objects.requireNonNull(request, "request");
    final TransportRequest transport =
        buildPostRequest(SUBSCRIPTIONS_PATH, request.toJsonBytes());
    notifyRequest(transport);
    return executeHttpRequest(transport, SubscriptionJsonParser::parseSubscriptionCreateResponse);
  }

  /**
   * Fetch a single subscription by id. Returns {@code null} when the subscription does not exist.
   */
  public CompletableFuture<SubscriptionListResponse.SubscriptionRecord> getSubscription(
      final String subscriptionId) {
    final String normalizedId = requireNonBlank(subscriptionId, "subscription_id");
    final String encodedId = urlEncode(normalizedId);
    final String path = SUBSCRIPTIONS_PATH + "/" + encodedId;
    final TransportRequest request = buildGetRequest(path, Map.of());
    notifyRequest(request);
    return executeHttpRequestAllowingNotFound(request, SubscriptionJsonParser::parseSubscriptionRecord);
  }

  public CompletableFuture<SubscriptionActionResponse> pauseSubscription(
      final String subscriptionId, final SubscriptionActionRequest request) {
    return executeSubscriptionAction(subscriptionId, "pause", request);
  }

  public CompletableFuture<SubscriptionActionResponse> resumeSubscription(
      final String subscriptionId, final SubscriptionActionRequest request) {
    return executeSubscriptionAction(subscriptionId, "resume", request);
  }

  public CompletableFuture<SubscriptionActionResponse> cancelSubscription(
      final String subscriptionId, final SubscriptionActionRequest request) {
    return executeSubscriptionAction(subscriptionId, "cancel", request);
  }

  public CompletableFuture<SubscriptionActionResponse> keepSubscription(
      final String subscriptionId, final SubscriptionActionRequest request) {
    return executeSubscriptionAction(subscriptionId, "keep", request);
  }

  public CompletableFuture<SubscriptionActionResponse> chargeSubscriptionNow(
      final String subscriptionId, final SubscriptionActionRequest request) {
    return executeSubscriptionAction(subscriptionId, "charge-now", request);
  }

  public CompletableFuture<SubscriptionActionResponse> recordSubscriptionUsage(
      final String subscriptionId, final SubscriptionUsageRequest request) {
    Objects.requireNonNull(request, "request");
    final String normalizedId = requireNonBlank(subscriptionId, "subscription_id");
    final String encodedId = urlEncode(normalizedId);
    final String path = SUBSCRIPTIONS_PATH + "/" + encodedId + "/usage";
    final TransportRequest transport = buildPostRequest(path, request.toJsonBytes());
    notifyRequest(transport);
    return executeHttpRequest(transport, SubscriptionJsonParser::parseActionResponse);
  }

  /** Exposes the underlying executor so auxiliary clients can share the same HTTP transport. */
  public HttpTransportExecutor executor() {
    return executor;
  }

  private CompletableFuture<SubscriptionActionResponse> executeSubscriptionAction(
      final String subscriptionId,
      final String action,
      final SubscriptionActionRequest request) {
    Objects.requireNonNull(request, "request");
    final String normalizedId = requireNonBlank(subscriptionId, "subscription_id");
    final String encodedId = urlEncode(normalizedId);
    final String path = SUBSCRIPTIONS_PATH + "/" + encodedId + "/" + action;
    final TransportRequest transport = buildPostRequest(path, request.toJsonBytes());
    notifyRequest(transport);
    return executeHttpRequest(transport, SubscriptionJsonParser::parseActionResponse);
  }

  private TransportRequest buildGetRequest(final String path, final Map<String, String> query) {
    final URI target = appendQuery(resolvePath(path), query == null ? Map.of() : query);
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod("GET").setTimeout(timeout);
    mergeHeaders().forEach(builder::addHeader);
    return builder.build();
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
    if (params == null || params.isEmpty()) {
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
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final SubscriptionToriiException error =
                    new SubscriptionToriiException("Subscription request failed", cause);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(response.statusCode(), response.body());
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final SubscriptionToriiException error =
                    new SubscriptionToriiException(
                        "Subscription request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.parse(response.body());
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (final RuntimeException ex) {
                final SubscriptionToriiException error =
                    new SubscriptionToriiException("Failed to parse subscription response", ex);
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private <T> CompletableFuture<T> executeHttpRequestAllowingNotFound(
      final TransportRequest request, final ResponseParser<T> parser) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    executor
        .execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final SubscriptionToriiException error =
                    new SubscriptionToriiException("Subscription request failed", cause);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final ClientResponse clientResponse =
                  new ClientResponse(response.statusCode(), response.body());
              if (response.statusCode() == 404) {
                notifyResponse(request, clientResponse);
                future.complete(null);
                return;
              }
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final SubscriptionToriiException error =
                    new SubscriptionToriiException(
                        "Subscription request failed with status " + response.statusCode());
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.parse(response.body());
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (final RuntimeException ex) {
                final SubscriptionToriiException error =
                    new SubscriptionToriiException("Failed to parse subscription response", ex);
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  @FunctionalInterface
  private interface ResponseParser<T> {
    T parse(byte[] payload);
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalStateException(field + " is required");
    }
    return value.trim();
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

    public SubscriptionToriiClient build() {
      if (executor == null) {
        throw new IllegalStateException("executor is required");
      }
      if (baseUri == null) {
        throw new IllegalStateException("baseUri is required");
      }
      return new SubscriptionToriiClient(this);
    }
  }
}
