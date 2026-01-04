package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.mock.ToriiMockServer;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;

/** Tests covering ClientConfig -> NoritoRpcClient conversion helpers. */
public final class ClientConfigNoritoRpcTests {

  private ClientConfigNoritoRpcTests() {}

  public static void main(final String[] args) throws Exception {
    if (!ToriiMockServer.isSupported()) {
      System.out.println(
          "[IrohaAndroid] Client config Norito RPC tests skipped (mock server cannot bind in this environment).");
      return;
    }
    configProducesNoritoRpcClientWithHeadersAndObservers();
    flowControllerCarriesOver();
    executorReuseSharesObservers();
    System.out.println("[IrohaAndroid] Client config Norito RPC tests passed.");
  }

  private static void configProducesNoritoRpcClientWithHeadersAndObservers() throws Exception {
    final RecordingObserver observer = new RecordingObserver("Bearer config");
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("http://127.0.0.1:0"))
            .setRequestTimeout(Duration.ofSeconds(2))
            .putDefaultHeader("Authorization", "Bearer config")
            .addObserver(observer)
            .build();

    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println(
          "[IrohaAndroid] Client config Norito RPC tests skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      server.enqueueSubmitResponse(ToriiMockServer.MockResponse.empty(200));
      final ClientConfig adjusted =
          config.toBuilder().setBaseUri(server.baseUri()).build();
      final NoritoRpcClient client =
          adjusted.toNoritoRpcClient(new UrlConnectionTransportExecutor());

      final byte[] response =
          client.call("/v1/pipeline/transactions", new byte[0]);
      assert response.length == 0 : "Mock server should return empty body by default";
      assert observer.requestCount.get() == 1 : "Observer should see Norito RPC requests";
      assert observer.responseCount.get() == 1 : "Observer should see Norito RPC responses";
      assert observer.failureCount.get() == 0 : "Observer should not see failures";
    }
  }

  private static void flowControllerCarriesOver() throws Exception {
    final RecordingFlowController flowController = new RecordingFlowController();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("http://127.0.0.1:0"))
            .setNoritoRpcFlowController(flowController)
            .build();

    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println(
          "[IrohaAndroid] Client config Norito RPC flow-control test skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      server.enqueueSubmitResponse(ToriiMockServer.MockResponse.empty(200));
      final ClientConfig adjusted = config.toBuilder().setBaseUri(server.baseUri()).build();
      final NoritoRpcClient client =
          adjusted.toNoritoRpcClient(new UrlConnectionTransportExecutor());
      final byte[] response = client.call("/v1/pipeline/transactions", new byte[] {0x01});
      assert response.length == 0 : "Mock server should return empty body";
      assert flowController.acquireCount() == 1
          : "Flow controller should wrap propagated client";
      assert flowController.releaseCount() == 1
          : "Flow controller should release after call";
    }
  }

  private static void executorReuseSharesObservers() throws Exception {
    final RecordingObserver observer = new RecordingObserver(null);
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(new URI("http://127.0.0.1:0"))
            .addObserver(observer)
            .build();
    final StubExecutor executor = new StubExecutor();
    executor.setResponse(new TransportResponse(204, new byte[0], "", java.util.Map.of()));
    final NoritoRpcClient client = config.toNoritoRpcClient(executor);
    final byte[] response =
        client.call("/v1/pipeline/transactions", new byte[0]);
    assert response.length == 0 : "Stub executor returns empty body";
    assert executor.lastRequest() != null : "Executor should capture RPC request";
    assert observer.requestCount.get() == 1 : "Observers should see executor-backed request";
    assert observer.responseCount.get() == 1 : "Observers should see executor-backed response";
  }

  private static final class RecordingObserver implements ClientObserver {
    private final AtomicInteger requestCount = new AtomicInteger();
    private final AtomicInteger responseCount = new AtomicInteger();
    private final AtomicInteger failureCount = new AtomicInteger();
    private final String expectedAuthorization;

    private RecordingObserver(final String expectedAuthorization) {
      this.expectedAuthorization = expectedAuthorization;
    }

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount.incrementAndGet();
      if (expectedAuthorization != null) {
        assert request.headers().getOrDefault("Authorization", java.util.List.of())
            .contains(expectedAuthorization) : "Config headers must be preserved";
      }
    }

    @Override
    public void onResponse(
        final TransportRequest request, final ClientResponse response) {
      responseCount.incrementAndGet();
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount.incrementAndGet();
    }
  }

  private static final class RecordingFlowController implements NoritoRpcFlowController {
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

  private static final class StubExecutor implements HttpTransportExecutor {
    private TransportResponse response;
    private TransportRequest lastRequest;

    void setResponse(final TransportResponse response) {
      this.response = response;
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
}
