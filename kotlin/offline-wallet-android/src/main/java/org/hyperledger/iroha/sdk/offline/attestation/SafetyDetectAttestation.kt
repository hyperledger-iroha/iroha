package org.hyperledger.iroha.sdk.offline.attestation

/**
 * Immutable attestation payload returned by Huawei Safety Detect requests.
 *
 * The token mirrors the JWS string required by Torii when submitting `OfflinePlatformProof`
 * envelopes.
 */
class SafetyDetectAttestation(
    /** Returns the raw JWS token emitted by Safety Detect. */
    val token: String,
    /** Unix timestamp (ms) when the attestation was fetched. */
    val fetchedAtMs: Long,
)
