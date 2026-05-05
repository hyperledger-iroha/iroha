package org.hyperledger.iroha.sdk.telemetry

import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.transport.TransportRequest

/**
 * [ClientObserver] that forwards request lifecycle events to a [TelemetrySink] using the
 * configured [TelemetryOptions] redaction policy.
 */
class TelemetryObserver(
    private val options: TelemetryOptions,
    private val sink: TelemetrySink,
) : ClientObserver {

    override fun onRequest(request: TransportRequest) {
        sink.onRequest(buildRecord(request))
    }

    override fun onResponse(request: TransportRequest, response: ClientResponse) {
        sink.onResponse(buildRecord(request), response)
    }

    override fun onFailure(request: TransportRequest, error: Throwable) {
        sink.onFailure(buildRecord(request), error)
    }

    private fun buildRecord(request: TransportRequest): TelemetryRecord {
        val authority = request.uri?.authority?.trim() ?: ""
        val authorityHash = if (options.redaction.enabled) {
            options.redaction.hashAuthority(authority).orElse(null)
        } else null
        return TelemetryRecord(
            authorityHash = authorityHash,
            saltVersion = if (options.redaction.enabled) options.redaction.saltVersion else null,
            route = request.uri?.rawPath,
            method = request.method,
        )
    }
}
