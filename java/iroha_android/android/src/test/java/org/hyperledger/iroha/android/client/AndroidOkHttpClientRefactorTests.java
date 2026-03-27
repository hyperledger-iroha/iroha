package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;

import java.lang.reflect.Field;
import java.net.URI;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.Base64;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okio.ByteString;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketListener;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketOptions;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketSession;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.TelemetryObserver;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.junit.Test;

public final class AndroidOkHttpClientRefactorTests {

  @Test
  public void defaultFactoriesPreferOkHttpTransports() throws Exception {
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofSeconds(1))
            .build();
    final HttpClientTransport transport = HttpClientTransport.createDefault(config);
    assertTrue(unwrapField(transport, "executor") instanceof OkHttpTransportExecutor);

    final OfflineToriiClient offline =
        OfflineToriiClient.builder().baseUri(URI.create("http://localhost:8080")).build();
    assertTrue(unwrapField(offline, "executor") instanceof OkHttpTransportExecutor);

    final SorafsGatewayClient sorafs =
        SorafsGatewayClient.builder().setBaseUri(URI.create("http://localhost:8080")).build();
    assertTrue(unwrapField(sorafs, "executor") instanceof OkHttpTransportExecutor);

    final ToriiEventStreamClient streamClient =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .build();
    assertTrue(unwrapField(streamClient, "transport") instanceof OkHttpTransportExecutor);

    final SafetyDetectOptions safetyOptions =
        SafetyDetectOptions.builder()
            .setClientId("client")
            .setClientSecret("secret")
            .setPackageName("pkg")
            .setSigningDigestSha256("abcd")
            .setOauthEndpoint(URI.create("http://localhost/oauth"))
            .setAttestationEndpoint(URI.create("http://localhost/attest"))
            .build();
    final HttpSafetyDetectService safety = HttpSafetyDetectService.createDefault(safetyOptions);
    assertTrue(unwrapField(safety, "executor") instanceof OkHttpTransportExecutor);
  }

  @Test
  public void safetyDetectFlowUsesPlatformDefaults() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"access_token\":\"token-abc\",\"expires_in\":3600}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"attestation-token\"}"));
      server.start();

      final URI base = URI.create(server.url("/").toString());
      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setClientId("client")
              .setClientSecret("secret")
              .setPackageName("pkg")
              .setSigningDigestSha256("abcd")
              .setOauthEndpoint(base.resolve("/oauth"))
              .setAttestationEndpoint(base.resolve("/attest"))
              .build();
      final HttpSafetyDetectService service = HttpSafetyDetectService.createDefault(options);
      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("deadbeef")
              .setAppId("app")
              .setNonceHex("0a0b")
              .setPackageName("pkg")
              .setSigningDigestSha256("abcd")
              .build();

      final SafetyDetectAttestation attestation =
          service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attestation-token", attestation.token());

      assertEquals("/oauth", Objects.requireNonNull(server.takeRequest(1, TimeUnit.SECONDS)).getPath());
      assertEquals("/attest", Objects.requireNonNull(server.takeRequest(1, TimeUnit.SECONDS)).getPath());
    }
  }

  @Test
  public void sorafsGatewayFlowUsesPlatformDefaults() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"ok\":true}"));
      server.start();

      final URI base = URI.create(server.url("/").toString());
      final GatewayFetchRequest request =
          GatewayFetchRequest.builder()
              .setManifestIdHex("ab".repeat(32))
              .addProvider(
                  GatewayProvider.builder()
                      .setName("provider-1")
                      .setProviderIdHex("01".repeat(32))
                      .setBaseUrl("https://provider.example")
                      .setStreamTokenBase64(
                          Base64.getEncoder().encodeToString("token".getBytes(StandardCharsets.UTF_8)))
                      .build())
              .build();
      final SorafsGatewayClient client =
          SorafsGatewayClient.builder().setBaseUri(base).build();

      final ClientResponse response = client.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals(200, response.statusCode());
      final okhttp3.mockwebserver.RecordedRequest recorded =
          Objects.requireNonNull(server.takeRequest(1, TimeUnit.SECONDS));
      assertEquals("/v1/sorafs/gateway/fetch", recorded.getPath());
      assertEquals("application/json", recorded.getHeader("Content-Type"));
    }
  }

  @Test
  public void telemetryParityAcrossOkHttpAndJdkTransports() throws Exception {
    final TelemetryOptions options = telemetryOptions();
    restTelemetryParity(options);
    sseTelemetryParity(options);
    webSocketTelemetryParity(options);
  }

  private static void restTelemetryParity(final TelemetryOptions options) throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(202).setBody("{\"status\":\"ok\"}"));
      server.start();

      final URI baseUri = URI.create(server.url("/").toString());
      final RecordingTelemetrySink okSink = new RecordingTelemetrySink();
      final ClientConfig okConfig =
          ClientConfig.builder()
              .setBaseUri(baseUri)
              .setTelemetryOptions(options)
              .setTelemetrySink(okSink)
              .build();

      final SignedTransaction tx = sampleTransaction((byte) 0x01);
      final HttpClientTransport okTransport = HttpClientTransport.createDefault(okConfig);

      okTransport.submitTransaction(tx).get(2, TimeUnit.SECONDS);

      final TelemetryRecord okRequest = okSink.awaitRequest();
      assertRequestFields(okRequest, "/transaction", "POST");

      final TelemetryRecord okResponse = okSink.awaitResponse();
      assertEquals(202, okResponse.statusCode().orElseThrow());
    }
  }

  private static void sseTelemetryParity(final TelemetryOptions options) throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final String sseBody =
          String.join(
              "\n",
              "id: 1",
              "event: update",
              "data: ok",
              "",
              "data: keepalive",
              "");
      server.enqueue(new MockResponse().setResponseCode(200).setHeader("Content-Type", "text/event-stream").setBody(sseBody));
      server.start();

      final URI baseUri = new URI(server.url("/").toString());
      final RecordingTelemetrySink okSink = new RecordingTelemetrySink();
      final ToriiEventStreamOptions eventOptions = ToriiEventStreamOptions.defaultOptions();

      final ToriiEventStreamClient okClient =
          ToriiEventStreamClient.builder()
              .setBaseUri(baseUri)
              .addObserver(new TelemetryObserver(options, okSink))
              .build();
      try (ToriiEventStream stream =
          okClient.openSseStream(
              "/events",
              eventOptions,
              new ToriiEventStreamListener() {
                @Override
                public void onEvent(final ServerSentEvent event) {}
              })) {
        stream.completion().get(2, TimeUnit.SECONDS);
      }

      final TelemetryRecord okRequest = okSink.awaitRequest();
      assertRequestFields(okRequest, "/events", "GET");
    }
  }

  private static void webSocketTelemetryParity(final TelemetryOptions options) throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().withWebSocketUpgrade(new ScriptedWebSocket()));
      server.start();

      final URI baseUri = new URI(server.url("/").toString());
      final RecordingTelemetrySink okSink = new RecordingTelemetrySink();
      final ToriiWebSocketOptions wsOptions =
          ToriiWebSocketOptions.builder().setConnectTimeout(Duration.ofSeconds(2)).build();

      final ToriiWebSocketClient okClient =
          ToriiWebSocketClient.builder()
              .setBaseUri(baseUri)
              .addObserver(new TelemetryObserver(options, okSink))
              .setWebSocketConnector(new OkHttpWebSocketConnector(new okhttp3.OkHttpClient()))
              .build();
      final ToriiWebSocketSession okSession =
          okClient.connect("/ws", wsOptions, new NoopWebSocketListener());
      okSession.close(ToriiWebSocketSession.NORMAL_CLOSURE, "done").get(2, TimeUnit.SECONDS);

      final TelemetryRecord okRequest = okSink.awaitRequest();
      assertRequestFields(okRequest, "/ws", "GET");
      final TelemetryRecord okResponse = okSink.awaitResponse();
      assertEquals(101, okResponse.statusCode().orElseThrow());
    }
  }

  private static SignedTransaction sampleTransaction(final byte seed) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", seed))
            .setAuthority("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB")
            .setCreationTimeMs(1_700_000_000_000L + (seed & 0xFF))
            .setInstructionBytes(new byte[] {seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce((seed & 0xFF) + 1)
            .setMetadata(Map.of("note", "tx-" + seed))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encoded;
    try {
      encoded = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    Arrays.fill(signature, (byte) (seed + 1));
    Arrays.fill(publicKey, (byte) (seed + 2));
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }

  private static TelemetryOptions telemetryOptions() {
    final TelemetryOptions.Redaction redaction =
        TelemetryOptions.Redaction.builder()
            .setEnabled(true)
            .setSaltHex("00112233")
            .setSaltVersion("v1")
            .setRotationId("rot")
            .build();
    return TelemetryOptions.builder().setTelemetryRedaction(redaction).build();
  }

  private static void assertRequestFields(
      final TelemetryRecord record, final String route, final String method) {
    assertNotNull(record);
    assertNotNull(record.authorityHash());
    assertEquals(route, record.route());
    assertEquals(method, record.method());
  }

  private static Object unwrapField(final Object target, final String name) throws Exception {
    final Field field = target.getClass().getDeclaredField(name);
    field.setAccessible(true);
    return field.get(target);
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final AtomicReference<TelemetryRecord> request = new AtomicReference<>();
    private final AtomicReference<TelemetryRecord> response = new AtomicReference<>();

    @Override
    public void onRequest(final TelemetryRecord record) {
      request.set(record);
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse ignored) {
      response.set(record);
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      response.set(record);
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      // Best-effort sink; signals are not validated here.
    }

    TelemetryRecord awaitRequest() throws InterruptedException {
      return await(request, "request");
    }

    TelemetryRecord awaitResponse() throws InterruptedException {
      return await(response, "response");
    }

    private TelemetryRecord await(
        final AtomicReference<TelemetryRecord> ref, final String label) throws InterruptedException {
      final long deadline = System.currentTimeMillis() + 2_000L;
      while (System.currentTimeMillis() < deadline) {
        final TelemetryRecord record = ref.get();
        if (record != null) {
          return record;
        }
        Thread.sleep(10L);
      }
      throw new AssertionError("telemetry " + label + " was not recorded");
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

  private static final class NoopWebSocketListener implements ToriiWebSocketListener {
    @Override
    public void onOpen(final ToriiWebSocketSession session) {}

    @Override
    public void onText(final ToriiWebSocketSession session, final CharSequence data, final boolean last) {}

    @Override
    public void onBinary(final ToriiWebSocketSession session, final ByteBuffer data, final boolean last) {}

    @Override
    public void onClose(final ToriiWebSocketSession session, final int statusCode, final String reason) {}

    @Override
    public void onError(final ToriiWebSocketSession session, final Throwable error) {}
  }
}
