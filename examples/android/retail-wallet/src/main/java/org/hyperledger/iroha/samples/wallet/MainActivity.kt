package org.hyperledger.iroha.samples.wallet

import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Bundle
import android.widget.Toast
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.observe
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textfield.TextInputEditText
import com.google.zxing.BarcodeFormat
import com.google.zxing.qrcode.QRCodeWriter
import java.time.Duration
import org.hyperledger.iroha.samples.wallet.databinding.ActivityMainBinding

/**
 * Retail wallet placeholder that will eventually exercise offline envelopes and recovery flows.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding
    private val viewModel: WalletPreviewViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.walletTitle.text = getString(R.string.wallet_title)
        binding.walletBody.text = getString(R.string.wallet_body)
        binding.walletSteps.text = getString(R.string.wallet_steps)

        viewModel.preview.observe(this) { preview ->
            val attestationStatus = if (preview.attestationAvailable) {
                getString(R.string.wallet_attestation_present)
            } else {
                getString(R.string.wallet_attestation_missing)
            }
            binding.walletDetails.text = getString(
                R.string.wallet_details,
                preview.signingAlias,
                preview.hash,
                attestationStatus
            )
        }

        viewModel.address.observe(this) { address ->
            binding.addressDomainHint.text = if (address.implicitDefault) {
                getString(R.string.wallet_address_default_note, address.defaultDomain)
            } else {
                getString(R.string.wallet_address_domain_note, address.defaultDomain)
            }
            binding.addressI105Label.text =
                getString(R.string.wallet_address_i105_label, address.networkPrefix)
            binding.addressI105Value.text = address.i105
            binding.addressI105DefaultValue.text = address.i105Default
            binding.addressI105DefaultWarning.text = address.i105Warning

            binding.addressCopyI105.setOnClickListener {
                copyAddressToClipboard(
                    label = "I105",
                    address = address.i105,
                    successMessage = getString(R.string.wallet_copy_success, "I105"),
                    warning = null,
                    mode = AddressCopyTelemetry.CopyMode.I105
                )
            }
            binding.addressCopyI105.setTag(
                R.id.copy_mode_tag,
                AddressCopyTelemetry.CopyMode.I105.analyticsLabel
            )
            binding.addressCopyI105Default.setOnClickListener {
                copyAddressToClipboard(
                    label = "i105_default",
                    address = address.i105Default,
                    successMessage = getString(R.string.wallet_copy_success, "i105_default"),
                    warning = address.i105Warning,
                    mode = AddressCopyTelemetry.CopyMode.I105_DEFAULT
                )
            }
            binding.addressCopyI105Default.setTag(
                R.id.copy_mode_tag,
                AddressCopyTelemetry.CopyMode.I105_DEFAULT.analyticsLabel
            )

            val qrBitmap = generateQrBitmap(address.i105)
            if (qrBitmap != null) {
                binding.addressQr.setImageBitmap(qrBitmap)
                binding.addressQr.contentDescription =
                    getString(R.string.wallet_qr_content_description, address.i105)
                binding.addressQrCaption.text = getString(R.string.wallet_qr_caption)
            } else {
                binding.addressQr.setImageBitmap(null)
                binding.addressQrCaption.text = getString(R.string.wallet_qr_generation_error)
            }
        }

        viewModel.revocations.observe(this) { state ->
            when (state) {
                is WalletPreviewViewModel.RevocationUiState.Loading -> {
                    binding.revocationStatus.text = getString(R.string.wallet_revocation_loading)
                    binding.revocationList.text = ""
                }
                is WalletPreviewViewModel.RevocationUiState.Error -> {
                    binding.revocationStatus.text =
                        getString(R.string.wallet_revocation_error, state.message)
                    binding.revocationList.text = ""
                }
                is WalletPreviewViewModel.RevocationUiState.Loaded -> {
                    if (state.items.isEmpty()) {
                        binding.revocationStatus.text =
                            getString(R.string.wallet_revocation_empty)
                        binding.revocationList.text = ""
                    } else {
                        binding.revocationStatus.text = getString(
                            R.string.wallet_revocation_status,
                            state.total
                        )
                        val entries = state.items.joinToString(separator = "\n") { item ->
                            getString(
                                R.string.wallet_revocation_entry,
                                item.shortVerdict(),
                                item.reason,
                                item.formattedTimestamp(),
                                item.issuerId,
                                item.note ?: getString(R.string.wallet_revocation_no_note)
                            )
                        }
                        binding.revocationList.text = entries
                    }
                }
            }
        }

        viewModel.policyKnobs.observe(this) { knobs ->
            binding.policyKnobSummary.text = formatPolicyKnobSummary(knobs)
            binding.policyConfigureButton.setOnClickListener {
                showPolicyKnobDialog(knobs)
            }
        }

        viewModel.policyStatus.observe(this) { status ->
            binding.policyVersion.text =
                getString(R.string.wallet_policy_version, status.version)
            val graceLabel = when (status.graceSource) {
                SecurityPolicy.GracePeriodSource.POLICY_DEFAULT ->
                    getString(R.string.wallet_policy_grace_label_policy)
                SecurityPolicy.GracePeriodSource.OVERRIDE ->
                    getString(R.string.wallet_policy_grace_label_override)
                SecurityPolicy.GracePeriodSource.PROFILE -> {
                    val profileLabel = status.graceProfile ?: getString(
                        R.string.wallet_policy_grace_label_profile_unknown
                    )
                    getString(R.string.wallet_policy_grace_label_profile, profileLabel)
                }
            }
            binding.policyGrace.text = getString(
                R.string.wallet_policy_grace,
                formatDuration(status.gracePeriodMs),
                graceLabel
            )
            val pinnedText = if (status.pinnedRootStatus.matched) {
                getString(
                    R.string.wallet_policy_pin_ok,
                    status.pinnedRootStatus.matchedAlias
                        ?: getString(R.string.wallet_policy_pin_unknown_alias),
                    status.pinnedRootStatus.actualSha256
                )
            } else {
                val configured = if (status.pinnedRootStatus.configuredAliases.isEmpty()) {
                    getString(R.string.wallet_policy_pin_none)
                } else {
                    status.pinnedRootStatus.configuredAliases.joinToString(", ")
                }
                getString(
                    R.string.wallet_policy_pin_mismatch,
                    configured,
                    status.pinnedRootStatus.actualSha256
                )
            }
            binding.policyPinnedRoot.text = pinnedText
            val verdictText = if (status.verdictStatus.isBlocked) {
                getString(
                    R.string.wallet_policy_verdict_blocked,
                    status.verdictStatus.blockedReason ?: "unknown"
                )
            } else {
                getString(
                    R.string.wallet_policy_verdict_ok,
                    status.verdictStatus.certificateId,
                    status.verdictStatus.deadlineIso ?: "n/a"
                )
            }
            binding.policyVerdictStatus.text = verdictText
            val warnings = status.verdictStatus.warnings
            binding.policyWarnings.text = if (warnings.isEmpty()) {
                getString(R.string.wallet_policy_no_warnings)
            } else {
                getString(R.string.wallet_policy_warnings, warnings.joinToString("\n"))
            }
        }

        viewModel.manifestStatus.observe(this) { status ->
            binding.manifestId.text = getString(
                R.string.wallet_manifest_id,
                status.manifestId,
                status.sequence
            )
            binding.manifestOperator.text = getString(
                R.string.wallet_manifest_operator,
                status.operator
            )
            val windowParts = status.validWindowLabel.split(" – ")
            val start = windowParts.getOrNull(0) ?: status.validWindowLabel
            val end = windowParts.getOrNull(1) ?: status.validWindowLabel
            binding.manifestWindow.text = getString(
                R.string.wallet_manifest_window,
                start,
                end
            )
            val rotationText = if (status.rotationLabel == "n/a") {
                getString(R.string.wallet_manifest_rotation_unknown)
            } else {
                getString(R.string.wallet_manifest_rotation, status.rotationLabel)
            }
            binding.manifestRotation.text = rotationText
            val dualText = if (status.dualStatusHealthy) {
                getString(R.string.wallet_manifest_dual_ok, status.dualStatusLabel)
            } else {
                getString(R.string.wallet_manifest_dual_alert, status.dualStatusLabel)
            }
            binding.manifestDualStatus.text = dualText
            binding.manifestDualStatus.setTextColor(
                if (status.dualStatusHealthy) {
                    getColor(android.R.color.black)
                } else {
                    getColor(android.R.color.holo_red_dark)
                }
            )
            val warningsText = if (status.warnings.isEmpty()) {
                getString(R.string.wallet_manifest_warnings_none)
            } else {
                getString(
                    R.string.wallet_manifest_warnings,
                    status.warnings.joinToString("; ")
                )
            }
            binding.manifestWarnings.text = warningsText
            binding.manifestRoots.text = getString(
                R.string.wallet_manifest_roots,
                formatManifestRoots(status)
            )
        }
    }

    private fun showPolicyKnobDialog(state: WalletPreviewViewModel.PolicyKnobState) {
        val options = mutableListOf<Pair<String, () -> Unit>>()
        options += getString(
            R.string.wallet_policy_knob_option_default,
            formatDuration(state.defaultGraceMs)
        ) to { viewModel.applyPolicyDefault() }
        val sortedProfiles = state.availableProfiles.entries.sortedBy { it.key.lowercase() }
        sortedProfiles.forEach { entry ->
            options += getString(
                R.string.wallet_policy_knob_option_profile,
                entry.key,
                formatDuration(entry.value)
            ) to { viewModel.applyPolicyProfile(entry.key) }
        }
        options += getString(R.string.wallet_policy_knob_option_custom) to {
            showCustomOverrideDialog(state)
        }
        MaterialAlertDialogBuilder(this)
            .setTitle(R.string.wallet_policy_knob_dialog_title)
            .setItems(options.map { it.first }.toTypedArray()) { _, which ->
                options.getOrNull(which)?.second?.invoke()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun showCustomOverrideDialog(state: WalletPreviewViewModel.PolicyKnobState) {
        val content = layoutInflater.inflate(R.layout.dialog_grace_override, null)
        val input = content.findViewById<TextInputEditText>(R.id.grace_override_minutes)
        val currentMinutes = state.overrideMs?.let { Duration.ofMillis(it).toMinutes() }
        if (currentMinutes != null && currentMinutes > 0) {
            input.setText(currentMinutes.toString())
        }
        MaterialAlertDialogBuilder(this)
            .setTitle(R.string.wallet_policy_override_title)
            .setView(content)
            .setPositiveButton(R.string.wallet_policy_override_positive) { _, _ ->
                val minutes = input.text?.toString()?.trim()?.toLongOrNull()
                if (minutes == null || minutes <= 0) {
                    Toast.makeText(
                        this,
                        R.string.wallet_policy_override_invalid,
                        Toast.LENGTH_SHORT
                    ).show()
                } else {
                    val millis = Duration.ofMinutes(minutes).toMillis()
                    viewModel.applyPolicyOverride(millis)
                }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun formatPolicyKnobSummary(state: WalletPreviewViewModel.PolicyKnobState): String {
        return when (state.graceSource) {
            SecurityPolicy.GracePeriodSource.POLICY_DEFAULT ->
                getString(
                    R.string.wallet_policy_knob_summary_default,
                    formatDuration(state.defaultGraceMs)
                )
            SecurityPolicy.GracePeriodSource.PROFILE -> {
                val profileLabel = state.activeProfile
                    ?: getString(R.string.wallet_policy_grace_label_profile_unknown)
                getString(
                    R.string.wallet_policy_knob_summary_profile,
                    profileLabel,
                    formatDuration(state.effectiveGraceMs)
                )
            }
            SecurityPolicy.GracePeriodSource.OVERRIDE ->
                getString(
                    R.string.wallet_policy_knob_summary_override,
                    formatDuration(state.effectiveGraceMs)
                )
        }
    }

    private fun copyAddressToClipboard(
        label: String,
        address: String,
        successMessage: String,
        warning: String?,
        mode: AddressCopyTelemetry.CopyMode
    ) {
        val clipboard = getSystemService(ClipboardManager::class.java)
        clipboard?.setPrimaryClip(ClipData.newPlainText(label, address))
        Toast.makeText(this, successMessage, Toast.LENGTH_SHORT).show()
        AddressCopyTelemetry.record(mode)
        warning?.let {
            Toast.makeText(this, it, Toast.LENGTH_SHORT).show()
        }
    }

    private fun generateQrBitmap(content: String, size: Int = 512): Bitmap? {
        return try {
            val bitMatrix = QRCodeWriter().encode(content, BarcodeFormat.QR_CODE, size, size)
            val pixels = IntArray(size * size)
            for (y in 0 until size) {
                for (x in 0 until size) {
                    pixels[y * size + x] = if (bitMatrix[x, y]) Color.BLACK else Color.WHITE
                }
            }
            Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888).apply {
                setPixels(pixels, 0, size, 0, 0, size, size)
            }
        } catch (ex: Exception) {
            null
        }
    }

    private fun formatDuration(durationMs: Long): String {
        if (durationMs <= 0) {
            return "0s"
        }
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

    private fun formatManifestRoots(status: ManifestStatus): String {
        if (status.backendRoots.isEmpty()) {
            return getString(R.string.wallet_manifest_roots_empty)
        }
        val lines = status.backendRoots.joinToString("\n") { root ->
            getString(
                R.string.wallet_manifest_root_line,
                root.label,
                root.role,
                root.statusLabel
            )
        }
        return lines
    }
}
