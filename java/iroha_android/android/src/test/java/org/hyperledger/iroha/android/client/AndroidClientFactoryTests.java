package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import okio.Buffer;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.junit.Test;

public final class AndroidClientFactoryTests {

  @Test
  public void restTelemetryAndHashingUseOkHttp() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("pong"));
      server.start();

      final TelemetryCaptureSink sink = new TelemetryCaptureSink(1, 1);
      final ClientConfig config = clientConfig(server, sink);
      final AndroidClientFactory factory = AndroidClientFactory.withDefaultClient();

      final NoritoRpcClient rpcClient = factory.createNoritoRpcClient(config);
      final byte[] response =
          rpcClient.call("/rpc/ping", "ping".getBytes(StandardCharsets.UTF_8));
      assertEquals("pong", new String(response, StandardCharsets.UTF_8));
      assertTrue(factory.httpExecutor() instanceof OkHttpTransportExecutor);

      assertTrue(sink.await());
      final TelemetryRecord request = sink.lastRequest();
      assertNotNull(request);
      final String authority = server.url("/").uri().getAuthority();
      final String expectedHash =
          config
              .telemetryOptions()
              .redaction()
              .hashAuthority(authority)
              .orElse(null);
      assertEquals(expectedHash, request.authorityHash());
      final TelemetryRecord responseRecord = sink.lastResponse();
      assertNotNull(responseRecord);
      assertEquals(request.method(), responseRecord.method());
      assertEquals(200, responseRecord.statusCode().orElse(-1));
    }
  }

  @Test
  public void sseAndWebSocketReuseOkHttpConnectors() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "text/event-stream")
              .setBody("data: one\n\n"));
      server.enqueue(
          new MockResponse()
              .withWebSocketUpgrade(
                  new okhttp3.WebSocketListener() {
                    @Override
                    public void onOpen(
                        final okhttp3.WebSocket webSocket, final okhttp3.Response response) {
                      webSocket.send("welcome");
                      webSocket.close(1000, "done");
                    }
                  }));
      server.start();

      final TelemetryCaptureSink sink = new TelemetryCaptureSink(2, 2);
      final ClientConfig config = clientConfig(server, sink);
      final AndroidClientFactory factory = AndroidClientFactory.withClient(new OkHttpClient());

      final ToriiEventStreamClient eventClient = factory.createEventStreamClient(config);
      final RecordingSseListener sseListener = new RecordingSseListener();
      final ToriiEventStream stream =
          eventClient.openSseStream(
              "/events", ToriiEventStreamOptions.defaultOptions(), sseListener);
      assertTrue(sseListener.await(2, TimeUnit.SECONDS));
      stream.completion().get(1, TimeUnit.SECONDS);
      stream.close();
      assertEquals(1, sseListener.events.size());
      assertEquals("one", sseListener.events.get(0).data());

      final ToriiWebSocketClient wsClient = factory.createWebSocketClient(config);
      final RecordingWebSocketListener wsListener = new RecordingWebSocketListener();
      final ToriiWebSocketSession session =
          wsClient.connect("/ws", ToriiWebSocketOptions.defaultOptions(), wsListener);
      assertTrue(wsListener.awaitOpen(2, TimeUnit.SECONDS));
      assertTrue(wsListener.awaitMessage(2, TimeUnit.SECONDS));
      session.close(ToriiWebSocketSession.NORMAL_CLOSURE, "done").get(1, TimeUnit.SECONDS);
      assertEquals(List.of("welcome"), wsListener.messages);

      assertTrue(sink.await());
      assertEquals(2, sink.requests.size());
      assertEquals(2, sink.responses.size());
    }
  }

  @Test
  public void safetyDetectAndSorafsUseOkHttpExecutor() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"access_token\":\"token-abc\",\"expires_in\":120}"));
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"token\":\"attestation-ok\"}"));
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Type", "application/json")
              .setBody("{\"ok\":true}"));
      server.start();

      final AndroidClientFactory factory = AndroidClientFactory.withDefaultClient();

      final SafetyDetectOptions safetyOptions =
          SafetyDetectOptions.builder()
              .setOauthEndpoint(server.url("/oauth").uri())
              .setAttestationEndpoint(server.url("/attest").uri())
              .setClientId("client-id")
              .setClientSecret("client-secret")
              .setPackageName("com.test.app")
              .setSigningDigestSha256("cafebabe")
              .setRequestTimeout(Duration.ofSeconds(5))
              .setTokenSkew(Duration.ZERO)
              .build();
      final HttpSafetyDetectService safetyService =
          factory.createSafetyDetectService(safetyOptions);
      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("aa11")
              .setAppId("app-1")
              .setNonceHex("00ff")
              .setPackageName("com.test.app")
              .setSigningDigestSha256("cafebabe")
              .build();
      final SafetyDetectAttestation attestation =
          safetyService.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attestation-ok", attestation.token());

      final RecordedRequest oauth = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(oauth);
      assertEquals("/oauth", oauth.getPath());
      assertEquals("POST", oauth.getMethod());
      final String oauthBody = oauth.getBody().readUtf8();
      assertTrue(oauthBody.contains("client_id=client-id"));
      assertTrue(oauthBody.contains("client_secret=client-secret"));

      final RecordedRequest attest = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(attest);
      assertEquals("/attest", attest.getPath());
      assertEquals("POST", attest.getMethod());
      assertEquals("Bearer token-abc", attest.getHeader("Authorization"));

      final TelemetryCaptureSink sink = new TelemetryCaptureSink(1, 1);
      final ClientConfig config = clientConfig(server, sink);
      final SorafsGatewayClient gateway = factory.createSorafsGatewayClient(config);
      final GatewayFetchRequest fetchRequest =
          GatewayFetchRequest.builder()
              .setManifestIdHex("feed".repeat(16))
              .setChunkerHandle("chunker-1")
              .addProvider(
                  GatewayProvider.builder()
                      .setName("provider-a")
                      .setProviderIdHex("01".repeat(32))
                      .setBaseUrl(server.url("/storage").toString())
                      .setStreamTokenBase64("dG9rZW4=")
                      .build())
              .build();

      final ClientResponse fetchResponse = gateway.fetch(fetchRequest).get(2, TimeUnit.SECONDS);
      assertEquals(200, fetchResponse.statusCode());
      assertArrayEquals("{\"ok\":true}".getBytes(StandardCharsets.UTF_8), fetchResponse.body());
      assertTrue(factory.httpExecutor() instanceof OkHttpTransportExecutor);

      assertTrue(sink.await());
      assertEquals(1, sink.requests.size());
      assertEquals(1, sink.responses.size());

      final RecordedRequest gatewayRequest = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(gatewayRequest);
      assertEquals("/v1/sorafs/gateway/fetch", gatewayRequest.getPath());
      final Buffer body = gatewayRequest.getBody();
      final String payload = body.readUtf8();
      assertTrue(payload.contains("manifest_id_hex"));
      assertTrue(payload.contains("provider-a"));
    }
  }

  private static ClientConfig clientConfig(
      final MockWebServer server, final TelemetrySink sink) {
    return ClientConfig.builder()
        .setBaseUri(server.url("/").uri())
        .setSorafsGatewayUri(server.url("/").uri())
        .setRequestTimeout(Duration.ofSeconds(5))
        .setTelemetryOptions(defaultTelemetryOptions())
        .setTelemetrySink(sink)
        .build();
  }

  private static TelemetryOptions defaultTelemetryOptions() {
    return TelemetryOptions.builder()
        .setEnabled(true)
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setEnabled(true)
                .setSaltHex("00112233445566778899aabbccddeeff")
                .setSaltVersion("test-salt-v1")
                .setRotationId("rotation-test")
                .build())
        .build();
  }

  private static final class RecordingSseListener implements ToriiEventStreamListener {
    private final CountDownLatch latch = new CountDownLatch(1);
    final List<ServerSentEvent> events = new ArrayList<>();

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
      latch.countDown();
    }

    @Override
    public void onError(final Throwable error) {
      latch.countDown();
    }

    boolean await(final long timeout, final TimeUnit unit) throws InterruptedException {
      return latch.await(timeout, unit);
    }
  }

  private static final class RecordingWebSocketListener implements ToriiWebSocketListener {
    private final CountDownLatch openLatch = new CountDownLatch(1);
    private final CountDownLatch messageLatch = new CountDownLatch(1);
    final List<String> messages = Collections.synchronizedList(new ArrayList<>());

    @Override
    public void onOpen(final ToriiWebSocketSession session) {
      openLatch.countDown();
    }

    @Override
    public void onText(
        final ToriiWebSocketSession session, final CharSequence data, final boolean last) {
      messages.add(data.toString());
      messageLatch.countDown();
    }

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {
      openLatch.countDown();
      messageLatch.countDown();
    }

    boolean awaitOpen(final long timeout, final TimeUnit unit) throws InterruptedException {
      return openLatch.await(timeout, unit);
    }

    boolean awaitMessage(final long timeout, final TimeUnit unit) throws InterruptedException {
      return messageLatch.await(timeout, unit);
    }
  }

  private static final class TelemetryCaptureSink implements TelemetrySink {
    private final CountDownLatch requestLatch;
    private final CountDownLatch responseLatch;
    final List<TelemetryRecord> requests = Collections.synchronizedList(new ArrayList<>());
    final List<TelemetryRecord> responses = Collections.synchronizedList(new ArrayList<>());
    final List<TelemetryRecord> failures = Collections.synchronizedList(new ArrayList<>());

    TelemetryCaptureSink(final int expectedRequests, final int expectedResponses) {
      this.requestLatch = new CountDownLatch(expectedRequests);
      this.responseLatch = new CountDownLatch(expectedResponses);
    }

    @Override
    public void onRequest(final TelemetryRecord record) {
      requests.add(record);
      requestLatch.countDown();
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      responses.add(record);
      responseLatch.countDown();
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      failures.add(record);
      responseLatch.countDown();
    }

    boolean await() throws InterruptedException {
      return requestLatch.await(2, TimeUnit.SECONDS)
          && responseLatch.await(2, TimeUnit.SECONDS);
    }

    TelemetryRecord lastRequest() {
      return requests.isEmpty() ? null : requests.get(requests.size() - 1);
    }

    TelemetryRecord lastResponse() {
      return responses.isEmpty() ? null : responses.get(responses.size() - 1);
    }
  }
}
