package org.hyperledger.iroha.android.client;

import org.hyperledger.iroha.android.client.transport.TransportRequest;

/**
 * Observer hooks for {@link IrohaClient} implementations.
 *
 * <p>Observers can be used to collect telemetry, emit logs, or thread tracing headers without
 * mutating the client transport. Implementations should execute quickly and avoid throwing unless
 * the request must be aborted.
 */
public interface ClientObserver {

  /**
   * Invoked once the request has been constructed and before it is dispatched to the transport
   * executor.
   */
  default void onRequest(final TransportRequest request) {}

  /** Invoked when a response is received successfully. */
  default void onResponse(final TransportRequest request, final ClientResponse response) {}

  /** Invoked when request execution fails. */
  default void onFailure(final TransportRequest request, final Throwable error) {}
}
