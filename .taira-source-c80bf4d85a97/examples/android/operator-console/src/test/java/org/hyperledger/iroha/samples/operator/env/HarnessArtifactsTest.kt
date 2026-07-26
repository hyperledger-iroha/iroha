package org.hyperledger.iroha.samples.operator.env

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HarnessArtifactsTest {

    @Test
    fun harnessArtifactsReflectEnvironmentValues() {
        val environment =
            SampleEnvironment(
                profile = "operator",
                toriiUrl = "https://torii.example",
                toriiAccountsPath = "/tmp/accounts.json",
                toriiLogPath = "/tmp/torii.log",
                toriiMetricsPath = "/tmp/torii.prom",
                sorafsScoreboardPath = "/tmp/scoreboard.json",
                sorafsSummaryPath = "/tmp/summary.json",
                sorafsScoreboardSha256 = "scoreboard-sha",
                sorafsSummarySha256 = "summary-sha",
                sorafsReceiptsPath = "/tmp/receipts.json",
                telemetryLogPath = "/tmp/telemetry.log",
                handoffEndpoint = "https://handoff.example",
                handoffInboxPath = "/tmp/handoff"
            )

        val artifacts = environment.harnessArtifacts()

        assertEquals("/tmp/accounts.json", artifacts.toriiAccountsPath)
        assertEquals("/tmp/torii.log", artifacts.toriiLogPath)
        assertEquals("/tmp/torii.prom", artifacts.toriiMetricsPath)
        assertEquals("/tmp/scoreboard.json", artifacts.sorafsScoreboardPath)
        assertEquals("/tmp/summary.json", artifacts.sorafsSummaryPath)
        assertEquals("scoreboard-sha", artifacts.sorafsScoreboardSha256)
        assertEquals("summary-sha", artifacts.sorafsSummarySha256)
        assertEquals("/tmp/receipts.json", artifacts.sorafsReceiptsPath)
        assertEquals("/tmp/telemetry.log", artifacts.telemetryLogPath)
        assertEquals("https://handoff.example", artifacts.handoffEndpoint)
        assertEquals("/tmp/handoff", artifacts.handoffInboxPath)
    }

    @Test
    fun missingMockHarnessArtifactsHighlightsGaps() {
        val artifacts =
            HarnessArtifacts(
                toriiAccountsPath = null,
                toriiLogPath = null,
                toriiMetricsPath = null,
                sorafsScoreboardPath = null,
                sorafsSummaryPath = "/tmp/summary.json",
                sorafsScoreboardSha256 = null,
                sorafsSummarySha256 = null,
                sorafsReceiptsPath = null,
                telemetryLogPath = "",
                handoffEndpoint = null,
                handoffInboxPath = null
            )

        val missing = artifacts.missingMockHarnessArtifacts()

        assertTrue(missing.contains(HarnessArtifactKind.SORAFS_SCOREBOARD))
        assertFalse(missing.contains(HarnessArtifactKind.SORAFS_SUMMARY))
        assertTrue(missing.contains(HarnessArtifactKind.SORAFS_RECEIPTS))
        assertTrue(missing.contains(HarnessArtifactKind.TELEMETRY_LOG))
        assertFalse(artifacts.hasMockHarnessBundle())
    }
}
