package org.hyperledger.iroha.android.client.stream;

import java.io.BufferedReader;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.URI;
import java.net.URLDecoder;
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
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.TransportSecurity;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;

/**
 * Minimal streaming client used to consume Torii server-sent event (SSE) feeds. The implementation
 * shares the same configuration surface as {@link org.hyperledger.iroha.android.client.HttpClientTransport}
 * so telemetry observers and authentication headers behave consistently across transports.
 */
public final class ToriiEventStreamClient {

  private static final String EVENT_STREAM_CONTENT_TYPE = "text/event-stream";
  private static final String DEFAULT_EVENT_NAME = "message";

  private final URI baseUri;
  private final TransportExecutor transport;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private ToriiEventStreamClient(final Builder builder) {
    this.baseUri = builder.baseUri;
    this.transport = builder.transport;
    this.defaultHeaders = Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.observers = List.copyOf(builder.observers);
  }

  public static Builder builder() {
    return new Builder();
  }

  /**
    * Opens an SSE stream against {@code path} using the supplied options.
    *
    * <p>Callers must close the returned {@link ToriiEventStream}; the listener is notified when the
    * stream receives frames, fails, or terminates.
    *
    * <p>When the configured transport supports streaming responses, frames are parsed as they
    * arrive; otherwise, the response body is buffered before parsing.
    */
  public ToriiEventStream openSseStream(
      final String path,
      final ToriiEventStreamOptions options,
      final ToriiEventStreamListener listener) {
    Objects.requireNonNull(listener, "listener");
    final ToriiEventStreamOptions resolved =
        options != null ? options : ToriiEventStreamOptions.defaultOptions();
    final TransportRequest request = buildRequest(path, resolved);
    notifyRequest(request);
    if (transport instanceof StreamingTransportExecutor streaming) {
      final CompletableFuture<TransportStreamResponse> responseFuture = streaming.openStream(request);
      final ActiveStream stream = new ActiveStream(request, responseFuture);
      responseFuture.whenComplete((response, throwable) ->
          handleStreamResponse(request, listener, stream, response, throwable));
      return stream;
    }

    final CompletableFuture<TransportResponse> responseFuture = transport.execute(request);
    final ActiveStream stream = new ActiveStream(request, responseFuture);
    responseFuture.whenComplete((response, throwable) ->
        handleBufferedResponse(request, listener, stream, response, throwable));
    return stream;
  }

  private TransportRequest buildRequest(final String path, final ToriiEventStreamOptions options) {
    final URI target =
        appendQueryParameters(
            normalizeEventSseUriFilter(resolvePath(path)),
            normalizeEventSseQueryParameters(options.queryParameters()));
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    headers.putIfAbsent("Accept", EVENT_STREAM_CONTENT_TYPE);
    headers.putIfAbsent("Cache-Control", "no-cache");
    headers.putIfAbsent("Connection", "keep-alive");
    options.headers().forEach(headers::put);
    TransportSecurity.requireHttpRequestAllowed(
        "ToriiEventStreamClient", baseUri, target, headers, null);
    final TransportRequest.Builder builder = TransportRequest.builder().setUri(target).setMethod("GET");
    Duration timeout = options.timeout();
    if (timeout != null) {
      builder.setTimeout(timeout);
    }
    headers.forEach(builder::addHeader);
    return builder.build();
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

  private static URI normalizeEventSseUriFilter(final URI target) {
    final String rawQuery = target.getRawQuery();
    if (rawQuery == null || rawQuery.isEmpty()) {
      return target;
    }
    final String normalizedQuery = normalizeEventSseQuery(rawQuery);
    if (normalizedQuery.equals(rawQuery)) {
      return target;
    }
    final String targetText = target.toString();
    final int fragmentIndex = targetText.indexOf('#');
    final String withoutFragment =
        fragmentIndex >= 0 ? targetText.substring(0, fragmentIndex) : targetText;
    final String fragment = fragmentIndex >= 0 ? targetText.substring(fragmentIndex) : "";
    final int queryIndex = withoutFragment.indexOf('?');
    final String base = queryIndex >= 0 ? withoutFragment.substring(0, queryIndex) : withoutFragment;
    return URI.create(base + "?" + normalizedQuery + fragment);
  }

  private static Map<String, String> normalizeEventSseQueryParameters(
      final Map<String, String> params) {
    if (!params.containsKey("filter")) {
      return params;
    }
    final Map<String, String> normalized = new LinkedHashMap<>(params);
    normalized.put("filter", normalizeEventFilterPayload(params.get("filter"), "eventFilter"));
    return normalized;
  }

  private static String normalizeEventSseQuery(final String rawQuery) {
    final String[] segments = rawQuery.split("&", -1);
    boolean changed = false;
    for (int i = 0; i < segments.length; i++) {
      final String segment = segments[i];
      if (segment.isEmpty()) {
        continue;
      }
      final int equals = segment.indexOf('=');
      final String rawName = equals >= 0 ? segment.substring(0, equals) : segment;
      final String rawValue = equals >= 0 ? segment.substring(equals + 1) : "";
      final String name = decodeQueryComponent(rawName);
      if (!"filter".equals(name)) {
        continue;
      }
      final String value = decodeQueryComponent(rawValue);
      final String normalized = normalizeEventFilterPayload(value, "eventFilter");
      if (!normalized.equals(value)) {
        segments[i] =
            URLEncoder.encode(name, StandardCharsets.UTF_8)
                + "="
                + URLEncoder.encode(normalized, StandardCharsets.UTF_8);
        changed = true;
      }
    }
    return changed ? String.join("&", segments) : rawQuery;
  }

  private static String decodeQueryComponent(final String value) {
    return URLDecoder.decode(value.replace("+", " "), StandardCharsets.UTF_8);
  }

  @SuppressWarnings("unchecked")
  private static String normalizeEventFilterPayload(final String filter, final String context) {
    final String trimmed = filter == null ? "" : filter.trim();
    if (trimmed.isEmpty() || (trimmed.charAt(0) != '{' && trimmed.charAt(0) != '[')) {
      return filter;
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(trimmed);
    } catch (final IllegalStateException ex) {
      return filter;
    }
    if (!(parsed instanceof Map<?, ?>)) {
      return filter;
    }
    final Map<String, Object> object = (Map<String, Object>) parsed;
    return normalizeProductionEventFilterObject(object, context)
        ? JsonEncoder.encode(object)
        : filter;
  }

  @SuppressWarnings("unchecked")
  private static boolean normalizeProductionEventFilterObject(
      final Map<String, Object> filter, final String context) {
    boolean changed = false;
    for (final String eventKind : List.of("VerifyingKey", "Proof")) {
      final Object bodyValue = filter.get(eventKind);
      if (!(bodyValue instanceof Map<?, ?>)) {
        continue;
      }
      final Map<String, Object> body = (Map<String, Object>) bodyValue;
      final Object matcherValue = body.get("id_matcher");
      if (!(matcherValue instanceof Map<?, ?>)) {
        continue;
      }
      final Map<String, Object> matcher = (Map<String, Object>) matcherValue;
      if (!matcher.containsKey("backend")) {
        continue;
      }
      final String backendContext = context + "." + eventKind + ".id_matcher.backend";
      final Object backendValue = matcher.get("backend");
      if (!(backendValue instanceof String backend)) {
        throw new IllegalArgumentException(backendContext + " must be a string");
      }
      final String normalizedBackend =
          VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend, backendContext);
      if (!normalizedBackend.equals(backend)) {
        matcher.put("backend", normalizedBackend);
        changed = true;
      }
      if ("Proof".equals(eventKind)) {
        changed =
            normalizeProofHashMatcher(
                    matcher, "hash_hex", context + "." + eventKind + ".id_matcher.hash_hex")
                || changed;
        changed =
            normalizeProofHashMatcher(
                    matcher,
                    "proof_hash_hex",
                    context + "." + eventKind + ".id_matcher.proof_hash_hex")
                || changed;
      } else {
        changed =
            normalizeVerifyingKeyNameMatcher(
                    matcher, context + "." + eventKind + ".id_matcher.name")
                || changed;
      }
    }
    return changed;
  }

  private static boolean normalizeVerifyingKeyNameMatcher(
      final Map<String, Object> matcher, final String context) {
    if (!matcher.containsKey("name")) {
      return false;
    }
    final Object rawValue = matcher.get("name");
    if (!(rawValue instanceof String raw)) {
      throw new IllegalArgumentException(context + " must be a string");
    }
    final String normalized = raw.trim();
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException(context + " must be a non-empty string");
    }
    if (normalized.contains(":")) {
      throw new IllegalArgumentException(context + " must not contain ':' characters");
    }
    if (normalized.equals(raw)) {
      return false;
    }
    matcher.put("name", normalized);
    return true;
  }

  private static boolean normalizeProofHashMatcher(
      final Map<String, Object> matcher, final String propertyName, final String context) {
    if (!matcher.containsKey(propertyName)) {
      return false;
    }
    final Object rawValue = matcher.get(propertyName);
    if (!(rawValue instanceof String raw)) {
      throw new IllegalArgumentException(context + " must be a string");
    }
    final String normalized = normalizeHex32String(raw, context);
    if (normalized.equals(raw)) {
      return false;
    }
    matcher.put(propertyName, normalized);
    return true;
  }

  private static String normalizeHex32String(final String raw, final String context) {
    String normalized = raw.trim().toLowerCase(java.util.Locale.ROOT);
    if (normalized.startsWith("0x")) {
      normalized = normalized.substring(2);
    }
    if (normalized.length() != 64 || !isLowerHex(normalized)) {
      throw new IllegalArgumentException(context + " must be a 32-byte hex string");
    }
    return normalized;
  }

  private static boolean isLowerHex(final String value) {
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) {
        return false;
      }
    }
    return true;
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

  private void parseEventStream(
      final InputStream stream,
      final ToriiEventStreamListener listener,
      final ActiveStream activeStream) {
    try (BufferedReader reader =
        new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
      final StringBuilder data = new StringBuilder();
      String eventName = null;
      String eventId = null;
      String line;
      while (!activeStream.closed() && (line = reader.readLine()) != null) {
        if (line.isEmpty()) {
          dispatchEvent(listener, data, eventName, eventId);
          eventName = null;
          eventId = null;
          continue;
        }
        if (line.startsWith(":")) {
          continue;
        }
        final int colon = line.indexOf(':');
        final String field;
        final String value;
        if (colon == -1) {
          field = line;
          value = "";
        } else {
          field = line.substring(0, colon);
          final String raw = line.substring(colon + 1);
          value = raw.startsWith(" ") ? raw.substring(1) : raw;
        }
        switch (field) {
          case "data":
            data.append(value).append('\n');
            break;
          case "event":
            eventName = value;
            break;
          case "id":
            eventId = value;
            break;
          case "retry":
            try {
              final long millis = Long.parseLong(value);
              if (millis >= 0) {
                listener.onRetryHint(Duration.ofMillis(millis));
              }
            } catch (final NumberFormatException ignored) {
              // Ignore invalid retry values.
            }
            break;
          default:
            break;
        }
      }
      dispatchEvent(listener, data, eventName, eventId);
    } catch (final IOException ex) {
      if (!activeStream.closed()) {
        throw new RuntimeException("Failed to parse Torii SSE stream", ex);
      }
    }
  }

  private void handleBufferedResponse(
      final TransportRequest request,
      final ToriiEventStreamListener listener,
      final ActiveStream stream,
      final TransportResponse response,
      final Throwable throwable) {
    if (throwable != null) {
      final Throwable cause = unwrapCompletion(throwable);
      if (cause instanceof CancellationException && stream.closedByCaller()) {
        stream.signalSuccess();
        return;
      }
      stream.signalFailure(cause);
      notifyFailure(request, cause);
      listener.onError(cause);
      return;
    }
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String message =
          response.body().length == 0 ? "" : new String(response.body(), StandardCharsets.UTF_8);
      final IOException error =
          new IOException(
              "Torii SSE request failed with status "
                  + response.statusCode()
                  + (message.isEmpty() ? "" : ": " + message));
      stream.signalFailure(error);
      notifyFailure(request, error);
      listener.onError(error);
      return;
    }

    final ClientResponse clientResponse = new ClientResponse(response.statusCode(), new byte[0]);
    notifyResponse(request, clientResponse);
    listener.onOpen();

    final CompletableFuture<Void> readerFuture =
        CompletableFuture.runAsync(
            () -> parseEventStream(new ByteArrayInputStream(response.body()), listener, stream));
    stream.attach(readerFuture);
    readerFuture.whenComplete((ignored, parseError) ->
        handleParseCompletion(request, listener, stream, parseError));
  }

  private void handleStreamResponse(
      final TransportRequest request,
      final ToriiEventStreamListener listener,
      final ActiveStream stream,
      final TransportStreamResponse response,
      final Throwable throwable) {
    if (throwable != null) {
      final Throwable cause = unwrapCompletion(throwable);
      if (cause instanceof CancellationException && stream.closedByCaller()) {
        stream.signalSuccess();
        return;
      }
      stream.signalFailure(cause);
      notifyFailure(request, cause);
      listener.onError(cause);
      return;
    }
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String message = readBody(response);
      final IOException error =
          new IOException(
              "Torii SSE request failed with status "
                  + response.statusCode()
                  + (message.isEmpty() ? "" : ": " + message));
      stream.signalFailure(error);
      notifyFailure(request, error);
      listener.onError(error);
      return;
    }

    final ClientResponse clientResponse = new ClientResponse(response.statusCode(), new byte[0]);
    notifyResponse(request, clientResponse);
    listener.onOpen();
    stream.attachStream(response);

    final CompletableFuture<Void> readerFuture =
        CompletableFuture.runAsync(() -> parseEventStream(response.body(), listener, stream));
    stream.attach(readerFuture);
    readerFuture.whenComplete((ignored, parseError) ->
        handleParseCompletion(request, listener, stream, parseError));
  }

  private void handleParseCompletion(
      final TransportRequest request,
      final ToriiEventStreamListener listener,
      final ActiveStream stream,
      final Throwable parseError) {
    stream.closeStreamResponse();
    if (parseError != null && !(parseError instanceof CancellationException)) {
      stream.signalFailure(parseError);
      notifyFailure(request, parseError);
      listener.onError(parseError);
    } else if (!stream.closedByCaller()) {
      listener.onClosed();
      stream.signalSuccess();
    } else {
      stream.signalSuccess();
    }
  }

  private static String readBody(final TransportStreamResponse response) {
    final byte[] data;
    try (InputStream body = response.body(); ByteArrayOutputStream buffer = new ByteArrayOutputStream()) {
      final byte[] chunk = new byte[4096];
      int read;
      while ((read = body.read(chunk)) != -1) {
        buffer.write(chunk, 0, read);
      }
      data = buffer.toByteArray();
    } catch (final IOException ignored) {
      return "";
    }
    return data.length == 0 ? "" : new String(data, StandardCharsets.UTF_8);
  }

  private static void dispatchEvent(
      final ToriiEventStreamListener listener,
      final StringBuilder data,
      final String eventName,
      final String eventId) {
    if (data.length() == 0 && eventName == null && eventId == null) {
      return;
    }
    if (data.length() > 0 && data.charAt(data.length() - 1) == '\n') {
      data.deleteCharAt(data.length() - 1);
    }
    final String payload = data.toString();
    data.setLength(0);
    final String name = eventName == null || eventName.isEmpty() ? DEFAULT_EVENT_NAME : eventName;
    listener.onEvent(new ServerSentEvent(name, payload, eventId));
  }

  private static Throwable unwrapCompletion(final Throwable error) {
    if (error instanceof java.util.concurrent.CompletionException completion
        && completion.getCause() != null) {
      return completion.getCause();
    }
    return error;
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

  public static final class Builder {
    private URI baseUri = URI.create("http://localhost:8080");
    private TransportExecutor transport = PlatformHttpTransportExecutor.createDefault();
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();

    private Builder() {}

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder setTransportExecutor(final TransportExecutor transport) {
      this.transport = Objects.requireNonNull(transport, "transport");
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

    public Builder addObserver(final ClientObserver observer) {
      observers.add(Objects.requireNonNull(observer, "observer"));
      return this;
    }

    public Builder observers(final List<ClientObserver> values) {
      observers.clear();
      if (values != null) {
        values.forEach(this::addObserver);
      }
      return this;
    }

    public ToriiEventStreamClient build() {
      if (transport == null) {
        transport = PlatformHttpTransportExecutor.createDefault();
      }
      return new ToriiEventStreamClient(this);
    }
  }

  private static final class ActiveStream implements ToriiEventStream {
    private final TransportRequest request;
    private final CompletableFuture<?> responseFuture;
    private final CompletableFuture<Void> completion = new CompletableFuture<>();
    private final AtomicBoolean closed = new AtomicBoolean(false);
    private final AtomicBoolean closedByCaller = new AtomicBoolean(false);
    private final AtomicReference<TransportStreamResponse> streamResponse = new AtomicReference<>(null);

    private volatile CompletableFuture<Void> readerFuture;

    ActiveStream(final TransportRequest request, final CompletableFuture<?> responseFuture) {
      this.request = request;
      this.responseFuture = responseFuture;
    }

    void attach(final CompletableFuture<Void> readerFuture) {
      this.readerFuture = readerFuture;
    }

    void attachStream(final TransportStreamResponse response) {
      streamResponse.set(response);
    }

    void closeStreamResponse() {
      final TransportStreamResponse response = streamResponse.getAndSet(null);
      if (response != null) {
        response.close();
      }
    }

    void signalFailure(final Throwable error) {
      completion.completeExceptionally(error);
    }

    void signalSuccess() {
      completion.complete(null);
    }

    boolean closed() {
      return closed.get();
    }

    boolean closedByCaller() {
      return closedByCaller.get();
    }

    @Override
    public boolean isOpen() {
      return !closed.get() && !completion.isDone();
    }

    @Override
    public CompletableFuture<Void> completion() {
      return completion;
    }

    @Override
    public void close() {
      if (!closed.compareAndSet(false, true)) {
        return;
      }
      closedByCaller.set(true);
      final TransportStreamResponse response = streamResponse.getAndSet(null);
      if (response != null) {
        response.close();
      }
      if (readerFuture != null) {
        readerFuture.cancel(true);
      }
      responseFuture.cancel(true);
      completion.complete(null);
    }
  }

}
