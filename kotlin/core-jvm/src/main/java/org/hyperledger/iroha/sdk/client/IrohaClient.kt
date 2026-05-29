package org.hyperledger.iroha.sdk.client

import java.util.Optional
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.tx.SignedTransaction

/** High-level client for interacting with Iroha nodes. */
interface IrohaClient {

    /**
     * Submits a signed transaction to the node.
     *
     * The returned future completes with a response summary. Implementations should ensure retries
     * remain deterministic and avoid replaying signatures unless explicitly requested.
     */
    fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse>

    /**
     * Submits a version-tagged SignedTransaction encoded as canonical Norito JSON.
     *
     * This helper is for callers that already have the direct Torii JSON ingress envelope.
     */
    fun submitTransactionJson(encodedVersionedTransactionJson: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionJson requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Submits an already versioned Norito transaction entrypoint to the node.
     *
     * This is intended for sealed commitment/reveal entrypoints and other non-legacy transaction
     * envelopes that are not represented as a plain [SignedTransaction].
     */
    fun submitTransactionEntrypoint(encodedVersionedEntrypoint: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionEntrypoint requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Submits a version-tagged TransactionEntrypoint encoded as canonical Norito JSON.
     */
    fun submitTransactionEntrypointJson(encodedVersionedEntrypointJson: ByteArray): CompletableFuture<ClientResponse> {
        val future = CompletableFuture<ClientResponse>()
        future.completeExceptionally(
            IllegalStateException("submitTransactionEntrypointJson requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Polls the pipeline status endpoint until the transaction reaches a terminal state.
     *
     * The default implementation reports that the operation is unsupported.
     */
    fun waitForTransactionStatus(
        hashHex: String,
        options: PipelineStatusOptions?,
    ): CompletableFuture<Map<String, Any>> {
        val future = CompletableFuture<Map<String, Any>>()
        future.completeExceptionally(
            IllegalStateException("waitForTransactionStatus requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Proposes a generic multisig instruction batch through Torii's `/v1/multisig/propose`.
     *
     * Implementations should encode request instructions as base64 native Norito `InstructionBox`
     * frames in the JSON body.
     */
    fun proposeMultisig(request: MultisigProposeRequest): CompletableFuture<MultisigResponse> {
        val future = CompletableFuture<MultisigResponse>()
        future.completeExceptionally(
            IllegalStateException("proposeMultisig requires a concrete IrohaClient implementation")
        )
        return future
    }

    /**
     * Resolves an account alias to its underlying Iroha account id via Torii's
     * `/v1/aliases/resolve` endpoint.
     *
     * The default implementation reports that the operation is unsupported.
     */
    fun resolveAccountAlias(alias: String): CompletableFuture<Optional<AccountAliasResolution>> {
        val future = CompletableFuture<Optional<AccountAliasResolution>>()
        future.completeExceptionally(
            IllegalStateException("resolveAccountAlias requires a concrete IrohaClient implementation")
        )
        return future
    }
}
