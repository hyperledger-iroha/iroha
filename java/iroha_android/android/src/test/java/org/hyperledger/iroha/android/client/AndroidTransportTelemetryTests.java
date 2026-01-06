package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.TelemetryObserver;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.junit.Test;

/** Verifies telemetry hashing on Android transports backed by OkHttp defaults. */
public final class AndroidTransportTelemetryTests {

  @Test
  public void sorafsFetchEmitsTelemetryWithDefaultExecutor() throws Exception {
    final TelemetryOptions telemetryOptions = telemetryOptions();
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryObserver observer = new TelemetryObserver(telemetryOptions, sink);

    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
      server.start();
      final URI baseUri = server.url("/").uri();

      final GatewayProvider provider =
          GatewayProvider.builder()
              .setName("provider-a")
              .setProviderIdHex("01".repeat(32))
              .setBaseUrl("http://example.com")
              .setStreamTokenBase64("c3R1Yg==")
              .build();
      final GatewayFetchRequest request =
          GatewayFetchRequest.builder().setManifestIdHex("ab".repeat(32)).addProvider(provider).build();

      final SorafsGatewayClient client =
          SorafsGatewayClient.builder().setBaseUri(baseUri).addObserver(observer).build();

      client.fetch(request).get(2, TimeUnit.SECONDS);

      final TelemetryRecord requestRecord = sink.singleRequest();
      final TelemetryRecord responseRecord = sink.singleResponse();
      final String expectedHash =
          telemetryOptions.redaction().hashAuthority(baseUri.getAuthority()).orElse(null);
      assertEquals(expectedHash, requestRecord.authorityHash());
      assertEquals("/v1/sorafs/gateway/fetch", requestRecord.route());
      assertEquals("POST", requestRecord.method());
      assertEquals(expectedHash, responseRecord.authorityHash());
      assertEquals(200, responseRecord.statusCode().orElseThrow());
    }
  }

  @Test
  public void sseEmitsTelemetryWithDefaultTransport() throws Exception {
    final TelemetryOptions telemetryOptions = telemetryOptions();
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryObserver observer = new TelemetryObserver(telemetryOptions, sink);

    try (MockWebServer server = new MockWebServer()) {
      final String body =
          String.join(
              "\n",
              "id: 1",
              "event: update",
              "data: hello",
              "",
              "data: bye",
              "");
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "text/event-stream")
              .setBody(body));
      server.start();
      final URI baseUri = server.url("/").uri();

      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder().setBaseUri(baseUri).addObserver(observer).build();
      final RecordingSseListener listener = new RecordingSseListener();
      try (ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener)) {
        stream.completion().get(2, TimeUnit.SECONDS);
      }

      final TelemetryRecord requestRecord = sink.singleRequest();
      final TelemetryRecord responseRecord = sink.singleResponse();
      final String expectedHash =
          telemetryOptions.redaction().hashAuthority(baseUri.getAuthority()).orElse(null);
      assertEquals(expectedHash, requestRecord.authorityHash());
      assertEquals("/events", requestRecord.route());
      assertEquals("GET", requestRecord.method());
      assertEquals(expectedHash, responseRecord.authorityHash());
      assertEquals(200, responseRecord.statusCode().orElseThrow());
      assertEquals(2, listener.events.size());
    }
  }

  @Test
  public void webSocketEmitsTelemetryWithDefaultConnector() throws Exception {
    final TelemetryOptions telemetryOptions = telemetryOptions();
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TelemetryObserver observer = new TelemetryObserver(telemetryOptions, sink);

    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .withWebSocketUpgrade(
                  new WebSocketListener() {
                    @Override
                    public void onOpen(
                        final okhttp3.WebSocket webSocket, final okhttp3.Response response) {
                      webSocket.send("hello");
                      webSocket.close(1000, "done");
                    }
                  }));
      server.start();
      final URI baseUri = server.url("/").uri();

      final ToriiWebSocketClient client =
          ToriiWebSocketClient.builder().setBaseUri(baseUri).addObserver(observer).build();
      final RecordingWsListener listener = new RecordingWsListener();
      final ToriiWebSocketSession session =
          client.connect("/ws", ToriiWebSocketOptions.defaultOptions(), listener);
      assertTrue("websocket did not open", listener.await());
      session.close(ToriiWebSocketSession.NORMAL_CLOSURE, "done").get(2, TimeUnit.SECONDS);

      final TelemetryRecord requestRecord = sink.singleRequest();
      final TelemetryRecord responseRecord = sink.singleResponse();
      final String expectedHash =
          telemetryOptions.redaction().hashAuthority(baseUri.getAuthority()).orElse(null);
      assertEquals(expectedHash, requestRecord.authorityHash());
      assertEquals("/ws", requestRecord.route());
      assertEquals("GET", requestRecord.method());
      assertEquals(expectedHash, responseRecord.authorityHash());
      assertEquals(101, responseRecord.statusCode().orElseThrow());
    }
  }

  private static TelemetryOptions telemetryOptions() {
    final TelemetryOptions.Redaction redaction =
        TelemetryOptions.Redaction.builder()
            .setEnabled(true)
            .setSalt("cafebabe".getBytes(StandardCharsets.UTF_8))
            .setSaltVersion("2026Q4")
            .setRotationId("android-test")
            .build();
    return TelemetryOptions.builder().setTelemetryRedaction(redaction).build();
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<TelemetryRecord> requests = new ArrayList<>();
    private final List<TelemetryRecord> responses = new ArrayList<>();
    private final List<TelemetryRecord> failures = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {
      requests.add(record);
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      responses.add(record);
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      failures.add(record);
    }

    TelemetryRecord singleRequest() {
      if (requests.size() != 1) {
        throw new AssertionError("expected exactly one telemetry request");
      }
      return requests.get(0);
    }

    TelemetryRecord singleResponse() {
      if (responses.size() != 1) {
        throw new AssertionError("expected exactly one telemetry response");
      }
      return responses.get(0);
    }
  }

  private static final class RecordingSseListener implements ToriiEventStreamListener {
    final List<ServerSentEvent> events = new ArrayList<>();

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
    }
  }

  private static final class RecordingWsListener implements ToriiWebSocketListener {
    private final CountDownLatch opened = new CountDownLatch(1);

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      opened.countDown();
    }

    @Override
    public void onText(
        final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
      // No-op; telemetry surfaced via observer callbacks.
    }

    @Override
    public void onBinary(
        final ToriiWebSocketSession session, final java.nio.ByteBuffer data, final boolean last) {
      // No-op; telemetry surfaced via observer callbacks.
    }

    @Override
    public void onClose(
        final ToriiWebSocketSession session, final int statusCode, final String reason) {
      opened.countDown();
    }

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {
      opened.countDown();
    }

    boolean await() throws InterruptedException {
      return opened.await(2, TimeUnit.SECONDS);
    }
  }
}
