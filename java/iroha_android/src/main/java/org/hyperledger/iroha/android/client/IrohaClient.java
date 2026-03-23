package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** High-level client for interacting with Iroha nodes from Android applications. */
public interface IrohaClient {

  /**
   * Submits a signed transaction to the node.
   *
   * <p>The returned future completes with a response summary. Implementations should ensure retries
   * remain deterministic and avoid replaying signatures unless explicitly requested.
   */
  CompletableFuture<ClientResponse> submitTransaction(SignedTransaction transaction);

  /**
   * Polls the pipeline status endpoint until the transaction reaches a terminal state.
   *
   * <p>The default implementation reports that the operation is unsupported.
   */
  default CompletableFuture<Map<String, Object>> waitForTransactionStatus(
      final String hashHex, final PipelineStatusOptions options) {
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("waitForTransactionStatus not supported"));
    return future;
  }

  default CompletableFuture<Map<String, Object>> waitForTransactionStatusStream(
      final String hashHex, final PipelineStatusOptions options) {
    final CompletableFuture<Map<String, Object>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("waitForTransactionStatusStream not supported"));
    return future;
  }
}
