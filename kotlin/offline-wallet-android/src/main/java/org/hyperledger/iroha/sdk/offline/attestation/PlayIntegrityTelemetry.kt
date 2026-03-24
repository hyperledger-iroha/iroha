package org.hyperledger.iroha.sdk.offline.attestation

/** Hook invoked whenever a Play Integrity attestation attempt succeeds or fails. */
interface PlayIntegrityTelemetry {

    fun onAttempt(request: PlayIntegrityRequest) {}

    fun onSuccess(request: PlayIntegrityRequest, attestation: PlayIntegrityAttestation?)

    fun onFailure(request: PlayIntegrityRequest, error: Throwable)

    companion object {
        @JvmField
        val NO_OP: PlayIntegrityTelemetry = object : PlayIntegrityTelemetry {
            override fun onAttempt(request: PlayIntegrityRequest) {}
            override fun onSuccess(request: PlayIntegrityRequest, attestation: PlayIntegrityAttestation?) {}
            override fun onFailure(request: PlayIntegrityRequest, error: Throwable) {}
        }
    }
}
