package org.hyperledger.iroha.android.client.okhttp;

import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpClientProvider;

/** Factory helpers for constructing {@link OkHttpTransportExecutor} instances. */
public final class OkHttpTransportExecutorFactory {

  private OkHttpTransportExecutorFactory() {}

  public static HttpTransportExecutor createDefault() {
    return OkHttpTransportExecutor.shared(OkHttpClientProvider.shared());
  }

  public static HttpTransportExecutor create(final OkHttpClient client) {
    return new OkHttpTransportExecutor(client);
  }

  /** Creates an executor with a custom buffered-response limit. */
  public static HttpTransportExecutor create(
      final OkHttpClient client, final long maximumResponseBytes) {
    return new OkHttpTransportExecutor(client, maximumResponseBytes);
  }
}
