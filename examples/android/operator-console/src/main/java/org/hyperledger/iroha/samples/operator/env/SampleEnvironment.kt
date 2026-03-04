package org.hyperledger.iroha.samples.operator.env

import java.net.URI
import org.hyperledger.iroha.samples.operator.BuildConfig

data class SampleEnvironment(
    val profile: String,
    val toriiUrl: String,
    val toriiAccountsPath: String?,
    val toriiLogPath: String?,
    val toriiMetricsPath: String?,
    val sorafsScoreboardPath: String?,
    val sorafsSummaryPath: String?,
    val sorafsScoreboardSha256: String?,
    val sorafsSummarySha256: String?,
    val sorafsReceiptsPath: String?,
    val telemetryLogPath: String?,
    val telemetrySaltHex: String?,
    val telemetrySaltVersion: String?,
    val telemetryRotationId: String?,
    val telemetryExporter: String?,
    val handoffEndpoint: String?,
    val handoffInboxPath: String?
) {
    fun resolvedToriiUrl(): String = toriiUrl.ifBlank { DEFAULT_TORII }

    fun resolvedToriiUri(): URI = try {
        URI.create(resolvedToriiUrl())
    } catch (_: IllegalArgumentException) {
        URI.create(DEFAULT_TORII)
    }

    companion object {
        const val DEFAULT_TORII: String = "https://torii.dev.sora.org"

        fun fromBuildConfig(): SampleEnvironment =
            SampleEnvironment(
                profile = BuildConfig.SAMPLE_PROFILE.ifBlank { "operator" },
                toriiUrl = BuildConfig.SAMPLE_TORII_URL,
                toriiAccountsPath = sanitize(BuildConfig.SAMPLE_TORII_ACCOUNTS),
                toriiLogPath = sanitize(BuildConfig.SAMPLE_TORII_LOG),
                toriiMetricsPath = sanitize(BuildConfig.SAMPLE_TORII_METRICS),
                sorafsScoreboardPath = sanitize(BuildConfig.SAMPLE_SORAFS_SCOREBOARD),
                sorafsSummaryPath = sanitize(BuildConfig.SAMPLE_SORAFS_SUMMARY),
                sorafsScoreboardSha256 = sanitize(BuildConfig.SAMPLE_SORAFS_SCOREBOARD_SHA256),
                sorafsSummarySha256 = sanitize(BuildConfig.SAMPLE_SORAFS_SUMMARY_SHA256),
                sorafsReceiptsPath = sanitize(BuildConfig.SAMPLE_SORAFS_RECEIPTS),
                telemetryLogPath = sanitize(BuildConfig.SAMPLE_TELEMETRY_LOG),
                telemetrySaltHex = sanitize(BuildConfig.SAMPLE_TELEMETRY_SALT_HEX),
                telemetrySaltVersion = sanitize(BuildConfig.SAMPLE_TELEMETRY_SALT_VERSION),
                telemetryRotationId = sanitize(BuildConfig.SAMPLE_TELEMETRY_ROTATION_ID),
                telemetryExporter = sanitize(BuildConfig.SAMPLE_TELEMETRY_EXPORTER),
                handoffEndpoint = sanitize(BuildConfig.SAMPLE_HANDOFF_ENDPOINT),
                handoffInboxPath = sanitize(BuildConfig.SAMPLE_HANDOFF_INBOX)
            )

        private fun sanitize(value: String?): String? {
            if (value == null) {
                return null
            }
            val trimmed = value.trim()
            return trimmed.takeIf { it.isNotEmpty() }
        }
    }
}

data class HarnessArtifacts(
    val toriiAccountsPath: String?,
    val toriiLogPath: String?,
    val toriiMetricsPath: String?,
    val sorafsScoreboardPath: String?,
    val sorafsSummaryPath: String?,
    val sorafsScoreboardSha256: String?,
    val sorafsSummarySha256: String?,
    val sorafsReceiptsPath: String?,
    val telemetryLogPath: String?,
    val handoffEndpoint: String?,
    val handoffInboxPath: String?
) {
    fun missingMockHarnessArtifacts(): Set<HarnessArtifactKind> {
        val missing = mutableSetOf<HarnessArtifactKind>()
        if (sorafsScoreboardPath.isNullOrBlank()) {
            missing += HarnessArtifactKind.SORAFS_SCOREBOARD
        }
        if (sorafsSummaryPath.isNullOrBlank()) {
            missing += HarnessArtifactKind.SORAFS_SUMMARY
        }
        if (sorafsReceiptsPath.isNullOrBlank()) {
            missing += HarnessArtifactKind.SORAFS_RECEIPTS
        }
        if (telemetryLogPath.isNullOrBlank()) {
            missing += HarnessArtifactKind.TELEMETRY_LOG
        }
        return missing
    }

    fun hasMockHarnessBundle(): Boolean = missingMockHarnessArtifacts().isEmpty()
}

enum class HarnessArtifactKind {
    SORAFS_SCOREBOARD,
    SORAFS_SUMMARY,
    SORAFS_RECEIPTS,
    TELEMETRY_LOG
}

fun SampleEnvironment.harnessArtifacts(): HarnessArtifacts =
    HarnessArtifacts(
        toriiAccountsPath = toriiAccountsPath,
        toriiLogPath = toriiLogPath,
        toriiMetricsPath = toriiMetricsPath,
        sorafsScoreboardPath = sorafsScoreboardPath,
        sorafsSummaryPath = sorafsSummaryPath,
        sorafsScoreboardSha256 = sorafsScoreboardSha256,
        sorafsSummarySha256 = sorafsSummarySha256,
        sorafsReceiptsPath = sorafsReceiptsPath,
        telemetryLogPath = telemetryLogPath,
        handoffEndpoint = handoffEndpoint,
        handoffInboxPath = handoffInboxPath
    )
