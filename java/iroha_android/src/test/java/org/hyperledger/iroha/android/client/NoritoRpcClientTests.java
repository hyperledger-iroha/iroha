package org.hyperledger.iroha.android.client;

import com.sun.net.httpserver.Headers;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Optional;
import java.util.List;
import java.util.Map;
import java.util.Locale;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.telemetry.DeviceProfile;
import org.hyperledger.iroha.android.telemetry.DeviceProfileProvider;
import org.hyperledger.iroha.android.telemetry.NetworkContext;
import org.hyperledger.iroha.android.telemetry.NetworkContextProvider;
import org.hyperledger.iroha.android.telemetry.TelemetryObserver;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;

/** Tests for {@link NoritoRpcClient}. */
public final class NoritoRpcClientTests {

  private NoritoRpcClientTests() {}

  public static void main(final String[] args) throws Exception {
    if (!isHttpServerSupported()) {
      System.out.println(
          "[IrohaAndroid] Norito RPC client tests skipped (HTTP server cannot bind in this environment).");
      return;
    }
    defaultHeadersAndPayloadAreApplied();
    requestOverridesApplyHeadersMethodAndQuery();
    nonSuccessStatusRaisesException();
    rejectsInsecureAuthorizationHeader();
    rejectsCredentialedAbsoluteHostOverride();
    observersReceiveCallbacks();
    flowControllerGuardsLifecycle();
    telemetrySignalsEmitForRpcCalls();
    callTransactionUsesCodecAdapter();
    System.out.println("[IrohaAndroid] Norito RPC client tests passed.");
  }

  private static void defaultHeadersAndPayloadAreApplied() throws Exception {
    final RecordingHandler handler = new RecordingHandler(200, "ok".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/default", handler)) {
      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .putDefaultHeader("X-Test-Token", "sample-token")
              .build();
      final byte[] requestPayload = "deadbeef".getBytes(StandardCharsets.UTF_8);
      final byte[] response = client.call("/rpc/default", requestPayload);
      assert new String(response, StandardCharsets.UTF_8).equals("ok")
          : "Response body must match server payload";

      final RecordedRequest recorded = handler.recorded();
      assert recorded != null : "Server should record the request";
      assert "POST".equals(recorded.method())
          : "Norito RPC client must default to POST";
      assert recorded.body().length == requestPayload.length : "Payload should be forwarded";
      assert "application/x-norito".equals(recorded.header("Content-Type"))
          : "Content-Type should default to application/x-norito";
      assert "application/x-norito".equals(recorded.header("Accept"))
          : "Accept should default to application/x-norito";
      assert "sample-token".equals(recorded.header("X-Test-Token"))
          : "Default headers must be preserved";
    }
  }

  private static void requestOverridesApplyHeadersMethodAndQuery() throws Exception {
    final RecordingHandler handler =
        new RecordingHandler(200, "ok".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/custom", handler)) {
      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final NoritoRpcRequestOptions options =
          NoritoRpcRequestOptions.builder()
              .method("GET")
              .accept(null)
              .putHeader("X-Telemetry-Key", "sample")
              .putQueryParameter("version", "1")
              .timeout(Duration.ofSeconds(2))
              .build();
      final byte[] response =
          client.call("/rpc/custom", "ping".getBytes(StandardCharsets.UTF_8), options);
      assert new String(response, StandardCharsets.UTF_8).equals("ok")
          : "Expected handler response";

      final RecordedRequest recorded = handler.recorded();
      assert recorded != null : "Server should record the request";
      assert "GET".equals(recorded.method()) : "Method override should be honoured";
      final String accept = recorded.header("Accept");
      if (accept != null) {
        assert !accept.toLowerCase(Locale.ROOT).contains("application/x-norito")
            : "Accept header should be omitted when set to null";
      }
      assert "sample".equals(recorded.header("X-Telemetry-Key"))
          : "Custom headers should propagate";
      assert recorded.query().contains("version=1")
          : "Query parameters should be attached to the request URI";
    }
  }

  private static void nonSuccessStatusRaisesException() throws Exception {
    final RecordingHandler handler =
        new RecordingHandler(503, "error".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/error", handler)) {
      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      boolean threw = false;
      try {
        client.call("/rpc/error", new byte[] {0x00});
      } catch (final NoritoRpcException ex) {
        threw = true;
        assert ex.getMessage().contains("503") : "Exception message should include status code";
      }
      assert threw : "Expected non-success status to raise NoritoRpcException";
    }
  }

  private static void rejectsInsecureAuthorizationHeader() {
    final NoritoRpcClient client =
        NoritoRpcClient.builder()
            .setBaseUri(URI.create("http://example.com"))
            .putDefaultHeader("Authorization", "Bearer token")
            .build();
    boolean threw = false;
    try {
      client.call("/rpc/default", new byte[] {0x00});
    } catch (final IllegalArgumentException ex) {
      threw = true;
      assert ex.getMessage().contains("refuses insecure transport")
          : "expected insecure transport error";
    }
    assert threw : "Expected insecure credentialed RPC request to be rejected";
  }

  private static void rejectsCredentialedAbsoluteHostOverride() {
    final NoritoRpcClient client =
        NoritoRpcClient.builder()
            .setBaseUri(URI.create("https://example.com"))
            .putDefaultHeader("Authorization", "Bearer token")
            .build();
    boolean threw = false;
    try {
      client.call("https://evil.example/rpc/default", new byte[] {0x00});
    } catch (final IllegalArgumentException ex) {
      threw = true;
      assert ex.getMessage().contains("mismatched host")
          : "expected host mismatch error";
    }
    assert threw : "Expected credentialed absolute host override to be rejected";
  }

  private static void observersReceiveCallbacks() throws Exception {
    final RecordingHandler handler =
        new RecordingHandler(200, "ack".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/observe", handler)) {
      final RecordingObserver observer = new RecordingObserver();
      final NoritoRpcClient client =
        NoritoRpcClient.builder()
            .setBaseUri(server.baseUri())
            .setTransportExecutor(new UrlConnectionTransportExecutor())
            .addObserver(observer)
            .build();
      client.call("/rpc/observe", new byte[] {0x42});
      assert observer.requestCount() == 1 : "Observer must receive onRequest";
      assert observer.responseCount() == 1 : "Observer must receive onResponse";
      assert observer.failureCount() == 0 : "Observer must not see failures";
    }
  }

  private static void flowControllerGuardsLifecycle() throws Exception {
    final RecordingHandler handler =
        new RecordingHandler(200, "ok".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/flow", handler)) {
      final CountingFlowController controller = new CountingFlowController();
      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .setFlowController(controller)
              .build();
      client.call("/rpc/flow", new byte[] {0x01});
      assert controller.acquireCount() == 1 : "Flow controller should guard acquire";
      assert controller.releaseCount() == 1 : "Flow controller should release after call";
    }
  }

  private static void telemetrySignalsEmitForRpcCalls() throws Exception {
    final RecordingHandler handler =
        new RecordingHandler(200, "telemetry".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/telemetry", handler)) {
      final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
      final TelemetryOptions telemetryOptions =
          TelemetryOptions.builder()
              .setTelemetryRedaction(
                  TelemetryOptions.Redaction.builder()
                      .setSaltHex("0a0b0c0d")
                      .setSaltVersion("2026Q2")
                      .setRotationId("android-rpc-test")
                      .build())
              .build();
      final DeviceProfileProvider deviceProvider =
          () -> Optional.of(DeviceProfile.of("pixel-pro"));
      final NetworkContextProvider networkProvider =
          () -> Optional.of(NetworkContext.of("wifi", true));

      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .setTelemetryOptions(telemetryOptions)
              .setTelemetrySink(telemetrySink)
              .setDeviceProfileProvider(deviceProvider)
              .setNetworkContextProvider(networkProvider)
              .addObserver(new TelemetryObserver(telemetryOptions, telemetrySink))
              .build();

      client.call("/rpc/telemetry", "ping".getBytes(StandardCharsets.UTF_8));
      client.call("/rpc/telemetry", "pong".getBytes(StandardCharsets.UTF_8));

      final GaugeEvent deviceSignal =
          telemetrySink.lastSignal("android.telemetry.device_profile");
      assert deviceSignal != null : "Device profile telemetry must be emitted";
      assert "pixel-pro".equals(deviceSignal.fields().get("profile_bucket"))
          : "Device profile bucket mismatch";
      assert telemetrySink.countSignals("android.telemetry.device_profile") == 1
          : "Device profile telemetry should emit once";

      final GaugeEvent networkSignal =
          telemetrySink.lastSignal("android.telemetry.network_context");
      assert networkSignal != null : "Network context telemetry must be emitted";
      assert "wifi".equals(networkSignal.fields().get("network_type"))
          : "Network type mismatch";
      assert Boolean.TRUE.equals(networkSignal.fields().get("roaming"))
          : "Roaming flag mismatch";
      assert telemetrySink.countSignals("android.telemetry.network_context") == 2
          : "Network context should emit per call";

      final TelemetryRecord lastRequest = telemetrySink.lastRequest();
      final String expectedHash =
          telemetryOptions
              .redaction()
              .hashAuthority(server.baseUri().getAuthority())
              .orElseThrow(() -> new IllegalStateException("Authority hash missing"));
      assert lastRequest != null : "Telemetry observer should record request";
      assert expectedHash.equals(lastRequest.authorityHash())
          : "Request telemetry should carry hashed authority";

      final GaugeEvent rpcSignal =
          telemetrySink.lastSignal("android.norito_rpc.call");
      assert rpcSignal != null : "Norito RPC call telemetry must be emitted";
      assert expectedHash.equals(rpcSignal.fields().get("authority_hash"))
          : "RPC telemetry should hash authority";
      assert "/rpc/telemetry".equals(rpcSignal.fields().get("route"))
          : "RPC telemetry should capture route";
      assert "POST".equals(rpcSignal.fields().get("method"))
          : "RPC telemetry should capture method";
      assert !rpcSignal.fields().containsKey("fallback")
          : "RPC telemetry should not emit fallback field";
      final Object status = rpcSignal.fields().get("status_code");
      assert status instanceof Number && ((Number) status).intValue() == 200
          : "RPC telemetry should record status";
      final Object latency = rpcSignal.fields().get("latency_ms");
      assert latency instanceof Number && ((Number) latency).longValue() >= 0L
          : "RPC telemetry should record latency";
      assert telemetrySink.countSignals("android.norito_rpc.call") == 2
          : "RPC telemetry should emit per call";
    }
  }

  private static void callTransactionUsesCodecAdapter() throws Exception {
    final String aliceAuthority = TestAccountIds.ed25519Authority(0x21);
    final String bobAuthority = TestAccountIds.ed25519Authority(0x22);
    final RecordingHandler handler =
        new RecordingHandler(200, "encoded".getBytes(StandardCharsets.UTF_8));
    try (SimpleHttpServer server = SimpleHttpServer.start("/rpc/codec", handler)) {
      final TransactionPayload payload =
          TransactionPayload.builder()
              .setChainId("00000001")
              .setAuthority(aliceAuthority)
              .setInstructionBytes(new byte[] {0x01, 0x02})
              .build();
      final StubCodecAdapter codec =
          new StubCodecAdapter(
              "encoded".getBytes(StandardCharsets.UTF_8),
              payload.toBuilder().setAuthority(bobAuthority).build());
      final NoritoRpcClient client =
          NoritoRpcClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final TransactionPayload decoded =
          client.callTransaction("/rpc/codec", payload, codec, null);
      assert codec.encodeCallCount() == 1 : "Codec should encode request once";
      assert codec.decodeCallCount() == 1 : "Codec should decode response once";
      assert bobAuthority.equals(decoded.authority())
          : "Decoded payload should come from codec adapter";
    }
  }

  private static final class SimpleHttpServer implements AutoCloseable {
    private final HttpServer server;

    private SimpleHttpServer(final HttpServer server) {
      this.server = server;
    }

    static SimpleHttpServer start(final String path, final HttpHandler handler) throws IOException {
      final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
      server.createContext(path, handler);
      server.setExecutor(null);
      server.start();
      return new SimpleHttpServer(server);
    }

    URI baseUri() {
      final InetSocketAddress address = server.getAddress();
      return URI.create(
          "http://"
              + address.getHostString()
              + ":"
              + Integer.toUnsignedString(address.getPort()));
    }

    @Override
    public void close() {
      server.stop(0);
    }
  }

  private static boolean isHttpServerSupported() {
    try {
      final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
      server.stop(0);
      return true;
    } catch (final IOException ex) {
      return false;
    }
  }

  private static final class RecordingHandler implements HttpHandler {
    private final int statusCode;
    private final byte[] responseBody;
    private volatile RecordedRequest recorded;

    private RecordingHandler(final int statusCode, final byte[] responseBody) {
      this.statusCode = statusCode;
      this.responseBody = responseBody == null ? new byte[0] : responseBody.clone();
    }

    @Override
    public void handle(final HttpExchange exchange) throws IOException {
      final byte[] body = exchange.getRequestBody().readAllBytes();
      recorded =
          new RecordedRequest(
              exchange.getRequestMethod(),
              exchange.getRequestURI(),
              exchange.getRequestHeaders(),
              body);
      exchange.sendResponseHeaders(statusCode, responseBody.length);
      exchange.getResponseBody().write(responseBody);
      exchange.close();
    }

    RecordedRequest recorded() {
      return recorded;
    }
  }

  private static final class RecordedRequest {
    private final String method;
    private final URI uri;
    private final Map<String, List<String>> headers;
    private final byte[] body;

    private RecordedRequest(
        final String method,
        final URI uri,
        final Headers headers,
        final byte[] body) {
      this.method = Objects.requireNonNull(method, "method");
      this.uri = Objects.requireNonNull(uri, "uri");
      this.headers = Map.copyOf(headers);
      this.body = body == null ? new byte[0] : body.clone();
    }

    String method() {
      return method;
    }

    String query() {
      return uri.getQuery() == null ? "" : uri.getQuery();
    }

    byte[] body() {
      return body.clone();
    }

    String header(final String name) {
      final List<String> values = headers.get(name);
      if (values == null || values.isEmpty()) {
        for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
          if (entry.getKey().equalsIgnoreCase(name) && !entry.getValue().isEmpty()) {
            return entry.getValue().get(0);
          }
        }
        return null;
      }
      return values.get(0);
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<TelemetryRecord> requests = new ArrayList<>();
    private final List<TelemetryRecord> responses = new ArrayList<>();
    private final List<TelemetryRecord> failures = new ArrayList<>();
    private final List<GaugeEvent> signals = new ArrayList<>();

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

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      signals.add(new GaugeEvent(signalId, Map.copyOf(fields)));
    }

    TelemetryRecord lastRequest() {
      if (requests.isEmpty()) {
        return null;
      }
      return requests.get(requests.size() - 1);
    }

    GaugeEvent lastSignal(final String signalId) {
      GaugeEvent match = null;
      for (final GaugeEvent event : signals) {
        if (event.signalId().equals(signalId)) {
          match = event;
        }
      }
      return match;
    }

    int countSignals(final String signalId) {
      int count = 0;
      for (final GaugeEvent event : signals) {
        if (event.signalId().equals(signalId)) {
          count++;
        }
      }
      return count;
    }
  }

  private static final class GaugeEvent {
    private final String signalId;
    private final Map<String, Object> fields;

    private GaugeEvent(final String signalId, final Map<String, Object> fields) {
      this.signalId = signalId;
      this.fields = fields;
    }

    String signalId() {
      return signalId;
    }

    Map<String, Object> fields() {
      return fields;
    }
  }

  private static final class CountingFlowController implements NoritoRpcFlowController {
    private final AtomicInteger acquireCount = new AtomicInteger();
    private final AtomicInteger releaseCount = new AtomicInteger();

    @Override
    public void acquire() {
      acquireCount.incrementAndGet();
    }

    @Override
    public void release() {
      releaseCount.incrementAndGet();
    }

    int acquireCount() {
      return acquireCount.get();
    }

    int releaseCount() {
      return releaseCount.get();
    }
  }

  private static final class StubCodecAdapter implements NoritoCodecAdapter {
    private final byte[] encodedPayload;
    private final TransactionPayload decodedPayload;
    private final AtomicInteger encodeCalls = new AtomicInteger();
    private final AtomicInteger decodeCalls = new AtomicInteger();

    private StubCodecAdapter(
        final byte[] encodedPayload, final TransactionPayload decodedPayload) {
      this.encodedPayload = encodedPayload.clone();
      this.decodedPayload = decodedPayload;
    }

    @Override
    public byte[] encodeTransaction(final TransactionPayload payload) throws NoritoException {
      encodeCalls.incrementAndGet();
      return encodedPayload.clone();
    }

    @Override
    public TransactionPayload decodeTransaction(final byte[] encoded) throws NoritoException {
      decodeCalls.incrementAndGet();
      return decodedPayload;
    }

    int encodeCallCount() {
      return encodeCalls.get();
    }

    int decodeCallCount() {
      return decodeCalls.get();
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    private int requestCount = 0;
    private int responseCount = 0;
    private int failureCount = 0;
    private int lastStatusCode = -1;

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount++;
    }

    @Override
    public void onResponse(
        final TransportRequest request, final ClientResponse response) {
      responseCount++;
      lastStatusCode = response.statusCode();
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount++;
    }

    int requestCount() {
      return requestCount;
    }

    int responseCount() {
      return responseCount;
    }

    int failureCount() {
      return failureCount;
    }

    int lastStatusCode() {
      return lastStatusCode;
    }
  }
}
