package org.hyperledger.iroha.android.client.queue;

import java.io.IOException;
import java.util.List;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/**
 * Abstraction for staging transactions while the device is offline or waiting for retry.
 *
 * <p>Implementations should preserve insertion order and avoid dropping entries unless an error is
 * returned to the caller.
 */
public interface PendingTransactionQueue {

  /**
   * Adds a transaction to the queue.
   *
   * @param transaction signed transaction ready for submission
   * @throws IOException when persistence fails
   */
  void enqueue(SignedTransaction transaction) throws IOException;

  /**
   * Removes and returns all queued transactions in insertion order.
   *
   * @return list of transactions to be retried
   * @throws IOException when persistence fails
   */
  List<SignedTransaction> drain() throws IOException;

  /**
   * Returns the number of queued entries. Implementations may compute this lazily (e.g., by
   * inspecting the backing store), so callers should avoid invoking this method on hot paths.
   *
   * @throws IOException when persistence fails
   */
  int size() throws IOException;

  /**
   * Identifier used by telemetry emitters when exporting queue metrics. Implementations can
   * override this to provide a stable name across releases.
   */
  default String telemetryQueueName() {
    return getClass().getSimpleName();
  }
}
