package org.hyperledger.iroha.samples.wallet

import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import org.json.JSONObject

class PosAuditLogger(context: Context) {

    private val logFile = File(context.filesDir, "pos_security_audit.log")
    private val prefs = context.getSharedPreferences("pos_security", Context.MODE_PRIVATE)
    private val dateFormat = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.US).apply {
        timeZone = TimeZone.getTimeZone("UTC")
    }

    fun logPinVerification(status: PinnedRootStatus, policyVersion: String) {
        append(
            "PIN_VERIFY",
            mapOf(
                "policy_version" to policyVersion,
                "matched_alias" to status.matchedAlias,
                "configured_aliases" to status.configuredAliases.joinToString(","),
                "configured_digests" to status.configuredDigests.joinToString(","),
                "expected_sha256" to status.expectedSha256,
                "actual_sha256" to status.actualSha256,
                "matched" to status.matched
            )
        )
        val current = status.expectedSha256
        if (!current.isNullOrBlank()) {
            val previous = prefs.getString(PREF_PINSET_HASH, null)
            if (previous == null || !previous.equals(current, ignoreCase = true)) {
                append(
                    "PINSET_ROTATION",
                    mapOf(
                        "previous" to previous,
                        "current" to current,
                        "policy_version" to policyVersion
                    )
                )
                prefs.edit().putString(PREF_PINSET_HASH, current).apply()
            }
        }
    }

    fun logVerdictStatus(status: VerdictStatus, policy: SecurityPolicy) {
        append(
            "VERDICT_STATUS",
            mapOf(
                "policy_version" to policy.version,
                "certificate" to status.certificateId,
                "attestation_nonce" to status.attestationNonce,
                "deadline" to status.deadlineIso,
                "blocked_reason" to status.blockedReason,
                "warnings" to status.warnings,
                "grace_period_ms" to policy.verdictGracePeriodMs,
                "grace_period_source" to policy.gracePeriodSource.name,
                "grace_period_profile" to policy.gracePeriodProfile
            )
        )
        logVerdictRotationIfNeeded(status)
    }

    private fun append(event: String, payload: Map<String, Any?>) {
        val entry = JSONObject()
        entry.put("timestamp", dateFormat.format(Date()))
        entry.put("event", event)
        for ((key, value) in payload) {
            entry.put(key, value)
        }
        logFile.parentFile?.mkdirs()
        logFile.appendText(entry.toString() + "\n")
    }

    companion object {
        private const val PREF_PINSET_HASH = "pinset_hash"
        private const val PREF_VERDICT_CERT = "verdict_certificate"
        private const val PREF_VERDICT_NONCE = "verdict_nonce"
        private const val PREF_MANIFEST_ID = "manifest_id"
        private const val PREF_MANIFEST_SEQUENCE = "manifest_sequence"
    }

    private fun logVerdictRotationIfNeeded(status: VerdictStatus) {
        val certificate = status.certificateId
        val nonce = status.attestationNonce
        if (certificate.isBlank()) {
            return
        }
        val previousCert = prefs.getString(PREF_VERDICT_CERT, null)
        val previousNonce = prefs.getString(PREF_VERDICT_NONCE, null)
        val certChanged = previousCert == null || !previousCert.equals(certificate, ignoreCase = true)
        val nonceChanged = (nonce != null && !nonce.equals(previousNonce, ignoreCase = true)) ||
            (previousNonce != null && nonce == null)
        if (certChanged || nonceChanged) {
            append(
                "VERDICT_ROTATION",
                mapOf(
                    "previous_certificate" to previousCert,
                    "new_certificate" to certificate,
                    "previous_nonce" to previousNonce,
                    "new_nonce" to nonce,
                    "blocked_reason" to status.blockedReason
                )
            )
            prefs.edit()
                .putString(PREF_VERDICT_CERT, certificate)
                .putString(PREF_VERDICT_NONCE, nonce)
                .apply()
        }
    }

    fun logManifestStatus(status: ManifestStatus) {
        append(
            "MANIFEST_STATUS",
            mapOf(
                "manifest_id" to status.manifestId,
                "sequence" to status.sequence,
                "operator" to status.operator,
                "window" to status.validWindowLabel,
                "rotation_hint" to status.rotationLabel,
                "dual_signature" to status.dualStatusLabel,
                "dual_signature_ok" to status.dualStatusHealthy,
                "warnings" to status.warnings
            )
        )
        logManifestRotationIfNeeded(status)
    }

    private fun logManifestRotationIfNeeded(status: ManifestStatus) {
        val previousId = prefs.getString(PREF_MANIFEST_ID, null)
        val previousSeq = prefs.getLong(PREF_MANIFEST_SEQUENCE, -1L)
        if (previousId == status.manifestId && previousSeq == status.sequence.toLong()) {
            return
        }
        append(
            "MANIFEST_ROTATION",
            mapOf(
                "previous_manifest" to previousId,
                "previous_sequence" to previousSeq,
                "new_manifest" to status.manifestId,
                "new_sequence" to status.sequence
            )
        )
        prefs.edit()
            .putString(PREF_MANIFEST_ID, status.manifestId)
            .putLong(PREF_MANIFEST_SEQUENCE, status.sequence.toLong())
            .apply()
    }
}
