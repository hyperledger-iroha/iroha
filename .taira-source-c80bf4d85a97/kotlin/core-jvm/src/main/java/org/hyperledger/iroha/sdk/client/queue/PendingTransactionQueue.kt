package org.hyperledger.iroha.sdk.client.queue

import java.io.IOException
import org.hyperledger.iroha.sdk.tx.SignedTransaction

/**
 * Abstraction for staging transactions while the device is offline or waiting for retry.
 *
 * Implementations should preserve insertion order and avoid dropping entries unless an error is
 * returned to the caller.
 */
interface PendingTransactionQueue {

    /**
     * Adds a transaction to the queue.
     *
     * @param transaction signed transaction ready for submission
     * @throws IOException when persistence fails
     */
    @Throws(IOException::class)
    fun enqueue(transaction: SignedTransaction)

    /**
     * Removes and returns all queued transactions in insertion order.
     *
     * @return list of transactions to be retried
     * @throws IOException when persistence fails
     */
    @Throws(IOException::class)
    fun drain(): List<SignedTransaction>

    /**
     * Returns the number of queued entries. Implementations may compute this lazily (e.g., by
     * inspecting the backing store), so callers should avoid invoking this method on hot paths.
     *
     * @throws IOException when persistence fails
     */
    @Throws(IOException::class)
    fun size(): Int

    /**
     * Identifier used by telemetry emitters when exporting queue metrics. Implementations can
     * override this to provide a stable name across releases.
     */
    fun telemetryQueueName(): String = this::class.java.simpleName
}
