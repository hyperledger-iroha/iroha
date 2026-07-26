package org.hyperledger.iroha.sdk.telemetry

/**
 * `Thread.UncaughtExceptionHandler` that emits `android.crash.report.capture` signals
 * whenever an uncaught exception escapes the application.
 *
 * Install the handler after configuring telemetry:
 *
 * ```
 * ClientConfig config =
 *     ClientConfig.builder()
 *         .setTelemetryOptions(telemetryOptions)
 *         .setTelemetrySink(telemetrySink)
 *         .enableCrashTelemetryHandler()
 *         .build();
 * ```
 */
class CrashTelemetryHandler(
    private val reporter: CrashTelemetryReporter,
    private val metadataProvider: MetadataProvider,
    private val delegate: Thread.UncaughtExceptionHandler?,
) : Thread.UncaughtExceptionHandler {

    override fun uncaughtException(thread: Thread, error: Throwable) {
        val metadata = metadataProvider.capture(thread, error)
        if (metadata != null) {
            reporter.recordCapture(
                metadata.crashId,
                metadata.signal,
                metadata.processState,
                metadata.hasNativeTrace,
                metadata.watchdogBucket,
            )
        }
        delegate?.uncaughtException(thread, error)
    }

    /**
     * Records an upload result (convenience wrapper around `CrashTelemetryReporter` for crash
     * pipelines that share the handler instance).
     */
    fun recordUpload(crashId: String, backend: String, status: String, retryCount: Int) {
        reporter.recordUpload(crashId, backend, status, retryCount)
    }

    /** Returns the underlying reporter for advanced integrations. */
    fun reporter(): CrashTelemetryReporter = reporter

    companion object {
        /**
         * Installs the crash telemetry handler as the process-wide default handler. Returns the
         * installed handler so callers can emit upload telemetry or uninstall the handler if needed.
         */
        @JvmStatic
        fun install(
            options: TelemetryOptions,
            sink: TelemetrySink,
            metadataProvider: MetadataProvider,
        ): CrashTelemetryHandler {
            val previous = Thread.getDefaultUncaughtExceptionHandler()
            val handler = CrashTelemetryHandler(
                CrashTelemetryReporter(options, sink),
                metadataProvider,
                previous,
            )
            Thread.setDefaultUncaughtExceptionHandler(handler)
            return handler
        }

        /** Returns a metadata provider that derives crash identifiers from the thread and throwable. */
        @JvmStatic
        fun defaultMetadataProvider(): MetadataProvider = DefaultMetadataProvider
    }
}

private object DefaultMetadataProvider : MetadataProvider {

    override fun capture(thread: Thread, error: Throwable): CrashMetadata {
        val crashId = buildCrashId(thread, error)
        val signal = error.javaClass.simpleName
        val processState = resolveProcessState(thread)
        val hasNativeTrace = hasNativeFrames(error)
        return CrashMetadata(crashId, signal, processState, hasNativeTrace, "")
    }

    private fun buildCrashId(thread: Thread, error: Throwable?): String {
        val builder = StringBuilder()
        if (error != null) {
            builder.append(error.javaClass.name)
            val elements = error.stackTrace
            if (elements.isNotEmpty()) {
                val top = elements[0]
                builder.append('@').append(top.className).append('#').append(top.methodName)
                if (top.lineNumber >= 0) {
                    builder.append(':').append(top.lineNumber)
                }
            }
        } else {
            builder.append("unknown")
        }
        builder.append('|').append(thread.name)
        return builder.toString()
    }

    private fun resolveProcessState(thread: Thread): String =
        if (thread.isDaemon) "daemon" else "foreground"

    private fun hasNativeFrames(error: Throwable?): Boolean {
        if (error == null) return false
        return error.stackTrace.any { it.isNativeMethod }
    }
}
