package org.hyperledger.iroha.android.client.websocket;

import java.net.http.HttpClient;
import java.net.http.WebSocket;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;

/** JVM connector that opens WebSocket sessions via {@link HttpClient}. */
public final class JdkWebSocketConnector implements ToriiWebSocketClient.WebSocketConnector {

  private final HttpClient httpClient;

  /** Creates a connector backed by {@link HttpClient#newHttpClient()}. */
  public JdkWebSocketConnector() {
    this(HttpClient.newHttpClient());
  }

  /**
   * Creates a connector backed by the provided {@link HttpClient}.
   *
   * @param httpClient HTTP client instance used to create WebSocket sessions
   */
  public JdkWebSocketConnector(final HttpClient httpClient) {
    this.httpClient = Objects.requireNonNull(httpClient, "httpClient");
  }

  @Override
  public CompletableFuture<TransportWebSocket> connect(
      final TransportRequest request,
      final ToriiWebSocketOptions options,
      final TransportWebSocket.Listener listener,
      final Map<String, String> defaultHeaders) {
    Objects.requireNonNull(request, "request");
    Objects.requireNonNull(listener, "listener");
    final WebSocket.Builder builder = httpClient.newWebSocketBuilder();
    if (options.connectTimeout() != null) {
      builder.connectTimeout(options.connectTimeout());
    }
    defaultHeaders.forEach(builder::header);
    options.headers().forEach(builder::header);
    if (!options.subprotocols().isEmpty()) {
      final var subprotocols = options.subprotocols();
      if (subprotocols.size() == 1) {
        builder.subprotocols(subprotocols.get(0));
      } else {
        final String first = subprotocols.get(0);
        final String[] rest = subprotocols.subList(1, subprotocols.size()).toArray(String[]::new);
        builder.subprotocols(first, rest);
      }
    }

    final JavaTransportWebSocket socket = new JavaTransportWebSocket(listener);
    return builder
        .buildAsync(request.uri(), socket)
        .thenApply(
            ws -> {
              socket.attach(ws);
              return socket;
            });
  }
}
