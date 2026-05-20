package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Optional;
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
   * Submits a version-tagged SignedTransaction encoded as canonical Norito JSON.
   *
   * <p>This helper is for callers that already have the direct Torii JSON ingress envelope.
   */
  default CompletableFuture<ClientResponse> submitTransactionJson(
      final byte[] encodedVersionedTransactionJson) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("submitTransactionJson not supported"));
    return future;
  }

  /**
   * Submits an already versioned Norito transaction entrypoint to the node.
   *
   * <p>This is intended for sealed commitment/reveal entrypoints and other non-legacy transaction
   * envelopes that are not represented as a plain {@link SignedTransaction}.
   */
  default CompletableFuture<ClientResponse> submitTransactionEntrypoint(
      final byte[] encodedVersionedEntrypoint) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("submitTransactionEntrypoint not supported"));
    return future;
  }

  /** Submits a version-tagged TransactionEntrypoint encoded as canonical Norito JSON. */
  default CompletableFuture<ClientResponse> submitTransactionEntrypointJson(
      final byte[] encodedVersionedEntrypointJson) {
    final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("submitTransactionEntrypointJson not supported"));
    return future;
  }

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

  /**
   * Proposes a generic multisig instruction batch via `POST /v1/multisig/propose`.
   *
   * <p>Request instructions are encoded as base64 native Norito {@code InstructionBox} frames in
   * the JSON body.
   */
  default CompletableFuture<MultisigResponse> proposeMultisig(
      final MultisigProposeRequest request) {
    final CompletableFuture<MultisigResponse> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("proposeMultisig not supported by this client"));
    return future;
  }

  /**
   * Resolves an account alias literal against the node's alias registry via
   * `POST /v1/aliases/resolve`.
   *
   * <p>The returned future resolves to {@link Optional#empty()} when the node responds with
   * HTTP 404. Implementations that cannot reach a node should fail the future exceptionally.
   */
  default CompletableFuture<Optional<AccountAliasResolution>> resolveAccountAlias(
      final String alias) {
    final CompletableFuture<Optional<AccountAliasResolution>> future = new CompletableFuture<>();
    future.completeExceptionally(
        new UnsupportedOperationException("resolveAccountAlias not supported by this client"));
    return future;
  }
}
