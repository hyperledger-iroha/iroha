package org.hyperledger.iroha.sdk.offline

/** Feature flags reported by Torii's Offline readiness endpoint. */
data class OfflineReadiness(
    val offlineNote: Boolean,
    val offlineOneUseKeys: Boolean,
    val offlineRecursiveNoteProof: Boolean,
    val offlineFountainQr: Boolean,
    val offlineSyncOptional: Boolean,
    val offlineTelemetry: Boolean,
    val offlineKagemushaAbi7: Boolean = false,
    val offlineKagemushaAbi7Mode: String? = null,
    val offlineKagemushaAbi7BridgeAbiVersion: Int? = null,
    val offlineKagemushaAbi7CircuitId: String? = null,
    val offlineKagemushaAbi7Artifacts: Boolean = false,
)
