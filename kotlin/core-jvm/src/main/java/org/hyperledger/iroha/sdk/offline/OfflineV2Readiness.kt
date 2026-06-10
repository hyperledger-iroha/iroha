package org.hyperledger.iroha.sdk.offline

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
data class OfflineV2Readiness(
    val offlineTelemetry: Boolean,
    val offlineKagemushaAbi7: Boolean = false,
    val offlineKagemushaAbi7Mode: String? = null,
    val offlineKagemushaAbi7BridgeAbiVersion: Int? = null,
    val offlineKagemushaAbi7CircuitId: String? = null,
    val offlineKagemushaAbi7Artifacts: Boolean = false,
)
