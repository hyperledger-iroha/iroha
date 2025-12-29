package org.hyperledger.iroha.android.telemetry;

import java.util.HashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/**
 * Emits crash telemetry signals required by the AND7 roadmap item. Callers can wire it into their
 * crash handlers to record capture/upload events while honouring telemetry redaction policy.
 */
public final class CrashTelemetryReporter {

  private static final String CAPTURE_SIGNAL = "android.crash.report.capture";
  private static final String UPLOAD_SIGNAL = "android.crash.report.upload";
  private static final String REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure";

  private final TelemetryOptions options;
  private final TelemetrySink sink;

  public CrashTelemetryReporter(final TelemetryOptions options, final TelemetrySink sink) {
    this.options = Objects.requireNonNull(options, "options");
    this.sink = Objects.requireNonNull(sink, "sink");
  }

  /**
   * Records a crash capture event. {@code crashId} is hashed using the shared telemetry redaction
   * salt when enabled, falling back to the trimmed identifier when redaction is disabled (e.g.,
   * local builds).
   */
  public void recordCapture(
      final String crashId,
      final String signal,
      final String processState,
      final boolean hasNativeTrace,
      final String watchdogBucket) {
    if (!options.enabled()) {
      return;
    }
    final Map<String, Object> fields = new HashMap<>();
    addCrashId(fields, crashId, CAPTURE_SIGNAL);
    fields.put("signal", nullToEmpty(signal));
    fields.put("process_state", nullToEmpty(processState));
    fields.put("has_native_trace", hasNativeTrace);
    fields.put("anr_watchdog_bucket", nullToEmpty(watchdogBucket));
    sink.emitSignal(CAPTURE_SIGNAL, Map.copyOf(fields));
  }

  /**
   * Records a crash upload counter. Mirrors the redaction and failure handling performed by {@link
   * #recordCapture(String, String, String, boolean, String)} and tags the event with backend/status
   * metadata.
   */
  public void recordUpload(
      final String crashId, final String backend, final String status, final int retryCount) {
    if (!options.enabled()) {
      return;
    }
    final Map<String, Object> fields = new HashMap<>();
    addCrashId(fields, crashId, UPLOAD_SIGNAL);
    fields.put("backend", nullToEmpty(backend));
    fields.put("status", nullToEmpty(status));
    fields.put("retry_count", retryCount);
    sink.emitSignal(UPLOAD_SIGNAL, Map.copyOf(fields));
  }

  private void addCrashId(
      final Map<String, Object> fields, final String crashId, final String signalId) {
    final String trimmed = crashId == null ? "" : crashId.trim();
    if (trimmed.isEmpty()) {
      emitRedactionFailure(signalId, "blank_crash_id");
      return;
    }
    final TelemetryOptions.Redaction redaction = options.redaction();
    final Optional<String> hashed = redaction.hashIdentifier(trimmed);
    if (hashed.isPresent()) {
      fields.put("crash_id", hashed.get());
      return;
    }
    if (!redaction.enabled()) {
      fields.put("crash_id", trimmed);
      return;
    }
    emitRedactionFailure(signalId, "hash_failed");
  }

  private void emitRedactionFailure(final String signalId, final String reason) {
    sink.emitSignal(
        REDACTION_FAILURE_SIGNAL,
        Map.of(
            "signal_id", signalId,
            "reason", reason));
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }
}
