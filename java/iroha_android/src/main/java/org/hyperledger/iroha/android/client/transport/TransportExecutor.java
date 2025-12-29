package org.hyperledger.iroha.android.client.transport;

import java.util.concurrent.CompletableFuture;

/** Platform-neutral transport executor interface to decouple from {@code java.net.http.HttpClient}. */
public interface TransportExecutor {

  CompletableFuture<TransportResponse> execute(TransportRequest request);
}
