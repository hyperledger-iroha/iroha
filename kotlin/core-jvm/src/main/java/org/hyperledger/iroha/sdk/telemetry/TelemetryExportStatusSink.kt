package org.hyperledger.iroha.sdk.telemetry

import org.hyperledger.iroha.sdk.client.ClientResponse

private const val SIGNAL_ID = "android.telemetry.export.status"

/**
 * `TelemetrySink` wrapper that emits `android.telemetry.export.status` for every
 * delegate invocation. The wrapper records `status="ok"` when a delegate call succeeds and
 * `status="error"` when the delegate throws.
 */
class TelemetryExportStatusSink private constructor(
    private val delegate: TelemetrySink,
    private val exporterName: String,
) : TelemetrySink {

    override fun onRequest(record: TelemetryRecord) {
        invokeWithStatus { delegate.onRequest(record) }
    }

    override fun onResponse(record: TelemetryRecord, response: ClientResponse) {
        invokeWithStatus { delegate.onResponse(record, response) }
    }

    override fun onFailure(record: TelemetryRecord, error: Throwable) {
        invokeWithStatus { delegate.onFailure(record, error) }
    }

    override fun emitSignal(signalId: String, fields: Map<String, Any>) {
        if (SIGNAL_ID == signalId) {
            delegate.emitSignal(signalId, fields)
            return
        }
        invokeWithStatus { delegate.emitSignal(signalId, fields) }
    }

    private fun invokeWithStatus(action: Runnable) {
        try {
            action.run()
            emitStatus("ok")
        } catch (ex: RuntimeException) {
            emitStatus("error")
            throw ex
        }
    }

    private fun emitStatus(status: String) {
        try {
            delegate.emitSignal(
                SIGNAL_ID,
                mapOf(
                    "exporter" to exporterName,
                    "status" to status,
                ),
            )
        } catch (_: RuntimeException) {
            // Best-effort metrics; swallow exporter failures to avoid infinite recursion.
        }
    }

    companion object {
        @JvmStatic
        fun wrap(delegate: TelemetrySink?, exporterName: String?): TelemetrySink? {
            if (delegate == null) return null
            if (delegate is TelemetryExportStatusSink) return delegate
            val resolvedName = resolveExporterName(delegate, exporterName)
            return TelemetryExportStatusSink(delegate, resolvedName)
        }

        @JvmStatic
        fun unwrap(sink: TelemetrySink?): TelemetrySink? =
            if (sink is TelemetryExportStatusSink) sink.delegate else sink

        @JvmStatic
        fun exporterName(sink: TelemetrySink): String? =
            if (sink is TelemetryExportStatusSink) sink.exporterName else null

        private fun resolveExporterName(delegate: TelemetrySink, exporterName: String?): String {
            val candidate = exporterName?.trim() ?: ""
            if (candidate.isNotEmpty()) return candidate
            val simpleName = delegate.javaClass.simpleName
            return if (simpleName.isNullOrEmpty()) "android_sdk" else simpleName
        }
    }
}
