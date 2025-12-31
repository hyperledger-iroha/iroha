package org.hyperledger.iroha.android.client.websocket;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;

/**
 * WebSocket client built on the transport abstractions shared between JVM and Android targets.
 *
 * <p>By default the platform connector is used (OkHttp on Android, JDK connector on JVM). Android
 * callers can still inject an explicit OkHttp connector to reuse a shared client instance.
 */
public final class ToriiWebSocketClient {

  /** Connector hook so platforms can supply their preferred WebSocket implementation. */
  @FunctionalInterface
  public interface WebSocketConnector {
    CompletableFuture<TransportWebSocket> connect(
        TransportRequest request,
        ToriiWebSocketOptions options,
        TransportWebSocket.Listener listener,
        Map<String, String> defaultHeaders);
  }

  private final URI baseUri;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;
  private final WebSocketConnector connector;

  private ToriiWebSocketClient(final Builder builder) {
    this.baseUri = builder.baseUri;
    this.defaultHeaders = Map.copyOf(builder.defaultHeaders);
    this.observers = List.copyOf(builder.observers);
    this.connector =
        builder.connector != null
            ? builder.connector
            : PlatformWebSocketConnector.createDefault();
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Opens a WebSocket session against the given path. */
  public ToriiWebSocketSession connect(
      final String path,
      final ToriiWebSocketOptions options,
      final ToriiWebSocketListener listener) {
    Objects.requireNonNull(listener, "listener");
    final ToriiWebSocketOptions resolved =
        options != null ? options : ToriiWebSocketOptions.defaultOptions();
    final TransportRequest request = buildRequest(path, resolved);
    notifyRequest(request);
    final ToriiWebSocketSessionImpl session = new ToriiWebSocketSessionImpl();
    final TransportWebSocket.Listener adapter = new Adapter(listener, session);
    connector
        .connect(request, resolved, adapter, defaultHeaders)
        .whenComplete(
            (socket, throwable) -> {
              if (throwable != null) {
                session.fail(throwable);
                notifyFailure(request, throwable);
                listener.onError(session, throwable);
                return;
              }
              session.attach(socket);
              notifyResponse(request);
            });
    return session;
  }

  private TransportRequest buildRequest(final String path, final ToriiWebSocketOptions options) {
    final URI httpUri = appendQueryParameters(resolvePath(path), options.queryParameters());
    final URI wsUri = toWebSocketUri(httpUri);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setMethod("GET")
            .setUri(wsUri)
            .setTimeout(options.connectTimeout());
    defaultHeaders.forEach(builder::addHeader);
    options.headers().forEach(builder::addHeader);
    if (!options.subprotocols().isEmpty()) {
      builder.addHeader("Sec-WebSocket-Protocol", String.join(",", options.subprotocols()));
    }
    return builder.build();
  }

  private URI resolvePath(final String path) {
    if (path == null || path.isBlank()) {
      return baseUri;
    }
    if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("ws://") || path.startsWith("wss://")) {
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

  private static URI toWebSocketUri(final URI httpUri) {
    final String scheme =
        "https".equalsIgnoreCase(httpUri.getScheme()) || "wss".equalsIgnoreCase(httpUri.getScheme())
            ? "wss"
            : "ws";
    return URI.create(
        scheme
            + "://"
            + httpUri.getAuthority()
            + (httpUri.getRawPath() == null ? "" : httpUri.getRawPath())
            + (httpUri.getRawQuery() == null ? "" : "?" + httpUri.getRawQuery()));
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(final TransportRequest request) {
    final ClientResponse response =
        new ClientResponse(101, new byte[0], "websocket_handshake", null);
    for (final ClientObserver observer : observers) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private static final class Adapter implements TransportWebSocket.Listener {
    private final ToriiWebSocketListener delegate;
    private final ToriiWebSocketSessionImpl session;

    Adapter(
        final ToriiWebSocketListener delegate, final ToriiWebSocketSessionImpl session) {
      this.delegate = delegate;
      this.session = session;
    }

    @Override
    public void onOpen(final TransportWebSocket socket) {
      delegate.onOpen(session);
    }

    @Override
    public void onText(
        final TransportWebSocket socket, final CharSequence data, final boolean last) {
      delegate.onText(session, data, last);
    }

    @Override
    public void onBinary(
        final TransportWebSocket socket, final java.nio.ByteBuffer data, final boolean last) {
      delegate.onBinary(session, data, last);
    }

    @Override
    public void onPing(final TransportWebSocket socket, final java.nio.ByteBuffer message) {
      delegate.onPing(session, message);
    }

    @Override
    public void onPong(final TransportWebSocket socket, final java.nio.ByteBuffer message) {
      delegate.onPong(session, message);
    }

    @Override
    public void onError(final TransportWebSocket socket, final Throwable error) {
      delegate.onError(session, error);
      session.fail(error);
    }

    @Override
    public void onClose(
        final TransportWebSocket socket, final int statusCode, final String reason) {
      delegate.onClose(session, statusCode, reason);
      session.close(statusCode, reason);
    }
  }

  private static final class ToriiWebSocketSessionImpl implements ToriiWebSocketSession {
    private final CompletableFuture<TransportWebSocket> ready = new CompletableFuture<>();

    void attach(final TransportWebSocket socket) {
      ready.complete(socket);
    }

    void fail(final Throwable error) {
      ready.completeExceptionally(error);
    }

    @Override
    public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
      return ready.thenCompose(ws -> ws.sendText(data, last));
    }

    @Override
    public CompletableFuture<Void> sendBinary(final java.nio.ByteBuffer data, final boolean last) {
      return ready.thenCompose(ws -> ws.sendBinary(data, last));
    }

    @Override
    public CompletableFuture<Void> sendPing(final java.nio.ByteBuffer message) {
      return ready.thenCompose(ws -> ws.ping(message));
    }

    @Override
    public CompletableFuture<Void> sendPong(final java.nio.ByteBuffer message) {
      return ready.thenCompose(ws -> ws.pong(message));
    }

    @Override
    public CompletableFuture<Void> close(final int statusCode, final String reason) {
      return ready.thenCompose(
          ws -> {
            if (!ws.isOpen()) {
              return CompletableFuture.completedFuture(null);
            }
            return ws.close(statusCode, reason);
          });
    }

    @Override
    public boolean isOpen() {
      final TransportWebSocket socket = ready.getNow(null);
      return socket != null && socket.isOpen();
    }

    @Override
    public String subprotocol() {
      final TransportWebSocket socket = ready.getNow(null);
      return socket == null ? "" : socket.subprotocol();
    }
  }

  public static final class Builder {
    private URI baseUri = URI.create("http://localhost:8080");
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();
    private WebSocketConnector connector = null;

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
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

    public Builder setWebSocketConnector(final WebSocketConnector connector) {
      this.connector = Objects.requireNonNull(connector, "connector");
      return this;
    }

    public ToriiWebSocketClient build() {
      return new ToriiWebSocketClient(this);
    }
  }
}
