package org.hyperledger.iroha.sdk.offline

/** Canonical proof payload returned by Torii (`/v1/offline/transfers/proof`). */
class OfflineProofRequestResult(
    val kind: OfflineProofRequestKind,
    /**
     * Canonical JSON representation of the proof payload; this string can be passed directly to the
     * FASTPQ prover.
     */
    val json: String,
)
