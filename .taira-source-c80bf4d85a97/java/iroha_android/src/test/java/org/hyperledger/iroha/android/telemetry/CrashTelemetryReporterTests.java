package org.hyperledger.iroha.android.telemetry;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.ClientResponse;

/** Tests covering {@link CrashTelemetryReporter}. */
public final class CrashTelemetryReporterTests {

  private CrashTelemetryReporterTests() {}

  public static void main(final String[] args) throws Exception {
    emitsHashedCrashIdWhenRedactionEnabled();
    recordsRawCrashIdWhenRedactionDisabled();
    emitsRedactionFailureWhenCrashIdBlank();
    System.out.println("[IrohaAndroid] Crash telemetry reporter tests passed.");
  }

  private static TelemetryOptions hashEnabledOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(
            TelemetryOptions.Redaction.builder()
                .setSaltHex("01020304")
                .setSaltVersion("2026Q1")
                .setRotationId("rot-7")
                .build())
        .build();
  }

  private static TelemetryOptions redactionDisabledOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(TelemetryOptions.Redaction.disabled())
        .build();
  }

  private static void emitsHashedCrashIdWhenRedactionEnabled() {
    final TelemetryOptions options = hashEnabledOptions();
    final RecordingSink sink = new RecordingSink();
    final CrashTelemetryReporter reporter = new CrashTelemetryReporter(options, sink);

    reporter.recordCapture("Crash-42", "SIGABRT", "foreground", true, "anr_5s");

    final RecordingSink.SignalEvent capture =
        sink.findLast("android.crash.report.capture");
    Objects.requireNonNull(capture, "capture signal missing");
    final Map<String, Object> fields = capture.fields();
    assert "9959c0d64d9aa31c02ac4c772b2aa263c79dba5f66547d47b9f6fc24bdfe300f"
            .equals(fields.get("crash_id"))
        : "hashed crash id mismatch";
    assert "SIGABRT".equals(fields.get("signal")) : "signal mismatch";
    assert "foreground".equals(fields.get("process_state")) : "process state mismatch";
    assert Boolean.TRUE.equals(fields.get("has_native_trace")) : "native trace flag mismatch";
    assert "anr_5s".equals(fields.get("anr_watchdog_bucket"))
        : "watchdog bucket mismatch";
  }

  private static void recordsRawCrashIdWhenRedactionDisabled() {
    final TelemetryOptions options = redactionDisabledOptions();
    final RecordingSink sink = new RecordingSink();
    final CrashTelemetryReporter reporter = new CrashTelemetryReporter(options, sink);

    reporter.recordUpload("CRASH-99", "sorafs", "success", 2);

    final RecordingSink.SignalEvent upload =
        sink.findLast("android.crash.report.upload");
    Objects.requireNonNull(upload, "upload signal missing");
    final Map<String, Object> fields = upload.fields();
    assert "CRASH-99".equals(fields.get("crash_id")) : "raw crash id should be preserved";
    assert "sorafs".equals(fields.get("backend")) : "backend mismatch";
    assert "success".equals(fields.get("status")) : "status mismatch";
    assert Integer.valueOf(2).equals(fields.get("retry_count")) : "retry count mismatch";
    final RecordingSink.SignalEvent failure =
        sink.findLast("android.telemetry.redaction.failure");
    assert failure == null : "redaction failure not expected when disabled";
  }

  private static void emitsRedactionFailureWhenCrashIdBlank() {
    final TelemetryOptions options = hashEnabledOptions();
    final RecordingSink sink = new RecordingSink();
    final CrashTelemetryReporter reporter = new CrashTelemetryReporter(options, sink);

    reporter.recordUpload("   ", "gateway", "failure", 0);

    final RecordingSink.SignalEvent upload =
        sink.findLast("android.crash.report.upload");
    Objects.requireNonNull(upload, "upload signal missing");
    assert !upload.fields().containsKey("crash_id")
        : "blank crash id must be omitted";
    final RecordingSink.SignalEvent failure =
        sink.findLast("android.telemetry.redaction.failure");
    Objects.requireNonNull(failure, "redaction failure signal missing");
    assert "android.crash.report.upload".equals(failure.fields().get("signal_id"))
        : "signal id mismatch";
    assert "blank_crash_id".equals(failure.fields().get("reason"))
        : "reason mismatch";
  }

  private static final class RecordingSink implements TelemetrySink {
    private final List<SignalEvent> events = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {
      // Not used in these tests.
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      // Not used in these tests.
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      // Not used in these tests.
    }

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      events.add(new SignalEvent(signalId, Map.copyOf(fields)));
    }

    SignalEvent findLast(final String signalId) {
      for (int i = events.size() - 1; i >= 0; --i) {
        final SignalEvent event = events.get(i);
        if (event.signalId.equals(signalId)) {
          return event;
        }
      }
      return null;
    }

    private static final class SignalEvent {
      private final String signalId;
      private final Map<String, Object> fields;

      private SignalEvent(final String signalId, final Map<String, Object> fields) {
        this.signalId = signalId;
        this.fields = fields;
      }

      Map<String, Object> fields() {
        return fields;
      }
    }
  }
}
