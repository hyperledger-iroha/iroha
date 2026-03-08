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
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectTelemetry;

public final class OfflineWalletSafetyDetectTests {

  private OfflineWalletSafetyDetectTests() {}

  public static void main(final String[] args) throws Exception {
    fetchesAttestationWhenConfigured();
    optionalPolicySuppressesFailures();
    disabledPolicyFailsFast();
    System.out.println("[IrohaAndroid] OfflineWalletSafetyDetectTests passed.");
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
              "android.integrity.policy": "hms_safety_detect",
              "android.hms_safety_detect.app_id": "app",
              "android.hms_safety_detect.package_names": ["pkg"],
              "android.hms_safety_detect.signing_digests_sha256": ["aaaa"],
              "android.hms_safety_detect.required_evaluations": ["strong_integrity"],
              "android.hms_safety_detect.max_token_age_ms": 3600000
            }
          }
        }
        """;
    return new OfflineAllowanceList.OfflineAllowanceItem(
        "deadbeef",
        "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7TTEp",
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
    final RecordingSafetyDetectClient client = new RecordingSafetyDetectClient();
    final RecordingTelemetry telemetry = new RecordingTelemetry();
    final OfflineWallet wallet =
        newWallet(
            journal,
            OfflineWallet.Options.builder()
                .safetyDetectEnabled(true)
                .safetyDetectService(client)
                .safetyDetectTelemetry(telemetry)
                .build());
    final SafetyDetectAttestation attestation = wallet.fetchSafetyDetectAttestation("deadbeef").join();
    assert "token-1".equals(attestation.token());
    assert telemetry.successCount == 1;
    assert client.attempts == 1;
    final OfflineVerdictMetadata.SafetyDetectTokenSnapshot cached =
        wallet.cachedSafetyDetectToken("deadbeef").orElseThrow();
    assert "token-1".equals(cached.token());
  }

  private static void optionalPolicySuppressesFailures() throws Exception {
    final OfflineVerdictJournal journal = newJournal();
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final SafetyDetectService failing =
        new SafetyDetectService() {
          @Override
          public CompletableFuture<SafetyDetectAttestation> fetch(
              final SafetyDetectRequest request) {
            final CompletableFuture<SafetyDetectAttestation> future = new CompletableFuture<>();
            future.completeExceptionally(new IllegalStateException("boom"));
            return future;
          }
        };
    final RecordingTelemetry telemetry = new RecordingTelemetry();
    final OfflineWallet wallet =
        newWallet(
            journal,
            OfflineWallet.Options.builder()
                .safetyDetectEnabled(true)
                .safetyDetectOptional(true)
                .safetyDetectService(failing)
                .safetyDetectTelemetry(telemetry)
                .build());
    final SafetyDetectAttestation attestation = wallet.fetchSafetyDetectAttestation("deadbeef").join();
    assert attestation == null : "optional failures should return null";
    assert telemetry.failureCount == 1 : "optional failures should still emit telemetry";
  }

  private static void disabledPolicyFailsFast() throws Exception {
    final OfflineVerdictJournal journal = newJournal();
    journal.upsert(List.of(sampleAllowance()), Instant.now());
    final OfflineWallet wallet =
        newWallet(journal, OfflineWallet.Options.builder().build());
    boolean failed = false;
    try {
      wallet.fetchSafetyDetectAttestation("deadbeef").join();
    } catch (Exception ex) {
      failed = true;
    }
    assert failed : "disabled policy should reject attestation calls";
  }

  private static final class RecordingSafetyDetectClient implements SafetyDetectService {
    private int attempts = 0;

    @Override
    public CompletableFuture<SafetyDetectAttestation> fetch(final SafetyDetectRequest request) {
      attempts++;
      return CompletableFuture.completedFuture(new SafetyDetectAttestation("token-1", System.currentTimeMillis()));
    }
  }

  private static final class RecordingTelemetry implements SafetyDetectTelemetry {
    private int successCount = 0;
    private int failureCount = 0;

    @Override
    public void onSuccess(
        final SafetyDetectRequest request, final SafetyDetectAttestation attestation) {
      successCount++;
    }

    @Override
    public void onFailure(final SafetyDetectRequest request, final Throwable error) {
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
