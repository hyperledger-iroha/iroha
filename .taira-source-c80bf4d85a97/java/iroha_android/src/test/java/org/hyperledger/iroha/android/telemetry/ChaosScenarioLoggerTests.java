package org.hyperledger.iroha.android.telemetry;

import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.Optional;

public final class ChaosScenarioLoggerTests {

  public static void main(final String[] args) {
    recordsScenarioEvents();
    System.out.println("[IrohaAndroid] Chaos scenario telemetry tests passed.");
  }

  private static void recordsScenarioEvents() {
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final ChaosScenarioLogger logger =
        ChaosScenarioLogger.from(
            telemetryOptions(), sink, () -> Optional.of(DeviceProfile.of("enterprise")));
    logger.recordScenario("ScenarioA", "Success", Duration.ofSeconds(3));

    final RecordingTelemetrySink.SignalEvent event =
        sink.lastEvent("android.telemetry.chaos.scenario");
    assert event != null;
    assert "scenarioa".equals(event.fields.get("scenario_id"));
    assert "success".equals(event.fields.get("outcome"));
    assert Long.valueOf(3000L).equals(event.fields.get("duration_ms"));
    assert "enterprise".equals(event.fields.get("device_profile"));
  }

  private static TelemetryOptions telemetryOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("0102030405")
                .setSaltVersion("2026Q2")
                .setRotationId("rot-telemetry")
                .build())
        .build();
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<SignalEvent> events = new java.util.ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final org.hyperledger.iroha.android.client.ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new SignalEvent(signalId, Map.copyOf(fields)));
    }

    SignalEvent lastEvent(final String signalId) {
      for (int i = events.size() - 1; i >= 0; --i) {
        final SignalEvent event = events.get(i);
        if (event.signalId.equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    private static final class SignalEvent {
      final String signalId;
      final Map<String, Object> fields;

      private SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }
    }
  }
}
