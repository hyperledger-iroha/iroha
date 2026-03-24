package org.hyperledger.iroha.sdk.offline.attestation

import java.util.concurrent.CompletableFuture

/**
 * Abstraction over the Safety Detect attestation backend. Implementations are responsible for
 * minting HMS tokens and returning the raw JWS payloads that Torii expects when admitting offline
 * bundles.
 */
interface SafetyDetectService {

    /**
     * Executes a Safety Detect attestation request.
     *
     * @param request Normalised attestation parameters (app id, nonce, package name, etc.).
     */
    fun fetch(request: SafetyDetectRequest): CompletableFuture<SafetyDetectAttestation>

    companion object {
        /**
         * Returns a service that always fails, used when the feature flag has not been enabled in the SDK
         * options.
         */
        @JvmStatic
        fun disabled(): SafetyDetectService = DisabledSafetyDetectService()
    }

    class DisabledSafetyDetectService : SafetyDetectService {
        override fun fetch(request: SafetyDetectRequest): CompletableFuture<SafetyDetectAttestation> {
            val future = CompletableFuture<SafetyDetectAttestation>()
            future.completeExceptionally(IllegalStateException("Safety Detect attestation is disabled"))
            return future
        }
    }
}
