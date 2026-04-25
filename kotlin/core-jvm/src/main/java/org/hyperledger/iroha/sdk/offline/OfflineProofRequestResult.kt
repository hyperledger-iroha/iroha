package org.hyperledger.iroha.sdk.offline

/** Canonical legacy proof payload retained for local fixture parsing. */
class OfflineProofRequestResult(
    val kind: OfflineProofRequestKind,
    /**
     * Canonical JSON representation of the proof payload; this string can be passed directly to the
     * FASTPQ prover.
     */
    val json: String,
)
