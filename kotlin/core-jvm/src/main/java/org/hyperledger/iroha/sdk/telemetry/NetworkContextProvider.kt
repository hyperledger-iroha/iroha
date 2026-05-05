package org.hyperledger.iroha.sdk.telemetry

import java.util.Optional

/**
 * Supplies sanitised network context snapshots for telemetry emission.
 *
 * SDK consumers provide the platform-specific implementation (pulling from Android's
 * `ConnectivityManager`, for example). The default provider returns an empty snapshot so
 * telemetry emission becomes a no-op when apps opt out or the device cannot determine its current
 * network.
 */
fun interface NetworkContextProvider {

    /**
     * Returns a snapshot of the current network context.
     *
     * @return `Optional.empty()` when network details are unavailable or should not be exported.
     */
    fun snapshot(): Optional<NetworkContext>

    companion object {
        /** Convenience helper that disables network-context emission. */
        @JvmStatic
        fun disabled(): NetworkContextProvider = NetworkContextProvider { Optional.empty() }
    }
}
