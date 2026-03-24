package org.hyperledger.iroha.sdk.offline.attestation

/** Hook invoked whenever a Safety Detect attestation attempt succeeds or fails. */
interface SafetyDetectTelemetry {

    fun onAttempt(request: SafetyDetectRequest) {}

    fun onSuccess(request: SafetyDetectRequest, attestation: SafetyDetectAttestation?)

    fun onFailure(request: SafetyDetectRequest, error: Throwable)

    companion object {
        @JvmField
        val NO_OP: SafetyDetectTelemetry = object : SafetyDetectTelemetry {
            override fun onAttempt(request: SafetyDetectRequest) {}
            override fun onSuccess(request: SafetyDetectRequest, attestation: SafetyDetectAttestation?) {}
            override fun onFailure(request: SafetyDetectRequest, error: Throwable) {}
        }
    }
}
