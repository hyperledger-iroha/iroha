package org.hyperledger.iroha.android.telemetry;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.ClientResponse;

/** Tests for {@link TelemetryExportStatusSink}. */
public final class TelemetryExportStatusSinkTests {

  private TelemetryExportStatusSinkTests() {}

  public static void main(final String[] args) throws Exception {
    emitsStatusOnSuccess();
    emitsStatusOnFailure();
    forwardsExistingStatusSignals();
    System.out.println("[IrohaAndroid] Telemetry export status sink tests passed.");
  }

  private static void emitsStatusOnSuccess() throws Exception {
    final RecordingSink delegate = new RecordingSink();
    final TelemetrySink sink = TelemetryExportStatusSink.wrap(delegate, "test_exporter");
    sink.onRequest(new TelemetryRecord(null, "none", "/", "GET"));
    sink.emitSignal("android.torii.http.request", Map.of("route", "/"));

    final RecordingSink.SignalEvent status = delegate.lastSignal();
    assert status != null : "status emission expected";
    assert "android.telemetry.export.status".equals(status.signalId)
        : "status signal id mismatch";
    assert "test_exporter".equals(status.fields.get("exporter"))
        : "exporter name mismatch";
    assert "ok".equals(status.fields.get("status")) : "expected ok status";
  }

  private static void emitsStatusOnFailure() {
    final RecordingSink delegate = new RecordingSink();
    delegate.failOnRequest = true;
    final TelemetrySink sink = TelemetryExportStatusSink.wrap(delegate, "otel");
    boolean threw = false;
    try {
      sink.onRequest(new TelemetryRecord(null, "none", "/", "POST"));
    } catch (final IllegalStateException ex) {
      threw = true;
    }
    assert threw : "delegate failure should propagate";
    final RecordingSink.SignalEvent status = delegate.lastSignal();
    assert status != null : "status emission expected";
    assert "error".equals(status.fields.get("status")) : "expected error status";
  }

  private static void forwardsExistingStatusSignals() {
    final RecordingSink delegate = new RecordingSink();
    final TelemetrySink sink = TelemetryExportStatusSink.wrap(delegate, "textfile");
    sink.emitSignal("android.telemetry.export.status", Map.of("exporter", "textfile", "status", "ok"));
    final RecordingSink.SignalEvent status = delegate.lastSignal();
    assert status != null : "status signal should pass through";
    assert "android.telemetry.export.status".equals(status.signalId)
        : "status signal id mismatch";
  }

  private static final class RecordingSink implements TelemetrySink {
    private final List<SignalEvent> signals = new ArrayList<>();
    private boolean failOnRequest;

    @Override
    public void onRequest(final TelemetryRecord record) {
      if (failOnRequest) {
        throw new IllegalStateException("failure");
      }
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      signals.add(new SignalEvent(signalId, Map.copyOf(fields)));
    }

    SignalEvent lastSignal() {
      if (signals.isEmpty()) {
        return null;
      }
      return signals.get(signals.size() - 1);
    }

    private static final class SignalEvent {
      private final String signalId;
      private final Map<String, Object> fields;

      private SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }
    }
  }
}
