package org.hyperledger.iroha.android.client.transport;

import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;

/**
 * OkHttp-backed transport executor for Android.
 *
 * <p>This wrapper delegates to the shared OkHttp implementation under
 * {@code org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor} so behaviour stays
 * consistent across the platform factories.
 */
public final class OkHttpTransportExecutor
    implements HttpTransportExecutor, StreamingTransportExecutor {

  private final org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor delegate;

  public OkHttpTransportExecutor(final OkHttpClient client) {
    this.delegate =
        new org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor(
            Objects.requireNonNull(client, "client"));
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    return delegate.execute(request);
  }

  @Override
  public CompletableFuture<TransportStreamResponse> openStream(final TransportRequest request) {
    return delegate.openStream(request);
  }

  @Override
  public boolean supportsClientUnwrap() {
    return delegate.supportsClientUnwrap();
  }

  /** Exposes the underlying OkHttp client used by the delegated executor. */
  public OkHttpClient unwrapClient() {
    return delegate.unwrapClient();
  }
}
