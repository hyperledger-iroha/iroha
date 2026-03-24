package org.hyperledger.iroha.sdk.telemetry

import java.time.Duration

/** Emits `android.telemetry.chaos.scenario` signals during chaos rehearsals. */
class ChaosScenarioLogger private constructor(
    private val sink: TelemetrySink?,
    private val deviceProfileProvider: DeviceProfileProvider,
) {

    /**
     * Records a chaos scenario outcome.
     *
     * @param scenarioId identifier of the executed scenario
     * @param outcome human-readable outcome (e.g., "success", "failure")
     * @param duration duration spent executing the scenario
     */
    fun recordScenario(scenarioId: String?, outcome: String?, duration: Duration?) {
        if (sink == null) return
        val id = normalise(scenarioId, "unknown")
        val result = normalise(outcome, "unknown")
        val durationMs = if (duration == null) 0L else maxOf(0L, duration.toMillis())
        val deviceProfile =
            deviceProfileProvider.snapshot().map(DeviceProfile::bucket).orElse("unknown")
        sink.emitSignal(
            "android.telemetry.chaos.scenario",
            mapOf(
                "scenario_id" to id,
                "outcome" to result,
                "duration_ms" to durationMs,
                "device_profile" to deviceProfile,
            ),
        )
    }

    companion object {
        private val NOOP = ChaosScenarioLogger(null, DeviceProfileProvider.disabled())

        /** Returns a no-op logger. */
        @JvmStatic
        fun noop(): ChaosScenarioLogger = NOOP

        /** Creates a logger backed by the supplied sink. */
        @JvmStatic
        fun from(
            options: TelemetryOptions?,
            sink: TelemetrySink?,
            provider: DeviceProfileProvider?,
        ): ChaosScenarioLogger {
            if (options == null || sink == null || !options.enabled) return noop()
            val safeProvider = provider ?: DeviceProfileProvider.disabled()
            return ChaosScenarioLogger(sink, safeProvider)
        }

        private fun normalise(value: String?, fallback: String): String {
            if (value == null) return fallback
            val trimmed = value.trim()
            return if (trimmed.isEmpty()) fallback else trimmed.lowercase()
        }
    }
}
