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
        queue.enqueue(fakeTransaction("queued-one"));
        queue.enqueue(fakeTransaction("queued-two"));

        final HttpClientTransport transport =
            new HttpClientTransport(new UrlConnectionTransportExecutor(), config);
        transport.submitTransaction(fakeTransaction("live-three")).get(5, TimeUnit.SECONDS);

        final List<ToriiMockServer.SubmitRequest> submissions = server.submittedTransactions();
        assert submissions.size() == 3 : "expected queued transactions to flush before live submit";
        assert submissions
            .get(0)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64("queued-one") + "\"")
            : "first submission should flush the first queued payload";
        assert submissions
            .get(1)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64("queued-two") + "\"")
            : "second submission should flush the second queued payload";
        assert submissions
            .get(2)
            .bodyUtf8()
            .contains("\"payload\":\"" + base64("live-three") + "\"")
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
    final byte[] payload = marker.getBytes(StandardCharsets.UTF_8);
    final byte[] signature = ("sig-" + marker).getBytes(StandardCharsets.UTF_8);
    final byte[] publicKey = ("pk-" + marker).getBytes(StandardCharsets.UTF_8);
    return new SignedTransaction(payload, signature, publicKey, "test.schema/" + marker);
  }

  private static String base64(final String value) {
    return Base64.getEncoder().encodeToString(value.getBytes(StandardCharsets.UTF_8));
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
    private volatile SignalEvent lastSignal;

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
      this.lastSignal = new SignalEvent(signalId, Map.copyOf(Objects.requireNonNull(fields, "fields")));
    }

    SignalEvent findSignal(final String id) {
      final SignalEvent event = lastSignal;
      if (event == null) {
        return null;
      }
      if (!Objects.equals(id, event.id())) {
        return null;
      }
      return event;
    }

    record SignalEvent(String id, Map<String, Object> fields) {}
  }
}
