package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.OfflineToriiClient;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.OfflineAllowanceCommitment;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificateDraft;
import org.hyperledger.iroha.android.offline.OfflineWalletPolicy;

public final class OfflineWalletTest {

  private OfflineWalletTest() {}

  public static void main(final String[] args) throws Exception {
    recordsTransferAuditFromReceipt();
    circulationsModesFlipAndNotify();
    verdictFreshnessGuards();
    safetyDetectSnapshotsPersistAndExpire();
    topUpRecordsVerdictMetadata();
    exportsVerdictJournalJson();
    System.out.println("[IrohaAndroid] OfflineWalletTest passed.");
  }

  private static Path verdictJournalPath(final Path auditLogPath) {
    if (auditLogPath == null) {
      return Path.of("offline_verdict_journal.json");
    }
    return auditLogPath.resolveSibling("offline_verdict_journal.json");
  }

  private static Path counterJournalPath(final Path auditLogPath) {
    if (auditLogPath == null) {
      return Path.of("offline_counter_journal.json");
    }
    return auditLogPath.resolveSibling("offline_counter_journal.json");
  }

  private static OfflineToriiClient stubClient() {
    final HttpTransportExecutor executor =
        request -> {
          final CompletableFuture<TransportResponse> future =
              new CompletableFuture<>();
          future.completeExceptionally(new UnsupportedOperationException("not used"));
          return future;
        };
    return OfflineToriiClient.builder()
        .executor(executor)
        .baseUri(URI.create("http://localhost"))
        .build();
  }

  private static void recordsTransferAuditFromReceipt() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_test", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final OfflineAuditLogger logger = new OfflineAuditLogger(logFile, true);
      final OfflineWallet wallet = new OfflineWallet(stubClient(), logger);
      final String transferJson =
          """
          {
            "receipts": [
              {
                "from": "alice@wonderland",
                "to": "merchant@wonderland",
                "asset": "usd#wonderland#merchant@wonderland",
                "amount": "3.14"
              }
            ],
            "balance_proof": {
              "claimed_delta": "3.14"
            }
          }
          """;
      final OfflineTransferList.OfflineTransferItem item =
          new OfflineTransferList.OfflineTransferItem(
              "bundle",
              "merchant@wonderland",
              "merchant@wonderland",
              "merchant@wonderland#deposit",
              "merchant@wonderland#deposit",
              "usd#wonderland",
              1,
              "3.14",
              "3.14",
              null,
              null,
              transferJson);
      wallet.recordTransferAudit(item);
      final List<OfflineAuditEntry> entries = wallet.auditEntries();
      assert entries.size() == 1 : "expected single audit entry";
      final OfflineAuditEntry entry = entries.get(0);
      assert "bundle".equals(entry.txId()) : "tx id mismatch";
      assert "alice@wonderland".equals(entry.senderId()) : "sender mismatch";
      assert "merchant@wonderland".equals(entry.receiverId()) : "receiver mismatch";
      assert "usd#wonderland#merchant@wonderland".equals(entry.assetId()) : "asset mismatch";
      assert "3.14".equals(entry.amount()) : "amount mismatch";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static void circulationsModesFlipAndNotify() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_mode_test", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final AtomicReference<OfflineWallet.CirculationMode> observedMode =
          new AtomicReference<>();
      final AtomicReference<String> warning = new AtomicReference<>();
      final OfflineWallet wallet =
          new OfflineWallet(
              stubClient(),
              logFile,
              false,
              OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
              (mode, notice) -> {
                observedMode.set(mode);
                warning.set(notice.details());
              });
      assert wallet.circulationMode() == OfflineWallet.CirculationMode.LEDGER_RECONCILABLE;
      assert wallet.requiresLedgerReconciliation();
      wallet.setCirculationMode(OfflineWallet.CirculationMode.OFFLINE_ONLY);
      assert wallet.circulationMode() == OfflineWallet.CirculationMode.OFFLINE_ONLY;
      assert !wallet.requiresLedgerReconciliation();
      assert observedMode.get() == OfflineWallet.CirculationMode.OFFLINE_ONLY
          : "mode change listener missing";
      final String warningText = warning.get();
      assert warningText != null && warningText.contains("bearer")
          : "expected offline warning text";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static void verdictFreshnessGuards() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_verdict_test", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final OfflineAuditLogger auditLogger = new OfflineAuditLogger(logFile, true);
      final OfflineVerdictJournal journal = new OfflineVerdictJournal(verdictFile);
      final OfflineCounterJournal counterJournal = new OfflineCounterJournal(counterFile);
      final OfflineWallet wallet =
          new OfflineWallet(
              stubClient(),
              auditLogger,
              journal,
              counterJournal,
              OfflineWallet.Options.builder().build(),
              OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
              null);
      final OfflineAllowanceList.OfflineAllowanceItem allowance =
          new OfflineAllowanceList.OfflineAllowanceItem(
              "deadbeef",
              "alice@sora",
              "Alice",
              "xor#wonderland",
              0L,
              5_000L,
              4_000L,
              2_000L,
              "c0ffee",
              "abcd",
              "10",
              "{\"allowance\":\"10\"}");
      journal.upsert(List.of(allowance), Instant.ofEpochMilli(1_000));

      wallet.ensureFreshVerdict("deadbeef", "abcd", Instant.ofEpochMilli(1_500));

      boolean nonceMismatch = false;
      try {
        wallet.ensureFreshVerdict("deadbeef", "ffff", Instant.ofEpochMilli(1_500));
      } catch (OfflineVerdictException verdictError) {
        assert verdictError.reason() == OfflineVerdictException.Reason.NONCE_MISMATCH;
        nonceMismatch = true;
      }
      assert nonceMismatch : "expected nonce mismatch guard";

      boolean refreshExpired = false;
      try {
        wallet.ensureFreshVerdict("deadbeef", "abcd", Instant.ofEpochMilli(2_500));
      } catch (OfflineVerdictException verdictError) {
        assert verdictError.reason() == OfflineVerdictException.Reason.DEADLINE_EXPIRED;
        assert verdictError.deadlineKind() == OfflineVerdictWarning.DeadlineKind.REFRESH;
        refreshExpired = true;
      }
      assert refreshExpired : "expected refresh deadline rejection";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static void safetyDetectSnapshotsPersistAndExpire() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_safety_detect", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final OfflineAuditLogger auditLogger = new OfflineAuditLogger(logFile, true);
      final OfflineVerdictJournal journal = new OfflineVerdictJournal(verdictFile);
      final OfflineCounterJournal counterJournal = new OfflineCounterJournal(counterFile);
      final OfflineWallet.Options options =
          OfflineWallet.Options.builder()
              .safetyDetectEnabled(true)
              .safetyDetectService(
                  request ->
                      CompletableFuture.completedFuture(
                          new org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation(
                              "attestation-token", System.currentTimeMillis())))
              .build();
      final OfflineWallet wallet =
          new OfflineWallet(
              stubClient(),
              auditLogger,
              journal,
              counterJournal,
              options,
              OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
              null);
      journal.upsert(
          List.of(safetyDetectAllowance("deadbeef", 60_000L)), Instant.ofEpochMilli(100));
      wallet.fetchSafetyDetectAttestation("deadbeef").join();
      final var snapshot = wallet.buildSafetyDetectPlatformTokenSnapshot("deadbeef");
      assert snapshot.isPresent() : "expected Safety Detect snapshot";
      assert "hms_safety_detect".equals(snapshot.get().policy());
      final String expected =
          Base64.getEncoder().encodeToString("attestation-token".getBytes(java.nio.charset.StandardCharsets.UTF_8));
      assert expected.equals(snapshot.get().attestationJwsB64()) : "unexpected attestation encoding";

      final OfflineWallet.Options staleOptions =
          OfflineWallet.Options.builder()
              .safetyDetectEnabled(true)
              .safetyDetectService(
                  request ->
                      CompletableFuture.completedFuture(
                          new org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation(
                              "stale-token", 0L)))
              .build();
      final OfflineWallet staleWallet =
          new OfflineWallet(
              stubClient(),
              auditLogger,
              journal,
              counterJournal,
              staleOptions,
              OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
              null);
      journal.upsert(
          List.of(safetyDetectAllowance("deadbeef", 1L)), Instant.ofEpochMilli(200));
      staleWallet.fetchSafetyDetectAttestation("deadbeef").join();
      final var expired = staleWallet.buildSafetyDetectPlatformTokenSnapshot("deadbeef");
      assert expired.isEmpty() : "expected expired Safety Detect token to be rejected";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static void topUpRecordsVerdictMetadata() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_topup_test", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final SequencedExecutor executor =
          new SequencedExecutor(
              List.of(
                  new StubResponse(
                      200,
                      """
                      {
                        "certificate_id_hex": "deadbeef",
                        "certificate": {
                          "controller": "alice@wonderland",
                          "operator": "alice@wonderland",
                          "allowance": { "asset": "usd#wonderland", "amount": "10", "commitment": [1, 2] },
                          "spend_public_key": "ed0120deadbeef",
                          "attestation_report": [3, 4],
                          "issued_at_ms": 100,
                          "expires_at_ms": 200,
                          "policy": { "max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200 },
                          "operator_signature": "AA",
                          "metadata": {},
                          "verdict_id": null,
                          "attestation_nonce": null,
                          "refresh_at_ms": null
                        }
                      }
                      """),
                  new StubResponse(
                      200,
                      """
                      {
                        "certificate_id_hex": "deadbeef"
                      }
                      """)));
      final OfflineToriiClient client =
          OfflineToriiClient.builder()
              .executor(executor)
              .baseUri(URI.create("https://example.com"))
              .build();
      final OfflineWallet wallet = new OfflineWallet(client, logFile, false);
      final OfflineAllowanceCommitment allowance =
          new OfflineAllowanceCommitment("usd#wonderland", "10", new byte[] {1, 2});
      final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
      final OfflineWalletCertificateDraft draft =
          new OfflineWalletCertificateDraft(
              "alice@wonderland",
              allowance,
              "ed0120deadbeef",
              new byte[] {3, 4},
              100L,
              200L,
              policy,
              null,
              null,
              null,
              null);
      wallet.topUpAllowance(draft, "treasury@wonderland", "deadbeef").join();
      final Optional<OfflineVerdictMetadata> verdict = wallet.verdictMetadata("deadbeef");
      assert verdict.isPresent() : "expected cached verdict metadata";
      assert "10".equals(verdict.get().remainingAmount()) : "remaining amount mismatch";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static void exportsVerdictJournalJson() throws Exception {
    final Path logFile = Files.createTempFile("offline_wallet_verdict_export", ".json");
    final Path verdictFile = verdictJournalPath(logFile);
    final Path counterFile = counterJournalPath(logFile);
    try {
      final OfflineAuditLogger logger = new OfflineAuditLogger(logFile, false);
      final OfflineVerdictJournal verdictJournal = new OfflineVerdictJournal(verdictFile);
      final OfflineCounterJournal counterJournal = new OfflineCounterJournal(counterFile);
      final OfflineWallet wallet =
          new OfflineWallet(
              stubClient(),
              logger,
              verdictJournal,
              counterJournal,
              OfflineWallet.Options.builder().build(),
              OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
              null);

      final String recordJson =
          """
          {
            "policy": {"max_balance": "50.00"},
            "provisioned_metadata": {"region": "wonderland"}
          }
          """;
      final OfflineAllowanceList.OfflineAllowanceItem allowance =
          new OfflineAllowanceList.OfflineAllowanceItem(
              "deadbeef",
              "merchant@wonderland",
              "Merchant Wonderland",
              "usd#wonderland",
              Instant.parse("2025-01-01T00:00:00Z").toEpochMilli(),
              Instant.parse("2025-02-01T00:00:00Z").toEpochMilli(),
              Instant.parse("2025-01-15T00:00:00Z").toEpochMilli(),
              Instant.parse("2025-01-10T00:00:00Z").toEpochMilli(),
              "verdict-id-01",
              "nonce-01",
              "42.00",
              recordJson);
      verdictJournal.upsert(List.of(allowance), Instant.parse("2025-01-05T00:00:00Z"));

      final byte[] jsonBytes = wallet.exportVerdictJournalJson();
      assert jsonBytes.length > 0 : "export should not be empty";
      final String payload = new String(jsonBytes, StandardCharsets.UTF_8).trim();
      final Object parsed = JsonParser.parse(payload);
      assert parsed instanceof Map<?, ?> : "verdict journal export must be a JSON object";
      @SuppressWarnings("unchecked")
      final Map<String, Object> records = (Map<String, Object>) parsed;
      assert records.containsKey("deadbeef") : "missing certificate entry";
      @SuppressWarnings("unchecked")
      final Map<String, Object> entry = (Map<String, Object>) records.get("deadbeef");
      assert "merchant@wonderland".equals(entry.get("controller_id"))
          : "controller id missing from export";
      assert entry.get("policy_expires_at_ms") instanceof Number
          : "policy deadline missing";
      assert ((Number) entry.get("recorded_at_ms")).longValue()
          == Instant.parse("2025-01-05T00:00:00Z").toEpochMilli()
          : "recorded_at_ms mismatch";
    } finally {
      Files.deleteIfExists(logFile);
      Files.deleteIfExists(verdictFile);
      Files.deleteIfExists(counterFile);
    }
  }

  private static final class StubResponse {
    private final int status;
    private final byte[] body;

    private StubResponse(final int status, final String body) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
    }
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private final java.util.List<StubResponse> queue;

    private SequencedExecutor(final java.util.List<StubResponse> responses) {
      this.queue = new java.util.ArrayList<>(responses);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(
        final org.hyperledger.iroha.android.client.transport.TransportRequest request) {
      if (queue.isEmpty()) {
        final TransportResponse response =
            new TransportResponse(500, "{}".getBytes(StandardCharsets.UTF_8), "", java.util.Map.of());
        return CompletableFuture.completedFuture(response);
      }
      final StubResponse next = queue.remove(0);
      final TransportResponse response =
          new TransportResponse(next.status, next.body, "", java.util.Map.of());
      return CompletableFuture.completedFuture(response);
    }
  }

  private static OfflineAllowanceList.OfflineAllowanceItem safetyDetectAllowance(
      final String certificateId, final long maxTokenAgeMs) {
    final String recordJson =
        """
        {
          "certificate": {
            "metadata": {
              "android.integrity.policy": "hms_safety_detect",
              "android.hms_safety_detect.app_id": "merchant-app",
              "android.hms_safety_detect.package_names": ["com.example.app"],
              "android.hms_safety_detect.signing_digests_sha256": ["abcdef"],
              "android.hms_safety_detect.required_evaluations": ["basic"],
              "android.hms_safety_detect.max_token_age_ms": %d
            }
          }
        }
        """
            .formatted(maxTokenAgeMs);
    return new OfflineAllowanceList.OfflineAllowanceItem(
        certificateId,
        "alice@sora",
        "Alice",
        "usd#wonderland",
        0L,
        5_000L,
        4_000L,
        2_000L,
        "c0ffee",
        "nonce",
        "10",
        recordJson);
  }
}
