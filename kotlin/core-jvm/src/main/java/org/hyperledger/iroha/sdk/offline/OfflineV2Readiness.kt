package org.hyperledger.iroha.sdk.offline

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
data class OfflineV2Readiness(
    val offlineTelemetry: Boolean,
    val offlineKagemushaRecursiveCompactAvailable: Boolean = false,
    val offlineKagemushaRecursiveCompactMode: String? = null,
    val offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion: Int? = null,
    val offlineKagemushaRecursiveCompactCircuitId: String? = null,
    val offlineKagemushaRecursiveCompactArtifactsAvailable: Boolean = false,
)
