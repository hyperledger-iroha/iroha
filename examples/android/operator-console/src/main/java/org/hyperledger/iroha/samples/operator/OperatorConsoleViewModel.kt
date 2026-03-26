package org.hyperledger.iroha.samples.operator

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import java.time.Instant
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import org.hyperledger.iroha.android.IrohaKeyManager
import org.hyperledger.iroha.android.IrohaKeyManager.KeySecurityPreference
import org.hyperledger.iroha.android.KeyManagementException
import org.hyperledger.iroha.android.SigningException
import org.hyperledger.iroha.android.norito.NoritoException
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter
import org.hyperledger.iroha.android.model.TransactionPayload
import org.hyperledger.iroha.android.tx.SignedTransactionHasher
import org.hyperledger.iroha.android.tx.TransactionBuilder
import org.hyperledger.iroha.samples.operator.env.HarnessArtifacts
import org.hyperledger.iroha.samples.operator.env.SampleClientFactory
import org.hyperledger.iroha.samples.operator.env.SampleEnvironment
import org.hyperledger.iroha.samples.operator.env.ToriiHealthProbe
import org.hyperledger.iroha.samples.operator.env.ToriiStatus
import org.hyperledger.iroha.samples.operator.env.harnessArtifacts

class OperatorConsoleViewModel(application: Application) : AndroidViewModel(application) {

    private val environment = SampleEnvironment.fromBuildConfig()
    private val harnessArtifacts = environment.harnessArtifacts()
    private val keyManager = IrohaKeyManager.withDefaultProviders()
    private val transactionBuilder = TransactionBuilder(NoritoJavaCodecAdapter(), keyManager)
    private val clientFactory = SampleClientFactory(application.applicationContext, environment)
    private val clientArtifactsResult = runCatching { clientFactory.createArtifacts() }
    private val clientArtifacts = clientArtifactsResult.getOrNull()

    private val _state =
        MutableLiveData(
            ConsoleState(
                profile = environment.profile,
                toriiEndpoint = environment.resolvedToriiUrl(),
                toriiStatus = ToriiStatus.pending(),
                queuePath = clientFactory.queuePath().toString(),
                queueDepth = 0,
                lastUpdated = null,
                strongBoxAvailable = keyManager.hasStrongBoxProvider(),
                providerCount = keyManager.providerMetadata().size,
                sampleTransactionHash = buildSampleHash(),
                artifacts = harnessArtifacts,
                pendingWorkstreams = if (harnessArtifacts.hasMockHarnessBundle()) 2 else 3,
                nextAction = if (harnessArtifacts.hasMockHarnessBundle()) {
                    "Run Torii health probe"
                } else {
                    "Connect mock harness feeds"
                },
                errorMessage = clientArtifactsResult.exceptionOrNull()?.message
            )
        )
    val state: LiveData<ConsoleState> = _state

    init {
        refreshStatus()
    }

    fun refreshStatus() {
        viewModelScope.launch(Dispatchers.IO) {
            val artifacts = clientArtifacts
            if (artifacts == null) {
                _state.postValue(
                    _state.value?.copy(
                        errorMessage =
                            clientArtifactsResult.exceptionOrNull()?.message
                                ?: "Transport unavailable"
                    )
                )
                return@launch
            }
            val queueDepthResult = runCatching { artifacts.queue.size() }
            val queueDepth = queueDepthResult.getOrElse { 0 }
            val queueError = queueDepthResult.exceptionOrNull()?.message
            val toriiStatus =
                ToriiHealthProbe.check(artifacts.httpClient, environment.resolvedToriiUrl())
            val harnessReady = harnessArtifacts.hasMockHarnessBundle()
            val pendingWorkstreams =
                when {
                    toriiStatus.reachable != true -> 2
                    harnessReady -> 1
                    else -> 2
                }
            val nextAction =
                when {
                    toriiStatus.reachable != true -> "Verify Torii endpoint or restart android_sample_env.sh"
                    harnessReady -> "Monitor pending queue and flush demo transactions"
                    else -> "Connect mock harness feeds"
                }
            val newState =
                _state.value?.copy(
                    toriiStatus = toriiStatus,
                    queueDepth = queueDepth,
                    lastUpdated = Instant.now(),
                    strongBoxAvailable = keyManager.hasStrongBoxProvider(),
                    providerCount = keyManager.providerMetadata().size,
                    sampleTransactionHash = buildSampleHash(),
                    artifacts = harnessArtifacts,
                    pendingWorkstreams = pendingWorkstreams,
                    nextAction = nextAction,
                    errorMessage = queueError
                )
            if (newState != null) {
                _state.postValue(newState)
            }
        }
    }

    private fun buildSampleHash(): String =
        try {
            val payload =
                TransactionPayload.builder()
                    .setAuthority("soraゴヂアニダベェユヌサヨニャノヲョネイッリニャネガヨペバヒョブルノホイキャヸムケチャピファノマオニツミチオウ")
                    .putMetadata("sample", "operator-console")
                    .build()
            val signed =
                transactionBuilder.encodeAndSign(
                    payload,
                    "operator-console-demo",
                    KeySecurityPreference.SOFTWARE_ONLY
                )
            SignedTransactionHasher.hashHex(signed)
        } catch (ex: Exception) {
            when (ex) {
                is NoritoException,
                is SigningException,
                is KeyManagementException -> "error:${ex.javaClass.simpleName}"
                else -> "error:unexpected"
            }
        }

    data class ConsoleState(
        val profile: String,
        val toriiEndpoint: String,
        val toriiStatus: ToriiStatus,
        val queuePath: String,
        val queueDepth: Int,
        val lastUpdated: Instant?,
        val strongBoxAvailable: Boolean,
        val providerCount: Int,
        val sampleTransactionHash: String,
        val artifacts: HarnessArtifacts,
        val pendingWorkstreams: Int,
        val nextAction: String,
        val errorMessage: String?
    )
}
