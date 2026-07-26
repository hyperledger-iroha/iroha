package org.hyperledger.iroha.android.sorafs;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;

/**
 * Minimal HTTP client that posts orchestrator fetch requests to a SoraFS gateway endpoint.
 *
 * <p>The client mirrors the CLI/SDK JSON schema and routes requests through {@link
 * HttpTransportExecutor} so tests can provide deterministic transport fakes. Responses surface the
 * raw HTTP payload allowing callers to parse orchestrator summaries or binary artefacts as needed.
 * A zero timeout is valid; negative durations are rejected instead of being rewritten.
 *
 * <p>Host syntax and literal addresses are validated here. A transport resolving DNS names must
 * additionally reject non-public answers and pin the approved answer for the request; the generic
 * executor interface cannot enforce that resolver-level DNS-rebinding boundary.
 */
public final class SorafsGatewayClient {

  private static final String DEFAULT_PATH = "/v1/sorafs/gateway/fetch";
  private static final int MAX_GATEWAY_REQUEST_BYTES = 32 * 1024 * 1024;
  private static final int MAX_GATEWAY_RESPONSE_BYTES = 16 * 1024 * 1024;
  private static final RequestPayloadEncoder DEFAULT_JSON_ENCODER =
      request -> JsonWriter.encodeBytes(Objects.requireNonNull(request, "request").toJson());

  private static volatile RequestPayloadEncoder requestPayloadEncoder = DEFAULT_JSON_ENCODER;

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;
  private final String fetchPath;

  private SorafsGatewayClient(final Builder builder) {
    this.executor = Objects.requireNonNull(builder.executor, "executor");
    this.baseUri =
        SorafsInputValidator.requireCanonicalGatewayBaseUri(builder.baseUri, "baseUri");
    this.timeout = builder.timeout;
    this.defaultHeaders =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.observers = List.copyOf(builder.observers);
    this.fetchPath =
        SorafsInputValidator.requireCanonicalGatewayFetchPath(builder.fetchPath, "fetchPath");
  }

  /** Creates a new builder with default configuration. */
  public static Builder builder() {
    return new Builder();
  }

  /**
   * Configure a custom payload encoder used when serialising {@link GatewayFetchRequest} instances.
   *
   * <p>This is primarily intended for the Android orchestrator bridge so the same encoder can be
   * reused for HTTP requests and local bindings.
   */
  public static void setRequestPayloadEncoder(final RequestPayloadEncoder encoder) {
    requestPayloadEncoder = Objects.requireNonNull(encoder, "encoder");
  }

  /** Restore the default JSON encoder for gateway requests. */
  public static void resetRequestPayloadEncoder() {
    requestPayloadEncoder = DEFAULT_JSON_ENCODER;
  }

  /** Returns the encoder currently in use for gateway request payloads. */
  public static RequestPayloadEncoder requestPayloadEncoder() {
    return requestPayloadEncoder;
  }

  /** Encode a gateway fetch request using the configured encoder. */
  public static byte[] encodeRequestPayload(final GatewayFetchRequest request) {
    return requestPayloadEncoder.encode(Objects.requireNonNull(request, "request"));
  }

  /**
   * Executes a SoraFS gateway fetch request.
   *
   * @param request structured request built via {@link GatewayFetchRequest}.
   * @return future resolving to the HTTP response.
   * @throws SorafsStorageException when the gateway rejects the request or the transport fails.
   */
  public CompletableFuture<ClientResponse> fetch(final GatewayFetchRequest request) {
    Objects.requireNonNull(request, "request");
    final TransportRequest httpRequest = buildRequest(request);
    notifyRequest(httpRequest);
    final CompletableFuture<ClientResponse> result = new CompletableFuture<>();
    executeBounded(httpRequest)
        .whenComplete((response, throwable) -> {
          if (throwable != null) {
            final Throwable cause =
                throwable instanceof CompletionException ? throwable.getCause() : throwable;
            final SorafsStorageException error =
                cause == null
                    ? new SorafsStorageException("SoraFS gateway fetch request failed")
                    : new SorafsStorageException(
                        "SoraFS gateway fetch request failed", cause);
            notifyFailure(httpRequest, error);
            result.completeExceptionally(error);
            return;
          }
          if (response.body().length > MAX_GATEWAY_RESPONSE_BYTES) {
            final SorafsStorageException error =
                new SorafsStorageException(
                    "SoraFS gateway response exceeds the "
                        + MAX_GATEWAY_RESPONSE_BYTES
                        + "-byte limit");
            notifyFailure(httpRequest, error);
            result.completeExceptionally(error);
            return;
          }
          final ClientResponse clientResponse =
              new ClientResponse(
                  response.statusCode(), response.body(), response.message(), null);
          if (response.statusCode() < 200 || response.statusCode() >= 300) {
            final SorafsStorageException error = errorForResponse(response);
            notifyFailure(httpRequest, error);
            result.completeExceptionally(error);
            return;
          }
          notifyResponse(httpRequest, clientResponse);
          result.complete(clientResponse);
        });
    return result;
  }

  /**
   * Executes a gateway fetch and parses the response JSON into a {@link GatewayFetchSummary}.
   */
  public CompletableFuture<GatewayFetchSummary> fetchSummary(final GatewayFetchRequest request) {
    return fetch(request)
        .thenApply(response -> {
          try {
            return GatewayFetchSummary.fromJsonBytes(response.body());
          } catch (final RuntimeException ex) {
            throw new CompletionException(
                new SorafsStorageException("Failed to parse gateway fetch summary", ex));
          }
        });
  }

  private TransportRequest buildRequest(final GatewayFetchRequest request) {
    final URI target = resolvePath(fetchPath);
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod("POST").setTimeout(timeout);
    mergeHeaders()
        .forEach(
            (name, value) -> {
              builder.addHeader(name, value);
            });
    final byte[] payload = encodeRequestPayload(request);
    if (payload.length > MAX_GATEWAY_REQUEST_BYTES) {
      throw new IllegalArgumentException(
          "SoraFS gateway request exceeds the "
              + MAX_GATEWAY_REQUEST_BYTES
              + "-byte limit");
    }
    builder.setBody(payload);
    return builder.build();
  }

  private CompletableFuture<TransportResponse> executeBounded(final TransportRequest request) {
    if (!(executor instanceof StreamingTransportExecutor)) {
      // Injected non-streaming transports must enforce the same network-read ceiling themselves.
      // The post-materialisation check in fetch() still prevents oversized parsing/use.
      return executor.execute(request);
    }
    final StreamingTransportExecutor streaming = (StreamingTransportExecutor) executor;
    return streaming
        .openStream(request)
        .thenApply(
            response -> {
              try (TransportStreamResponse closeable = response) {
                final Long declaredLength = canonicalContentLength(response);
                if (declaredLength != null && declaredLength > MAX_GATEWAY_RESPONSE_BYTES) {
                  throw new IOException(
                      "SoraFS gateway declared a response larger than "
                          + MAX_GATEWAY_RESPONSE_BYTES
                          + " bytes");
                }
                final int initialCapacity =
                    declaredLength == null ? 8192 : declaredLength.intValue();
                try (ByteArrayOutputStream output = new ByteArrayOutputStream(initialCapacity)) {
                  final byte[] chunk = new byte[8192];
                  int total = 0;
                  while (true) {
                    final int read = response.body().read(chunk);
                    if (read == -1) {
                      break;
                    }
                    if (read == 0) {
                      throw new IOException("SoraFS gateway response stream made no progress");
                    }
                    try {
                      total = Math.addExact(total, read);
                    } catch (final ArithmeticException ex) {
                      throw new IOException("SoraFS gateway response length overflow", ex);
                    }
                    if (total > MAX_GATEWAY_RESPONSE_BYTES) {
                      throw new IOException(
                          "SoraFS gateway response exceeds the "
                              + MAX_GATEWAY_RESPONSE_BYTES
                              + "-byte limit");
                    }
                    output.write(chunk, 0, read);
                  }
                  if (declaredLength != null && declaredLength.longValue() != total) {
                    throw new IOException(
                        "SoraFS gateway response length does not match Content-Length");
                  }
                  return new TransportResponse(
                      response.statusCode(),
                      output.toByteArray(),
                      response.message(),
                      response.headers());
                }
              } catch (final IOException ex) {
                throw new CompletionException(ex);
              }
            });
  }

  private static Long canonicalContentLength(final TransportStreamResponse response)
      throws IOException {
    List<String> values = null;
    int matchingHeaders = 0;
    for (final Map.Entry<String, List<String>> entry : response.headers().entrySet()) {
      if ("Content-Length".equalsIgnoreCase(entry.getKey())) {
        matchingHeaders++;
        values = entry.getValue();
      }
    }
    if (matchingHeaders == 0) {
      return null;
    }
    if (matchingHeaders != 1 || values == null || values.size() != 1) {
      throw new IOException("SoraFS gateway returned ambiguous Content-Length headers");
    }
    final String value = values.get(0);
    if (value == null || value.isEmpty() || (value.length() > 1 && value.charAt(0) == '0')) {
      throw new IOException("SoraFS gateway returned a noncanonical Content-Length");
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (c < '0' || c > '9') {
        throw new IOException("SoraFS gateway returned a noncanonical Content-Length");
      }
    }
    try {
      return Long.valueOf(value);
    } catch (final NumberFormatException ex) {
      throw new IOException("SoraFS gateway returned an overflowing Content-Length", ex);
    }
  }

  private URI resolvePath(final String path) {
    return baseUri.resolve(path);
  }

  private Map<String, String> mergeHeaders() {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Content-Type", "application/json");
    ensureHeader(headers, "Accept", "application/json");
    return headers;
  }

  private SorafsStorageException errorForResponse(
      final TransportResponse response) {
    final StringBuilder message =
        new StringBuilder("SoraFS gateway fetch failed with status ")
            .append(response.statusCode());
    if (!response.message().isEmpty()) {
      message.append(" (").append(response.message()).append(')');
    }
    final byte[] body = response.body();
    if (body.length > 0) {
      final String snippet = new String(body, StandardCharsets.UTF_8);
      if (!snippet.isBlank()) {
        final String trimmed =
            snippet.length() > 256 ? snippet.substring(0, 256) + "…" : snippet;
        message.append(": ").append(trimmed);
      }
    }
    return new SorafsStorageException(message.toString());
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

  private static void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    final String existing = findHeader(headers, name);
    if (existing == null) {
      headers.put(name, value);
      return;
    }
    if (headers.get(existing) == null || headers.get(existing).isBlank()) {
      headers.put(existing, value);
    }
  }

  private static String findHeader(final Map<String, String> headers, final String needle) {
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(needle)) {
        return key;
      }
    }
    return null;
  }

  /** Builder for {@link SorafsGatewayClient}. */
  public static final class Builder {
    private HttpTransportExecutor executor = PlatformHttpTransportExecutor.createDefault();
    private URI baseUri;
    private Duration timeout = Duration.ofSeconds(15);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();
    private String fetchPath = DEFAULT_PATH;

    public Builder setExecutor(final HttpTransportExecutor executor) {
      this.executor = Objects.requireNonNull(executor, "executor");
      return this;
    }

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder setTimeout(final Duration timeout) {
      if (timeout != null) {
        if (timeout.isNegative()) {
          throw new IllegalArgumentException("timeout must be non-negative");
        }
        this.timeout = timeout;
      }
      return this;
    }

    public Builder setFetchPath(final String fetchPath) {
      this.fetchPath =
          SorafsInputValidator.requireCanonicalGatewayFetchPath(fetchPath, "fetchPath");
      return this;
    }

    public Builder putDefaultHeader(final String name, final String value) {
      defaultHeaders.put(
          Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setDefaultHeaders(final Map<String, String> headers) {
      defaultHeaders.clear();
      if (headers != null) {
        headers.forEach(this::putDefaultHeader);
      }
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder setObservers(final List<ClientObserver> observers) {
      this.observers.clear();
      if (observers != null) {
        observers.forEach(this::addObserver);
      }
      return this;
    }

    public SorafsGatewayClient build() {
      if (executor == null) {
        executor = PlatformHttpTransportExecutor.createDefault();
      }
      if (baseUri == null) {
        throw new IllegalStateException("baseUri must be explicitly configured");
      }
      return new SorafsGatewayClient(this);
    }
  }

  /** Strategy for encoding gateway fetch requests into HTTP payloads. */
  @FunctionalInterface
  public interface RequestPayloadEncoder {
    byte[] encode(GatewayFetchRequest request);
  }
}
