package org.hyperledger.iroha.sdk.telemetry

/**
 * Sanitised snapshot of the device network state used by `android.telemetry.network_context`.
 *
 * The snapshot intentionally captures a coarse `network_type` label (e.g. `wifi`,
 * `cellular`) and a roaming flag only. Carrier identifiers are excluded by design so the
 * signal complies with the AND7 redaction policy.
 */
class NetworkContext(
    networkType: String = "unknown",
    @JvmField val roaming: Boolean = false,
) {
    @JvmField val networkType: String = normaliseNetworkType(networkType)

    /** Serialises this snapshot to the map expected by `TelemetrySink.emitSignal`. */
    fun toTelemetryFields(): Map<String, Any> = mapOf(
        "network_type" to networkType,
        "roaming" to roaming,
    )

    companion object {
        /** Convenience helper that creates a snapshot for `network_type` and `roaming`. */
        @JvmStatic
        fun of(networkType: String, roaming: Boolean): NetworkContext =
            NetworkContext(networkType, roaming)

        private fun normaliseNetworkType(value: String): String {
            val trimmed = value.trim()
            check(trimmed.isNotEmpty()) { "networkType must be non-empty" }
            return trimmed.lowercase()
        }
    }
}
