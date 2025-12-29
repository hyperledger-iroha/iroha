package org.hyperledger.iroha.android.telemetry;

import java.util.Map;
import java.util.Objects;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.ClientResponse;

/** Tests covering {@link CrashTelemetryHandler}. */
public final class CrashTelemetryHandlerTests {

  private CrashTelemetryHandlerTests() {}

  public static void main(final String[] args) throws Exception {
    emitsCaptureEventAndDelegates();
    recordUploadForwardsToReporter();
    defaultMetadataProviderEmitsDeterministicFields();
    System.out.println("[IrohaAndroid] Crash telemetry handler tests passed.");
  }

  private static TelemetryOptions telemetryOptions() {
    return TelemetryOptions.builder()
        .setTelemetryRedaction(TelemetryOptions.Redaction.disabled())
        .build();
  }

  private static void emitsCaptureEventAndDelegates() {
    final RecordingSink sink = new RecordingSink();
    final TelemetryOptions options = telemetryOptions();
    final CrashTelemetryReporter reporter = new CrashTelemetryReporter(options, sink);
    final AtomicReference<Thread> delegatedThread = new AtomicReference<>();
    final AtomicReference<Throwable> delegatedThrowable = new AtomicReference<>();
    final Thread.UncaughtExceptionHandler delegate =
        (thread, error) -> {
          delegatedThread.set(thread);
          delegatedThrowable.set(error);
        };
    final CrashTelemetryHandler handler =
        new CrashTelemetryHandler(
            reporter,
            (thread, error) ->
                new CrashTelemetryHandler.Metadata(
                    "capture-id", "SIGABRT", "foreground", true, "anr_5s"),
            delegate);
    final Thread crashThread = new Thread(() -> {}, "demo-thread");
    final Throwable throwable = new IllegalStateException("boom");

    handler.uncaughtException(crashThread, throwable);

    final RecordingSink.SignalEvent capture =
        sink.findLast("android.crash.report.capture");
    Objects.requireNonNull(capture, "capture signal missing");
    final Map<String, Object> fields = capture.fields();
    assert "capture-id".equals(fields.get("crash_id")) : "crash id mismatch";
    assert "SIGABRT".equals(fields.get("signal")) : "signal mismatch";
    assert "foreground".equals(fields.get("process_state")) : "state mismatch";
    assert Boolean.TRUE.equals(fields.get("has_native_trace")) : "native flag mismatch";
    assert "anr_5s".equals(fields.get("anr_watchdog_bucket")) : "bucket mismatch";
    assert delegatedThread.get() == crashThread : "delegate thread mismatch";
    assert delegatedThrowable.get() == throwable : "delegate throwable mismatch";
  }

  private static void recordUploadForwardsToReporter() {
    final RecordingSink sink = new RecordingSink();
    final CrashTelemetryHandler handler =
        new CrashTelemetryHandler(
            new CrashTelemetryReporter(telemetryOptions(), sink),
            CrashTelemetryHandler.defaultMetadataProvider(),
            null);

    handler.recordUpload("crash-123", "sorafs", "success", 1);

    final RecordingSink.SignalEvent upload =
        sink.findLast("android.crash.report.upload");
    Objects.requireNonNull(upload, "upload signal missing");
    final Map<String, Object> fields = upload.fields();
    assert "crash-123".equals(fields.get("crash_id")) : "upload crash id mismatch";
    assert "sorafs".equals(fields.get("backend")) : "backend mismatch";
    assert "success".equals(fields.get("status")) : "status mismatch";
    assert Integer.valueOf(1).equals(fields.get("retry_count")) : "retry mismatch";
  }

  private static void defaultMetadataProviderEmitsDeterministicFields() {
    final CrashTelemetryHandler.MetadataProvider provider =
        CrashTelemetryHandler.defaultMetadataProvider();
    final Thread thread = new Thread(() -> {}, "foreground-thread");
    final Throwable throwable = new NullPointerException();
    throwable.setStackTrace(
        new StackTraceElement[] {
          new StackTraceElement("org.example.Crash", "panic", "Crash.java", 42)
        });

    final CrashTelemetryHandler.Metadata metadata = provider.capture(thread, throwable);

    assert metadata.crashId().contains("NullPointerException@org.example.Crash#panic:42")
        : "crash id must include top frame";
    assert metadata.crashId().contains("foreground-thread")
        : "crash id should include thread name";
    assert "NullPointerException".equals(metadata.signal()) : "signal mismatch";
    assert "foreground".equals(metadata.processState()) : "process state mismatch";
    assert !metadata.hasNativeTrace() : "native flag should be false";
  }

  private static final class RecordingSink implements TelemetrySink {
    private final java.util.List<SignalEvent> events = new java.util.ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

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
