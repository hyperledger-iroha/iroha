package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CancellationException;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryObserver;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/**
 * Minimal HTTP client that speaks the Torii Norito RPC surface.
 *
 * <p>The client reuses a configurable {@link HttpTransportExecutor} (defaulting to the platform
 * implementation) and exposes helper builders for per-request overrides so SDK consumers can issue
 * binary Norito RPC payloads alongside the REST pipeline flows.
 */
public final class NoritoRpcClient {

  static final String DEFAULT_ACCEPT = "application/x-norito";
  private static final String DEFAULT_CONTENT_TYPE = "application/x-norito";
  private static final String RPC_CALL_SIGNAL = "android.norito_rpc.call";
  private static final String REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure";

  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final TelemetryOptions telemetryOptions;
  private final TelemetrySink telemetrySink;
  private final NetworkContextProvider networkContextProvider;
  private final DeviceProfileProvider deviceProfileProvider;
  private final HttpTransportExecutor transportExecutor;
  private final List<ClientObserver> observers;
  private final NoritoRpcFlowController flowController;
  private final AtomicBoolean deviceProfileEmitted = new AtomicBoolean(false);

  private NoritoRpcClient(final Builder builder) {
    this.baseUri = Objects.requireNonNull(builder.baseUri, "baseUri");
    this.timeout = builder.timeout;
    this.defaultHeaders = Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.telemetryOptions =
        builder.telemetryOptions != null
            ? builder.telemetryOptions
            : TelemetryOptions.disabled();
    this.telemetrySink = builder.telemetrySink;
    this.networkContextProvider =
        builder.networkContextProvider != null
            ? builder.networkContextProvider
            : NetworkContextProvider.disabled();
    this.deviceProfileProvider =
        builder.deviceProfileProvider != null
            ? builder.deviceProfileProvider
            : DeviceProfileProvider.disabled();
    this.transportExecutor =
        builder.transportExecutor != null
            ? builder.transportExecutor
            : PlatformHttpTransportExecutor.createDefault();
    final List<ClientObserver> observerList = new ArrayList<>(builder.observers);
    if (telemetryOptions.enabled()
        && telemetrySink != null
        && observerList.stream().noneMatch(observer -> observer instanceof TelemetryObserver)) {
      observerList.add(new TelemetryObserver(telemetryOptions, telemetrySink));
    }
    this.observers = List.copyOf(observerList);
    this.flowController =
        builder.flowController != null
            ? builder.flowController
            : NoritoRpcFlowController.unlimited();
  }

  /** Creates a new builder for {@link NoritoRpcClient}. */
  public static Builder builder() {
    return new Builder();
  }

  /** Returns the base URI used to resolve request paths. */
  public URI baseUri() {
    return baseUri;
  }

  /** Issues a Norito RPC call with default request options. */
  public byte[] call(final String path, final byte[] payload) {
    return call(path, payload, null);
  }

  /**
   * Issues a Norito RPC call with optional overrides (headers, method, timeout, etc.).
   */
  public byte[] call(
      final String path, final byte[] payload, final NoritoRpcRequestOptions options) {
    Objects.requireNonNull(path, "path");
    final NoritoRpcRequestOptions resolved =
        options != null ? options : NoritoRpcRequestOptions.defaultOptions();
    final URI target = appendQueryParameters(resolvePath(path), resolved.queryParameters());
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod(resolved.method());
    final Duration requestTimeout = pickTimeout(resolved.timeout());
    builder.setTimeout(requestTimeout);
    mergeHeaders(resolved).forEach(builder::addHeader);
    final byte[] requestPayload = payload == null ? null : payload.clone();
    builder.setBody(requestPayload);
    final TransportRequest request = builder.build();
    final NoritoRpcCallContext context =
        new NoritoRpcCallContext(baseUri, target, path, requestPayload, resolved, request);
    final long startNano = System.nanoTime();
    emitDeviceProfileTelemetry();
    emitNetworkContextTelemetry();
    notifyRequest(request);
    try {
      flowController.acquire();
      try {
        final TransportResponse response = executeRequest(request);
        final long elapsedMillis = elapsedMillis(startNano);
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
          final String message =
              "Norito RPC request failed with status "
                  + response.statusCode()
                  + (response.body().length == 0
                      ? ""
                      : ": " + new String(response.body(), StandardCharsets.UTF_8));
          final NoritoRpcException error = new NoritoRpcException(message);
          notifyFailure(request, error);
          emitRpcCallTelemetry(
              request,
              response.statusCode(),
              "failure",
              error.getClass().getSimpleName(),
              elapsedMillis,
              false);
          return handleFallback(context, error, request, startNano);
        }
        final ClientResponse clientResponse =
            new ClientResponse(response.statusCode(), response.body());
        notifyResponse(request, clientResponse);
        emitRpcCallTelemetry(
            request, response.statusCode(), "success", null, elapsedMillis, false);
        return response.body();
      } finally {
        flowController.release();
      }
    } catch (final InterruptedException ex) {
      Thread.currentThread().interrupt();
      final NoritoRpcException error =
          new NoritoRpcException("Norito RPC request interrupted", ex);
      notifyFailure(request, error);
      emitRpcCallTelemetry(
          request,
          null,
          "failure",
          error.getClass().getSimpleName(),
          elapsedMillis(startNano),
          false);
      return handleFallback(context, error, request, startNano);
    } catch (final IOException ex) {
      final NoritoRpcException error = new NoritoRpcException("Norito RPC request failed", ex);
      notifyFailure(request, error);
      emitRpcCallTelemetry(
          request,
          null,
          "failure",
          error.getClass().getSimpleName(),
          elapsedMillis(startNano),
          false);
      return handleFallback(context, error, request, startNano);
    }
  }

  /**
   * Issues a Norito RPC call by encoding/decoding the payload via the provided codec adapter.
   *
   * @throws NoritoException when encoding or decoding fails
   */
  public TransactionPayload callTransaction(
      final String path,
      final TransactionPayload payload,
      final NoritoCodecAdapter codec,
      final NoritoRpcRequestOptions options)
      throws NoritoException {
    Objects.requireNonNull(codec, "codec");
    final TransactionPayload requestPayload =
        Objects.requireNonNull(payload, "payload");
    final byte[] encoded = codec.encodeTransaction(requestPayload);
    final byte[] response = call(path, encoded, options);
    return codec.decodeTransaction(response);
  }

  private void emitDeviceProfileTelemetry() {
    if (!telemetryOptions.enabled()) {
      return;
    }
    if (!deviceProfileEmitted.compareAndSet(false, true)) {
      return;
    }
    if (telemetrySink == null) {
      return;
    }
    final Optional<DeviceProfile> profile = deviceProfileProvider.snapshot();
    if (profile.isEmpty()) {
      return;
    }
    telemetrySink.emitSignal(
        "android.telemetry.device_profile",
        Map.of("profile_bucket", profile.get().bucket()));
  }

  private void emitNetworkContextTelemetry() {
    if (!telemetryOptions.enabled() || telemetrySink == null) {
      return;
    }
    final Optional<NetworkContext> context = networkContextProvider.snapshot();
    if (context.isEmpty()) {
      return;
    }
    telemetrySink.emitSignal(
        "android.telemetry.network_context", context.get().toTelemetryFields());
  }

  private void emitRpcCallTelemetry(
      final TransportRequest request,
      final Integer statusCode,
      final String outcome,
      final String errorKind,
      final long latencyMillis,
      final boolean fallback) {
    if (!telemetryOptions.enabled() || telemetrySink == null) {
      return;
    }
    final Map<String, Object> fields = new LinkedHashMap<>();
    maybePutAuthorityHash(fields, request);
    fields.put("route", resolveRoute(request));
    fields.put("method", resolveMethod(request));
    fields.put("outcome", outcome == null ? "" : outcome);
    if (statusCode != null) {
      fields.put("status_code", statusCode);
    }
    if (errorKind != null && !errorKind.isBlank()) {
      fields.put("error_kind", errorKind);
    }
    fields.put("latency_ms", latencyMillis);
    fields.put("fallback", fallback);
    telemetrySink.emitSignal(RPC_CALL_SIGNAL, fields);
  }

  private void maybePutAuthorityHash(
      final Map<String, Object> fields, final TransportRequest request) {
    final TelemetryOptions.Redaction redaction = telemetryOptions.redaction();
    if (!redaction.enabled()) {
      return;
    }
    final String authority = resolveAuthority(request);
    if (authority.isBlank()) {
      emitRedactionFailure("blank_authority");
      return;
    }
    redaction
        .hashAuthority(authority)
        .ifPresentOrElse(
            hashed -> fields.put("authority_hash", hashed),
            () -> emitRedactionFailure("hash_failed"));
  }

  private static long elapsedMillis(final long startNano) {
    return (System.nanoTime() - startNano) / 1_000_000L;
  }

  private static String resolveRoute(final TransportRequest request) {
    final URI uri = request == null ? null : request.uri();
    if (uri == null) {
      return "";
    }
    final String path = uri.getRawPath();
    return path == null ? "" : path;
  }

  private static String resolveMethod(final TransportRequest request) {
    return request == null ? "" : nullToEmpty(request.method());
  }

  private static String resolveAuthority(final TransportRequest request) {
    if (request == null) {
      return "";
    }
    final URI uri = request.uri();
    if (uri != null && uri.getAuthority() != null) {
      return uri.getAuthority().trim();
    }
    final List<String> hosts = request.headers().get("Host");
    return hosts == null || hosts.isEmpty() ? "" : hosts.get(0).trim();
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }

  private void emitRedactionFailure(final String reason) {
    if (telemetrySink == null) {
      return;
    }
    telemetrySink.emitSignal(
        REDACTION_FAILURE_SIGNAL, Map.of("signal_id", RPC_CALL_SIGNAL, "reason", reason));
  }

  private TransportResponse executeRequest(final TransportRequest request)
      throws IOException, InterruptedException {
    final CompletableFuture<TransportResponse> future;
    try {
      future = transportExecutor.execute(request);
    } catch (final RuntimeException ex) {
      throw new IOException("Norito RPC executor rejected request", ex);
    }
    try {
      return future.get();
    } catch (final InterruptedException ex) {
      Thread.currentThread().interrupt();
      throw ex;
    } catch (final ExecutionException ex) {
      final Throwable cause = ex.getCause();
      if (cause instanceof IOException) {
        throw (IOException) cause;
      }
      if (cause instanceof InterruptedException) {
        Thread.currentThread().interrupt();
        throw (InterruptedException) cause;
      }
      throw new IOException("Norito RPC executor failed", cause);
    } catch (final CancellationException ex) {
      throw new IOException("Norito RPC executor cancelled request", ex);
    }
  }

  private Duration pickTimeout(final Duration override) {
    if (override != null) {
      return override.isNegative() ? Duration.ZERO : override;
    }
    return timeout;
  }

  private Map<String, String> mergeHeaders(final NoritoRpcRequestOptions options) {
    final Map<String, String> merged = new LinkedHashMap<>(defaultHeaders);
    options.headers().forEach(merged::put);
    ensureContentType(merged);
    applyAcceptHeader(merged, options);
    return merged;
  }

  private static void ensureContentType(final Map<String, String> headers) {
    if (findHeader(headers, "Content-Type") == null) {
      headers.put("Content-Type", DEFAULT_CONTENT_TYPE);
    }
  }

  private static void applyAcceptHeader(
      final Map<String, String> headers, final NoritoRpcRequestOptions options) {
    if (options.acceptConfigured()) {
      final String currentKey = findHeader(headers, "Accept");
      if (options.accept() == null) {
        if (currentKey != null) {
          headers.remove(currentKey);
        }
      } else {
        if (currentKey != null) {
          headers.put(currentKey, options.accept());
        } else {
          headers.put("Accept", options.accept());
        }
      }
      return;
    }
    if (findHeader(headers, "Accept") == null) {
      headers.put("Accept", DEFAULT_ACCEPT);
    }
  }

  private static String findHeader(final Map<String, String> headers, final String needle) {
    for (final String name : headers.keySet()) {
      if (name.equalsIgnoreCase(needle)) {
        return name;
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

  private static URI appendQueryParameters(final URI target, final Map<String, String> params) {
    if (params.isEmpty()) {
      return target;
    }
    final StringBuilder builder = new StringBuilder(target.toString());
    final String query = encodeQuery(params);
    if (target.getQuery() == null || target.getQuery().isEmpty()) {
      builder.append(target.toString().contains("?") ? "&" : "?");
    } else {
      builder.append("&");
    }
    builder.append(query);
    return URI.create(builder.toString());
  }

  private static String encodeQuery(final Map<String, String> params) {
    final StringBuilder builder = new StringBuilder();
    boolean first = true;
    for (final Map.Entry<String, String> entry : params.entrySet()) {
      if (!first) {
        builder.append('&');
      }
      builder
          .append(URLEncoder.encode(entry.getKey(), StandardCharsets.UTF_8))
          .append('=')
          .append(URLEncoder.encode(entry.getValue(), StandardCharsets.UTF_8));
      first = false;
    }
    return builder.toString();
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

  private void notifyFailure(final TransportRequest request, final RuntimeException error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private byte[] handleFallback(
      final NoritoRpcCallContext context,
      final NoritoRpcException error,
      final TransportRequest request,
      final long startNano) {
    final byte[] fallbackResponse = attemptFallback(context, error);
    emitRpcCallTelemetry(
        request,
        FALLBACK_STATUS_CODE,
        "fallback",
        error.getClass().getSimpleName(),
        elapsedMillis(startNano),
        true);
    return fallbackResponse;
  }

  private byte[] attemptFallback(
      final NoritoRpcCallContext context, final NoritoRpcException error) {
    if (fallbackHandler == null) {
      throw error;
    }
    final byte[] fallbackResponse;
    try {
      fallbackResponse = fallbackHandler.handle(context, error);
    } catch (final RuntimeException runtime) {
      throw runtime;
    } catch (final Exception ex) {
      throw new NoritoRpcException("Norito RPC fallback handler failed", ex);
    }
    if (fallbackResponse == null) {
      throw error;
    }
    final ClientResponse clientResponse =
        new ClientResponse(FALLBACK_STATUS_CODE, fallbackResponse);
    notifyResponse(context.request(), clientResponse);
    return fallbackResponse;
  }

  /** Builder for {@link NoritoRpcClient}. */
  public static final class Builder {
    private URI baseUri = URI.create("http://localhost:8080");
    private Duration timeout = Duration.ofSeconds(10);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private TelemetryOptions telemetryOptions = TelemetryOptions.disabled();
    private TelemetrySink telemetrySink;
    private NetworkContextProvider networkContextProvider = NetworkContextProvider.disabled();
    private DeviceProfileProvider deviceProfileProvider = DeviceProfileProvider.disabled();
    private HttpTransportExecutor transportExecutor;
    private final List<ClientObserver> observers = new ArrayList<>();
    private NoritoRpcFlowController flowController = NoritoRpcFlowController.unlimited();
    private NoritoRpcFallbackHandler fallbackHandler;

    private Builder() {}

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder setTimeout(final Duration timeout) {
      if (timeout == null) {
        this.timeout = null;
      } else {
        this.timeout = timeout.isNegative() ? Duration.ZERO : timeout;
      }
      return this;
    }

    public Builder putDefaultHeader(final String name, final String value) {
      defaultHeaders.put(
          Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder defaultHeaders(final Map<String, String> headers) {
      defaultHeaders.clear();
      if (headers != null) {
        headers.forEach(this::putDefaultHeader);
      }
      return this;
    }

    public Builder setTelemetryOptions(final TelemetryOptions telemetryOptions) {
      this.telemetryOptions = Objects.requireNonNull(telemetryOptions, "telemetryOptions");
      return this;
    }

    public Builder setTelemetrySink(final TelemetrySink telemetrySink) {
      this.telemetrySink = telemetrySink;
      return this;
    }

    public Builder setNetworkContextProvider(final NetworkContextProvider provider) {
      this.networkContextProvider = Objects.requireNonNull(provider, "networkContextProvider");
      return this;
    }

    public Builder setDeviceProfileProvider(final DeviceProfileProvider provider) {
      this.deviceProfileProvider = Objects.requireNonNull(provider, "deviceProfileProvider");
      return this;
    }

    public Builder setTransportExecutor(final HttpTransportExecutor executor) {
      this.transportExecutor = executor;
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder observers(final List<ClientObserver> observers) {
      this.observers.clear();
      if (observers != null) {
        observers.forEach(this::addObserver);
      }
      return this;
    }

    public Builder setFlowController(final NoritoRpcFlowController flowController) {
      this.flowController = Objects.requireNonNull(flowController, "flowController");
      return this;
    }

    public Builder setMaxConcurrentRequests(final int maxConcurrentRequests) {
      this.flowController = NoritoRpcFlowController.semaphore(maxConcurrentRequests);
      return this;
    }

    public Builder setFallbackHandler(final NoritoRpcFallbackHandler fallbackHandler) {
      this.fallbackHandler = fallbackHandler;
      return this;
    }

    public NoritoRpcClient build() {
      return new NoritoRpcClient(this);
    }
  }
}
