package org.hyperledger.iroha.android.client.transport;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;

/**
 * Adapter that bridges the legacy {@link HttpTransportExecutor} API to the new transport interfaces
 * for JVM callers. Android builds should prefer an OkHttp-backed executor instead of this adapter.
 */
public final class LegacyHttpTransportExecutorAdapter implements TransportExecutor {

  private final HttpTransportExecutor legacy;

  /**
   * Creates an adapter for the legacy executor.
   *
   * @param legacy legacy executor instance
   */
  public LegacyHttpTransportExecutorAdapter(final HttpTransportExecutor legacy) {
    this.legacy = Objects.requireNonNull(legacy, "legacy");
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    return legacy.execute(request);
  }
}
