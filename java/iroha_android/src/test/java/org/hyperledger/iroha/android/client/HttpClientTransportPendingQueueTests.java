package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.TimeUnit;
import org.hyperledger.iroha.android.client.mock.ToriiMockServer;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Integration tests covering pending-queue flushing and telemetry emission. */
public final class HttpClientTransportPendingQueueTests {

  private HttpClientTransportPendingQueueTests() {}

  public static void main(final String[] args) throws Exception {
    if (!ToriiMockServer.isSupported()) {
      System.out.println(
          "[IrohaAndroid] HttpClientTransportPendingQueueTests skipped (mock server unavailable).");
      return;
    }
    flushesPendingQueueBeforeNewSubmission();
    queuesFailedSubmissionAndEmitsTelemetry();
    System.out.println("[IrohaAndroid] HttpClientTransportPendingQueueTests passed.");
  }

  private static void flushesPendingQueueBeforeNewSubmission() throws Exception {
    try (ToriiMockServer server = ToriiMockServer.create()) {
      final Path queueDir =
          Files.createTempDirectory("http_client_transport_pending_queue_flush");
      try {
        final ClientConfig config =
            ClientConfig.builder()
                .setBaseUri(server.baseUri())
                .enableDirectoryPendingQueue(queueDir)
                .build();
        final PendingTransactionQueue queue = config.pendingQueue();
        final SignedTransaction queuedOne = fakeTransaction("queued-one");
        final SignedTransaction queuedTwo = fakeTransaction("queued-two");
        queue.enqueue(queuedOne);
        queue.enqueue(queuedTwo);

        final HttpClientTransport transport =
            new HttpClientTransport(new UrlConnectionTransportExecutor(), config);
        final SignedTransaction live = fakeTransaction("live-three");
        transport.submitTransaction(live).get(5, TimeUnit.SECONDS);

        final List<ToriiMockServer.SubmitRequest> submissions = server.submittedTransactions();
        assert submissions.size() == 3 : "expected queued transactions to flush before live submit";
        assert submissions
            .get(0)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64Payload(queuedOne) + "\"")
            : "first submission should flush the first queued payload";
        assert submissions
            .get(1)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64Payload(queuedTwo) + "\"")
            : "second submission should flush the second queued payload";
        assert submissions
            .get(2)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64Payload(live) + "\"")
            : "final submission should be the live transaction";
        assert queue.size() == 0 : "pending queue should be empty after successful flush";
      } finally {
        deleteRecursively(queueDir);
      }
    }
  }

  private static void queuesFailedSubmissionAndEmitsTelemetry() throws Exception {
    final RecordingTelemetrySink telemetrySink = new RecordingTelemetrySink();
    final Path queueDir =
        Files.createTempDirectory("http_client_transport_pending_queue_telemetry");
    try {
      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(new URI("http://127.0.0.1"))
              .enableDirectoryPendingQueue(queueDir)
              .setTelemetryOptions(TelemetryOptions.builder().build())
              .setTelemetrySink(telemetrySink)
              .build();
      final HttpTransportExecutor failingExecutor =
          request ->
              CompletableFuture.completedFuture(new TransportResponse(503, new byte[0], "", Map.of()));
      final HttpClientTransport transport = new HttpClientTransport(failingExecutor, config);
      try {
        transport.submitTransaction(fakeTransaction("telemetry-failure")).join();
        throw new AssertionError("Expected submission to fail and queue the transaction");
      } catch (final CompletionException expected) {
        // Expected path.
      }

      final PendingTransactionQueue queue = config.pendingQueue();
      assert queue.size() == 1 : "failed submission should be queued for later replay";
      final RecordingTelemetrySink.SignalEvent event =
          telemetrySink.findSignal("android.pending_queue.depth");
      assert event != null : "pending-queue depth telemetry should be emitted";
      assert "oem_directory".equals(event.fields().get("queue"))
          : "queue name should reflect the directory-backed queue";
      final Object depth = event.fields().get("depth");
      assert depth instanceof Number : "depth should be recorded as a numeric value";
      assert ((Number) depth).longValue() == 1L : "queued depth should reflect the failed submit";
    } finally {
      deleteRecursively(queueDir);
    }
  }

  private static SignedTransaction fakeTransaction(final String marker) {
    final int nonce = Math.floorMod(marker.hashCode(), 1000) + 1;
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", nonce))
            .setAuthority("queue@wonderland")
            .setCreationTimeMs(1_700_000_000_000L + nonce)
            .setInstructionBytes(marker.getBytes(StandardCharsets.UTF_8))
            .setTimeToLiveMs(5_000L)
            .setNonce(nonce)
            .setMetadata(Map.of("marker", marker))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encoded;
    try {
      encoded = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    final byte[] signature = ("sig-" + marker).getBytes(StandardCharsets.UTF_8);
    final byte[] publicKey = ("pk-" + marker).getBytes(StandardCharsets.UTF_8);
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }

  private static String base64Payload(final SignedTransaction transaction) {
    return Base64.getEncoder().encodeToString(transaction.encodedPayload());
  }

  private static void deleteRecursively(final Path root) throws Exception {
    if (root == null) {
      return;
    }
    if (!Files.exists(root)) {
      return;
    }
    try (var paths = Files.walk(root)) {
      paths
          .sorted((a, b) -> b.getNameCount() - a.getNameCount())
          .forEach(
              path -> {
                try {
                  Files.deleteIfExists(path);
                } catch (final Exception ignored) {
                  // Best-effort cleanup for test artefacts.
                }
              });
    }
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final java.util.concurrent.ConcurrentLinkedQueue<SignalEvent> signals =
        new java.util.concurrent.ConcurrentLinkedQueue<>();

    @Override
    public void onRequest(final org.hyperledger.iroha.android.telemetry.TelemetryRecord record) {
      // No-op for this test.
    }

    @Override
    public void onResponse(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record,
        final ClientResponse response) {
      // No-op for this test.
    }

    @Override
    public void onFailure(
        final org.hyperledger.iroha.android.telemetry.TelemetryRecord record,
        final Throwable error) {
      // No-op for this test.
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      signals.add(new SignalEvent(signalId, Map.copyOf(Objects.requireNonNull(fields, "fields"))));
    }

    SignalEvent findSignal(final String id) {
      SignalEvent match = null;
      for (final SignalEvent event : signals) {
        if (Objects.equals(id, event.id())) {
          match = event;
        }
      }
      return match;
    }

    record SignalEvent(String id, Map<String, Object> fields) {}
  }
}
