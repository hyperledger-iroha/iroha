package org.hyperledger.iroha.android.client.stream;

import java.util.concurrent.CompletableFuture;

/** Handle for an active Torii event stream. */
public interface ToriiEventStream extends AutoCloseable {

  /** Returns {@code true} while the underlying HTTP stream is open. */
  boolean isOpen();

  /**
   * Returns a future that completes once the stream terminates (either normally or due to an
   * error). Callers can use this hook to await graceful shutdown without polling {@link #isOpen()}.
   */
  CompletableFuture<Void> completion();

  /**
   * Closes the stream and releases resources. Repeated invocations are no-ops. Implementations must
   * not throw.
   */
  @Override
  void close();
}
