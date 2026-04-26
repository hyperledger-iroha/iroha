package org.hyperledger.iroha.samples.wallet

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
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
import org.hyperledger.iroha.android.norito.NoritoException
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.android.model.TransactionPayload
import org.hyperledger.iroha.android.tx.SignedTransactionHasher
import org.hyperledger.iroha.android.tx.TransactionBuilder

class WalletPreviewViewModel(application: Application) : AndroidViewModel(application) {

    private val keyManager = IrohaKeyManager.withDefaultProviders()
    private val builder = TransactionBuilder(NoritoJavaCodecAdapter(), keyManager)
    private val appContext = application.applicationContext
    private val policyOverrideStore = PolicyOverrideStore(appContext)
    @Volatile private var securityPolicy: SecurityPolicy =
        SecurityPolicyLoader.load(appContext, policyOverrideStore.current())
    private val auditLogger = PosAuditLogger(appContext)
    private val _preview = MutableLiveData(generatePreview())
    val preview: LiveData<EnvelopePreview> = _preview

    private val _address = MutableLiveData(generateAddressDisplay())
    val address: LiveData<AddressDisplay> = _address

    private val _revocations = MutableLiveData<RevocationUiState>(
        RevocationUiState.Loaded(0, emptyList())
    )
    val revocations: LiveData<RevocationUiState> = _revocations
    private val _policyStatus = MutableLiveData<PolicyStatus>()
    val policyStatus: LiveData<PolicyStatus> = _policyStatus
    private val _policyKnobs = MutableLiveData<PolicyKnobState>()
    val policyKnobs: LiveData<PolicyKnobState> = _policyKnobs
    private val _manifestStatus = MutableLiveData<ManifestStatus>()
    val manifestStatus: LiveData<ManifestStatus> = _manifestStatus

    init {
        viewModelScope.launch(Dispatchers.IO) {
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
                .setAuthority("sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53")
                .putMetadata("scenario", "preview")
                .build()
            val transaction = builder.encodeAndSign(
                payload,
                alias,
                KeySecurityPreference.SOFTWARE_ONLY
            )
            val hash = SignedTransactionHasher.hashHex(transaction)
            EnvelopePreview(
                signingAlias = alias,
                hash = hash,
                attestationAvailable = false
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
                i105Warning = formats.i105Warning,
                defaultDomain = AccountAddress.DEFAULT_DOMAIN_NAME,
                implicitDefault = true,
                networkPrefix = formats.networkPrefix
            )
        } catch (ex: AccountAddressException) {
            AddressDisplay(
                i105 = "address-unavailable",
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

    private fun evaluateVerdictStatus(policy: SecurityPolicy): VerdictStatus {
        val status = VerdictStatus(
            certificateId = policy.enforcedCertificateIdHex,
            attestationNonce = policy.expectedAttestationNonceHex,
            deadlineIso = null,
            warnings = emptyList(),
            blockedReason = null
        )
        auditLogger.logVerdictStatus(status, policy)
        return status
    }
}
