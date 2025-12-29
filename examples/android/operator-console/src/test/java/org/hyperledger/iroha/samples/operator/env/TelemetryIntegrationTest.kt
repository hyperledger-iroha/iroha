package org.hyperledger.iroha.samples.operator.env

import java.nio.file.Files
import java.time.Clock
import java.time.Instant
import java.time.ZoneOffset
import org.hyperledger.iroha.android.client.ClientResponse
import org.hyperledger.iroha.android.telemetry.TelemetryRecord
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TelemetryIntegrationTest {

    @Test
    fun `writes request response and signal entries`() {
        val tempFile = Files.createTempFile("telemetry", ".log")
        val sink =
            TelemetryLogSink(
                tempFile.toString(),
                exporter = "operator-console",
                rotationId = "rotation-sample",
                clock = Clock.fixed(Instant.parse("2027-04-01T00:00:00Z"), ZoneOffset.UTC)
            )
        val record =
            TelemetryRecord("ab12", "v2027", "/healthz", "GET").withOutcome(42L, 200, null)

        sink.onRequest(record)
        sink.onResponse(record, ClientResponse(200, "ok".toByteArray()))
        sink.onFailure(record, IllegalStateException("boom"))
        sink.emitSignal("android.telemetry.demo", mapOf("counter" to 3, "label" to "demo"))

        val lines = Files.readAllLines(tempFile)
        assertTrue(lines.any { it.contains("\"event\":\"request\"") && it.contains("/healthz") })
        assertTrue(lines.any { it.contains("\"event\":\"response\"") && it.contains("\"status_code\":200") })
        assertTrue(lines.any { it.contains("\"event\":\"failure\"") && it.contains("boom") })
        assertTrue(lines.any { it.contains("\"event\":\"signal\"") && it.contains("\"signal_id\":\"android.telemetry.demo\"") })
    }

    @Test
    fun `invalid redaction falls back to disabled telemetry`() {
        val enabledConfig =
            TelemetryConfig(
                enabled = true,
                logPath = "/tmp/telemetry.log",
                saltHex = "aa11bb22",
                saltVersion = "v1",
                rotationId = "rot",
                exporter = "sample"
            )
        assertTrue(enabledConfig.toTelemetryOptions().enabled())

        val invalidConfig = enabledConfig.copy(saltHex = "xyz")
        assertFalse(invalidConfig.toTelemetryOptions().enabled())
    }
}
