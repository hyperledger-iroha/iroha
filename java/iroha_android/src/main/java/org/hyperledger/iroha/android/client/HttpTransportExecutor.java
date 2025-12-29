package org.hyperledger.iroha.android.client;

import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/**
 * Abstraction over the HTTP execution layer so transports can be tested without real network calls.
 *
 * <p>The interface is intentionally expressed in terms of {@link TransportRequest} and {@link
 * TransportResponse} to avoid leaking JVM-specific HTTP client types into Android binaries.
 */
public interface HttpTransportExecutor extends TransportExecutor {

  @Override
  CompletableFuture<TransportResponse> execute(TransportRequest request);

  /** Returns true when this executor can surface an underlying HTTP client for reuse. */
  default boolean supportsClientUnwrap() {
    return false;
  }
}
