package org.hyperledger.iroha.android.client;

import java.util.Objects;
import okhttp3.OkHttpClient;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClient;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;

/**
 * Convenience factory that wires Android clients to a shared OkHttp transport stack.
 *
 * <p>The factory centralises construction of HTTP/WebSocket executors so callers can reuse a single
 * {@link OkHttpClient} across REST, Norito RPC, SSE, WebSocket, Safety Detect, and SoraFS gateway
 * flows while keeping telemetry observers attached via {@link ClientConfig}.
 */
public final class AndroidClientFactory {

  private final OkHttpClient okHttpClient;
  private final OkHttpTransportExecutor httpExecutor;
  private final OkHttpWebSocketConnector webSocketConnector;

  private AndroidClientFactory(final OkHttpClient okHttpClient) {
    this.okHttpClient = Objects.requireNonNull(okHttpClient, "okHttpClient");
    this.httpExecutor = new OkHttpTransportExecutor(this.okHttpClient);
    this.webSocketConnector = new OkHttpWebSocketConnector(this.okHttpClient);
  }

  /** Creates a factory backed by a fresh {@link OkHttpClient}. */
  public static AndroidClientFactory withDefaultClient() {
    return new AndroidClientFactory(new OkHttpClient());
  }

  /** Creates a factory that reuses the caller-provided {@link OkHttpClient}. */
  public static AndroidClientFactory withClient(final OkHttpClient okHttpClient) {
    return new AndroidClientFactory(okHttpClient);
  }

  /** Returns the shared {@link OkHttpClient} backing all generated clients. */
  public OkHttpClient okHttpClient() {
    return okHttpClient;
  }

  /** Returns the OkHttp-backed HTTP executor. */
  public HttpTransportExecutor httpExecutor() {
    return httpExecutor;
  }

  /** Returns the OkHttp-backed WebSocket connector. */
  public ToriiWebSocketClient.WebSocketConnector webSocketConnector() {
    return webSocketConnector;
  }

  /** Builds an {@link HttpClientTransport} backed by the shared OkHttp executor. */
  public HttpClientTransport createHttpClientTransport(final ClientConfig config) {
    return new HttpClientTransport(httpExecutor, Objects.requireNonNull(config, "config"));
  }

  /** Builds a Norito RPC client that reuses the shared OkHttp executor and config observers. */
  public NoritoRpcClient createNoritoRpcClient(final ClientConfig config) {
    return Objects.requireNonNull(config, "config").toNoritoRpcClient(httpExecutor);
  }

  /** Builds an SSE client that reuses the shared OkHttp executor and {@link ClientConfig} headers. */
  public ToriiEventStreamClient createEventStreamClient(final ClientConfig config) {
    final ClientConfig resolved = Objects.requireNonNull(config, "config");
    return ToriiEventStreamClient.builder()
        .setBaseUri(resolved.baseUri())
        .setTransportExecutor(httpExecutor)
        .defaultHeaders(resolved.defaultHeaders())
        .observers(resolved.observers())
        .build();
  }

  /**
   * Builds a WebSocket client that reuses the shared OkHttp executor/connector and {@link
   * ClientConfig} headers.
   */
  public ToriiWebSocketClient createWebSocketClient(final ClientConfig config) {
    final ClientConfig resolved = Objects.requireNonNull(config, "config");
    return ToriiWebSocketClient.builder()
        .setBaseUri(resolved.baseUri())
        .defaultHeaders(resolved.defaultHeaders())
        .observers(resolved.observers())
        .setWebSocketConnector(webSocketConnector)
        .build();
  }

  /** Builds a Safety Detect HTTP client backed by the shared OkHttp executor. */
  public HttpSafetyDetectService createSafetyDetectService(final SafetyDetectOptions options) {
    return new HttpSafetyDetectService(httpExecutor, Objects.requireNonNull(options, "options"));
  }

  /** Builds a SoraFS gateway client that reuses the shared OkHttp executor and {@link ClientConfig}. */
  public SorafsGatewayClient createSorafsGatewayClient(final ClientConfig config) {
    final ClientConfig resolved = Objects.requireNonNull(config, "config");
    return SorafsGatewayClient.builder()
        .setExecutor(httpExecutor)
        .setBaseUri(resolved.sorafsGatewayUri())
        .setTimeout(resolved.requestTimeout())
        .setDefaultHeaders(resolved.defaultHeaders())
        .setObservers(resolved.observers())
        .build();
  }
}
