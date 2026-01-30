package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Instant;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.OfflineToriiClient;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityAttestation;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityRequest;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityService;
import org.hyperledger.iroha.android.offline.attestation.PlayIntegrityTelemetry;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectTelemetry;

/**
 * Convenience facade that combines the Torii offline client with the local audit logger so Android
 * apps can flip auditing per jurisdiction and stream bundle summaries deterministically.
 */
public final class OfflineWallet {

  /** Describes how an allowance is expected to re-enter the ledger. */
  public enum CirculationMode {
    LEDGER_RECONCILABLE(
        new CirculationNotice(
            "Ledger reconciliation required",
            "Offline receipts must be staged back to Torii within the policy window. "
                + "Keep audit logging enabled and plan controller treasury checks before circulating funds.")),
    OFFLINE_ONLY(
        new CirculationNotice(
            "Pure offline circulation",
            "Allowances stay off-ledger and behave like bearer instruments; receipts are final "
                + "between participants. Operators assume treasury liability and users must "
                + "acknowledge that recovery relies on their local journal and cannot be replayed "
                + "through Torii."));

    private final CirculationNotice notice;

    CirculationMode(final CirculationNotice notice) {
      this.notice = notice;
    }

    public CirculationNotice notice() {
      return notice;
    }

    public boolean requiresLedgerReconciliation() {
      return this == LEDGER_RECONCILABLE;
    }
  }

  /** Lightweight payload that SDKs can display when modes change. */
  public static final class CirculationNotice {
    private final String headline;
    private final String details;

    public CirculationNotice(final String headline, final String details) {
      this.headline = Objects.requireNonNull(headline, "headline");
      this.details = Objects.requireNonNull(details, "details");
    }

    public String headline() {
      return headline;
    }

    public String details() {
      return details;
    }
  }

  /** Listener used to surface warnings whenever the circulation mode flips. */
  public interface ModeChangeListener {
    void onModeChanged(CirculationMode mode, CirculationNotice notice);
  }

  /** Optional configuration for auxiliary integrations (Safety Detect, telemetry, etc.). */
  public static final class Options {
    private final boolean safetyDetectEnabled;
    private final SafetyDetectService safetyDetectService;
    private final boolean safetyDetectOptional;
    private final SafetyDetectTelemetry safetyDetectTelemetry;
    private final boolean playIntegrityEnabled;
    private final PlayIntegrityService playIntegrityService;
    private final boolean playIntegrityOptional;
    private final PlayIntegrityTelemetry playIntegrityTelemetry;

    private Options(final Builder builder) {
      this.safetyDetectEnabled = builder.safetyDetectEnabled;
      this.safetyDetectService =
          builder.safetyDetectEnabled
              ? Objects.requireNonNull(
                  builder.safetyDetectService, "Safety Detect service must be provided when enabled")
              : SafetyDetectService.disabled();
      this.safetyDetectOptional = builder.safetyDetectOptional;
      this.safetyDetectTelemetry =
          builder.safetyDetectTelemetry == null
              ? SafetyDetectTelemetry.NO_OP
              : builder.safetyDetectTelemetry;
      this.playIntegrityEnabled = builder.playIntegrityEnabled;
      this.playIntegrityService =
          builder.playIntegrityEnabled
              ? Objects.requireNonNull(
                  builder.playIntegrityService,
                  "Play Integrity service must be provided when enabled")
              : PlayIntegrityService.disabled();
      this.playIntegrityOptional = builder.playIntegrityOptional;
      this.playIntegrityTelemetry =
          builder.playIntegrityTelemetry == null
              ? PlayIntegrityTelemetry.NO_OP
              : builder.playIntegrityTelemetry;
    }

    public static Builder builder() {
      return new Builder();
    }

    public static final class Builder {
      private boolean safetyDetectEnabled;
      private boolean safetyDetectOptional;
      private SafetyDetectService safetyDetectService;
      private SafetyDetectTelemetry safetyDetectTelemetry;
      private boolean playIntegrityEnabled;
      private boolean playIntegrityOptional;
      private PlayIntegrityService playIntegrityService;
      private PlayIntegrityTelemetry playIntegrityTelemetry;

      private Builder() {}

      public Builder safetyDetectEnabled(final boolean enabled) {
        this.safetyDetectEnabled = enabled;
        return this;
      }

      public Builder safetyDetectOptional(final boolean optional) {
        this.safetyDetectOptional = optional;
        return this;
      }

      public Builder safetyDetectService(final SafetyDetectService safetyDetectService) {
        this.safetyDetectService = safetyDetectService;
        return this;
      }

      public Builder safetyDetectTelemetry(final SafetyDetectTelemetry safetyDetectTelemetry) {
        this.safetyDetectTelemetry = safetyDetectTelemetry;
        return this;
      }

      public Builder playIntegrityEnabled(final boolean enabled) {
        this.playIntegrityEnabled = enabled;
        return this;
      }

      public Builder playIntegrityOptional(final boolean optional) {
        this.playIntegrityOptional = optional;
        return this;
      }

      public Builder playIntegrityService(final PlayIntegrityService playIntegrityService) {
        this.playIntegrityService = playIntegrityService;
        return this;
      }

      public Builder playIntegrityTelemetry(final PlayIntegrityTelemetry playIntegrityTelemetry) {
        this.playIntegrityTelemetry = playIntegrityTelemetry;
        return this;
      }

      public Options build() {
        return new Options(this);
      }
    }
  }

  private final OfflineToriiClient toriiClient;
  private final OfflineAuditLogger auditLogger;
  private final OfflineVerdictJournal verdictJournal;
  private final OfflineCounterJournal counterJournal;
  private final SafetyDetectService safetyDetectService;
  private final boolean safetyDetectEnabled;
  private final boolean safetyDetectOptional;
  private final SafetyDetectTelemetry safetyDetectTelemetry;
  private final PlayIntegrityService playIntegrityService;
  private final boolean playIntegrityEnabled;
  private final boolean playIntegrityOptional;
  private final PlayIntegrityTelemetry playIntegrityTelemetry;
  private volatile CirculationMode circulationMode;
  private final ModeChangeListener modeChangeListener;

  public OfflineWallet(
      final OfflineToriiClient toriiClient,
      final Path auditLogPath,
      final boolean auditLoggingEnabled)
      throws IOException {
    this(
        toriiClient,
        new OfflineAuditLogger(auditLogPath, auditLoggingEnabled),
        new OfflineVerdictJournal(deriveVerdictJournalPath(auditLogPath)),
        new OfflineCounterJournal(deriveCounterJournalPath(auditLogPath)),
        Options.builder().build(),
        CirculationMode.LEDGER_RECONCILABLE,
        null);
  }

  public OfflineWallet(
      final OfflineToriiClient toriiClient,
      final Path auditLogPath,
      final boolean auditLoggingEnabled,
      final CirculationMode circulationMode,
      final ModeChangeListener modeChangeListener)
      throws IOException {
    this(
        toriiClient,
        new OfflineAuditLogger(auditLogPath, auditLoggingEnabled),
        new OfflineVerdictJournal(deriveVerdictJournalPath(auditLogPath)),
        new OfflineCounterJournal(deriveCounterJournalPath(auditLogPath)),
        Options.builder().build(),
        circulationMode,
        modeChangeListener);
  }

  public OfflineWallet(
      final OfflineToriiClient toriiClient, final OfflineAuditLogger auditLogger) {
    this(
        toriiClient,
        auditLogger,
        defaultVerdictJournal(auditLogger),
        defaultCounterJournal(auditLogger),
        Options.builder().build(),
        CirculationMode.LEDGER_RECONCILABLE,
        null);
  }

  public OfflineWallet(
      final OfflineToriiClient toriiClient,
      final OfflineAuditLogger auditLogger,
      final OfflineVerdictJournal verdictJournal,
      final OfflineCounterJournal counterJournal,
      final CirculationMode circulationMode,
      final ModeChangeListener modeChangeListener) {
    this(
        toriiClient,
        auditLogger,
        verdictJournal,
        counterJournal,
        Options.builder().build(),
        circulationMode,
        modeChangeListener);
  }

  public OfflineWallet(
      final OfflineToriiClient toriiClient,
      final OfflineAuditLogger auditLogger,
      final OfflineVerdictJournal verdictJournal,
      final OfflineCounterJournal counterJournal,
      final Options options,
      final CirculationMode circulationMode,
      final ModeChangeListener modeChangeListener) {
    this.toriiClient = Objects.requireNonNull(toriiClient, "toriiClient");
    this.auditLogger = Objects.requireNonNull(auditLogger, "auditLogger");
    this.verdictJournal = Objects.requireNonNull(verdictJournal, "verdictJournal");
    this.counterJournal = Objects.requireNonNull(counterJournal, "counterJournal");
    final Options resolved = options == null ? Options.builder().build() : options;
    this.safetyDetectEnabled = resolved.safetyDetectEnabled;
    this.safetyDetectService = resolved.safetyDetectService;
    this.safetyDetectOptional = resolved.safetyDetectOptional;
    this.safetyDetectTelemetry = resolved.safetyDetectTelemetry;
    this.playIntegrityEnabled = resolved.playIntegrityEnabled;
    this.playIntegrityService = resolved.playIntegrityService;
    this.playIntegrityOptional = resolved.playIntegrityOptional;
    this.playIntegrityTelemetry = resolved.playIntegrityTelemetry;
    this.circulationMode =
        circulationMode == null ? CirculationMode.LEDGER_RECONCILABLE : circulationMode;
    this.modeChangeListener = modeChangeListener;
    notifyModeChange();
  }

  public CompletableFuture<OfflineAllowanceList> fetchAllowances(
      final OfflineListParams params) {
    return toriiClient
        .listAllowances(params)
        .whenComplete(this::recordVerdictMetadata);
  }

  /**
   * Issues and registers an offline allowance. By default the issued certificate is recorded in
   * the verdict journal so refresh checks work without a follow-up list call.
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowance(
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex) {
    return topUpAllowance(draft, authority, privateKeyHex, true);
  }

  /**
   * Issues and registers an offline allowance, optionally recording verdict metadata locally.
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowance(
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex,
      final boolean recordVerdict) {
    return toriiClient
        .topUpAllowance(draft, authority, privateKeyHex)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null || response == null || !recordVerdict) {
                return;
              }
              try {
                recordVerdictMetadata(response.certificate());
              } catch (IOException e) {
                throw new CompletionException(e);
              }
            });
  }

  /**
   * Issues and registers a renewed offline allowance. By default the issued certificate is
   * recorded in the verdict journal so refresh checks work without a follow-up list call.
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowanceRenewal(
      final String certificateIdHex,
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex) {
    return topUpAllowanceRenewal(certificateIdHex, draft, authority, privateKeyHex, true);
  }

  /**
   * Issues and registers a renewed offline allowance, optionally recording verdict metadata
   * locally.
   */
  public CompletableFuture<OfflineTopUpResponse> topUpAllowanceRenewal(
      final String certificateIdHex,
      final OfflineWalletCertificateDraft draft,
      final String authority,
      final String privateKeyHex,
      final boolean recordVerdict) {
    return toriiClient
        .topUpAllowanceRenewal(certificateIdHex, draft, authority, privateKeyHex)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null || response == null || !recordVerdict) {
                return;
              }
              try {
                recordVerdictMetadata(response.certificate());
              } catch (IOException e) {
                throw new CompletionException(e);
              }
            });
  }

  public CompletableFuture<OfflineTransferList> fetchTransfers(
      final OfflineListParams params) {
    return toriiClient.listTransfers(params);
  }

  public CompletableFuture<OfflineSummaryList> fetchSummaries(
      final OfflineListParams params) {
    return toriiClient
        .listSummaries(params)
        .whenComplete(this::recordSummaryCounters);
  }

  public CompletableFuture<OfflineAllowanceList> fetchAllowances(
      final OfflineQueryEnvelope envelope) {
    return toriiClient
        .queryAllowances(envelope)
        .whenComplete(this::recordVerdictMetadata);
  }

  public CompletableFuture<OfflineTransferList> fetchTransfers(
      final OfflineQueryEnvelope envelope) {
    return toriiClient.queryTransfers(envelope);
  }

  public CompletableFuture<OfflineSummaryList> fetchSummaries(
      final OfflineQueryEnvelope envelope) {
    return toriiClient
        .querySummaries(envelope)
        .whenComplete(this::recordSummaryCounters);
  }

  public CompletableFuture<OfflineRevocationList> fetchRevocations(
      final OfflineListParams params) {
    return toriiClient.listRevocations(params);
  }

  public CompletableFuture<OfflineRevocationList> fetchRevocations(
      final OfflineQueryEnvelope envelope) {
    return toriiClient.queryRevocations(envelope);
  }

  /**
   * Fetches offline transfers and records every entry in the audit log (when enabled).
   *
   * <p>Useful when reconciling deposits: the returned list is identical to {@link
   * #fetchTransfers(OfflineListParams)}, but the audit logger captures each bundle for regulator
   * exports without additional bookkeeping.
   */
  public CompletableFuture<OfflineTransferList> fetchTransfersWithAudit(
      final OfflineListParams params) {
    return toriiClient
        .listTransfers(params)
        .whenComplete(
            (list, throwable) -> {
              if (throwable != null || list == null || !auditLogger.isEnabled()) {
                return;
              }
              for (OfflineTransferList.OfflineTransferItem item : list.items()) {
                try {
                  recordTransferAudit(item);
                } catch (IOException e) {
                  throw new CompletionException(e);
                }
              }
            });
  }

  public CompletableFuture<OfflineTransferList> fetchTransfersWithAudit(
      final OfflineQueryEnvelope envelope) {
    return toriiClient
        .queryTransfers(envelope)
        .whenComplete(
            (list, throwable) -> {
              if (throwable != null || list == null || !auditLogger.isEnabled()) {
                return;
              }
              for (OfflineTransferList.OfflineTransferItem item : list.items()) {
                try {
                  recordTransferAudit(item);
                } catch (IOException e) {
                  throw new CompletionException(e);
                }
              }
            });
  }

  public boolean isAuditLoggingEnabled() {
    return auditLogger.isEnabled();
  }

  public void setAuditLoggingEnabled(final boolean enabled) {
    auditLogger.setEnabled(enabled);
  }

  public List<OfflineVerdictWarning> verdictWarnings(final long warningThresholdMs) {
    return verdictJournal.warnings(warningThresholdMs, Instant.now());
  }

  public List<OfflineVerdictMetadata> verdictMetadata() {
    return verdictJournal.entries();
  }

  public Optional<OfflineVerdictMetadata> verdictMetadata(final String certificateIdHex) {
    return verdictJournal.find(certificateIdHex);
  }

  public OfflineVerdictMetadata recordVerdictMetadata(
      final OfflineCertificateIssueResponse response) throws IOException {
    return recordVerdictMetadata(response, Instant.now());
  }

  public OfflineVerdictMetadata recordVerdictMetadata(
      final OfflineCertificateIssueResponse response, final Instant recordedAt) throws IOException {
    Objects.requireNonNull(response, "response");
    return verdictJournal.upsert(response, recordedAt);
  }

  public List<OfflineCounterJournal.OfflineCounterCheckpoint> counterCheckpoints() {
    return counterJournal.entries();
  }

  public Optional<OfflineCounterJournal.OfflineCounterCheckpoint> counterCheckpoint(
      final String certificateIdHex) {
    return counterJournal.find(certificateIdHex);
  }

  public OfflineCounterJournal.OfflineCounterCheckpoint recordAppleCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String keyId,
      final long counter) throws IOException {
    return counterJournal.advanceAppleCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        keyId,
        counter,
        Instant.now());
  }

  public OfflineCounterJournal.OfflineCounterCheckpoint recordAndroidSeriesCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String series,
      final long counter) throws IOException {
    return counterJournal.advanceAndroidSeriesCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        series,
        counter,
        Instant.now());
  }

  public OfflineCounterJournal.OfflineCounterCheckpoint recordProvisionedCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final AndroidProvisionedProof proof) throws IOException {
    return counterJournal.advanceProvisionedCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        proof,
        Instant.now());
  }

  public OfflineVerdictMetadata ensureFreshVerdict(final String certificateIdHex) {
    return ensureFreshVerdict(certificateIdHex, null, Instant.now());
  }

  public OfflineVerdictMetadata ensureFreshVerdict(
      final String certificateIdHex, final String attestationNonceHex) {
    return ensureFreshVerdict(certificateIdHex, attestationNonceHex, Instant.now());
  }

  public OfflineVerdictMetadata ensureFreshVerdict(
      final OfflineAllowanceList.OfflineAllowanceItem allowance) {
    return ensureFreshVerdict(
        allowance.certificateIdHex(),
        allowance.attestationNonceHex(),
        Instant.now());
  }

  public Path verdictJournalPath() {
    return verdictJournal.journalFile();
  }

  public Path counterJournalPath() {
    return counterJournal.journalFile();
  }

  public Path auditLogFile() {
    return auditLogger.logFile();
  }

  public void recordAuditEntry(
      final String txId,
      final String senderId,
      final String receiverId,
      final String assetId,
      final String amount) throws IOException {
    final long timestampMs = Instant.now().toEpochMilli();
    auditLogger.record(
        new OfflineAuditEntry(
            txId,
            senderId,
            receiverId,
            assetId,
            amount,
            timestampMs));
  }

  public java.util.List<OfflineAuditEntry> auditEntries() {
    return auditLogger.entries();
  }

  public byte[] exportAuditJson() throws IOException {
    return auditLogger.exportJson();
  }

  /**
   * Exports the verdict metadata journal as canonical JSON, mirroring the audit log export. Attach
   * this payload to regulator packets that require per-certificate countdown evidence.
   */
  public byte[] exportVerdictJournalJson() throws IOException {
    return verdictJournal.exportJson();
  }

  /** Exports the counter journal as canonical JSON for audit trails. */
  public byte[] exportCounterJournalJson() throws IOException {
    return counterJournal.exportJson();
  }

  public void clearAuditLog() throws IOException {
    auditLogger.clear();
  }

  /**
   * Fetches a Safety Detect attestation token for the provided certificate id.
   *
   * <p>When the {@link Options} are configured with `safetyDetectOptional = true`, failures are
   * reported through telemetry and the returned future resolves to {@code null}. For
   * `safetyDetectOptional = false`, failures propagate to the caller.
   */
  public CompletableFuture<SafetyDetectAttestation> fetchSafetyDetectAttestation(
      final String certificateIdHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    if (!safetyDetectEnabled) {
      return failedFuture(new IllegalStateException("Safety Detect attestation is disabled"));
    }
    final OfflineVerdictMetadata metadata =
        verdictJournal
            .find(certificateIdHex)
            .orElseThrow(
                () ->
                    new IllegalArgumentException(
                        "No verdict metadata recorded for " + certificateIdHex));
    final String policy = metadata.integrityPolicy();
    if (policy == null || !policy.equals("hms_safety_detect")) {
      return failedFuture(
          new IllegalStateException(
              "Allowance "
                  + certificateIdHex
                  + " does not advertise the hms_safety_detect policy"));
    }
    final String nonceHex = metadata.attestationNonceHex();
    if (nonceHex == null || nonceHex.isBlank()) {
      return failedFuture(
          new IllegalStateException(
              "Allowance "
                  + certificateIdHex
                  + " does not advertise attestation_nonce metadata"));
    }
    final OfflineVerdictMetadata.SafetyDetectMetadata safetyDetect =
        metadata.hmsSafetyDetect();
    if (safetyDetect == null) {
      return failedFuture(
          new IllegalStateException(
              "Allowance " + certificateIdHex + " is missing HMS Safety Detect metadata"));
    }
    if (safetyDetect.packageNames().isEmpty()) {
      return failedFuture(
          new IllegalStateException("Safety Detect metadata missing package names"));
    }
    if (safetyDetect.signingDigestsSha256().isEmpty()) {
      return failedFuture(
          new IllegalStateException("Safety Detect metadata missing signing digests"));
    }
    if (safetyDetect.appId() == null || safetyDetect.appId().isBlank()) {
      return failedFuture(
          new IllegalStateException("Safety Detect metadata missing app id"));
    }
    final String packageName = safetyDetect.packageNames().get(0);
    final String signingDigest = safetyDetect.signingDigestsSha256().get(0);
    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex(metadata.certificateIdHex())
            .setAppId(safetyDetect.appId())
            .setNonceHex(nonceHex)
            .setPackageName(packageName)
            .setSigningDigestSha256(signingDigest)
            .setRequiredEvaluations(safetyDetect.requiredEvaluations())
            .setMaxTokenAgeMs(safetyDetect.maxTokenAgeMs())
            .setMetadata(safetyDetect)
            .build();
    safetyDetectTelemetry.onAttempt(request);
    final CompletableFuture<SafetyDetectAttestation> future = safetyDetectService.fetch(request);
    if (safetyDetectOptional) {
      return future
          .whenComplete((attestation, error) -> handleSafetyDetectCompletion(request, attestation, error))
          .exceptionally(error -> null);
    }
    return future.whenComplete(
        (attestation, error) -> handleSafetyDetectCompletion(request, attestation, error));
  }

  public Optional<OfflineVerdictMetadata.SafetyDetectTokenSnapshot> cachedSafetyDetectToken(
      final String certificateIdHex) {
    return verdictJournal.find(certificateIdHex).map(OfflineVerdictMetadata::hmsSafetyDetectToken);
  }

  /**
   * Fetches a Play Integrity attestation token for the provided certificate id.
   *
   * <p>When {@link Options#playIntegrityOptional} is true, failures are reported through telemetry
   * and the returned future resolves to {@code null}. Otherwise failures propagate to the caller.
   */
  public CompletableFuture<PlayIntegrityAttestation> fetchPlayIntegrityAttestation(
      final String certificateIdHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    if (!playIntegrityEnabled) {
      return failedFuture(new IllegalStateException("Play Integrity attestation is disabled"));
    }
    final OfflineVerdictMetadata metadata =
        verdictJournal
            .find(certificateIdHex)
            .orElseThrow(
                () ->
                    new IllegalArgumentException(
                        "No verdict metadata recorded for " + certificateIdHex));
    final String policy = metadata.integrityPolicy();
    if (policy == null || !policy.equals("play_integrity")) {
      return failedFuture(
          new IllegalStateException(
              "Allowance "
                  + certificateIdHex
                  + " does not advertise the play_integrity policy"));
    }
    final String nonceHex = metadata.attestationNonceHex();
    if (nonceHex == null || nonceHex.isBlank()) {
      return failedFuture(
          new IllegalStateException(
              "Allowance "
                  + certificateIdHex
                  + " does not advertise attestation_nonce metadata"));
    }
    final OfflineVerdictMetadata.PlayIntegrityMetadata playIntegrity = metadata.playIntegrity();
    if (playIntegrity == null) {
      return failedFuture(
          new IllegalStateException(
              "Allowance " + certificateIdHex + " is missing Play Integrity metadata"));
    }
    if (playIntegrity.packageNames().isEmpty()) {
      return failedFuture(
          new IllegalStateException("Play Integrity metadata missing package names"));
    }
    if (playIntegrity.signingDigestsSha256().isEmpty()) {
      return failedFuture(
          new IllegalStateException("Play Integrity metadata missing signing digests"));
    }
    if (playIntegrity.environment() == null || playIntegrity.environment().isBlank()) {
      return failedFuture(
          new IllegalStateException("Play Integrity metadata missing environment"));
    }
    if (playIntegrity.cloudProjectNumber() <= 0) {
      return failedFuture(
          new IllegalStateException("Play Integrity metadata missing cloud project number"));
    }
    final PlayIntegrityRequest request =
        PlayIntegrityRequest.builder()
            .setCertificateIdHex(metadata.certificateIdHex())
            .setNonceHex(nonceHex)
            .setCloudProjectNumber(playIntegrity.cloudProjectNumber())
            .setEnvironment(playIntegrity.environment())
            .setPackageName(playIntegrity.packageNames().get(0))
            .setSigningDigestSha256(playIntegrity.signingDigestsSha256().get(0))
            .setAllowedAppVerdicts(playIntegrity.allowedAppVerdicts())
            .setAllowedDeviceVerdicts(playIntegrity.allowedDeviceVerdicts())
            .setMaxTokenAgeMs(playIntegrity.maxTokenAgeMs())
            .setMetadata(playIntegrity)
            .build();
    playIntegrityTelemetry.onAttempt(request);
    final CompletableFuture<PlayIntegrityAttestation> future = playIntegrityService.fetch(request);
    if (playIntegrityOptional) {
      return future
          .whenComplete((attestation, error) -> handlePlayIntegrityCompletion(request, attestation, error))
          .exceptionally(error -> null);
    }
    return future.whenComplete(
        (attestation, error) -> handlePlayIntegrityCompletion(request, attestation, error));
  }

  public Optional<OfflineVerdictMetadata.PlayIntegrityTokenSnapshot> cachedPlayIntegrityToken(
      final String certificateIdHex) {
    return verdictJournal.find(certificateIdHex).map(OfflineVerdictMetadata::playIntegrityToken);
  }

  public CirculationMode circulationMode() {
    return circulationMode;
  }

  public CirculationNotice circulationModeNotice() {
    return circulationMode.notice();
  }

  public boolean requiresLedgerReconciliation() {
    return circulationMode.requiresLedgerReconciliation();
  }

  public void setCirculationMode(final CirculationMode mode) {
    final CirculationMode resolved = Objects.requireNonNull(mode, "mode");
    if (resolved == circulationMode) {
      return;
    }
    circulationMode = resolved;
    notifyModeChange();
  }

  private void notifyModeChange() {
    if (modeChangeListener != null) {
      modeChangeListener.onModeChanged(circulationMode, circulationMode.notice());
    }
  }

  public void recordTransferAudit(final OfflineTransferList.OfflineTransferItem transfer)
      throws IOException {
    if (transfer == null) {
      return;
    }
    final Optional<OfflineTransferList.OfflineTransferItem.ReceiptSummary> summary =
        transfer.firstReceiptSummary();
    final String senderId = summary.map(OfflineTransferList.OfflineTransferItem.ReceiptSummary::senderId)
        .orElse(transfer.receiverId());
    final String receiverId = summary.map(OfflineTransferList.OfflineTransferItem.ReceiptSummary::receiverId)
        .orElse(transfer.depositAccountId());
    final String assetId = summary
        .map(OfflineTransferList.OfflineTransferItem.ReceiptSummary::assetId)
        .filter(value -> value != null && !value.isBlank())
        .orElseGet(() -> {
          final String asset = transfer.assetId();
          return asset != null ? asset : "";
        });
    final String amount = summary
        .map(OfflineTransferList.OfflineTransferItem.ReceiptSummary::amount)
        .filter(value -> value != null && !value.isBlank())
        .orElseGet(() -> {
          final String claimedDelta = transfer.claimedDelta();
          if (claimedDelta != null && !claimedDelta.isBlank()) {
            return claimedDelta;
          }
          return transfer.totalAmount();
        });

    recordAuditEntry(
        transfer.bundleIdHex(),
        senderId,
        receiverId,
        assetId,
        amount);
  }

  /**
   * Returns an HMS Safety Detect snapshot when a valid attestation token is available on-device.
   *
   * <p>Call this helper after {@link #fetchSafetyDetectAttestation(String)} and embed the result in
   * a `SubmitOfflineToOnlineTransfer` bundle. The snapshot is only returned when the allowance uses
   * the {@code hms_safety_detect} policy and the cached token has not exceeded
   * {@code max_token_age_ms}.
   */
  public Optional<PlatformTokenSnapshot> buildSafetyDetectPlatformTokenSnapshot(
      final String certificateIdHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    if (!safetyDetectEnabled) {
      return Optional.empty();
    }
    final String normalized = normalizeHex(certificateIdHex);
    final Optional<OfflineVerdictMetadata> verdict = verdictJournal.find(normalized);
    if (verdict.isEmpty()) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata metadata = verdict.get();
    if (!"hms_safety_detect".equals(metadata.integrityPolicy())) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata.SafetyDetectMetadata hmsMetadata = metadata.hmsSafetyDetect();
    if (hmsMetadata == null) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata.SafetyDetectTokenSnapshot token = metadata.hmsSafetyDetectToken();
    if (token == null) {
      return Optional.empty();
    }
    final Long maxAgeMs = hmsMetadata.maxTokenAgeMs();
    if (maxAgeMs != null && maxAgeMs > 0) {
      final long expiresAt = token.fetchedAtMs() + maxAgeMs;
      if (expiresAt <= System.currentTimeMillis()) {
        return Optional.empty();
      }
    }
    final String encoded =
        Base64.getEncoder().encodeToString(token.token().getBytes(StandardCharsets.UTF_8));
    return Optional.of(new PlatformTokenSnapshot("hms_safety_detect", encoded));
  }

  /** Returns a Play Integrity snapshot when a valid attestation token is cached. */
  public Optional<PlatformTokenSnapshot> buildPlayIntegrityPlatformTokenSnapshot(
      final String certificateIdHex) {
    Objects.requireNonNull(certificateIdHex, "certificateIdHex");
    if (!playIntegrityEnabled) {
      return Optional.empty();
    }
    final String normalized = normalizeHex(certificateIdHex);
    final Optional<OfflineVerdictMetadata> verdict = verdictJournal.find(normalized);
    if (verdict.isEmpty()) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata metadata = verdict.get();
    if (!"play_integrity".equals(metadata.integrityPolicy())) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata.PlayIntegrityMetadata playIntegrity = metadata.playIntegrity();
    if (playIntegrity == null) {
      return Optional.empty();
    }
    final OfflineVerdictMetadata.PlayIntegrityTokenSnapshot token = metadata.playIntegrityToken();
    if (token == null) {
      return Optional.empty();
    }
    final Long maxAgeMs = playIntegrity.maxTokenAgeMs();
    if (maxAgeMs != null && maxAgeMs > 0) {
      final long expiresAt = token.fetchedAtMs() + maxAgeMs;
      if (expiresAt <= System.currentTimeMillis()) {
        return Optional.empty();
      }
    }
    final String encoded =
        Base64.getEncoder().encodeToString(token.token().getBytes(StandardCharsets.UTF_8));
    return Optional.of(new PlatformTokenSnapshot("play_integrity", encoded));
  }

  private void handleSafetyDetectCompletion(
      final SafetyDetectRequest request,
      final SafetyDetectAttestation attestation,
      final Throwable error) {
    if (error == null) {
      persistSafetyDetectToken(request, attestation);
      safetyDetectTelemetry.onSuccess(request, attestation);
    } else {
      safetyDetectTelemetry.onFailure(request, unwrap(error));
    }
  }

  private void persistSafetyDetectToken(
      final SafetyDetectRequest request, final SafetyDetectAttestation attestation) {
    if (attestation == null) {
      return;
    }
    final OfflineVerdictMetadata.SafetyDetectTokenSnapshot snapshot =
        new OfflineVerdictMetadata.SafetyDetectTokenSnapshot(
            attestation.token(), attestation.fetchedAtMs());
    try {
      verdictJournal.updateSafetyDetectToken(request.certificateIdHex(), snapshot);
    } catch (IOException e) {
      throw new CompletionException(e);
    }
  }

  private void handlePlayIntegrityCompletion(
      final PlayIntegrityRequest request,
      final PlayIntegrityAttestation attestation,
      final Throwable error) {
    if (error == null) {
      persistPlayIntegrityToken(request, attestation);
      playIntegrityTelemetry.onSuccess(request, attestation);
    } else {
      playIntegrityTelemetry.onFailure(request, unwrap(error));
    }
  }

  private void persistPlayIntegrityToken(
      final PlayIntegrityRequest request, final PlayIntegrityAttestation attestation) {
    if (attestation == null) {
      return;
    }
    final OfflineVerdictMetadata.PlayIntegrityTokenSnapshot snapshot =
        new OfflineVerdictMetadata.PlayIntegrityTokenSnapshot(
            attestation.token(), attestation.fetchedAtMs());
    try {
      verdictJournal.updatePlayIntegrityToken(request.certificateIdHex(), snapshot);
    } catch (IOException e) {
      throw new CompletionException(e);
    }
  }

  private void recordVerdictMetadata(
      final OfflineAllowanceList list, final Throwable throwable) {
    if (throwable != null || list == null || list.items().isEmpty()) {
      return;
    }
    try {
      verdictJournal.upsert(list.items(), Instant.now());
    } catch (IOException e) {
      throw new CompletionException(e);
    }
  }

  private void recordSummaryCounters(
      final OfflineSummaryList list, final Throwable throwable) {
    if (throwable != null || list == null || list.items().isEmpty()) {
      return;
    }
    try {
      counterJournal.upsert(list.items(), Instant.now());
    } catch (IOException e) {
      throw new CompletionException(e);
    }
  }

  private static Path deriveVerdictJournalPath(final Path auditLogPath) {
    if (auditLogPath == null) {
      return Path.of("offline_verdict_journal.json");
    }
    return auditLogPath.resolveSibling("offline_verdict_journal.json");
  }

  private static Path deriveCounterJournalPath(final Path auditLogPath) {
    if (auditLogPath == null) {
      return Path.of("offline_counter_journal.json");
    }
    return auditLogPath.resolveSibling("offline_counter_journal.json");
  }

  private static OfflineVerdictJournal defaultVerdictJournal(
      final OfflineAuditLogger logger) {
    try {
      return new OfflineVerdictJournal(deriveVerdictJournalPath(logger.logFile()));
    } catch (IOException e) {
      throw new IllegalStateException("Failed to initialize verdict journal", e);
    }
  }

  private static OfflineCounterJournal defaultCounterJournal(
      final OfflineAuditLogger logger) {
    try {
      return new OfflineCounterJournal(deriveCounterJournalPath(logger.logFile()));
    } catch (IOException e) {
      throw new IllegalStateException("Failed to initialize counter journal", e);
    }
  }

  OfflineVerdictMetadata ensureFreshVerdict(
      final String certificateIdHex,
      final String attestationNonceHex,
      final Instant now) {
    final String normalized = normalizeHex(certificateIdHex);
    final OfflineVerdictMetadata metadata =
        verdictJournal
            .find(normalized)
            .orElseThrow(
                () ->
                    new OfflineVerdictException(
                        OfflineVerdictException.Reason.METADATA_MISSING,
                        normalized,
                        null,
                        null,
                        null,
                        attestationNonceHex,
                        "Missing cached verdict metadata for " + normalized));
    validateNonce(metadata, attestationNonceHex, normalized);
    final long nowMs = (now == null ? Instant.now() : now).toEpochMilli();
    validateDeadlines(metadata, nowMs, normalized);
    return metadata;
  }

  private static void validateNonce(
      final OfflineVerdictMetadata metadata,
      final String providedNonceHex,
      final String certificateIdHex) {
    if (providedNonceHex == null || providedNonceHex.isBlank()) {
      return;
    }
    final String expected = metadata.attestationNonceHex();
    if (expected == null) {
      throw new OfflineVerdictException(
          OfflineVerdictException.Reason.NONCE_MISMATCH,
          certificateIdHex,
          null,
          null,
          null,
          providedNonceHex,
          "No attestation nonce recorded for " + certificateIdHex);
    }
    if (!expected.equalsIgnoreCase(providedNonceHex)) {
      throw new OfflineVerdictException(
          OfflineVerdictException.Reason.NONCE_MISMATCH,
          certificateIdHex,
          null,
          null,
          expected,
          providedNonceHex,
          "Attestation nonce mismatch for " + certificateIdHex);
    }
  }

  private static void validateDeadlines(
      final OfflineVerdictMetadata metadata,
      final long nowMs,
      final String certificateIdHex) {
    final Long refreshAt = metadata.refreshAtMs();
    if (refreshAt != null && refreshAt > 0 && nowMs >= refreshAt) {
      throw new OfflineVerdictException(
          OfflineVerdictException.Reason.DEADLINE_EXPIRED,
          certificateIdHex,
          OfflineVerdictWarning.DeadlineKind.REFRESH,
          refreshAt,
          metadata.attestationNonceHex(),
          null,
          "Cached verdict expired for " + certificateIdHex);
    }
    final long policyDeadline = metadata.policyExpiresAtMs();
    if (policyDeadline > 0 && nowMs >= policyDeadline) {
      throw new OfflineVerdictException(
          OfflineVerdictException.Reason.DEADLINE_EXPIRED,
          certificateIdHex,
          OfflineVerdictWarning.DeadlineKind.POLICY,
          policyDeadline,
          metadata.attestationNonceHex(),
          null,
          "Policy expired for " + certificateIdHex);
    }
    final long certificateDeadline = metadata.certificateExpiresAtMs();
    if (certificateDeadline > 0 && nowMs >= certificateDeadline) {
      throw new OfflineVerdictException(
          OfflineVerdictException.Reason.DEADLINE_EXPIRED,
          certificateIdHex,
          OfflineVerdictWarning.DeadlineKind.CERTIFICATE,
          certificateDeadline,
          metadata.attestationNonceHex(),
          null,
          "Allowance expired for " + certificateIdHex);
    }
  }

  /** Snapshot emitted when attaching platform tokens to bundle submissions. */
  public static final class PlatformTokenSnapshot {
    private final String policy;
    private final String attestationJwsB64;

    public PlatformTokenSnapshot(final String policy, final String attestationJwsB64) {
      this.policy = Objects.requireNonNull(policy, "policy");
      this.attestationJwsB64 =
          Objects.requireNonNull(attestationJwsB64, "attestationJwsB64");
    }

    public String policy() {
      return policy;
    }

    public String attestationJwsB64() {
      return attestationJwsB64;
    }

    /**
     * Returns a JSON-ready map representation of the snapshot ({@code policy}, {@code attestation_jws_b64}).
     */
    public Map<String, String> toJsonMap() {
      final Map<String, String> snapshot = new LinkedHashMap<>();
      snapshot.put("policy", policy);
      snapshot.put("attestation_jws_b64", attestationJwsB64);
      return snapshot;
    }
  }

  private static String normalizeHex(final String value) {
    if (value == null) {
      return "";
    }
    return value.trim().toLowerCase(Locale.ROOT);
  }

  private static <T> CompletableFuture<T> failedFuture(final Throwable error) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    future.completeExceptionally(error);
    return future;
  }

  private static Throwable unwrap(final Throwable error) {
    if (error instanceof CompletionException completion && completion.getCause() != null) {
      return completion.getCause();
    }
    return error;
  }
}
