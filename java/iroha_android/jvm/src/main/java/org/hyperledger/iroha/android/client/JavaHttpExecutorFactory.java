package org.hyperledger.iroha.android.client;

import java.net.http.HttpClient;
import java.util.Objects;

/** Factory helpers for creating {@link JavaHttpExecutor} instances. */
public final class JavaHttpExecutorFactory {

  private JavaHttpExecutorFactory() {}

  /** Creates an executor backed by {@link HttpClient#newHttpClient()}. */
  public static HttpTransportExecutor createDefault() {
    return create(HttpClient.newHttpClient());
  }

  /** Creates an executor backed by the provided {@link HttpClient}. */
  public static HttpTransportExecutor create(final HttpClient client) {
    return new JavaHttpExecutor(Objects.requireNonNull(client, "client"));
  }

  /**
   * Convenience helper that wraps a JDK {@link HttpClient} in the SDK transport executor and
   * constructs a {@link HttpClientTransport} with the provided configuration.
   */
  public static HttpClientTransport createTransport(
      final HttpClient client, final ClientConfig config) {
    return new HttpClientTransport(create(client), config);
  }

  /** Builds a {@link HttpClientTransport} backed by a default {@link HttpClient}. */
  public static HttpClientTransport createTransport(final ClientConfig config) {
    return createTransport(HttpClient.newHttpClient(), config);
  }
}
