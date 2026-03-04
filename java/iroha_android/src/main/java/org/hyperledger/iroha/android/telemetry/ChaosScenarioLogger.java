package org.hyperledger.iroha.android.telemetry;

import java.time.Duration;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/** Emits {@code android.telemetry.chaos.scenario} signals during chaos rehearsals. */
public final class ChaosScenarioLogger {

  private static final ChaosScenarioLogger NOOP =
      new ChaosScenarioLogger(null, DeviceProfileProvider.disabled());

  private final TelemetrySink sink;
  private final DeviceProfileProvider deviceProfileProvider;

  private ChaosScenarioLogger(final TelemetrySink sink, final DeviceProfileProvider provider) {
    this.sink = sink;
    this.deviceProfileProvider = provider;
  }

  /** Returns a no-op logger. */
  public static ChaosScenarioLogger noop() {
    return NOOP;
  }

  /** Creates a logger backed by the supplied sink. */
  public static ChaosScenarioLogger from(
      final TelemetryOptions options,
      final TelemetrySink sink,
      final DeviceProfileProvider provider) {
    if (options == null || sink == null || !options.enabled()) {
      return noop();
    }
    final DeviceProfileProvider safeProvider =
        provider == null ? DeviceProfileProvider.disabled() : provider;
    return new ChaosScenarioLogger(sink, safeProvider);
  }

  /**
   * Records a chaos scenario outcome.
   *
   * @param scenarioId identifier of the executed scenario
   * @param outcome human-readable outcome (e.g., "success", "failure")
   * @param duration duration spent executing the scenario
   */
  public void recordScenario(
      final String scenarioId, final String outcome, final Duration duration) {
    if (sink == null) {
      return;
    }
    final String id = normalise(scenarioId, "unknown");
    final String result = normalise(outcome, "unknown");
    final long durationMs = duration == null ? 0L : Math.max(0L, duration.toMillis());
    final String deviceProfile =
        deviceProfileProvider.snapshot().map(DeviceProfile::bucket).orElse("unknown");
    sink.emitSignal(
        "android.telemetry.chaos.scenario",
        Map.of(
            "scenario_id", id,
            "outcome", result,
            "duration_ms", durationMs,
            "device_profile", deviceProfile));
  }

  private static String normalise(final String value, final String fallback) {
    if (value == null) {
      return fallback;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? fallback : trimmed.toLowerCase(Locale.ROOT);
  }
}
