package org.hyperledger.iroha.sdk.offline

import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardOpenOption
import java.time.Duration
import java.time.Instant
import java.util.Locale
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.offline.OfflineVerdictWarning.DeadlineKind

/** File-backed store that keeps verdict metadata and emits countdown warnings. */
class OfflineVerdictJournal(val journalFile: Path) {

    private val lock = Any()
    private val entries = LinkedHashMap<String, OfflineVerdictMetadata>()

    init {
        val parent = journalFile.parent
        if (parent != null) Files.createDirectories(parent)
        if (Files.exists(journalFile) && Files.size(journalFile) > 0) loadExisting()
    }

    fun entries(): List<OfflineVerdictMetadata> {
        synchronized(lock) { return entries.values.toList() }
    }

    fun exportJson(): ByteArray {
        synchronized(lock) {
            return JsonEncoder.encode(serializeEntriesLocked()).toByteArray(Charsets.UTF_8)
        }
    }

    fun find(certificateIdHex: String?): java.util.Optional<OfflineVerdictMetadata> {
        if (certificateIdHex.isNullOrBlank()) return java.util.Optional.empty()
        synchronized(lock) {
            val direct = entries[certificateIdHex]
            if (direct != null) return java.util.Optional.of(direct)
            val normalized = certificateIdHex.trim()
            for ((key, value) in entries) {
                if (key.equals(normalized, ignoreCase = true)) return java.util.Optional.of(value)
            }
            return java.util.Optional.empty()
        }
    }

    @Throws(IOException::class)
    fun updateSafetyDetectToken(
        certificateIdHex: String,
        snapshot: OfflineVerdictMetadata.SafetyDetectTokenSnapshot,
    ) {
        synchronized(lock) {
            val key = resolveCertificateKeyLocked(certificateIdHex)
                ?: throw IllegalArgumentException("No verdict metadata recorded for $certificateIdHex")
            val existing = entries[key]!!
            entries[key] = existing.withSafetyDetectToken(snapshot)
            persistLocked()
        }
    }

    @Throws(IOException::class)
    fun updatePlayIntegrityToken(
        certificateIdHex: String,
        snapshot: OfflineVerdictMetadata.PlayIntegrityTokenSnapshot,
    ) {
        synchronized(lock) {
            val key = resolveCertificateKeyLocked(certificateIdHex)
                ?: throw IllegalArgumentException("No verdict metadata recorded for $certificateIdHex")
            val existing = entries[key]!!
            entries[key] = existing.withPlayIntegrityToken(snapshot)
            persistLocked()
        }
    }

    @Throws(IOException::class)
    fun upsert(
        allowances: List<OfflineAllowanceItem>,
        recordedAt: Instant,
    ): List<OfflineVerdictMetadata> {
        val recordedAtMs = recordedAt.toEpochMilli()
        synchronized(lock) {
            val inserted = ArrayList<OfflineVerdictMetadata>(allowances.size)
            for (allowance in allowances) {
                val metadata = fromAllowance(allowance, recordedAtMs)
                entries[metadata.certificateIdHex] = metadata
                inserted.add(metadata)
            }
            persistLocked()
            return inserted
        }
    }

    @Throws(IOException::class)
    fun upsert(
        response: OfflineCertificateIssueResponse,
        recordedAt: Instant,
    ): OfflineVerdictMetadata {
        val recordedAtMs = recordedAt.toEpochMilli()
        synchronized(lock) {
            val certificate = response.certificate
            val snapshot = parseIntegritySnapshotFromMetadata(certificate.metadata)
            val metadata = OfflineVerdictMetadata(
                response.certificateIdHex,
                certificate.controller,
                certificate.controller,
                certificate.verdictIdHex,
                certificate.attestationNonceHex,
                certificate.expiresAtMs,
                certificate.policy.expiresAtMs,
                certificate.refreshAtMs,
                certificate.allowance.amount,
                recordedAtMs,
                snapshot.policy,
                snapshot.playIntegrityMetadata,
                null,
                snapshot.hmsMetadata,
                null,
                snapshot.provisionedMetadata,
            )
            entries[metadata.certificateIdHex] = metadata
            persistLocked()
            return metadata
        }
    }

    fun warnings(warningThresholdMs: Long, now: Instant): List<OfflineVerdictWarning> {
        val nowMs = now.toEpochMilli()
        synchronized(lock) {
            val warnings = ArrayList<OfflineVerdictWarning>()
            for (metadata in entries.values) {
                val warning = buildWarning(metadata, nowMs, warningThresholdMs)
                if (warning != null) warnings.add(warning)
            }
            return warnings
        }
    }

    private fun resolveCertificateKeyLocked(certificateIdHex: String?): String? {
        if (certificateIdHex.isNullOrBlank()) return null
        if (entries.containsKey(certificateIdHex)) return certificateIdHex
        val normalized = certificateIdHex.trim()
        for (key in entries.keys) {
            if (key.equals(normalized, ignoreCase = true)) return key
        }
        return null
    }

    @Throws(IOException::class)
    private fun loadExisting() {
        val json = String(Files.readAllBytes(journalFile), Charsets.UTF_8).trim()
        if (json.isEmpty()) return
        val root = JsonParser.parse(json)
        if (root !is Map<*, *>) throw IOException("Verdict journal must be a JSON object")
        entries.clear()
        for ((key, value) in root) {
            val certificateId = key?.toString() ?: continue
            val metadata = OfflineVerdictMetadata.fromJson(certificateId, value)
            entries[certificateId] = metadata
        }
    }

    @Throws(IOException::class)
    private fun persistLocked() {
        val serialized = serializeEntriesLocked()
        Files.write(
            journalFile,
            JsonEncoder.encode(serialized).toByteArray(Charsets.UTF_8),
            StandardOpenOption.CREATE,
            StandardOpenOption.TRUNCATE_EXISTING,
        )
    }

    private fun serializeEntriesLocked(): Map<String, Any?> {
        val serialized = LinkedHashMap<String, Any?>(entries.size)
        for ((key, value) in entries) serialized[key] = value.toJson()
        return serialized
    }

    private class IntegritySnapshot(
        val policy: String?,
        val playIntegrityMetadata: OfflineVerdictMetadata.PlayIntegrityMetadata?,
        val hmsMetadata: OfflineVerdictMetadata.SafetyDetectMetadata?,
        val provisionedMetadata: OfflineVerdictMetadata.ProvisionedMetadata?,
    ) {
        companion object {
            fun empty() = IntegritySnapshot(null, null, null, null)
        }
    }

    companion object {
        private fun fromAllowance(
            allowance: OfflineAllowanceItem,
            recordedAtMs: Long,
        ): OfflineVerdictMetadata {
            val snapshot = parseIntegritySnapshot(allowance.recordJson)
            return OfflineVerdictMetadata(
                allowance.certificateIdHex,
                allowance.controllerId,
                allowance.controllerDisplay,
                allowance.verdictIdHex,
                allowance.attestationNonceHex,
                allowance.certificateExpiresAtMs,
                allowance.policyExpiresAtMs,
                allowance.refreshAtMs,
                allowance.remainingAmount,
                recordedAtMs,
                snapshot.policy,
                snapshot.playIntegrityMetadata,
                null,
                snapshot.hmsMetadata,
                null,
                snapshot.provisionedMetadata,
            )
        }

        private fun buildWarning(
            metadata: OfflineVerdictMetadata,
            nowMs: Long,
            thresholdMs: Long,
        ): OfflineVerdictWarning? {
            val refreshDeadline = metadata.refreshAtMs
            val kind: DeadlineKind
            val deadlineMs: Long
            if (refreshDeadline != null && refreshDeadline > 0) {
                kind = DeadlineKind.REFRESH
                deadlineMs = refreshDeadline
            } else if (metadata.policyExpiresAtMs > 0) {
                kind = DeadlineKind.POLICY
                deadlineMs = metadata.policyExpiresAtMs
            } else if (metadata.certificateExpiresAtMs > 0) {
                kind = DeadlineKind.CERTIFICATE
                deadlineMs = metadata.certificateExpiresAtMs
            } else {
                return null
            }

            val delta = deadlineMs - nowMs
            val state: OfflineVerdictWarning.State = when {
                delta <= 0 -> OfflineVerdictWarning.State.EXPIRED
                delta <= thresholdMs -> OfflineVerdictWarning.State.WARNING
                else -> return null
            }

            val headline = if (state == OfflineVerdictWarning.State.EXPIRED) {
                when (kind) {
                    DeadlineKind.REFRESH -> "Cached verdict expired"
                    DeadlineKind.POLICY -> "Policy expired"
                    DeadlineKind.CERTIFICATE -> "Allowance expired"
                }
            } else {
                when (kind) {
                    DeadlineKind.REFRESH -> "Refresh cached verdict soon"
                    DeadlineKind.POLICY -> "Policy expiry approaching"
                    DeadlineKind.CERTIFICATE -> "Allowance expiry approaching"
                }
            }

            val remaining = formatDuration(Math.abs(delta))
            val verb = if (state == OfflineVerdictWarning.State.EXPIRED) "missed" else "must meet"
            val details = String.format(
                Locale.ROOT,
                "%s (%s) %s its %s deadline at %s. Verdict=%s Remaining=%s Amount=%s",
                metadata.controllerDisplay,
                metadata.certificateIdHex,
                verb,
                kind.name.lowercase(Locale.ROOT),
                Instant.ofEpochMilli(deadlineMs),
                metadata.verdictIdHex ?: "unknown",
                remaining,
                metadata.remainingAmount,
            )

            return OfflineVerdictWarning(
                metadata.certificateIdHex, metadata.controllerId, metadata.controllerDisplay,
                metadata.verdictIdHex, kind, deadlineMs, delta, state, headline, details,
            )
        }

        private fun formatDuration(millis: Long): String {
            val duration = Duration.ofMillis(millis)
            val days = duration.toDays()
            val hours = duration.minusDays(days).toHours()
            val minutes = duration.minusDays(days).minusHours(hours).toMinutes()
            return when {
                days > 0 -> String.format(Locale.ROOT, "%dd %dh %dm", days, hours, minutes)
                hours > 0 -> String.format(Locale.ROOT, "%dh %dm", hours, minutes)
                else -> String.format(Locale.ROOT, "%dm", minutes)
            }
        }

        private fun parseIntegritySnapshot(recordJson: String?): IntegritySnapshot {
            if (recordJson.isNullOrBlank()) return IntegritySnapshot.empty()
            val parsed = JsonParser.parse(recordJson)
            if (parsed !is Map<*, *>) return IntegritySnapshot.empty()
            val certificate = parsed["certificate"]
            if (certificate !is Map<*, *>) return IntegritySnapshot.empty()
            return parseIntegritySnapshotFromMetadata(certificate["metadata"])
        }

        private fun parseIntegritySnapshotFromMetadata(metadataObj: Any?): IntegritySnapshot {
            if (metadataObj !is Map<*, *>) return IntegritySnapshot.empty()
            val policyRaw = optionalString(metadataObj["android.integrity.policy"])
            if (policyRaw.isNullOrBlank()) return IntegritySnapshot.empty()
            val policy = policyRaw.trim().lowercase(Locale.ROOT)
            return when (policy) {
                "hms_safety_detect" -> {
                    val metadata = parseSafetyDetectMetadata(metadataObj)
                    IntegritySnapshot(policy, null, metadata, null)
                }
                "play_integrity" -> {
                    val metadata = parsePlayIntegrityMetadata(metadataObj)
                    IntegritySnapshot(policy, metadata, null, null)
                }
                "provisioned" -> {
                    val provisioned = parseProvisionedMetadata(metadataObj)
                    IntegritySnapshot(policy, null, null, provisioned)
                }
                else -> IntegritySnapshot(policy, null, null, null)
            }
        }

        private fun parseSafetyDetectMetadata(metadataMap: Map<*, *>): OfflineVerdictMetadata.SafetyDetectMetadata? {
            val appId = optionalString(metadataMap["android.hms_safety_detect.app_id"])
            if (appId.isNullOrBlank()) return null
            val packages = stringList(metadataMap["android.hms_safety_detect.package_names"])
            val digests = stringList(metadataMap["android.hms_safety_detect.signing_digests_sha256"])
            if (packages.isEmpty() || digests.isEmpty()) return null
            val evaluations = stringList(metadataMap["android.hms_safety_detect.required_evaluations"])
            val maxAge = optionalLong(metadataMap["android.hms_safety_detect.max_token_age_ms"])
            return OfflineVerdictMetadata.SafetyDetectMetadata(appId, packages, digests, evaluations, maxAge)
        }

        private fun parsePlayIntegrityMetadata(metadataMap: Map<*, *>): OfflineVerdictMetadata.PlayIntegrityMetadata? {
            val project = optionalLong(metadataMap["android.play_integrity.cloud_project_number"])
            val environment = optionalString(metadataMap["android.play_integrity.environment"])
            if (project == null || project <= 0 || environment.isNullOrBlank()) return null
            val packages = stringList(metadataMap["android.play_integrity.package_names"])
            val digests = stringList(metadataMap["android.play_integrity.signing_digests_sha256"])
            val appVerdicts = stringList(metadataMap["android.play_integrity.allowed_app_verdicts"])
            val deviceVerdicts = stringList(metadataMap["android.play_integrity.allowed_device_verdicts"])
            if (packages.isEmpty() || digests.isEmpty() || appVerdicts.isEmpty() || deviceVerdicts.isEmpty()) {
                return null
            }
            val maxAge = optionalLong(metadataMap["android.play_integrity.max_token_age_ms"])
            return OfflineVerdictMetadata.PlayIntegrityMetadata(
                project, environment.trim(), packages, digests, appVerdicts, deviceVerdicts, maxAge,
            )
        }

        private fun parseProvisionedMetadata(metadataMap: Map<*, *>): OfflineVerdictMetadata.ProvisionedMetadata? {
            val inspector = optionalString(metadataMap["android.provisioned.inspector_public_key"])
            val schema = optionalString(metadataMap["android.provisioned.manifest_schema"])
            if (inspector.isNullOrBlank() || schema.isNullOrBlank()) return null
            val version = optionalInteger(metadataMap["android.provisioned.manifest_version"])
            val maxAge = optionalLong(metadataMap["android.provisioned.max_manifest_age_ms"])
            val digest = optionalString(metadataMap["android.provisioned.manifest_digest"])
            return OfflineVerdictMetadata.ProvisionedMetadata(inspector, schema, version, maxAge, digest)
        }

        private fun optionalString(value: Any?): String? =
            if (value == null) null else value.toString()

        private fun optionalLong(value: Any?): Long? {
            if (value is Number) {
                if (value is Float || value is Double) return null
                return value.toLong()
            }
            if (value == null) return null
            return try { value.toString().toLong() } catch (_: NumberFormatException) { null }
        }

        private fun optionalInteger(value: Any?): Int? {
            if (value is Number) {
                if (value is Float || value is Double) return null
                return value.toInt()
            }
            if (value == null) return null
            return try { value.toString().toInt() } catch (_: NumberFormatException) { null }
        }

        private fun stringList(value: Any?): List<String> {
            if (value !is List<*>) return emptyList()
            return value.filterNotNull().map { it.toString() }
        }
    }
}
