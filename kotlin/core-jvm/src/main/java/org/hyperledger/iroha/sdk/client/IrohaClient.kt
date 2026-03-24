package org.hyperledger.iroha.sdk.client

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
            UnsupportedOperationException("waitForTransactionStatus not supported")
        )
        return future
    }
}
