package org.hyperledger.iroha.android.client.okhttp;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportWebSocket;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;

public final class OkHttpWebSocketConnectorTests {

  private OkHttpWebSocketConnectorTests() {}

  public static void main(final String[] args) throws Exception {
    connectsAndForwardsMessages();
    connectTimeoutOverridesDefault();
    System.out.println("[IrohaAndroid] OkHttpWebSocketConnectorTests passed.");
  }

  private static void connectsAndForwardsMessages() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final CountDownLatch closed = new CountDownLatch(1);
      server.enqueue(
          new MockResponse()
              .withWebSocketUpgrade(
                  new WebSocketListener() {
                    @Override
                    public void onOpen(final WebSocket webSocket, final Response response) {
                      webSocket.send("hello");
                      webSocket.close(1000, "server_done");
                    }
                  }));
      server.start();

      final URI wsUri =
          URI.create(server.url("/ws").toString().replaceFirst("^http", "ws"));
      final TransportRequest request = TransportRequest.builder().setUri(wsUri).build();
      final List<String> events = new ArrayList<>();
      final OkHttpWebSocketConnector connector = new OkHttpWebSocketConnector(new OkHttpClient());

      final TransportWebSocket socket =
          connector
              .connect(
                  request,
                  ToriiWebSocketOptions.defaultOptions(),
                  new TransportWebSocket.Listener() {
                    @Override
                    public void onOpen(final TransportWebSocket ws) {
                      events.add("open");
                    }

                    @Override
                    public void onText(
                        final TransportWebSocket ws, final CharSequence data, final boolean last) {
                      events.add("text:" + data);
                    }

                    @Override
                    public void onClose(
                        final TransportWebSocket ws, final int statusCode, final String reason) {
                      events.add("close:" + statusCode);
                      closed.countDown();
                    }

                    @Override
                    public void onError(final TransportWebSocket ws, final Throwable error) {
                      events.add("error:" + error.getMessage());
                      closed.countDown();
                    }
                  },
                  Map.of())
              .join();

      socket.sendText("client", true).join();
      closed.await(1, TimeUnit.SECONDS);
      socket.close(1000, "client_done").join();

      if (!events.contains("open") || !events.contains("text:hello") || !events.contains("close:1000")) {
        throw new AssertionError("Unexpected WebSocket events: " + events);
      }
    }
  }

  private static void connectTimeoutOverridesDefault() {
    final OkHttpClient base =
        new OkHttpClient.Builder().connectTimeout(1, TimeUnit.SECONDS).build();
    final OkHttpWebSocketConnector connector = new OkHttpWebSocketConnector(base);

    final OkHttpClient nullTimeout = connector.resolveClient(null);
    if (nullTimeout != base) {
      throw new AssertionError("Expected null timeout to reuse the base client");
    }

    final OkHttpClient sameTimeout = connector.resolveClient(Duration.ofSeconds(1));
    if (sameTimeout != base) {
      throw new AssertionError("Expected matching timeout to reuse the base client");
    }

    final OkHttpClient adjusted = connector.resolveClient(Duration.ofSeconds(2));
    if (adjusted == base) {
      throw new AssertionError("Expected timeout override to clone the base client");
    }
    if (adjusted.connectTimeoutMillis() != 2_000) {
      throw new AssertionError(
          "Expected connectTimeoutMillis=2000, got " + adjusted.connectTimeoutMillis());
    }
  }
}
