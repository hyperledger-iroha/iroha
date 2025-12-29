package org.hyperledger.iroha.android.telemetry;

import java.util.Objects;

/**
 * {@link Thread.UncaughtExceptionHandler} that emits {@code android.crash.report.capture} signals
 * whenever an uncaught exception escapes the application.
 *
 * <p>Install the handler after configuring telemetry:
 *
 * <pre>{@code
 * ClientConfig config =
 *     ClientConfig.builder()
 *         .setTelemetryOptions(telemetryOptions)
 *         .setTelemetrySink(telemetrySink)
 *         .enableCrashTelemetryHandler()
 *         .build();
 * }</pre>
 */
public final class CrashTelemetryHandler implements Thread.UncaughtExceptionHandler {

  private final CrashTelemetryReporter reporter;
  private final MetadataProvider metadataProvider;
  private final Thread.UncaughtExceptionHandler delegate;

  public CrashTelemetryHandler(
      final CrashTelemetryReporter reporter,
      final MetadataProvider metadataProvider,
      final Thread.UncaughtExceptionHandler delegate) {
    this.reporter = Objects.requireNonNull(reporter, "reporter");
    this.metadataProvider = Objects.requireNonNull(metadataProvider, "metadataProvider");
    this.delegate = delegate;
  }

  /**
     * Installs the crash telemetry handler as the process-wide default handler. Returns the
   * installed handler so callers can emit upload telemetry or uninstall the handler if needed.
   */
  public static CrashTelemetryHandler install(
      final TelemetryOptions options,
      final TelemetrySink sink,
      final MetadataProvider metadataProvider) {
    Objects.requireNonNull(options, "options");
    Objects.requireNonNull(sink, "sink");
    Objects.requireNonNull(metadataProvider, "metadataProvider");
    final Thread.UncaughtExceptionHandler previous =
        Thread.getDefaultUncaughtExceptionHandler();
    final CrashTelemetryHandler handler =
        new CrashTelemetryHandler(
            new CrashTelemetryReporter(options, sink), metadataProvider, previous);
    Thread.setDefaultUncaughtExceptionHandler(handler);
    return handler;
  }

  /** Returns a metadata provider that derives crash identifiers from the thread and throwable. */
  public static MetadataProvider defaultMetadataProvider() {
    return DefaultMetadataProvider.INSTANCE;
  }

  @Override
  public void uncaughtException(final Thread thread, final Throwable error) {
    final Metadata metadata = metadataProvider.capture(thread, error);
    if (metadata != null) {
      reporter.recordCapture(
          metadata.crashId(),
          metadata.signal(),
          metadata.processState(),
          metadata.hasNativeTrace(),
          metadata.watchdogBucket());
    }
    if (delegate != null) {
      delegate.uncaughtException(thread, error);
    }
  }

  /**
   * Records an upload result (convenience wrapper around {@link CrashTelemetryReporter} for crash
   * pipelines that share the handler instance).
   */
  public void recordUpload(
      final String crashId, final String backend, final String status, final int retryCount) {
    reporter.recordUpload(crashId, backend, status, retryCount);
  }

  /** Returns the underlying reporter for advanced integrations. */
  public CrashTelemetryReporter reporter() {
    return reporter;
  }

  /** Supplies metadata describing a captured crash. */
  public interface MetadataProvider {
    Metadata capture(Thread thread, Throwable error);
  }

  /** Value object describing the recorded crash. */
  public static final class Metadata {
    private final String crashId;
    private final String signal;
    private final String processState;
    private final boolean hasNativeTrace;
    private final String watchdogBucket;

    public Metadata(
        final String crashId,
        final String signal,
        final String processState,
        final boolean hasNativeTrace,
        final String watchdogBucket) {
      this.crashId = nullToEmpty(crashId);
      this.signal = nullToEmpty(signal);
      this.processState = nullToEmpty(processState);
      this.hasNativeTrace = hasNativeTrace;
      this.watchdogBucket = nullToEmpty(watchdogBucket);
    }

    public String crashId() {
      return crashId;
    }

    public String signal() {
      return signal;
    }

    public String processState() {
      return processState;
    }

    public boolean hasNativeTrace() {
      return hasNativeTrace;
    }

    public String watchdogBucket() {
      return watchdogBucket;
    }
  }

  private static final class DefaultMetadataProvider implements MetadataProvider {
    private static final DefaultMetadataProvider INSTANCE = new DefaultMetadataProvider();

    @Override
    public Metadata capture(final Thread thread, final Throwable error) {
      final String crashId = buildCrashId(thread, error);
      final String signal =
          error != null ? error.getClass().getSimpleName() : "UnknownThrowable";
      final String processState = resolveProcessState(thread);
      final boolean hasNativeTrace = hasNativeFrames(error);
      return new Metadata(crashId, signal, processState, hasNativeTrace, "");
    }

    private static String buildCrashId(final Thread thread, final Throwable error) {
      final StringBuilder builder = new StringBuilder();
      if (error != null) {
        builder.append(error.getClass().getName());
        final StackTraceElement[] elements = error.getStackTrace();
        if (elements.length > 0) {
          final StackTraceElement top = elements[0];
          builder.append('@').append(top.getClassName()).append('#').append(top.getMethodName());
          if (top.getLineNumber() >= 0) {
            builder.append(':').append(top.getLineNumber());
          }
        }
      } else {
        builder.append("unknown");
      }
      if (thread != null) {
        builder.append('|').append(thread.getName());
      }
      return builder.toString();
    }

    private static String resolveProcessState(final Thread thread) {
      if (thread == null) {
        return "unknown";
      }
      return thread.isDaemon() ? "daemon" : "foreground";
    }

    private static boolean hasNativeFrames(final Throwable error) {
      if (error == null) {
        return false;
      }
      for (final StackTraceElement element : error.getStackTrace()) {
        if (element.isNativeMethod()) {
          return true;
        }
      }
      return false;
    }
  }

  private static String nullToEmpty(final String value) {
    return value == null ? "" : value;
  }
}
