package org.hyperledger.iroha.sdk.client.stream

import java.util.concurrent.CompletableFuture

/** Handle for an active Torii event stream. */
interface ToriiEventStream : AutoCloseable {

    /** Returns `true` while the underlying HTTP stream is open. */
    fun isOpen(): Boolean

    /**
     * Returns a future that completes once the stream terminates (either normally or due to an
     * error). Callers can use this hook to await graceful shutdown without polling `isOpen()`.
     */
    fun completion(): CompletableFuture<Void>

    /**
     * Closes the stream and releases resources. Repeated invocations are no-ops. Implementations must
     * not throw.
     */
    override fun close()
}
