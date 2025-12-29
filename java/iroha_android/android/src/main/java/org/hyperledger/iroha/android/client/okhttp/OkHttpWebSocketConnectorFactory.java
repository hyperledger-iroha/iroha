package org.hyperledger.iroha.android.client.okhttp;

import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.okhttp.OkHttpClientProvider;

/** Factory helpers for constructing {@link OkHttpWebSocketConnector} instances. */
public final class OkHttpWebSocketConnectorFactory {

  private OkHttpWebSocketConnectorFactory() {}

  /** Creates a connector backed by a new {@link OkHttpClient} with default settings. */
  public static OkHttpWebSocketConnector createDefault() {
    return new OkHttpWebSocketConnector(OkHttpClientProvider.shared());
  }

  /** Creates a connector backed by the supplied {@link OkHttpClient}. */
  public static OkHttpWebSocketConnector create(final OkHttpClient client) {
    return new OkHttpWebSocketConnector(client);
  }
}
