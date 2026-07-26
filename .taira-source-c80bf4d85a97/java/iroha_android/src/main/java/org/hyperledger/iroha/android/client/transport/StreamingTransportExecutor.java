package org.hyperledger.iroha.android.client.transport;

import java.util.concurrent.CompletableFuture;

/**
 * Transport executor that can surface streaming responses without buffering the full payload.
 */
public interface StreamingTransportExecutor extends TransportExecutor {

  /**
   * Executes the request and returns a streaming response once headers are available.
   */
  CompletableFuture<TransportStreamResponse> openStream(TransportRequest request);
}
