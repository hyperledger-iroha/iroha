package org.hyperledger.iroha.android.client.websocket;

import java.net.http.HttpClient;

/** Factory helpers for creating {@link JdkWebSocketConnector} instances. */
public final class JdkWebSocketConnectorFactory {

  private JdkWebSocketConnectorFactory() {}

  /** Creates a connector backed by {@link HttpClient#newHttpClient()}. */
  public static JdkWebSocketConnector createDefault() {
    return new JdkWebSocketConnector(HttpClient.newHttpClient());
  }

  /** Creates a connector backed by the provided {@link HttpClient}. */
  public static JdkWebSocketConnector create(final HttpClient httpClient) {
    return new JdkWebSocketConnector(httpClient);
  }
}
