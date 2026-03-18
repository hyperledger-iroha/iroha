package org.hyperledger.iroha.samples.operator

import android.os.Bundle
import android.widget.TextView
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.annotation.StringRes
import androidx.core.view.isVisible
import androidx.lifecycle.observe
import java.time.format.DateTimeFormatter
import org.hyperledger.iroha.samples.operator.databinding.ActivityMainBinding
import org.hyperledger.iroha.samples.operator.env.HarnessArtifactKind

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    private val viewModel: OperatorConsoleViewModel by viewModels()
    private val instantFormatter = DateTimeFormatter.ISO_INSTANT

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.operatorConsoleHeadline.text = getString(R.string.operator_console_title)
        binding.operatorConsoleBody.text = getString(R.string.operator_console_body)
        binding.operatorRefreshButton.setOnClickListener { viewModel.refreshStatus() }

        viewModel.state.observe(this) { state ->
            binding.operatorConsoleStatus.text = getString(
                R.string.operator_console_status,
                state.pendingWorkstreams,
                state.nextAction
            )
            binding.operatorConsolePosture.text = getString(
                R.string.operator_console_posture,
                state.providerCount,
                if (state.strongBoxAvailable) getString(R.string.common_yes) else getString(R.string.common_no)
            )
            binding.operatorConsoleHash.text =
                getString(R.string.operator_console_hash, state.sampleTransactionHash)
            binding.operatorConsoleEnv.text =
                getString(R.string.operator_console_environment, state.profile)
            binding.operatorConsoleToriiEndpoint.text =
                getString(R.string.operator_console_torii_endpoint, state.toriiEndpoint)
            val toriiStatusText = when (state.toriiStatus.reachable) {
                true -> getString(
                    R.string.operator_console_torii_status_online,
                    state.toriiStatus.message
                )
                false -> getString(
                    R.string.operator_console_torii_status_offline,
                    state.toriiStatus.message
                )
                else -> getString(
                    R.string.operator_console_torii_status_pending,
                    state.toriiStatus.message
                )
            }
            binding.operatorConsoleToriiStatus.text = toriiStatusText
            binding.operatorConsoleQueue.text = getString(
                R.string.operator_console_queue,
                state.queueDepth,
                state.queuePath
            )
            val updatedLabel =
                state.lastUpdated?.let { instantFormatter.format(it) }
                    ?: getString(R.string.operator_console_updated_never)
            binding.operatorConsoleUpdated.text =
                getString(R.string.operator_console_updated_at, updatedLabel)

            val artifacts = state.artifacts
            bindOptionalLabel(
                binding.operatorConsoleScoreboard,
                labelWithDigest(
                    artifacts.sorafsScoreboardPath,
                    artifacts.sorafsScoreboardSha256
                ),
                R.string.operator_console_scoreboard
            )
            bindOptionalLabel(
                binding.operatorConsoleSorafsSummary,
                labelWithDigest(
                    artifacts.sorafsSummaryPath,
                    artifacts.sorafsSummarySha256
                ),
                R.string.operator_console_sorafs_summary
            )
            bindOptionalLabel(
                binding.operatorConsoleSorafsReceipts,
                artifacts.sorafsReceiptsPath,
                R.string.operator_console_sorafs_receipts
            )
            bindOptionalLabel(
                binding.operatorConsoleAccounts,
                artifacts.toriiAccountsPath,
                R.string.operator_console_accounts
            )
            bindOptionalLabel(
                binding.operatorConsoleToriiLog,
                artifacts.toriiLogPath,
                R.string.operator_console_torii_log
            )
            bindOptionalLabel(
                binding.operatorConsoleToriiMetrics,
                artifacts.toriiMetricsPath,
                R.string.operator_console_torii_metrics
            )
            bindOptionalLabel(
                binding.operatorConsoleTelemetry,
                artifacts.telemetryLogPath,
                R.string.operator_console_telemetry_log
            )
            bindOptionalLabel(
                binding.operatorConsoleHandoffEndpoint,
                artifacts.handoffEndpoint,
                R.string.operator_console_handoff_endpoint
            )
            bindOptionalLabel(
                binding.operatorConsoleHandoffInbox,
                artifacts.handoffInboxPath,
                R.string.operator_console_handoff_inbox
            )
            bindHarnessStatus(binding, artifacts.missingMockHarnessArtifacts())

            val hasError = !state.errorMessage.isNullOrBlank()
            binding.operatorConsoleError.isVisible = hasError
            if (hasError) {
                binding.operatorConsoleError.text = getString(
                    R.string.operator_console_error,
                    state.errorMessage
                )
            }
        }
    }

    private fun bindOptionalLabel(
        view: TextView,
        value: String?,
        @StringRes template: Int
    ) {
        val present = !value.isNullOrBlank()
        view.isVisible = present
        if (present) {
            view.text = getString(template, value)
        }
    }

    private fun labelWithDigest(value: String?, digest: String?): String? {
        val trimmed = value?.trim().orEmpty()
        if (trimmed.isEmpty()) {
            return null
        }
        val digestSuffix =
            digest?.trim()?.takeIf { it.isNotEmpty() }?.let { " (sha256: $it)" }.orEmpty()
        return trimmed + digestSuffix
    }

    private fun bindHarnessStatus(
        binding: ActivityMainBinding,
        missingArtifacts: Set<HarnessArtifactKind>
    ) {
        binding.operatorConsoleHarnessStatus.isVisible = true
        if (missingArtifacts.isEmpty()) {
            binding.operatorConsoleHarnessStatus.text =
                getString(R.string.operator_console_harness_ready)
            return
        }
        val sorted = missingArtifacts.sortedBy { it.ordinal }
        val missingLabels =
            sorted.joinToString(", ") { kind ->
                when (kind) {
                    HarnessArtifactKind.SORAFS_SCOREBOARD ->
                        getString(R.string.operator_console_harness_item_scoreboard)
                    HarnessArtifactKind.SORAFS_SUMMARY ->
                        getString(R.string.operator_console_harness_item_summary)
                    HarnessArtifactKind.SORAFS_RECEIPTS ->
                        getString(R.string.operator_console_harness_item_receipts)
                    HarnessArtifactKind.TELEMETRY_LOG ->
                        getString(R.string.operator_console_harness_item_telemetry)
                }
            }
        binding.operatorConsoleHarnessStatus.text =
            getString(R.string.operator_console_harness_missing, missingLabels)
    }
}
