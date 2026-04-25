package org.hyperledger.iroha.sdk.offline

/** Feature flags reported by Torii's Offline V2 readiness endpoint. */
data class OfflineV2Readiness(
    val offlineNoteV2: Boolean,
    val offlineOneUseKeys: Boolean,
    val offlineRecursiveNoteProof: Boolean,
    val offlineFountainQrV1: Boolean,
    val offlineSyncOptional: Boolean,
    val offlineTelemetry: Boolean,
)
