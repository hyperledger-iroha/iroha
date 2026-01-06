package org.hyperledger.iroha.android.telemetry;

import java.net.URI;
import java.time.Duration;
import java.util.Map;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutorFactory;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorFactory;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;

/** Integration coverage proving telemetry hooks fire on OkHttp transports. */
public final class OkHttpTelemetryIntegrationTests {

  private OkHttpTelemetryIntegrationTests() {}

  public static void main(final String[] args) throws Exception {
    httpTelemetryViaOkHttp();
    sseTelemetryViaOkHttp();
    webSocketTelemetryViaOkHttp();
    System.out.println("[IrohaAndroid] OkHttpTelemetryIntegrationTests passed.");
  }

  private static TelemetryOptions telemetryOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("1122334455667788")
                .setSaltVersion("2026Q1")
                .setRotationId("rot-telemetry-okhttp")
                .build())
        .build();
  }

  private static void httpTelemetryViaOkHttp() throws Exception {
    final TelemetryOptions options = telemetryOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);
    final GatewayFetchRequest request =
        GatewayFetchRequest.builder()
            .setManifestIdHex("bead".repeat(16))
            .setChunkerHandle("sorafs.sf1@1.0.0")
            .setOptions(GatewayFetchOptions.builder().build())
            .addProvider(
                GatewayProvider.builder()
                    .setName("alpha")
                    .setProviderIdHex("01".repeat(32))
                    .setBaseUrl("https://provider.example")
                    .setStreamTokenBase64("c3RyZWFtLXRva2Vu")
                    .build())
            .build();
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"status\":\"ok\"}"));
      server.start();

      final HttpTransportExecutor executor = OkHttpTransportExecutorFactory.createDefault();
      final SorafsGatewayClient client =
          SorafsGatewayClient.builder()
              .setExecutor(executor)
              .setBaseUri(server.url("/").uri())
              .setTimeout(Duration.ofSeconds(2))
              .addObserver(observer)
              .build();
      final var response = client.fetch(request).join();
      assert response.statusCode() == 200 : "gateway response should be successful";
      assert new String(response.body(), java.nio.charset.StandardCharsets.UTF_8)
          .contains("\"status\":\"ok\"") : "gateway body should match";

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assert recorded != null : "gateway request should reach server";
      assert "/v1/sorafs/gateway/fetch".equals(recorded.getPath()) : "route mismatch";
      assert "POST".equals(recorded.getMethod()) : "HTTP method mismatch";

      assert sink.requestCount.get() == 1 : "telemetry should capture request";
      assert sink.responseCount.get() == 1 : "telemetry should capture response";
      final TelemetryRecord record = sink.lastResponseRecord;
      assert record != null : "response record should be populated";
      final String authority =
          options
              .redaction()
              .hashAuthority(server.getHostName() + ":" + server.getPort())
              .orElseThrow(() -> new IllegalStateException("authority hash missing"));
      assert authority.equals(record.authorityHash()) : "authority hash mismatch";
      assert "/v1/sorafs/gateway/fetch".equals(record.route()) : "route should include path";
      assert "POST".equals(record.method()) : "method should be POST";
    }
  }

  private static void sseTelemetryViaOkHttp() throws Exception {
    final TelemetryOptions options = telemetryOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);
    try (MockWebServer server = new MockWebServer()) {
      final MockResponse response =
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "text/event-stream")
              .setBody("event: update\n" + "data: hello\n\n");
      server.enqueue(response);
      server.start();

      final ToriiEventStreamListener listener =
          event -> {
            // no-op
          };
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(server.url("/").uri())
              .setTransportExecutor(OkHttpTransportExecutorFactory.createDefault())
              .addObserver(observer)
              .build();
      try (ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener)) {
        stream.completion().get(1, TimeUnit.SECONDS);
      }

      assert sink.requestCount.get() == 1 : "telemetry should record SSE request";
      assert sink.responseCount.get() == 1 : "telemetry should record SSE response";
      final TelemetryRecord record = sink.lastResponseRecord;
      assert record != null : "SSE telemetry record missing";
      final String expectedHash =
          options
              .redaction()
              .hashAuthority(server.getHostName() + ":" + server.getPort())
              .orElseThrow(() -> new IllegalStateException("missing authority hash"));
      assert expectedHash.equals(record.authorityHash()) : "SSE authority hash mismatch";
      assert "/events".equals(record.route()) : "SSE route mismatch";
      assert "GET".equals(record.method()) : "SSE should use GET";
    }
  }

  private static void webSocketTelemetryViaOkHttp() throws Exception {
    final TelemetryOptions options = telemetryOptions();
    final RecordingSink sink = new RecordingSink();
    final TelemetryObserver observer = new TelemetryObserver(options, sink);
    try (MockWebServer server = new MockWebServer()) {
      final CountDownLatch closed = new CountDownLatch(1);
      server.enqueue(
          new MockResponse()
              .withWebSocketUpgrade(
                  new okhttp3.WebSocketListener() {
                    @Override
                    public void onOpen(
                        final okhttp3.WebSocket webSocket, final okhttp3.Response response) {
                      webSocket.send("ws-okhttp");
                      webSocket.close(1000, "done");
                    }
                  }));
      server.start();

      final URI baseUri = URI.create(server.url("/").toString());
      final ToriiWebSocketClient client =
          ToriiWebSocketClient.builder()
              .setBaseUri(baseUri)
              .setWebSocketConnector(OkHttpWebSocketConnectorFactory.createDefault())
              .addObserver(observer)
              .build();
      final ToriiWebSocketSession session =
          client.connect(
              "/ws", ToriiWebSocketOptions.builder().setConnectTimeout(Duration.ofSeconds(2)).build(), new ToriiWebSocketListener() {
                @Override
                public void onOpen(final ToriiWebSocketSession session) {}

                @Override
                public void onText(
                    final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
                  closed.countDown();
                }

                @Override
                public void onClose(
                    final ToriiWebSocketSession session, final int statusCode, final String reason) {
                  closed.countDown();
                }

                @Override
                public void onError(final ToriiWebSocketSession session, final Throwable error) {
                  closed.countDown();
                }
              });
      closed.await(1, TimeUnit.SECONDS);
      session.close(ToriiWebSocketSession.NORMAL_CLOSURE, "client").get(1, TimeUnit.SECONDS);

      assert sink.requestCount.get() == 1 : "telemetry should capture WS request";
      assert sink.responseCount.get() == 1 : "telemetry should capture WS handshake";
      final TelemetryRecord record = sink.lastResponseRecord;
      assert record != null : "WS telemetry record missing";
      final String expectedHash =
          options
              .redaction()
              .hashAuthority(server.getHostName() + ":" + server.getPort())
              .orElseThrow(() -> new IllegalStateException("missing authority hash"));
      assert expectedHash.equals(record.authorityHash()) : "WS authority hash mismatch";
      assert "/ws".equals(record.route()) : "WS route mismatch";
      assert "GET".equals(record.method()) : "WebSocket handshake should use GET";
    }
  }

  private static final class RecordingSink implements TelemetrySink {
    final AtomicInteger requestCount = new AtomicInteger(0);
    final AtomicInteger responseCount = new AtomicInteger(0);
    final AtomicInteger failureCount = new AtomicInteger(0);
    volatile TelemetryRecord lastRequestRecord = null;
    volatile TelemetryRecord lastResponseRecord = null;
    volatile TelemetryRecord lastFailureRecord = null;

    @Override
    public void onRequest(final TelemetryRecord record) {
      lastRequestRecord = record;
      requestCount.incrementAndGet();
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      lastResponseRecord = record;
      responseCount.incrementAndGet();
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      lastFailureRecord = record;
      failureCount.incrementAndGet();
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      // Ignored for this integration test.
    }
  }
}
