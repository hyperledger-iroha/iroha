package org.hyperledger.iroha.android.telemetry;

import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.ClientResponse;

/**
 * {@link TelemetrySink} wrapper that emits {@code android.telemetry.export.status} for every
 * delegate invocation. The wrapper records {@code status="ok"} when a delegate call succeeds and
 * {@code status="error"} when the delegate throws.
 */
public final class TelemetryExportStatusSink implements TelemetrySink {

  private static final String SIGNAL_ID = "android.telemetry.export.status";

  private final TelemetrySink delegate;
  private final String exporterName;

  private TelemetryExportStatusSink(final TelemetrySink delegate, final String exporterName) {
    this.delegate = delegate;
    this.exporterName = exporterName;
  }

  public static TelemetrySink wrap(final TelemetrySink delegate, final String exporterName) {
    if (delegate == null) {
      return null;
    }
    if (delegate instanceof TelemetryExportStatusSink) {
      return delegate;
    }
    final String resolvedName = resolveExporterName(delegate, exporterName);
    return new TelemetryExportStatusSink(delegate, resolvedName);
  }

  public static TelemetrySink unwrap(final TelemetrySink sink) {
    if (sink instanceof TelemetryExportStatusSink statusSink) {
      return statusSink.delegate;
    }
    return sink;
  }

  static String exporterName(final TelemetrySink sink) {
    if (sink instanceof TelemetryExportStatusSink statusSink) {
      return statusSink.exporterName;
    }
    return null;
  }

  private static String resolveExporterName(
      final TelemetrySink delegate, final String exporterName) {
    final String candidate = exporterName == null ? "" : exporterName.trim();
    if (!candidate.isEmpty()) {
      return candidate;
    }
    final String simpleName = delegate.getClass().getSimpleName();
    return simpleName == null || simpleName.isEmpty() ? "android_sdk" : simpleName;
  }

  @Override
  public void onRequest(final TelemetryRecord record) {
    invokeWithStatus(() -> delegate.onRequest(record));
  }

  @Override
  public void onResponse(final TelemetryRecord record, final ClientResponse response) {
    invokeWithStatus(() -> delegate.onResponse(record, response));
  }

  @Override
  public void onFailure(final TelemetryRecord record, final Throwable error) {
    invokeWithStatus(() -> delegate.onFailure(record, error));
  }

  @Override
  public void emitSignal(final String signalId, final Map<String, Object> fields) {
    if (Objects.equals(SIGNAL_ID, signalId)) {
      delegate.emitSignal(signalId, fields);
      return;
    }
    invokeWithStatus(() -> delegate.emitSignal(signalId, fields));
  }

  private void invokeWithStatus(final Runnable runnable) {
    try {
      runnable.run();
      emitStatus("ok");
    } catch (final RuntimeException ex) {
      emitStatus("error");
      throw ex;
    }
  }

  private void emitStatus(final String status) {
    try {
      delegate.emitSignal(
          SIGNAL_ID,
          Map.of(
              "exporter", exporterName,
              "status", status));
    } catch (final RuntimeException ignored) {
      // Best-effort metrics; swallow exporter failures to avoid infinite recursion.
    }
  }
}
