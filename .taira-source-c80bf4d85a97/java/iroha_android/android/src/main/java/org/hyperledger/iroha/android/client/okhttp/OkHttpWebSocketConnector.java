package org.hyperledger.iroha.android.client.okhttp;

import java.time.Duration;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;

/** Android connector that opens WebSocket sessions via OkHttp. */
public final class OkHttpWebSocketConnector implements ToriiWebSocketClient.WebSocketConnector {

  private final OkHttpClient client;

  public OkHttpWebSocketConnector(final OkHttpClient client) {
    this.client = Objects.requireNonNull(client, "client");
  }

  /** Exposes the underlying OkHttp client so callers can reuse connection pools. */
  public OkHttpClient unwrapClient() {
    return client;
  }

  @Override
  public CompletableFuture<TransportWebSocket> connect(
      final TransportRequest request,
      final ToriiWebSocketOptions options,
      final TransportWebSocket.Listener listener,
      final Map<String, String> defaultHeaders) {
    Objects.requireNonNull(request, "request");
    Objects.requireNonNull(listener, "listener");

    final CompletableFuture<TransportWebSocket> ready = new CompletableFuture<>();
    final OkHttpTransportWebSocket socket = new OkHttpTransportWebSocket(listener, ready);

    final Request.Builder builder =
        new Request.Builder()
            .url(request.uri().toString());
    defaultHeaders.forEach(builder::addHeader);
    options.headers().forEach(builder::addHeader);
    if (!options.subprotocols().isEmpty()) {
      builder.addHeader("Sec-WebSocket-Protocol", String.join(",", options.subprotocols()));
    }

    final Duration timeout = request.timeout() != null ? request.timeout() : options.connectTimeout();
    final OkHttpClient target = resolveClient(timeout);
    target.newWebSocket(builder.build(), socket);
    return ready;
  }

  OkHttpClient resolveClient(final Duration timeout) {
    if (timeout == null) {
      return client;
    }
    final long timeoutMs = Math.max(0L, timeout.toMillis());
    if (client.connectTimeoutMillis() == timeoutMs) {
      return client;
    }
    return client.newBuilder().connectTimeout(timeoutMs, TimeUnit.MILLISECONDS).build();
  }
}
