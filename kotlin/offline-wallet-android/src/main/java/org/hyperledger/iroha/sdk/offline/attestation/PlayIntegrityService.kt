package org.hyperledger.iroha.sdk.offline.attestation

import java.util.concurrent.CompletableFuture

/**
 * Abstraction over the Play Integrity backend. Implementations are responsible for minting tokens
 * and returning the raw payload required by Torii when admitting offline bundles.
 */
interface PlayIntegrityService {

    /**
     * Executes a Play Integrity attestation request.
     *
     * @param request Normalised attestation parameters (cloud project, nonce, package name, etc.).
     */
    fun fetch(request: PlayIntegrityRequest): CompletableFuture<PlayIntegrityAttestation>

    companion object {
        /** Returns a service that always fails, used when the feature is disabled. */
        @JvmStatic
        fun disabled(): PlayIntegrityService = DisabledPlayIntegrityService()
    }

    class DisabledPlayIntegrityService : PlayIntegrityService {
        override fun fetch(request: PlayIntegrityRequest): CompletableFuture<PlayIntegrityAttestation> {
            val future = CompletableFuture<PlayIntegrityAttestation>()
            future.completeExceptionally(IllegalStateException("Play Integrity attestation is disabled"))
            return future
        }
    }
}
