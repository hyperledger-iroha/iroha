package org.hyperledger.iroha.sdk.offline

import java.io.IOException
import java.nio.file.Path
import java.time.Instant
import java.util.Base64
import java.util.Locale
import java.util.Optional
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.hyperledger.iroha.sdk.client.OfflineToriiClient
import org.hyperledger.iroha.sdk.offline.attestation.PlayIntegrityAttestation
import org.hyperledger.iroha.sdk.offline.attestation.PlayIntegrityRequest
import org.hyperledger.iroha.sdk.offline.attestation.PlayIntegrityService
import org.hyperledger.iroha.sdk.offline.attestation.PlayIntegrityTelemetry
import org.hyperledger.iroha.sdk.offline.attestation.SafetyDetectAttestation
import org.hyperledger.iroha.sdk.offline.attestation.SafetyDetectRequest
import org.hyperledger.iroha.sdk.offline.attestation.SafetyDetectService
import org.hyperledger.iroha.sdk.offline.attestation.SafetyDetectTelemetry

/**
 * Convenience facade that combines the Torii offline client with the local audit logger so Android
 * apps can flip auditing per jurisdiction and stream bundle summaries deterministically.
 */
class OfflineWallet : Any {

    /** Describes how an allowance is expected to re-enter the ledger. */
    enum class CirculationMode(val notice: CirculationNotice) {
        LEDGER_RECONCILABLE(
            CirculationNotice(
                "Ledger reconciliation required",
                "Offline receipts must be staged back to Torii within the policy window. " +
                    "Keep audit logging enabled and plan controller treasury checks before circulating funds."
            )
        ),
        OFFLINE_ONLY(
            CirculationNotice(
                "Pure offline circulation",
                "Allowances stay off-ledger and behave like bearer instruments; receipts are final " +
                    "between participants. Operators assume treasury liability and users must " +
                    "acknowledge that recovery relies on their local journal and cannot be replayed " +
                    "through Torii."
            )
        );

        fun requiresLedgerReconciliation(): Boolean = this == LEDGER_RECONCILABLE
    }

    /** Lightweight payload that SDKs can display when modes change. */
    class CirculationNotice(val headline: String, val details: String)

    /** Listener used to surface warnings whenever the circulation mode flips. */
    fun interface ModeChangeListener {
        fun onModeChanged(mode: CirculationMode, notice: CirculationNotice)
    }

    /** Optional configuration for auxiliary integrations (Safety Detect, telemetry, etc.). */
    class Options private constructor(
        val safetyDetectEnabled: Boolean,
        val safetyDetectService: SafetyDetectService,
        val safetyDetectOptional: Boolean,
        val safetyDetectTelemetry: SafetyDetectTelemetry,
        val playIntegrityEnabled: Boolean,
        val playIntegrityService: PlayIntegrityService,
        val playIntegrityOptional: Boolean,
        val playIntegrityTelemetry: PlayIntegrityTelemetry,
    ) {
        class Builder {
            var safetyDetectEnabled = false; private set
            var safetyDetectOptional = false; private set
            var safetyDetectService: SafetyDetectService? = null; private set
            var safetyDetectTelemetry: SafetyDetectTelemetry? = null; private set
            var playIntegrityEnabled = false; private set
            var playIntegrityOptional = false; private set
            var playIntegrityService: PlayIntegrityService? = null; private set
            var playIntegrityTelemetry: PlayIntegrityTelemetry? = null; private set

            fun safetyDetectEnabled(enabled: Boolean) = apply { safetyDetectEnabled = enabled }
            fun safetyDetectOptional(optional: Boolean) = apply { safetyDetectOptional = optional }
            fun safetyDetectService(service: SafetyDetectService) = apply { safetyDetectService = service }
            fun safetyDetectTelemetry(telemetry: SafetyDetectTelemetry) = apply { safetyDetectTelemetry = telemetry }
            fun playIntegrityEnabled(enabled: Boolean) = apply { playIntegrityEnabled = enabled }
            fun playIntegrityOptional(optional: Boolean) = apply { playIntegrityOptional = optional }
            fun playIntegrityService(service: PlayIntegrityService) = apply { playIntegrityService = service }
            fun playIntegrityTelemetry(telemetry: PlayIntegrityTelemetry) = apply { playIntegrityTelemetry = telemetry }

            fun build(): Options {
                val sdService = if (safetyDetectEnabled) {
                    requireNotNull(safetyDetectService) { "Safety Detect service must be provided when enabled" }
                } else SafetyDetectService.disabled()
                val sdTelemetry = safetyDetectTelemetry ?: SafetyDetectTelemetry.NO_OP
                val piService = if (playIntegrityEnabled) {
                    requireNotNull(playIntegrityService) { "Play Integrity service must be provided when enabled" }
                } else PlayIntegrityService.disabled()
                val piTelemetry = playIntegrityTelemetry ?: PlayIntegrityTelemetry.NO_OP
                return Options(
                    safetyDetectEnabled, sdService, safetyDetectOptional, sdTelemetry,
                    playIntegrityEnabled, piService, playIntegrityOptional, piTelemetry,
                )
            }
        }

        companion object {
            @JvmStatic
            fun builder() = Builder()
        }
    }

    private val toriiClient: OfflineToriiClient
    private val auditLogger: OfflineAuditLogger
    private val verdictJournal: OfflineVerdictJournal
    private val counterJournal: OfflineCounterJournal
    private val safetyDetectService: SafetyDetectService
    private val safetyDetectEnabled: Boolean
    private val safetyDetectOptional: Boolean
    private val safetyDetectTelemetry: SafetyDetectTelemetry
    private val playIntegrityService: PlayIntegrityService
    private val playIntegrityEnabled: Boolean
    private val playIntegrityOptional: Boolean
    private val playIntegrityTelemetry: PlayIntegrityTelemetry
    @Volatile var circulationMode: CirculationMode; private set
    private val modeChangeListener: ModeChangeListener?

    @Throws(IOException::class)
    constructor(
        toriiClient: OfflineToriiClient,
        auditLogPath: Path,
        auditLoggingEnabled: Boolean,
    ) : this(
        toriiClient,
        OfflineAuditLogger(auditLogPath, auditLoggingEnabled),
        OfflineVerdictJournal(deriveVerdictJournalPath(auditLogPath)),
        OfflineCounterJournal(deriveCounterJournalPath(auditLogPath)),
        Options.builder().build(),
        CirculationMode.LEDGER_RECONCILABLE,
        null,
    )

    @Throws(IOException::class)
    constructor(
        toriiClient: OfflineToriiClient,
        auditLogPath: Path,
        auditLoggingEnabled: Boolean,
        circulationMode: CirculationMode,
        modeChangeListener: ModeChangeListener?,
    ) : this(
        toriiClient,
        OfflineAuditLogger(auditLogPath, auditLoggingEnabled),
        OfflineVerdictJournal(deriveVerdictJournalPath(auditLogPath)),
        OfflineCounterJournal(deriveCounterJournalPath(auditLogPath)),
        Options.builder().build(),
        circulationMode,
        modeChangeListener,
    )

    constructor(
        toriiClient: OfflineToriiClient,
        auditLogger: OfflineAuditLogger,
    ) : this(
        toriiClient, auditLogger, defaultVerdictJournal(auditLogger),
        defaultCounterJournal(auditLogger), Options.builder().build(),
        CirculationMode.LEDGER_RECONCILABLE, null,
    )

    constructor(
        toriiClient: OfflineToriiClient,
        auditLogger: OfflineAuditLogger,
        verdictJournal: OfflineVerdictJournal,
        counterJournal: OfflineCounterJournal,
        circulationMode: CirculationMode,
        modeChangeListener: ModeChangeListener?,
    ) : this(
        toriiClient, auditLogger, verdictJournal, counterJournal,
        Options.builder().build(), circulationMode, modeChangeListener,
    )

    constructor(
        toriiClient: OfflineToriiClient,
        auditLogger: OfflineAuditLogger,
        verdictJournal: OfflineVerdictJournal,
        counterJournal: OfflineCounterJournal,
        options: Options?,
        circulationMode: CirculationMode?,
        modeChangeListener: ModeChangeListener?,
    ) {
        this.toriiClient = toriiClient
        this.auditLogger = auditLogger
        this.verdictJournal = verdictJournal
        this.counterJournal = counterJournal
        val resolved = options ?: Options.builder().build()
        this.safetyDetectEnabled = resolved.safetyDetectEnabled
        this.safetyDetectService = resolved.safetyDetectService
        this.safetyDetectOptional = resolved.safetyDetectOptional
        this.safetyDetectTelemetry = resolved.safetyDetectTelemetry
        this.playIntegrityEnabled = resolved.playIntegrityEnabled
        this.playIntegrityService = resolved.playIntegrityService
        this.playIntegrityOptional = resolved.playIntegrityOptional
        this.playIntegrityTelemetry = resolved.playIntegrityTelemetry
        this.circulationMode = circulationMode ?: CirculationMode.LEDGER_RECONCILABLE
        this.modeChangeListener = modeChangeListener
        notifyModeChange()
    }

    fun fetchTransfers(params: OfflineListParams): CompletableFuture<OfflineTransferList> =
        toriiClient.listTransfers(params)

    fun fetchTransfers(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineTransferList> =
        toriiClient.queryTransfers(envelope)

    fun fetchRevocations(params: OfflineListParams): CompletableFuture<OfflineRevocationList> =
        toriiClient.listRevocations(params)

    fun fetchTransfersWithAudit(params: OfflineListParams): CompletableFuture<OfflineTransferList> =
        toriiClient.listTransfers(params).whenComplete { list, throwable ->
            if (throwable != null || list == null || !auditLogger.isEnabled) return@whenComplete
            for (item in list.items) {
                try { recordTransferAudit(item) } catch (e: IOException) { throw CompletionException(e) }
            }
        }

    fun fetchTransfersWithAudit(envelope: OfflineQueryEnvelope): CompletableFuture<OfflineTransferList> =
        toriiClient.queryTransfers(envelope).whenComplete { list, throwable ->
            if (throwable != null || list == null || !auditLogger.isEnabled) return@whenComplete
            for (item in list.items) {
                try { recordTransferAudit(item) } catch (e: IOException) { throw CompletionException(e) }
            }
        }

    var isAuditLoggingEnabled: Boolean
        get() = auditLogger.isEnabled
        set(value) { auditLogger.isEnabled = value }

    fun verdictWarnings(warningThresholdMs: Long): List<OfflineVerdictWarning> =
        verdictJournal.warnings(warningThresholdMs, Instant.now())

    fun verdictMetadata(): List<OfflineVerdictMetadata> = verdictJournal.entries()

    fun verdictMetadata(certificateIdHex: String): Optional<OfflineVerdictMetadata> =
        verdictJournal.find(certificateIdHex)

    fun counterCheckpoints(): List<OfflineCounterJournal.OfflineCounterCheckpoint> =
        counterJournal.entries()

    fun counterCheckpoint(certificateIdHex: String): Optional<OfflineCounterJournal.OfflineCounterCheckpoint> =
        Optional.ofNullable(counterJournal.find(certificateIdHex))

    @Throws(IOException::class)
    fun recordAppleCounter(
        certificateIdHex: String, controllerId: String, controllerDisplay: String,
        keyId: String, counter: Long,
    ): OfflineCounterJournal.OfflineCounterCheckpoint =
        counterJournal.advanceAppleCounter(
            certificateIdHex, controllerId, controllerDisplay, keyId, counter, Instant.now())

    @Throws(IOException::class)
    fun recordAndroidSeriesCounter(
        certificateIdHex: String, controllerId: String, controllerDisplay: String,
        series: String, counter: Long,
    ): OfflineCounterJournal.OfflineCounterCheckpoint =
        counterJournal.advanceAndroidSeriesCounter(
            certificateIdHex, controllerId, controllerDisplay, series, counter, Instant.now())

    @Throws(IOException::class)
    fun recordProvisionedCounter(
        certificateIdHex: String, controllerId: String, controllerDisplay: String,
        proof: AndroidProvisionedProof,
    ): OfflineCounterJournal.OfflineCounterCheckpoint =
        counterJournal.advanceProvisionedCounter(
            certificateIdHex, controllerId, controllerDisplay, proof, Instant.now())

    fun ensureFreshVerdict(certificateIdHex: String): OfflineVerdictMetadata =
        ensureFreshVerdict(certificateIdHex, null, Instant.now())

    fun ensureFreshVerdict(certificateIdHex: String, attestationNonceHex: String?): OfflineVerdictMetadata =
        ensureFreshVerdict(certificateIdHex, attestationNonceHex, Instant.now())

    fun ensureFreshVerdict(allowance: OfflineAllowanceItem): OfflineVerdictMetadata =
        ensureFreshVerdict(allowance.certificateIdHex, allowance.attestationNonceHex, Instant.now())

    fun verdictJournalPath(): Path = verdictJournal.journalFile
    fun counterJournalPath(): Path = counterJournal.journalFile
    fun auditLogFile(): Path = auditLogger.logFile

    @Throws(IOException::class)
    fun recordAuditEntry(
        txId: String, senderId: String, receiverId: String, assetId: String, amount: String,
    ) {
        val timestampMs = Instant.now().toEpochMilli()
        auditLogger.record(OfflineAuditEntry(txId, senderId, receiverId, assetId, amount, timestampMs))
    }

    fun auditEntries(): List<OfflineAuditEntry> = auditLogger.entries()

    @Throws(IOException::class)
    fun exportAuditJson(): ByteArray = auditLogger.exportJson()

    @Throws(IOException::class)
    fun exportVerdictJournalJson(): ByteArray = verdictJournal.exportJson()

    @Throws(IOException::class)
    fun exportCounterJournalJson(): ByteArray = counterJournal.exportJson()

    @Throws(IOException::class)
    fun clearAuditLog() = auditLogger.clear()

    fun fetchSafetyDetectAttestation(certificateIdHex: String): CompletableFuture<SafetyDetectAttestation> {
        if (!safetyDetectEnabled) return failedFuture(IllegalStateException("Safety Detect attestation is disabled"))
        val metadata = verdictJournal.find(certificateIdHex).orElseThrow {
            IllegalArgumentException("No verdict metadata recorded for $certificateIdHex")
        }
        val policy = metadata.integrityPolicy
        if (policy == null || policy != "hms_safety_detect") {
            return failedFuture(IllegalStateException("Allowance $certificateIdHex does not advertise the hms_safety_detect policy"))
        }
        val nonceHex = metadata.attestationNonceHex
        if (nonceHex.isNullOrBlank()) {
            return failedFuture(IllegalStateException("Allowance $certificateIdHex does not advertise attestation_nonce metadata"))
        }
        val safetyDetect = metadata.hmsSafetyDetect
            ?: return failedFuture(IllegalStateException("Allowance $certificateIdHex is missing HMS Safety Detect metadata"))
        if (safetyDetect.packageNames.isEmpty()) return failedFuture(IllegalStateException("Safety Detect metadata missing package names"))
        if (safetyDetect.signingDigestsSha256.isEmpty()) return failedFuture(IllegalStateException("Safety Detect metadata missing signing digests"))
        if (safetyDetect.appId.isNullOrBlank()) return failedFuture(IllegalStateException("Safety Detect metadata missing app id"))
        val request = SafetyDetectRequest.builder()
            .setCertificateIdHex(metadata.certificateIdHex)
            .setAppId(safetyDetect.appId)
            .setNonceHex(nonceHex)
            .setPackageName(safetyDetect.packageNames[0])
            .setSigningDigestSha256(safetyDetect.signingDigestsSha256[0])
            .setRequiredEvaluations(safetyDetect.requiredEvaluations)
            .setMaxTokenAgeMs(safetyDetect.maxTokenAgeMs)
            .setMetadata(safetyDetect)
            .build()
        safetyDetectTelemetry.onAttempt(request)
        val future = safetyDetectService.fetch(request)
        return if (safetyDetectOptional) {
            future.whenComplete { attestation, error -> handleSafetyDetectCompletion(request, attestation, error) }
                .exceptionally { null }
        } else {
            future.whenComplete { attestation, error -> handleSafetyDetectCompletion(request, attestation, error) }
        }
    }

    fun cachedSafetyDetectToken(certificateIdHex: String): Optional<OfflineVerdictMetadata.SafetyDetectTokenSnapshot> =
        verdictJournal.find(certificateIdHex).map { it.hmsSafetyDetectToken }

    fun fetchPlayIntegrityAttestation(certificateIdHex: String): CompletableFuture<PlayIntegrityAttestation> {
        if (!playIntegrityEnabled) return failedFuture(IllegalStateException("Play Integrity attestation is disabled"))
        val metadata = verdictJournal.find(certificateIdHex).orElseThrow {
            IllegalArgumentException("No verdict metadata recorded for $certificateIdHex")
        }
        val policy = metadata.integrityPolicy
        if (policy == null || policy != "play_integrity") {
            return failedFuture(IllegalStateException("Allowance $certificateIdHex does not advertise the play_integrity policy"))
        }
        val nonceHex = metadata.attestationNonceHex
        if (nonceHex.isNullOrBlank()) {
            return failedFuture(IllegalStateException("Allowance $certificateIdHex does not advertise attestation_nonce metadata"))
        }
        val playIntegrity = metadata.playIntegrity
            ?: return failedFuture(IllegalStateException("Allowance $certificateIdHex is missing Play Integrity metadata"))
        if (playIntegrity.packageNames.isEmpty()) return failedFuture(IllegalStateException("Play Integrity metadata missing package names"))
        if (playIntegrity.signingDigestsSha256.isEmpty()) return failedFuture(IllegalStateException("Play Integrity metadata missing signing digests"))
        if (playIntegrity.environment.isNullOrBlank()) return failedFuture(IllegalStateException("Play Integrity metadata missing environment"))
        if (playIntegrity.cloudProjectNumber <= 0) return failedFuture(IllegalStateException("Play Integrity metadata missing cloud project number"))
        val request = PlayIntegrityRequest.builder()
            .setCertificateIdHex(metadata.certificateIdHex)
            .setNonceHex(nonceHex)
            .setCloudProjectNumber(playIntegrity.cloudProjectNumber)
            .setEnvironment(playIntegrity.environment)
            .setPackageName(playIntegrity.packageNames[0])
            .setSigningDigestSha256(playIntegrity.signingDigestsSha256[0])
            .setAllowedAppVerdicts(playIntegrity.allowedAppVerdicts)
            .setAllowedDeviceVerdicts(playIntegrity.allowedDeviceVerdicts)
            .setMaxTokenAgeMs(playIntegrity.maxTokenAgeMs)
            .setMetadata(playIntegrity)
            .build()
        playIntegrityTelemetry.onAttempt(request)
        val future = playIntegrityService.fetch(request)
        return if (playIntegrityOptional) {
            future.whenComplete { attestation, error -> handlePlayIntegrityCompletion(request, attestation, error) }
                .exceptionally { null }
        } else {
            future.whenComplete { attestation, error -> handlePlayIntegrityCompletion(request, attestation, error) }
        }
    }

    fun cachedPlayIntegrityToken(certificateIdHex: String): Optional<OfflineVerdictMetadata.PlayIntegrityTokenSnapshot> =
        verdictJournal.find(certificateIdHex).map { it.playIntegrityToken }

    fun circulationModeNotice(): CirculationNotice = circulationMode.notice

    fun requiresLedgerReconciliation(): Boolean = circulationMode.requiresLedgerReconciliation()

    fun setCirculationMode(mode: CirculationMode) {
        if (mode == circulationMode) return
        circulationMode = mode
        notifyModeChange()
    }

    private fun notifyModeChange() {
        modeChangeListener?.onModeChanged(circulationMode, circulationMode.notice)
    }

    @Throws(IOException::class)
    fun recordTransferAudit(transfer: OfflineTransferList.OfflineTransferItem?) {
        if (transfer == null) return
        val summary = transfer.firstReceiptSummary()
        val senderId = summary.map { it.senderId }.orElse(transfer.receiverId)
        val receiverId = summary.map { it.receiverId }.orElse(transfer.depositAccountId)
        val assetId = summary
            .map { it.assetId }
            .filter { !it.isNullOrBlank() }
            .orElseGet { transfer.assetId ?: "" }
        val amount = summary
            .map { it.amount }
            .filter { !it.isNullOrBlank() }
            .orElseGet {
                val claimedDelta = transfer.claimedDelta
                if (!claimedDelta.isNullOrBlank()) claimedDelta else transfer.totalAmount
            }
        recordAuditEntry(transfer.bundleIdHex, senderId ?: "", receiverId ?: "", assetId ?: "", amount ?: "")
    }

    fun buildSafetyDetectPlatformTokenSnapshot(certificateIdHex: String): Optional<PlatformTokenSnapshot> {
        if (!safetyDetectEnabled) return Optional.empty()
        val normalized = normalizeHex(certificateIdHex)
        val verdict = verdictJournal.find(normalized)
        if (verdict.isEmpty) return Optional.empty()
        val metadata = verdict.get()
        if ("hms_safety_detect" != metadata.integrityPolicy) return Optional.empty()
        val hmsMetadata = metadata.hmsSafetyDetect ?: return Optional.empty()
        val token = metadata.hmsSafetyDetectToken ?: return Optional.empty()
        val maxAgeMs = hmsMetadata.maxTokenAgeMs
        if (maxAgeMs != null && maxAgeMs > 0) {
            val expiresAt = token.fetchedAtMs + maxAgeMs
            if (expiresAt <= System.currentTimeMillis()) return Optional.empty()
        }
        val encoded = Base64.getEncoder().encodeToString(token.token.toByteArray(Charsets.UTF_8))
        return Optional.of(PlatformTokenSnapshot("hms_safety_detect", encoded))
    }

    fun buildPlayIntegrityPlatformTokenSnapshot(certificateIdHex: String): Optional<PlatformTokenSnapshot> {
        if (!playIntegrityEnabled) return Optional.empty()
        val normalized = normalizeHex(certificateIdHex)
        val verdict = verdictJournal.find(normalized)
        if (verdict.isEmpty) return Optional.empty()
        val metadata = verdict.get()
        if ("play_integrity" != metadata.integrityPolicy) return Optional.empty()
        val playIntegrity = metadata.playIntegrity ?: return Optional.empty()
        val token = metadata.playIntegrityToken ?: return Optional.empty()
        val maxAgeMs = playIntegrity.maxTokenAgeMs
        if (maxAgeMs != null && maxAgeMs > 0) {
            val expiresAt = token.fetchedAtMs + maxAgeMs
            if (expiresAt <= System.currentTimeMillis()) return Optional.empty()
        }
        val encoded = Base64.getEncoder().encodeToString(token.token.toByteArray(Charsets.UTF_8))
        return Optional.of(PlatformTokenSnapshot("play_integrity", encoded))
    }

    private fun handleSafetyDetectCompletion(
        request: SafetyDetectRequest, attestation: SafetyDetectAttestation?, error: Throwable?,
    ) {
        if (error == null) {
            persistSafetyDetectToken(request, attestation)
            safetyDetectTelemetry.onSuccess(request, attestation)
        } else {
            safetyDetectTelemetry.onFailure(request, unwrap(error))
        }
    }

    private fun persistSafetyDetectToken(request: SafetyDetectRequest, attestation: SafetyDetectAttestation?) {
        if (attestation == null) return
        val snapshot = OfflineVerdictMetadata.SafetyDetectTokenSnapshot(attestation.token, attestation.fetchedAtMs)
        try { verdictJournal.updateSafetyDetectToken(request.certificateIdHex, snapshot) }
        catch (e: IOException) { throw CompletionException(e) }
    }

    private fun handlePlayIntegrityCompletion(
        request: PlayIntegrityRequest, attestation: PlayIntegrityAttestation?, error: Throwable?,
    ) {
        if (error == null) {
            persistPlayIntegrityToken(request, attestation)
            playIntegrityTelemetry.onSuccess(request, attestation)
        } else {
            playIntegrityTelemetry.onFailure(request, unwrap(error))
        }
    }

    private fun persistPlayIntegrityToken(request: PlayIntegrityRequest, attestation: PlayIntegrityAttestation?) {
        if (attestation == null) return
        val snapshot = OfflineVerdictMetadata.PlayIntegrityTokenSnapshot(attestation.token, attestation.fetchedAtMs)
        try { verdictJournal.updatePlayIntegrityToken(request.certificateIdHex, snapshot) }
        catch (e: IOException) { throw CompletionException(e) }
    }

    internal fun ensureFreshVerdict(
        certificateIdHex: String, attestationNonceHex: String?, now: Instant,
    ): OfflineVerdictMetadata {
        val normalized = normalizeHex(certificateIdHex)
        val metadata = verdictJournal.find(normalized).orElseThrow {
            OfflineVerdictException(
                OfflineVerdictException.Reason.METADATA_MISSING, normalized, null, null,
                null, attestationNonceHex, "Missing cached verdict metadata for $normalized")
        }
        validateNonce(metadata, attestationNonceHex, normalized)
        val nowMs = now.toEpochMilli()
        validateDeadlines(metadata, nowMs, normalized)
        return metadata
    }

    /** Snapshot emitted when attaching platform tokens to bundle submissions. */
    class PlatformTokenSnapshot(val policy: String, val attestationJwsB64: String) {
        fun toJsonMap(): Map<String, String> {
            val snapshot = LinkedHashMap<String, String>()
            snapshot["policy"] = policy
            snapshot["attestation_jws_b64"] = attestationJwsB64
            return snapshot
        }
    }

    companion object {
        private fun deriveVerdictJournalPath(auditLogPath: Path?): Path =
            auditLogPath?.resolveSibling("offline_verdict_journal.json")
                ?: Path.of("offline_verdict_journal.json")

        private fun deriveCounterJournalPath(auditLogPath: Path?): Path =
            auditLogPath?.resolveSibling("offline_counter_journal.json")
                ?: Path.of("offline_counter_journal.json")

        private fun defaultVerdictJournal(logger: OfflineAuditLogger): OfflineVerdictJournal =
            try { OfflineVerdictJournal(deriveVerdictJournalPath(logger.logFile)) }
            catch (e: IOException) { throw IllegalStateException("Failed to initialize verdict journal", e) }

        private fun defaultCounterJournal(logger: OfflineAuditLogger): OfflineCounterJournal =
            try { OfflineCounterJournal(deriveCounterJournalPath(logger.logFile)) }
            catch (e: IOException) { throw IllegalStateException("Failed to initialize counter journal", e) }

        private fun validateNonce(
            metadata: OfflineVerdictMetadata, providedNonceHex: String?, certificateIdHex: String,
        ) {
            if (providedNonceHex.isNullOrBlank()) return
            val expected = metadata.attestationNonceHex
                ?: throw OfflineVerdictException(
                    OfflineVerdictException.Reason.NONCE_MISMATCH, certificateIdHex, null, null,
                    null, providedNonceHex, "No attestation nonce recorded for $certificateIdHex")
            if (!expected.equals(providedNonceHex, ignoreCase = true)) {
                throw OfflineVerdictException(
                    OfflineVerdictException.Reason.NONCE_MISMATCH, certificateIdHex, null, null,
                    expected, providedNonceHex, "Attestation nonce mismatch for $certificateIdHex")
            }
        }

        private fun validateDeadlines(
            metadata: OfflineVerdictMetadata, nowMs: Long, certificateIdHex: String,
        ) {
            val refreshAt = metadata.refreshAtMs
            if (refreshAt != null && refreshAt > 0 && nowMs >= refreshAt) {
                throw OfflineVerdictException(
                    OfflineVerdictException.Reason.DEADLINE_EXPIRED, certificateIdHex,
                    OfflineVerdictWarning.DeadlineKind.REFRESH, refreshAt,
                    metadata.attestationNonceHex, null, "Cached verdict expired for $certificateIdHex")
            }
            val policyDeadline = metadata.policyExpiresAtMs
            if (policyDeadline > 0 && nowMs >= policyDeadline) {
                throw OfflineVerdictException(
                    OfflineVerdictException.Reason.DEADLINE_EXPIRED, certificateIdHex,
                    OfflineVerdictWarning.DeadlineKind.POLICY, policyDeadline,
                    metadata.attestationNonceHex, null, "Policy expired for $certificateIdHex")
            }
            val certificateDeadline = metadata.certificateExpiresAtMs
            if (certificateDeadline > 0 && nowMs >= certificateDeadline) {
                throw OfflineVerdictException(
                    OfflineVerdictException.Reason.DEADLINE_EXPIRED, certificateIdHex,
                    OfflineVerdictWarning.DeadlineKind.CERTIFICATE, certificateDeadline,
                    metadata.attestationNonceHex, null, "Allowance expired for $certificateIdHex")
            }
        }

        private fun normalizeHex(value: String?): String =
            value?.trim()?.lowercase(Locale.ROOT) ?: ""

        private fun <T> failedFuture(error: Throwable): CompletableFuture<T> {
            val future = CompletableFuture<T>()
            future.completeExceptionally(error)
            return future
        }

        private fun unwrap(error: Throwable): Throwable =
            if (error is CompletionException && error.cause != null) error.cause!! else error
    }
}
