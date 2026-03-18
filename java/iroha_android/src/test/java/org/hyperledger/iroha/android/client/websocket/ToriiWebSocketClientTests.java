package org.hyperledger.iroha.android.client.websocket;

import java.net.URI;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;

public final class ToriiWebSocketClientTests {

  private ToriiWebSocketClientTests() {}

  public static void main(final String[] args) {
    connectDeliversEvents();
    connectorFailuresPropagate();
    System.out.println("[IrohaAndroid] ToriiWebSocketClientTests passed.");
  }

  private static void connectDeliversEvents() {
    final RecordingConnector connector = new RecordingConnector();
    final List<String> events = new ArrayList<>();
    final ToriiWebSocketListener listener =
        new ToriiWebSocketListener() {
          @Override
          public void onText(
              final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
            events.add(data.toString());
          }

          @Override
          public void onOpen(final ToriiWebSocketSession session) {
            events.add("open");
          }

          @Override
          public void onClose(
              final ToriiWebSocketSession session, final int statusCode, final String reason) {
            events.add("closed");
          }
        };
    final ToriiWebSocketClient client =
        ToriiWebSocketClient.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setWebSocketConnector(connector)
            .build();
    final ToriiWebSocketSession session =
        client.connect(
            "/ws",
            ToriiWebSocketOptions.builder()
                .putHeader("Authorization", "Bearer token")
                .addSubprotocol("torii")
                .build(),
            listener);
    connector.await();
    connector.emitText("payload");
    connector.emitClose();
    if (!events.contains("open") || !events.contains("payload") || !events.contains("closed")) {
      throw new AssertionError("Listener did not observe expected events: " + events);
    }
    session.close();
  }

  private static void connectorFailuresPropagate() {
    final ToriiWebSocketClient client =
        ToriiWebSocketClient.builder()
            .setWebSocketConnector(
                (request, options, listener, defaultHeaders) -> {
                  final CompletableFuture<TransportWebSocket> failed = new CompletableFuture<>();
                  failed.completeExceptionally(new IllegalStateException("boom"));
                  return failed;
                })
            .build();
    final List<Throwable> errors = new ArrayList<>();
    final ToriiWebSocketListener listener =
        new ToriiWebSocketListener() {
          @Override
          public void onError(final ToriiWebSocketSession session, final Throwable error) {
            errors.add(error);
          }
        };
    client.connect("/ws", ToriiWebSocketOptions.defaultOptions(), listener);
    if (errors.isEmpty() || !(errors.get(0) instanceof IllegalStateException)) {
      throw new AssertionError("Expected connector exception to propagate");
    }
  }

  private static final class RecordingConnector implements ToriiWebSocketClient.WebSocketConnector {
    private CompletableFuture<TransportWebSocket> future;
    private FakeTransportWebSocket socket;

    @Override
    public CompletableFuture<TransportWebSocket> connect(
        final TransportRequest request,
        final ToriiWebSocketOptions options,
        final TransportWebSocket.Listener listener,
        final Map<String, String> defaultHeaders) {
      future = new CompletableFuture<>();
      socket = new FakeTransportWebSocket(listener);
      future.complete(socket);
      return future;
    }

    void await() {
      future.join();
    }

    void emitText(final String value) {
      socket.emitText(value);
    }

    void emitClose() {
      socket.emitClose();
    }
  }

  private static final class FakeTransportWebSocket implements TransportWebSocket {
    private final Listener listener;
    private boolean closed = false;

    FakeTransportWebSocket(final Listener listener) {
      this.listener = listener;
      listener.onOpen(this);
    }

    void emitText(final String data) {
      listener.onText(this, data, true);
    }

    void emitClose() {
      closed = true;
      listener.onClose(this, 1000, "done");
    }

    @Override
    public CompletableFuture<Void> sendText(final CharSequence data, final boolean last) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> sendBinary(final ByteBuffer data, final boolean last) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> ping(final ByteBuffer data) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> pong(final ByteBuffer data) {
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public CompletableFuture<Void> close(final int statusCode, final String reason) {
      closed = true;
      return CompletableFuture.completedFuture(null);
    }

    @Override
    public boolean isOpen() {
      return !closed;
    }

    @Override
    public String subprotocol() {
      return "torii";
    }
  }
}
