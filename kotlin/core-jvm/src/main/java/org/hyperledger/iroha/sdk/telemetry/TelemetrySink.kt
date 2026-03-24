package org.hyperledger.iroha.sdk.telemetry

import org.hyperledger.iroha.sdk.client.ClientResponse

/**
 * Sink that consumes telemetry events emitted by `ClientObserver`
 * implementations.
 *
 * Mobile apps can supply an implementation backed by their preferred observability stack (e.g.,
 * OpenTelemetry, structured logs, or Norito exporters).
 */
interface TelemetrySink {

    fun onRequest(record: TelemetryRecord)

    fun onResponse(record: TelemetryRecord, response: ClientResponse)

    fun onFailure(record: TelemetryRecord, error: Throwable)

    /**
     * Emits structured telemetry signals that are not tied to HTTP lifecycle callbacks (e.g., queue
     * depth gauges). Default implementation is a no-op so existing sinks remain compatible.
     */
    fun emitSignal(signalId: String, fields: Map<String, Any>) {
        // Intentionally empty.
    }
}
