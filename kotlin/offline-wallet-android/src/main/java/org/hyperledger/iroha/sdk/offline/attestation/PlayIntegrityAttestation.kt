package org.hyperledger.iroha.sdk.offline.attestation

/** Immutable attestation payload returned by Play Integrity requests. */
class PlayIntegrityAttestation(
    /** Returns the raw Play Integrity token payload. */
    val token: String,
    /** Unix timestamp (ms) when the attestation was fetched. */
    val fetchedAtMs: Long,
)
