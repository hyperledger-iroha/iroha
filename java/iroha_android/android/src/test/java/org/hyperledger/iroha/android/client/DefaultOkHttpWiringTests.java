package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Field;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.Base64;
import java.util.concurrent.TimeUnit;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.nexus.UaidBindingsResponse;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.junit.Test;

/** Verifies Android clients default to the OkHttp transport when none is provided. */
public final class DefaultOkHttpWiringTests {

  @Test
  public void httpClientTransportUsesOkHttpByDefault() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody(
                  """
                  {
                    "uaid": "did:ua:abc",
                    "dataspaces": [
                      {"dataspace_id": 1, "dataspace_alias": "alpha", "accounts": ["user@test"]}
                    ]
                  }
                  """));
      server.start();

      final RecordingObserver observer = new RecordingObserver();
      final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(new URI(server.url("/").toString()))
              .addObserver(observer)
              .setTelemetryOptions(TelemetryOptions.builder().setEnabled(true).build())
              .setTelemetrySink(telemetrySink)
              .build();

      final HttpClientTransport transport = HttpClientTransport.createDefault(config);
      final UaidBindingsResponse response =
          transport.getUaidBindings("did:ua:abc").get(2, TimeUnit.SECONDS);

      assertEquals("did:ua:abc", response.uaid());
      assertEquals(1, response.dataspaces().size());
      assertEquals(1L, response.dataspaces().get(0).dataspaceId());
      assertTrue(observer.requestCount > 0);
      assertTrue(telemetrySink.requestCount > 0);
      assertTrue(extractExecutor(transport) instanceof OkHttpTransportExecutor);

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("request not received", recorded);
      assertEquals("/v1/space-directory/uaids/did%3Aua%3Aabc", recorded.getPath());
      assertEquals("GET", recorded.getMethod());
    }
  }

  @Test
  public void toriiEventStreamUsesOkHttpWhenUnset() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "text/event-stream")
              .setBody("id: 1\nevent: ping\ndata: ok\n\n"));
      server.start();

      final RecordingSseListener listener = new RecordingSseListener();
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(new URI(server.url("/").toString()))
              .build();

      try (ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener)) {
        stream.completion().get(2, TimeUnit.SECONDS);
      }

      assertEquals(1, listener.events.size());
      final ServerSentEvent event = listener.events.get(0);
      assertEquals("ping", event.event());
      assertEquals("ok", event.data());
      assertTrue(extractField(client, "transport") instanceof OkHttpTransportExecutor);

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("request not received", recorded);
      assertEquals("/events", recorded.getPath());
      assertEquals("text/event-stream", recorded.getHeader("Accept"));
    }
  }

  @Test
  public void toriiWebSocketDefaultsToOkHttpConnector() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().withWebSocketUpgrade(new ScriptedWebSocket()));
      server.start();

      final RecordingWebSocketListener listener = new RecordingWebSocketListener();
      final ToriiWebSocketClient client =
          ToriiWebSocketClient.builder()
              .setBaseUri(new URI(server.url("/").toString()))
              .build();

      final ToriiWebSocketSession session =
          client.connect(
              "/ws",
              ToriiWebSocketOptions.builder()
                  .setConnectTimeout(Duration.ofSeconds(2))
                  .build(),
              listener);

      assertTrue("websocket open/close", listener.await());
      session.close(ToriiWebSocketSession.NORMAL_CLOSURE, "done").get(2, TimeUnit.SECONDS);
      assertTrue(extractField(client, "connector") instanceof OkHttpWebSocketConnector);

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("websocket request not received", recorded);
      assertEquals("/ws", recorded.getPath());
    }
  }

  @Test
  public void sorafsGatewayDefaultsExecutorToOkHttp() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"status\":\"ok\"}"));
      server.start();

      final GatewayProvider provider =
          GatewayProvider.builder()
              .setName("alpha")
              .setProviderIdHex("aa".repeat(32))
              .setBaseUrl("https://providers.example")
              .setStreamTokenBase64(Base64.getEncoder().encodeToString("token".getBytes(StandardCharsets.UTF_8)))
              .build();
      final GatewayFetchRequest request =
          GatewayFetchRequest.builder()
              .setManifestIdHex("deadbeef".repeat(8))
              .setOptions(GatewayFetchOptions.builder().build())
              .addProvider(provider)
              .build();

      final SorafsGatewayClient client =
          SorafsGatewayClient.builder()
              .setBaseUri(new URI(server.url("/").toString()))
              .build();

      final ClientResponse response = client.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals(200, response.statusCode());
      assertTrue(extractExecutor(client) instanceof OkHttpTransportExecutor);

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("request not received", recorded);
      assertEquals("/v1/sorafs/gateway/fetch", recorded.getPath());
      assertEquals("POST", recorded.getMethod());
      final String body = recorded.getBody().readString(StandardCharsets.UTF_8);
      assertTrue(body.contains("\"manifest_id_hex\":\"" + "deadbeef".repeat(8) + "\""));
      assertEquals("application/json", recorded.getHeader("Content-Type"));
    }
  }

  @Test
  public void safetyDetectCreateDefaultUsesOkHttp() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"access_token\":\"abc123\",\"expires_in\":3600}"));
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"token\":\"attest-token\"}"));
      server.start();

      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setClientId("client-id")
              .setClientSecret("secret")
              .setPackageName("org.test.app")
              .setSigningDigestSha256("aa")
              .setOauthEndpoint(new URI(server.url("/oauth").toString()))
              .setAttestationEndpoint(new URI(server.url("/attest").toString()))
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();

      final HttpSafetyDetectService service =
          HttpSafetyDetectService.createDefault(options);

      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("ff")
              .setAppId("app-1")
              .setNonceHex("00aa")
              .setPackageName("org.test.app")
              .setSigningDigestSha256("aa")
              .build();

      final SafetyDetectAttestation attestation =
          service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attest-token", attestation.token());
      assertTrue(extractField(service, "executor") instanceof OkHttpTransportExecutor);

      final RecordedRequest oauth = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("oauth request missing", oauth);
      assertEquals("/oauth", oauth.getPath());
      final String oauthBody = oauth.getBody().readString(StandardCharsets.UTF_8);
      assertTrue(oauthBody.contains("client_id=client-id"));
      assertTrue(oauthBody.contains("client_secret=secret"));

      final RecordedRequest attest = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("attestation request missing", attest);
      assertEquals("/attest", attest.getPath());
      assertEquals("Bearer abc123", attest.getHeader("Authorization"));
    }
  }

  private static Object extractExecutor(final Object target) throws Exception {
    return extractField(target, "executor");
  }

  private static Object extractField(final Object target, final String name) throws Exception {
    final Field field = target.getClass().getDeclaredField(name);
    field.setAccessible(true);
    return field.get(target);
  }

  private static final class RecordingObserver implements ClientObserver {
    int requestCount;
    int responseCount;
    int failureCount;

    @Override
    public void onRequest(final org.hyperledger.iroha.android.client.transport.TransportRequest request) {
      requestCount++;
    }

    @Override
    public void onResponse(
        final org.hyperledger.iroha.android.client.transport.TransportRequest request,
        final ClientResponse response) {
      responseCount++;
    }

    @Override
    public void onFailure(
        final org.hyperledger.iroha.android.client.transport.TransportRequest request,
        final Throwable error) {
      failureCount++;
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    int requestCount;
    int responseCount;
    TelemetryRecord lastRequest;

    @Override
    public void onRequest(final TelemetryRecord record) {
      requestCount++;
      lastRequest = record;
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      responseCount++;
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}
  }

  private static final class RecordingSseListener implements ToriiEventStreamListener, AutoCloseable {
    final java.util.List<ServerSentEvent> events = new java.util.ArrayList<>();

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
    }

    @Override
    public void close() {}
  }

  private static final class RecordingWebSocketListener implements ToriiWebSocketListener {
    final java.util.concurrent.CountDownLatch opened = new java.util.concurrent.CountDownLatch(1);
    final java.util.concurrent.CountDownLatch closed = new java.util.concurrent.CountDownLatch(1);
    final java.util.List<String> textMessages = new java.util.ArrayList<>();

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      opened.countDown();
    }

    @Override
    public void onText(
        final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
      textMessages.add(data.toString());
    }

    @Override
    public void onBinary(
        final ToriiWebSocketSession session, final ByteBuffer data, final boolean last) {
      // Not used in this test.
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

    boolean await() throws InterruptedException {
      return opened.await(2, TimeUnit.SECONDS) && closed.await(2, TimeUnit.SECONDS);
    }
  }

  private static final class ScriptedWebSocket extends WebSocketListener {
    @Override
    public void onOpen(final WebSocket webSocket, final okhttp3.Response response) {
      webSocket.send("hello");
      webSocket.send(ByteString.of(new byte[] {0x01, 0x02}));
      webSocket.close(1000, "done");
    }
  }
}
