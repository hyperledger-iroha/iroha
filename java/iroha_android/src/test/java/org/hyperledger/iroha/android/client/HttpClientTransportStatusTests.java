package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class HttpClientTransportStatusTests {

  private HttpClientTransportStatusTests() {}

  public static void main(final String[] args) {
    waitForTransactionStatusResolvesOnCommit();
    waitForTransactionStatusThrowsOnFailure();
    waitForTransactionStatusHonoursMaxAttempts();
    submitTransactionProvidesCanonicalHashForPolling();
    System.out.println("[IrohaAndroid] HTTP client status tests passed.");
  }

  private static void waitForTransactionStatusResolvesOnCommit() {
    final SequencedExecutor executor = new SequencedExecutor(
        newResponse(202, statusPayload("Pending")),
        newResponse(200, statusPayload("Committed")));
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofSeconds(5))
            .build());

    final StringBuilder observed = new StringBuilder();
    final PipelineStatusOptions options =
        PipelineStatusOptions.builder()
            .intervalMillis(0L)
            .observer((status, payload, attempt) -> {
              observed.append(status).append("@").append(attempt).append(";");
            })
            .build();

    final Map<String, Object> result =
        transport.waitForTransactionStatus("deadbeef", options).join();

    assert "Committed".equals(
            PipelineStatusExtractor.extractStatusKind(result).orElse(null))
        : "Expected committed status";
    assert observed.toString().contains("Pending@1")
        && observed.toString().contains("Committed@2")
        : "Observer should capture pending and committed statuses";
  }

  private static void waitForTransactionStatusThrowsOnFailure() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayload("Rejected"))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              "cafebabe", PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusException : "Expected TransactionStatusException";
      assert "Rejected".equals(((TransactionStatusException) cause).status())
          : "Expected rejected status";
    }
    assert threw : "Expected waitForTransactionStatus to throw on failure status";
  }

  private static void waitForTransactionStatusHonoursMaxAttempts() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayload("Pending"))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              "feed", PipelineStatusOptions.builder().intervalMillis(0L).maxAttempts(2).timeoutMillis(null).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionTimeoutException : "Expected TransactionTimeoutException";
      assert ((TransactionTimeoutException) cause).attempts() == 2 : "Expected two attempts";
    }
    assert threw : "Expected waitForTransactionStatus to time out";
  }

  private static void submitTransactionProvidesCanonicalHashForPolling() {
    final SignedTransaction transaction = sampleTransaction((byte) 0x10);
    final String expectedHash = SignedTransactionHasher.hashHex(transaction);
    final TrackingExecutor executor = new TrackingExecutor(expectedHash);
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRequestTimeout(Duration.ofSeconds(1))
                .build());

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert expectedHash.equals(response.hashHex().orElse(null))
        : "submitTransaction must return canonical hash";

    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                expectedHash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();

    assert "Committed".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "Expected committed status after polling";
    assert executor.observedExpectedHash()
        : "Status polling must include canonical hash in request URI";
  }

  private static TransportResponse newResponse(final int status, final byte[] body) {
    return new TransportResponse(status, body, "", Map.of());
  }

  private static byte[] statusPayload(final String kind) {
    final String json = "{\"kind\":\"Transaction\",\"content\":{\"status\":{\"kind\":\""
        + kind + "\"}}}";
    return json.getBytes(StandardCharsets.UTF_8);
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
    Arrays.fill(signature, (byte) (seed + 1));
    Arrays.fill(publicKey, (byte) (seed + 2));
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private final TransportResponse[] responses;
    private int index = 0;

    private SequencedExecutor(final TransportResponse... responses) {
      this.responses = Objects.requireNonNull(responses, "responses");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (index >= responses.length) {
        return CompletableFuture.completedFuture(responses[responses.length - 1]);
      }
      return CompletableFuture.completedFuture(responses[index++]);
    }
  }

  private static final class TrackingExecutor implements HttpTransportExecutor {
    private final String expectedHash;
    private final AtomicInteger pollCount = new AtomicInteger(0);
    private volatile boolean observedExpectedHash = false;

    private TrackingExecutor(final String expectedHash) {
      this.expectedHash = expectedHash;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if ("POST".equals(request.method())) {
        return CompletableFuture.completedFuture(new TransportResponse(202, new byte[0], "", Map.of()));
      }
      if ("GET".equals(request.method())) {
        final String query = request.uri().getQuery();
        if (query != null && query.contains("hash=" + expectedHash)) {
          observedExpectedHash = true;
        }
        final int count = pollCount.getAndIncrement();
        if (count == 0) {
          return CompletableFuture.completedFuture(
              new TransportResponse(202, statusPayload("Pending"), "", Map.of()));
        }
        return CompletableFuture.completedFuture(
            new TransportResponse(200, statusPayload("Committed"), "", Map.of()));
      }
      throw new IllegalStateException("Unexpected HTTP method " + request.method());
    }

    boolean observedExpectedHash() {
      return observedExpectedHash;
    }
  }
}
