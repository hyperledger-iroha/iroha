package org.hyperledger.iroha.sdk.telemetry

private const val CAPTURE_SIGNAL = "android.crash.report.capture"
private const val UPLOAD_SIGNAL = "android.crash.report.upload"
private const val REDACTION_FAILURE_SIGNAL = "android.telemetry.redaction.failure"

/**
 * Emits crash telemetry signals required by the AND7 roadmap item. Callers can wire it into their
 * crash handlers to record capture/upload events while honouring telemetry redaction policy.
 */
class CrashTelemetryReporter(
    private val options: TelemetryOptions,
    private val sink: TelemetrySink,
) {

    /**
     * Records a crash capture event. `crashId` is hashed using the shared telemetry redaction
     * salt when enabled, falling back to the trimmed identifier when redaction is disabled (e.g.,
     * local builds).
     */
    fun recordCapture(
        crashId: String?,
        signal: String?,
        processState: String?,
        hasNativeTrace: Boolean,
        watchdogBucket: String?,
    ) {
        if (!options.enabled) return
        val fields = HashMap<String, Any>()
        addCrashId(fields, crashId, CAPTURE_SIGNAL)
        fields["signal"] = nullToEmpty(signal)
        fields["process_state"] = nullToEmpty(processState)
        fields["has_native_trace"] = hasNativeTrace
        fields["anr_watchdog_bucket"] = nullToEmpty(watchdogBucket)
        sink.emitSignal(CAPTURE_SIGNAL, fields.toMap())
    }

    /**
     * Records a crash upload counter. Mirrors the redaction and failure handling performed by
     * `recordCapture` and tags the event with backend/status metadata.
     */
    fun recordUpload(
        crashId: String?,
        backend: String?,
        status: String?,
        retryCount: Int,
    ) {
        if (!options.enabled) return
        val fields = HashMap<String, Any>()
        addCrashId(fields, crashId, UPLOAD_SIGNAL)
        fields["backend"] = nullToEmpty(backend)
        fields["status"] = nullToEmpty(status)
        fields["retry_count"] = retryCount
        sink.emitSignal(UPLOAD_SIGNAL, fields.toMap())
    }

    private fun addCrashId(fields: MutableMap<String, Any>, crashId: String?, signalId: String) {
        val trimmed = crashId?.trim() ?: ""
        if (trimmed.isEmpty()) {
            emitRedactionFailure(signalId, "blank_crash_id")
            return
        }
        val redaction = options.redaction
        val hashed = redaction.hashIdentifier(trimmed)
        if (hashed.isPresent) {
            fields["crash_id"] = hashed.get()
            return
        }
        if (!redaction.enabled) {
            fields["crash_id"] = trimmed
            return
        }
        emitRedactionFailure(signalId, "hash_failed")
    }

    private fun emitRedactionFailure(signalId: String, reason: String) {
        sink.emitSignal(
            REDACTION_FAILURE_SIGNAL,
            mapOf(
                "signal_id" to signalId,
                "reason" to reason,
            ),
        )
    }
}

private fun nullToEmpty(value: String?): String = value ?: ""
