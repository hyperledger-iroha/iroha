package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.mock.ToriiMockServer;
import org.hyperledger.iroha.android.client.mock.ToriiMockServer.MockResponse;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

/** Integration tests exercising {@link HttpClientTransport} against the Torii mock harness. */
public final class HttpClientTransportHarnessTests {

  private HttpClientTransportHarnessTests() {}

  public static void main(final String[] args) throws Exception {
    sorafsGatewayClientSharesConfig();
    noritoRpcClientUsesTransportExecutor();
    if (!ToriiMockServer.isSupported()) {
      System.out.println(
          "[IrohaAndroid] HTTP client harness tests skipped (mock server cannot bind in this environment).");
      return;
    }
    submitSurfacedRejectCode();
    submitAndPollAgainstMockTorii();
    statusRetryContinuesAfter404();
    transportExposesNoritoRpcClient();
    System.out.println("[IrohaAndroid] HTTP client harness tests passed.");
  }

  private static void submitAndPollAgainstMockTorii() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println("[IrohaAndroid] submit/poll harness skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      final HttpClientTransport transport =
          new HttpClientTransport(
              new UrlConnectionTransportExecutor(),
              ClientConfig.builder()
                  .setBaseUri(server.baseUri())
                  .setRequestTimeout(Duration.ofSeconds(5))
                  .putDefaultHeader("Authorization", "Bearer harness")
                  .build());

      final SignedTransaction transaction = sampleTransaction((byte) 0x01);
      final String hashHex = SignedTransactionHasher.hashHex(transaction);

      server.enqueueSubmitResponse(
          MockResponse.bytes(
              202,
              new byte[] {0x01, 0x02, 0x03},
              Map.of("Content-Type", "application/x-norito")));
      server.enqueueStatusResponse(hashHex, MockResponse.json(202, statusPayload("Pending")));
      server.enqueueStatusResponse(hashHex, MockResponse.json(200, statusPayload("Committed")));

      final ClientResponse response = transport.submitTransaction(transaction).join();
      assert response.statusCode() == 202 : "Submission should succeed via mock server";
      assert hashHex.equals(response.hashHex().orElse(null))
          : "Canonical hash must propagate through ClientResponse";

      final Map<String, Object> status =
          transport
              .waitForTransactionStatus(
                  hashHex, PipelineStatusOptions.builder().intervalMillis(0L).build())
              .join();
      assert "Committed".equals(PipelineStatusExtractor.extractStatusKind(status).orElse(null))
          : "Status polling should observe committed payload";

      final var submissions = server.submittedTransactions();
      assert submissions.size() == 1 : "Server must record submissions";
      final var submission = submissions.get(0);
      assert "/v1/pipeline/transactions".equals(submission.path())
          : "Submission should target Torii pipeline route";
      assert Arrays.equals(submission.body(), SignedTransactionEncoder.encodeVersioned(transaction))
          : "Serialized payload should match Norito encoding";
      assert "Bearer harness".equals(submission.header("Authorization").orElse(null))
          : "Custom headers must be forwarded";
      assert server.statusRequests().size() >= 2 : "Status polling should reach the mock server";
    }
  }

  private static void submitSurfacedRejectCode() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println("[IrohaAndroid] reject-code harness skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      final HttpClientTransport transport =
          new HttpClientTransport(
              new UrlConnectionTransportExecutor(),
              ClientConfig.builder().setBaseUri(server.baseUri()).setRequestTimeout(Duration.ofSeconds(5)).build());

      final SignedTransaction transaction = sampleTransaction((byte) 0x0a);

      server.enqueueSubmitResponse(
          MockResponse.bytes(
              400,
              "{\"error\":\"rejected\"}".getBytes(StandardCharsets.UTF_8),
              Map.of(
                  "Content-Type", "application/json",
                  "x-iroha-reject-code", "PRTRY:TX_SIGNATURE_MISSING")));

      final ClientResponse response = transport.submitTransaction(transaction).join();
      assert response.statusCode() == 400 : "Submission should capture rejection status";
      assert "PRTRY:TX_SIGNATURE_MISSING"
              .equals(response.rejectCode().orElse(null))
          : "Reject header must propagate to ClientResponse";
    }
  }

  private static void sorafsGatewayClientSharesConfig() {
    final StubExecutor executor = new StubExecutor();
    final CountingObserver observer = new CountingObserver();
    final ClientConfig transportConfig =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .setRequestTimeout(Duration.ofSeconds(3))
            .putDefaultHeader("Authorization", "Bearer orchestrator")
            .addObserver(observer)
            .build();
    executor.setResponse(
        new TransportResponse(
            200, "{\"status\":\"ok\"}".getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, transportConfig);

    final SorafsGatewayClient gatewayClient =
        transport.newSorafsGatewayClient(URI.create("https://gateway.example"));

    gatewayClient.fetch(sampleGatewayRequest()).join();

    final TransportRequest recorded =
        Objects.requireNonNull(executor.lastRequest(), "gateway request must be recorded");
    assert "POST".equals(recorded.method());
    assert "https://gateway.example/v1/sorafs/gateway/fetch".equals(recorded.uri().toString());
    assert Duration.ofSeconds(3).equals(recorded.timeout()) : "timeout should inherit transport config";
    final var authHeaders = recorded.headers().getOrDefault("Authorization", java.util.List.of());
    assert authHeaders.contains("Bearer orchestrator") : "default headers should carry over";
    assert observer.requestCount.get() == 1 : "observer should see gateway request";
    assert observer.responseCount.get() == 1 : "observer should see gateway response";
  }

  private static void noritoRpcClientUsesTransportExecutor() {
    final StubExecutor executor = new StubExecutor();
    final CountingObserver observer = new CountingObserver();
    final ClientConfig transportConfig =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://rpc.example"))
            .putDefaultHeader("Authorization", "Bearer executor")
            .addObserver(observer)
            .build();
    executor.setResponse(new TransportResponse(204, new byte[0], "", Map.of()));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(executor, transportConfig);
    final NoritoRpcClient rpcClient = transport.newNoritoRpcClient();
    rpcClient.call("/v1/pipeline/transactions", new byte[0]);
    final TransportRequest recorded =
        Objects.requireNonNull(executor.lastRequest(), "RPC request must be captured");
    assert "https://rpc.example/v1/pipeline/transactions"
        .equals(recorded.uri().toString()) : "RPC path should use base URI";
    assert recorded.headers().getOrDefault("Authorization", java.util.List.of())
        .contains("Bearer executor") : "Default headers must apply to RPC calls";
    assert observer.requestCount.get() == 1
        : "Observer should see executor-backed RPC request";
    assert observer.responseCount.get() == 1
        : "Observer should see executor-backed RPC response";
  }

  private static void statusRetryContinuesAfter404() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println("[IrohaAndroid] status retry harness skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      final HttpClientTransport transport =
          new HttpClientTransport(
              new UrlConnectionTransportExecutor(),
              ClientConfig.builder()
                  .setBaseUri(server.baseUri())
                  .setRequestTimeout(Duration.ofSeconds(5))
                  .build());

      final SignedTransaction transaction = sampleTransaction((byte) 0x21);
      final String hashHex = SignedTransactionHasher.hashHex(transaction);

      server.enqueueSubmitResponse(MockResponse.json(202, "{}"));
      server.enqueueStatusResponse(hashHex, MockResponse.empty(404));
      server.enqueueStatusResponse(hashHex, MockResponse.json(200, statusPayload("Committed")));

      transport.submitTransaction(transaction).join();
      final Map<String, Object> status =
          transport
              .waitForTransactionStatus(
                  hashHex,
                  PipelineStatusOptions.builder().intervalMillis(0L).maxAttempts(5).build())
              .join();
      assert "Committed".equals(PipelineStatusExtractor.extractStatusKind(status).orElse(null))
          : "Client should continue polling after 404 responses";
      assert server.statusRequests().size() >= 2 : "Multiple polls should hit the harness";
    }
  }

  private static void transportExposesNoritoRpcClient() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println("[IrohaAndroid] Norito RPC harness skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      server.enqueueSubmitResponse(MockResponse.empty(204));
      final CountingObserver observer = new CountingObserver();
      final HttpClientTransport transport =
          new HttpClientTransport(
              new UrlConnectionTransportExecutor(),
              ClientConfig.builder()
                  .setBaseUri(server.baseUri())
                  .addObserver(observer)
                  .putDefaultHeader("Authorization", "Bearer harness")
                  .build());
      final NoritoRpcClient rpcClient = transport.newNoritoRpcClient();
      rpcClient.call("/v1/pipeline/transactions", new byte[0]);
      assert observer.requestCount.get() == 1 : "RPC client should trigger observer request";
      assert observer.responseCount.get() == 1 : "RPC client should trigger observer response";
      final var submissions = server.submittedTransactions();
      assert !submissions.isEmpty() : "Mock server should record RPC submission";
      assert "Bearer harness".equals(submissions.get(0).header("Authorization").orElse(null))
          : "Config headers must carry over to Norito RPC client";
    }
  }

  private static String statusPayload(final String kind) {
    return "{\"kind\":\"Transaction\",\"content\":{\"status\":{\"kind\":\""
        + kind + "\"}}}";
  }

  private static SignedTransaction sampleTransaction(final byte seed) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", seed))
            .setAuthority("alice@wonderland")
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
    Arrays.fill(signature, (byte) (seed + 3));
    Arrays.fill(publicKey, (byte) (seed + 5));
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }

  private static GatewayFetchRequest sampleGatewayRequest() {
    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("alpha")
            .setProviderIdHex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .setBaseUrl("https://provider.example")
            .setStreamTokenBase64("c3RyZWFtLXRva2Vu")
            .build();
    return GatewayFetchRequest.builder()
        .setManifestIdHex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .setChunkerHandle("sorafs.sf1@1.0.0")
        .setOptions(
            GatewayFetchOptions.builder()
                .setClientId("android-sdk")
                .setTelemetryRegion("iad-test")
                .build())
        .addProvider(provider)
        .build();
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private TransportResponse response;
    private TransportRequest lastRequest;

    void setResponse(final TransportResponse response) {
      this.response = Objects.requireNonNull(response, "response");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      if (response == null) {
        throw new AssertionError("response not configured");
      }
      return CompletableFuture.completedFuture(response);
    }

    TransportRequest lastRequest() {
      return lastRequest;
    }
  }

  private static final class CountingObserver implements ClientObserver {
    private final java.util.concurrent.atomic.AtomicInteger requestCount =
        new java.util.concurrent.atomic.AtomicInteger();
    private final java.util.concurrent.atomic.AtomicInteger responseCount =
        new java.util.concurrent.atomic.AtomicInteger();

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount.incrementAndGet();
    }

    @Override
    public void onResponse(
        final TransportRequest request, final ClientResponse response) {
      responseCount.incrementAndGet();
    }
  }
}
