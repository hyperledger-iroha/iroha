package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.OfflineToriiClient;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityAttestation;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityRequest;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityService;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityTelemetry;

public final class OfflineWalletPlayIntegrityTests {

  private OfflineWalletPlayIntegrityTests() {}

  public static void main(final String[] args) throws Exception {
    fetchesAttestationWhenConfigured();
    optionalPolicySuppressesFailures();
    disabledPolicyFailsFast();
    System.out.println("[IrohaAndroid] OfflineWalletPlayIntegrityTests passed.");
  }

  private static OfflineWallet newWallet(
      final OfflineVerdictJournal journal, final OfflineWallet.Options options) throws IOException {
    final OfflineToriiClient offlineClient =
        OfflineToriiClient.builder()
            .executor(new NoopExecutor())
            .baseUri(URI.create("https://example.com"))
            .build();
    final Path log = Files.createTempFile("offline-wallet", ".json");
    final OfflineAuditLogger logger = new OfflineAuditLogger(log, false);
    final OfflineCounterJournal counterJournal =
        new OfflineCounterJournal(Files.createTempFile("offline-counters", ".json"));
    return new OfflineWallet(
        offlineClient,
        logger,
        journal,
        counterJournal,
        options,
        OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
        null);
  }

  private static OfflineVerdictJournal newJournal() throws IOException {
    final Path file = Files.createTempFile("offline-verdicts", ".json");
    return new OfflineVerdictJournal(file);
  }

  private static OfflineAllowanceList.OfflineAllowanceItem sampleAllowance() {
    final String recordJson =
        """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "play_integrity",
              "android.play_integrity.cloud_project_number": 4242,
              "android.play_integrity.environment": "testing",
              "android.play_integrity.package_names": ["com.example.pos"],
              "android.play_integrity.signing_digests_sha256": ["ab12"],
              "android.play_integrity.allowed_app_verdicts": ["PLAY_RECOGNIZED"],
              "android.play_integrity.allowed_device_verdicts": ["MEETS_DEVICE_INTEGRITY"],
              "android.play_integrity.max_token_age_ms": 3600000
            }
          }
        }
        """;
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "deadbeef",
        "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV",
        "Alice",
        "norito:00",
        Instant.now().toEpochMilli(),
        Instant.now().plusSeconds(3600).toEpochMilli(),
        Instant.now().plusSeconds(7200).toEpochMilli(),
        Instant.now().plusSeconds(1800).toEpochMilli(),
        "cafefeed",
        "001122",
        "10",
        recordJson);
  }

  private static void fetchesAttestationWhenConfigured() throws Exception {
    final OfflineVerdictJournal journal = newJournal();
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final RecordingPlayIntegrityClient client = new RecordingPlayIntegrityClient();
    final RecordingTelemetry telemetry = new RecordingTelemetry();
    final OfflineWallet wallet =
        newWallet(
            journal,
            OfflineWallet.Options.builder()
                .playIntegrityEnabled(true)
                .playIntegrityService(client)
                .playIntegrityTelemetry(telemetry)
                .build());
    final PlayIntegrityAttestation attestation =
        wallet.fetchPlayIntegrityAttestation("deadbeef").join();
    assert "token-1".equals(attestation.token());
    assert telemetry.successCount == 1;
    assert client.attempts == 1;
    final OfflineVerdictMetadata.PlayIntegrityTokenSnapshot cached =
        wallet.cachedPlayIntegrityToken("deadbeef").orElseThrow();
    assert "token-1".equals(cached.token());
  }

  private static void optionalPolicySuppressesFailures() throws Exception {
    final OfflineVerdictJournal journal = newJournal();
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final PlayIntegrityService failing =
        new PlayIntegrityService() {
          @Override
          public CompletableFuture<PlayIntegrityAttestation> fetch(
              final PlayIntegrityRequest request) {
            final CompletableFuture<PlayIntegrityAttestation> future = new CompletableFuture<>();
            future.completeExceptionally(new IllegalStateException("boom"));
            return future;
          }
        };
    final RecordingTelemetry telemetry = new RecordingTelemetry();
    final OfflineWallet wallet =
        newWallet(
            journal,
            OfflineWallet.Options.builder()
                .playIntegrityEnabled(true)
                .playIntegrityOptional(true)
                .playIntegrityService(failing)
                .playIntegrityTelemetry(telemetry)
                .build());
    final PlayIntegrityAttestation attestation =
        wallet.fetchPlayIntegrityAttestation("deadbeef").join();
    assert attestation == null : "optional failures should return null";
    assert telemetry.failureCount == 1 : "optional failures should emit telemetry";
  }

  private static void disabledPolicyFailsFast() throws Exception {
    final OfflineVerdictJournal journal = newJournal();
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final OfflineWallet wallet =
        newWallet(journal, OfflineWallet.Options.builder().build());
    boolean failed = false;
    try {
      wallet.fetchPlayIntegrityAttestation("deadbeef").join();
    } catch (Exception ex) {
      failed = true;
    }
    assert failed : "disabled policy should reject attestation calls";
  }

  private static final class RecordingPlayIntegrityClient implements PlayIntegrityService {
    private int attempts = 0;

    @Override
    public CompletableFuture<PlayIntegrityAttestation> fetch(
        final PlayIntegrityRequest request) {
      attempts++;
      return CompletableFuture.completedFuture(
          new PlayIntegrityAttestation("token-1", System.currentTimeMillis()));
    }
  }

  private static final class RecordingTelemetry implements PlayIntegrityTelemetry {
    private int successCount = 0;
    private int failureCount = 0;

    @Override
    public void onSuccess(
        final PlayIntegrityRequest request, final PlayIntegrityAttestation attestation) {
      successCount++;
    }

    @Override
    public void onFailure(final PlayIntegrityRequest request, final Throwable error) {
      failureCount++;
    }
  }

  private static final class NoopExecutor implements HttpTransportExecutor {
    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      future.completeExceptionally(new UnsupportedOperationException("not expected"));
      return future;
    }
  }
}
