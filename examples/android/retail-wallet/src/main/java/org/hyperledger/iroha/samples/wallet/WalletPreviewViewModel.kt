package org.hyperledger.iroha.samples.wallet

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import java.io.File
import java.io.IOException
import java.net.URI
import java.time.Duration
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Locale
import java.util.concurrent.CompletableFuture
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import org.hyperledger.iroha.android.IrohaKeyManager
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference
import org.hyperledger.iroha.android.KeyManagementException
import org.hyperledger.iroha.android.SigningException
import org.hyperledger.iroha.android.address.AccountAddress
import org.hyperledger.iroha.android.address.AccountAddressException
import org.hyperledger.iroha.android.client.OfflineToriiClient
import org.hyperledger.iroha.android.norito.NoritoException
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.android.offline.OfflineAuditLogger
import org.hyperledger.iroha.android.offline.OfflineListParams
import org.hyperledger.iroha.android.offline.OfflineRevocationList
import org.hyperledger.iroha.android.offline.OfflineToriiException
import org.hyperledger.iroha.android.offline.OfflineWallet
import org.hyperledger.iroha.android.offline.OfflineVerdictException
import org.hyperledger.iroha.android.offline.OfflineVerdictJournal
import org.hyperledger.iroha.android.offline.OfflineVerdictMetadata
import org.hyperledger.iroha.android.offline.OfflineVerdictWarning
import org.hyperledger.iroha.android.model.TransactionPayload
import org.hyperledger.iroha.android.tx.SignedTransactionHasher
import org.hyperledger.iroha.android.tx.TransactionBuilder
import org.hyperledger.iroha.android.tx.offline.OfflineEnvelopeOptions

class WalletPreviewViewModel(application: Application) : AndroidViewModel(application) {

    private val keyManager = IrohaKeyManager.withDefaultProviders()
    private val builder = TransactionBuilder(NoritoJavaCodecAdapter(), keyManager)
    private val assetExecutor = AssetHttpExecutor(
        application,
        mapOf("/v1/offline/revocations" to AssetHttpExecutor.Route("offline_revocations.json"))
    )
    private val offlineClient: OfflineToriiClient by lazy {
        OfflineToriiClient.builder()
            .executor(assetExecutor)
            .baseUri(URI.create(BuildConfig.TORII_ENDPOINT))
            .build()
    }
    private val appContext = application.applicationContext
    private val policyOverrideStore = PolicyOverrideStore(appContext)
    @Volatile private var securityPolicy: SecurityPolicy =
        SecurityPolicyLoader.load(appContext, policyOverrideStore.current())
    private val auditLogger = PosAuditLogger(appContext)
    private val verdictJournalFile = File(appContext.filesDir, "offline_verdict_journal.json")
    private val auditLogFile = File(appContext.filesDir, "retail_wallet_audit.log")
    private val offlineWallet: OfflineWallet by lazy { buildOfflineWallet() }

    private val _preview = MutableLiveData(generatePreview())
    val preview: LiveData<EnvelopePreview> = _preview

    private val _address = MutableLiveData(generateAddressDisplay())
    val address: LiveData<AddressDisplay> = _address

    private val _revocations = MutableLiveData<RevocationUiState>(RevocationUiState.Loading)
    val revocations: LiveData<RevocationUiState> = _revocations
    private val _policyStatus = MutableLiveData<PolicyStatus>()
    val policyStatus: LiveData<PolicyStatus> = _policyStatus
    private val _policyKnobs = MutableLiveData<PolicyKnobState>()
    val policyKnobs: LiveData<PolicyKnobState> = _policyKnobs
    private val _manifestStatus = MutableLiveData<ManifestStatus>()
    val manifestStatus: LiveData<ManifestStatus> = _manifestStatus

    init {
        viewModelScope.launch(Dispatchers.IO) {
            loadRevocations()
        }
        viewModelScope.launch(Dispatchers.IO) {
            seedVerdictJournalIfMissing()
            refreshPolicySnapshot()
        }
        viewModelScope.launch(Dispatchers.IO) {
            refreshManifestStatus()
        }
    }

    fun applyPolicyDefault() {
        policyOverrideStore.applyDefault()
        viewModelScope.launch(Dispatchers.IO) {
            refreshPolicySnapshot()
        }
    }

    fun applyPolicyProfile(profile: String) {
        policyOverrideStore.applyProfile(profile)
        viewModelScope.launch(Dispatchers.IO) {
            refreshPolicySnapshot()
        }
    }

    fun applyPolicyOverride(durationMs: Long) {
        policyOverrideStore.applyOverrideMs(durationMs)
        viewModelScope.launch(Dispatchers.IO) {
            refreshPolicySnapshot()
        }
    }

    private suspend fun refreshPolicySnapshot() {
        val overrides = policyOverrideStore.current()
        val loadedPolicy = SecurityPolicyLoader.load(appContext, overrides)
        securityPolicy = loadedPolicy
        val pinnedStatus = PinnedCertificateVerifier.verifyPinnedRoot(appContext, loadedPolicy, auditLogger)
        val verdictStatus = evaluateVerdictStatus(loadedPolicy)
        _policyStatus.postValue(
            PolicyStatus(
                version = loadedPolicy.version,
                pinnedRootStatus = pinnedStatus,
                verdictStatus = verdictStatus,
                gracePeriodMs = loadedPolicy.verdictGracePeriodMs,
                graceSource = loadedPolicy.gracePeriodSource,
                graceProfile = loadedPolicy.gracePeriodProfile
            )
        )
        _policyKnobs.postValue(
            PolicyKnobState(
                defaultGraceMs = loadedPolicy.defaultGracePeriodMs,
                effectiveGraceMs = loadedPolicy.verdictGracePeriodMs,
                graceSource = loadedPolicy.gracePeriodSource,
                activeProfile = loadedPolicy.gracePeriodProfile,
                overrideMs = overrides.graceOverrideMs
                    ?: loadedPolicy.verdictGracePeriodMs.takeIf {
                        loadedPolicy.gracePeriodSource == SecurityPolicy.GracePeriodSource.OVERRIDE
                    },
                availableProfiles = loadedPolicy.availableGraceProfiles
            )
        )
    }

    private suspend fun refreshManifestStatus() {
        val status = try {
            val manifest = PosManifestLoader.loadFromAssets(appContext)
            ManifestStatus.from(manifest)
        } catch (ex: Exception) {
            ManifestStatus(
                manifestId = "unavailable",
                sequence = -1,
                operator = "n/a",
                validWindowLabel = "n/a",
                rotationLabel = "n/a",
                dualStatusLabel = ex.message ?: "manifest unavailable",
                dualStatusHealthy = false,
                warnings = listOf("manifest unavailable: ${ex.message ?: "unknown error"}"),
                backendRoots = emptyList()
            )
        }
        auditLogger.logManifestStatus(status)
        _manifestStatus.postValue(status)
    }

    private fun generatePreview(): EnvelopePreview {
        return try {
            val alias = "retail-wallet-demo"
            val payload = TransactionPayload.builder()
                .setAuthority("6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV")
                .putMetadata("scenario", "preview")
                .build()
            val options = OfflineEnvelopeOptions.builder()
                .putMetadata("source", "retail-wallet-sample")
                .build()
            val bundle = builder.encodeAndSignEnvelopeWithAttestation(
                payload,
                alias,
                KeySecurityPreference.SOFTWARE_ONLY,
                options,
                null
            )
            val hash = SignedTransactionHasher.hashHex(bundle.envelope().toSignedTransaction())
            EnvelopePreview(
                signingAlias = alias,
                hash = hash,
                attestationAvailable = bundle.attestation().isPresent
            )
        } catch (ex: Exception) {
            val reason = when (ex) {
                is NoritoException,
                is SigningException,
                is KeyManagementException -> ex.javaClass.simpleName
                else -> "Unexpected"
            }
            EnvelopePreview(
                signingAlias = "error",
                hash = "failed:$reason",
                attestationAvailable = false
            )
        }
    }

    private fun generateAddressDisplay(): AddressDisplay {
        return try {
            val sampleKey = ByteArray(32) { index -> ((index * 13) and 0xFF).toByte() }
            val accountAddress = AccountAddress.fromAccount(
                AccountAddress.DEFAULT_DOMAIN_NAME,
                sampleKey,
                "ed25519"
            )
            val networkPrefix = AccountAddress.DEFAULT_I105_DISCRIMINANT
            val formats = accountAddress.displayFormats(networkPrefix)
            AddressDisplay(
                i105 = formats.i105,
                i105Default = formats.i105Default,
                i105Warning = formats.i105Warning,
                defaultDomain = AccountAddress.DEFAULT_DOMAIN_NAME,
                implicitDefault = true,
                networkPrefix = formats.networkPrefix
            )
        } catch (ex: AccountAddressException) {
            AddressDisplay(
                i105 = "address-unavailable",
                i105Default = "sora-unavailable",
                i105Warning = AccountAddress.i105WarningMessage(),
                defaultDomain = AccountAddress.DEFAULT_DOMAIN_NAME,
                implicitDefault = true,
                networkPrefix = AccountAddress.DEFAULT_I105_DISCRIMINANT
            )
        }
    }

    data class EnvelopePreview(
        val signingAlias: String,
        val hash: String,
        val attestationAvailable: Boolean
    )

    data class AddressDisplay(
        val i105: String,
        val i105Default: String,
        val i105Warning: String,
        val defaultDomain: String,
        val implicitDefault: Boolean,
        val networkPrefix: Int
    )

    data class RevocationDisplay(
        val verdictIdHex: String,
        val issuerId: String,
        val reason: String,
        val revokedAtMs: Long,
        val note: String?
    ) {
        fun shortVerdict(): String = verdictIdHex.take(12)

        fun formattedTimestamp(): String =
            REVOCATION_TIMESTAMP_FORMATTER.format(Instant.ofEpochMilli(revokedAtMs))
    }

    data class PolicyStatus(
        val version: String,
        val pinnedRootStatus: PinnedRootStatus,
        val verdictStatus: VerdictStatus,
        val gracePeriodMs: Long,
        val graceSource: SecurityPolicy.GracePeriodSource,
        val graceProfile: String?
    )

    data class PolicyKnobState(
        val defaultGraceMs: Long,
        val effectiveGraceMs: Long,
        val graceSource: SecurityPolicy.GracePeriodSource,
        val activeProfile: String?,
        val overrideMs: Long?,
        val availableProfiles: Map<String, Long>
    )

    data class VerdictStatus(
        val certificateId: String,
        val attestationNonce: String?,
        val deadlineIso: String?,
        val warnings: List<String>,
        val blockedReason: String?
    ) {
        val isBlocked: Boolean = blockedReason != null
    }

    sealed interface RevocationUiState {
        data object Loading : RevocationUiState
        data class Loaded(val total: Int, val items: List<RevocationDisplay>) : RevocationUiState
        data class Error(val message: String) : RevocationUiState
    }

    private suspend fun loadRevocations() {
        _revocations.postValue(RevocationUiState.Loading)
        try {
            val params = OfflineListParams.builder()
                .limit(5L)
                .build()
            val response = offlineClient.listRevocations(params).await()
            val items = response.items().map { it.toDisplay() }
            _revocations.postValue(
                RevocationUiState.Loaded(
                    total = response.total().toInt(),
                    items = items
                )
            )
        } catch (ex: Exception) {
            val reason = when (ex) {
                is OfflineToriiException -> ex.message ?: "offline Torii error"
                else -> ex.message ?: "unable to load revocations"
            }
            _revocations.postValue(RevocationUiState.Error(reason))
        }
    }

    private fun OfflineRevocationList.OfflineRevocationItem.toDisplay(): RevocationDisplay {
        return RevocationDisplay(
            verdictIdHex = verdictIdHex(),
            issuerId = issuerDisplay(),
            reason = reason(),
            revokedAtMs = revokedAtMs(),
            note = note()
        )
    }

    private suspend fun <T> CompletableFuture<T>.await(): T =
        suspendCancellableCoroutine { cont ->
            this.whenComplete { value, throwable ->
                if (throwable != null) {
                    cont.resumeWithException(throwable)
                } else {
                    cont.resume(value)
                }
            }
            cont.invokeOnCancellation { this.cancel(true) }
        }

    companion object {
        private val REVOCATION_TIMESTAMP_FORMATTER =
            DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm 'UTC'").withZone(ZoneOffset.UTC)
    }

    private fun seedVerdictJournalIfMissing() {
        if (verdictJournalFile.exists() && verdictJournalFile.length() > 0) {
            return
        }
        verdictJournalFile.parentFile?.mkdirs()
        appContext.assets.open("offline_verdict_journal.json").use { input ->
            verdictJournalFile.outputStream().use { output ->
                input.copyTo(output)
            }
        }
    }

    private fun buildOfflineWallet(): OfflineWallet {
        return try {
            OfflineWallet(
                offlineClient,
                auditLogFile.toPath(),
                true
            )
        } catch (ex: IOException) {
            throw IllegalStateException("Unable to initialize OfflineWallet", ex)
        }
    }

    private fun evaluateVerdictStatus(policy: SecurityPolicy): VerdictStatus {
        return try {
            val metadata = offlineWallet.ensureFreshVerdict(
                policy.enforcedCertificateIdHex,
                policy.expectedAttestationNonceHex
            )
            val warnings = collectWarnings(policy)
            val deadlineIso = formatDeadline(metadata)
            val status = VerdictStatus(
                certificateId = metadata.certificateIdHex(),
                attestationNonce = metadata.attestationNonceHex(),
                deadlineIso = deadlineIso,
                warnings = warnings,
                blockedReason = null
            )
            auditLogger.logVerdictStatus(status, policy)
            status
        } catch (ex: OfflineVerdictException) {
            val status = VerdictStatus(
                certificateId = policy.enforcedCertificateIdHex,
                attestationNonce = policy.expectedAttestationNonceHex,
                deadlineIso = null,
                warnings = emptyList(),
                blockedReason = ex.message ?: ex.reason().name
            )
            auditLogger.logVerdictStatus(status, policy)
            status
        } catch (io: IOException) {
            val status = VerdictStatus(
                certificateId = policy.enforcedCertificateIdHex,
                attestationNonce = policy.expectedAttestationNonceHex,
                deadlineIso = null,
                warnings = emptyList(),
                blockedReason = io.message ?: "I/O error"
            )
            auditLogger.logVerdictStatus(status, policy)
            status
        }
    }

    private fun collectWarnings(policy: SecurityPolicy): List<String> {
        return try {
            val journal = OfflineVerdictJournal(verdictJournalFile.toPath())
            val now = Instant.now()
            journal.warnings(policy.verdictGracePeriodMs, now)
                .filter { warning ->
                    warning.certificateIdHex().equals(policy.enforcedCertificateIdHex, ignoreCase = true)
                }
                .map(::formatWarning)
        } catch (ex: IOException) {
            emptyList()
        }
    }

    private fun formatWarning(warning: OfflineVerdictWarning): String {
        val deadline = Instant.ofEpochMilli(warning.deadlineMs())
        val stateLabel = warning.state().name.lowercase(Locale.US)
        val deadlineLabel = DateTimeFormatter.ISO_INSTANT.format(deadline)
        val remaining = if (warning.state() == OfflineVerdictWarning.State.WARNING) {
            formatDuration(warning.millisecondsRemaining())
        } else {
            "expired"
        }
        return "${warning.deadlineKind().name.lowercase(Locale.US)} deadline $stateLabel ($remaining) on $deadlineLabel"
    }

    private fun formatDeadline(metadata: OfflineVerdictMetadata): String? {
        val refresh = metadata.refreshAtMs()
        val policy = metadata.policyExpiresAtMs()
        val certificate = metadata.certificateExpiresAtMs()
        val next = listOfNotNull(
            refresh?.let { it to "refresh" },
            if (policy > 0) policy to "policy" else null,
            if (certificate > 0) certificate to "certificate" else null
        ).minByOrNull { it.first } ?: return null
        val instant = Instant.ofEpochMilli(next.first)
        return "${next.second}: ${DateTimeFormatter.ISO_INSTANT.format(instant)}"
    }

    private fun formatDuration(durationMs: Long): String {
        val duration = Duration.ofMillis(durationMs)
        val days = duration.toDays()
        val hours = duration.minusDays(days).toHours()
        val minutes = duration.minusDays(days).minusHours(hours).toMinutes()
        val parts = mutableListOf<String>()
        if (days > 0) {
            parts.add("${days}d")
        }
        if (hours > 0) {
            parts.add("${hours}h")
        }
        if (minutes > 0) {
            parts.add("${minutes}m")
        }
        if (parts.isEmpty()) {
            parts.add("${duration.seconds}s")
        }
        return parts.joinToString(" ")
    }
}
