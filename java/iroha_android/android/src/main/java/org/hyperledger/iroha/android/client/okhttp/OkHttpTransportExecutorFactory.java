package org.hyperledger.iroha.android.client.okhttp;

import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpClientProvider;

/** Factory helpers for constructing {@link OkHttpTransportExecutor} instances. */
public final class OkHttpTransportExecutorFactory {

  private OkHttpTransportExecutorFactory() {}

  public static HttpTransportExecutor createDefault() {
    return create(OkHttpClientProvider.shared());
  }

  public static HttpTransportExecutor create(final OkHttpClient client) {
    return new OkHttpTransportExecutor(client);
  }
}
