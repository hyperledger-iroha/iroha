package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.TimeUnit;
import org.hyperledger.iroha.android.client.mock.ToriiMockServer;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Integration tests proving transaction submission never replays or fills pending queues. */
public final class HttpClientTransportPendingQueueTests {

  private HttpClientTransportPendingQueueTests() {}

  public static void main(final String[] args) throws Exception {
    leavesPendingQueueUntouchedDuringNewSubmission();
    failedSubmissionDoesNotQueueOrEmitDepthTelemetry();
    System.out.println("[IrohaAndroid] HttpClientTransportPendingQueueTests passed.");
  }

  private static void leavesPendingQueueUntouchedDuringNewSubmission() throws Exception {
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
        assert submissions.size() == 1 : "only the live transaction may be dispatched";
        assert Arrays.equals(
            submissions.get(0).body(),
            SignedTransactionEncoder.encodeVersioned(live))
            : "submission must use the caller-supplied transaction";
        assert queue.size() == 2 : "submission must not drain persisted signed bytes";
      } finally {
        deleteRecursively(queueDir);
      }
    }
  }

  private static void failedSubmissionDoesNotQueueOrEmitDepthTelemetry() throws Exception {
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
        throw new AssertionError("Expected submission to fail");
      } catch (final CompletionException expected) {
        // Expected path.
      }

      final PendingTransactionQueue queue = config.pendingQueue();
      assert queue.size() == 0 : "failed submission must not queue signed bytes for replay";
      final RecordingTelemetrySink.SignalEvent event =
          telemetrySink.findSignal("android.pending_queue.depth");
      assert event == null : "submission must not emit automatic pending-queue telemetry";
    } finally {
      deleteRecursively(queueDir);
    }
  }

  private static SignedTransaction fakeTransaction(final String marker) {
    final int nonce = Math.floorMod(marker.hashCode(), 1000) + 1;
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList(), 1L))
            .setNetworkId(
                org.hyperledger.iroha.android.testing.TestNetworkIds.fromSeed(nonce))
            .setAuthority(TestAccountIds.ed25519Authority(0x24))
            .setCreationTimeMs(1_700_000_000_000L + nonce)
            .setInstructionBytes(marker.getBytes(StandardCharsets.UTF_8))
            .setTimeToLiveMs(5_000L)
            .setNonce(nonce)
            .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
            .setMetadata(Map.of("marker", marker))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT);
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
