package org.hyperledger.iroha.android.client.okhttp;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CompletionException;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.okhttp.OkHttpClientProvider;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnector;
import org.hyperledger.iroha.android.client.okhttp.OkHttpWebSocketConnectorFactory;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class OkHttpTransportExecutorFactoryTests {

  private OkHttpTransportExecutorFactoryTests() {}

  public static void main(final String[] args) throws Exception {
    shouldCreateExecutorWithDefaultClient();
    shouldRejectNullClient();
    defaultsReuseSharedClient();
    System.out.println("[IrohaAndroid] OkHttpTransportExecutorFactory tests passed.");
  }

  private static void shouldCreateExecutorWithDefaultClient() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("ok"));
      server.start();
      final HttpTransportExecutor executor = OkHttpTransportExecutorFactory.createDefault();
      final TransportRequest request =
          TransportRequest.builder().setMethod("GET").setUri(new URI(server.url("/ping").toString())).build();
      final TransportResponse response = executor.execute(request).join();
      assert response.statusCode() == 200 : "expected HTTP 200";
      assert new String(response.body(), StandardCharsets.UTF_8).equals("ok") : "expected response body";
    }
  }

  private static void shouldRejectNullClient() {
    try {
      OkHttpTransportExecutorFactory.create((OkHttpClient) null);
      throw new AssertionError("Expected NullPointerException for null client");
    } catch (final CompletionException unexpected) {
      throw new AssertionError("Unexpected completion exception", unexpected);
    } catch (final NullPointerException expected) {
      // expected
    }
  }

  private static void defaultsReuseSharedClient() {
    final OkHttpClient shared = new OkHttpClient.Builder().build();
    OkHttpClientProvider.installForTests(shared);
    try {
      final HttpTransportExecutor executor = OkHttpTransportExecutorFactory.createDefault();
      final OkHttpWebSocketConnector connector = OkHttpWebSocketConnectorFactory.createDefault();
      assert executor instanceof OkHttpTransportExecutor : "default should return OkHttpTransportExecutor";
      assert ((OkHttpTransportExecutor) executor).unwrapClient() == shared
          : "executor should reuse shared client";
      assert connector.unwrapClient() == shared : "websocket connector should reuse shared client";
    } finally {
      OkHttpClientProvider.resetForTests();
    }
  }
}
