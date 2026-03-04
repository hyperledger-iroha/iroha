package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.junit.Test;

public final class OkHttpTransportExecutorTests {

  @Test
  public void executesRequestAndMapsResponse() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(201)
              .setBody("hello")
              .addHeader("X-Test", "ok"));
      server.start();

      final OkHttpClient client = new OkHttpClient();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(client);
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("POST")
              .setUri(server.url("/hello").uri())
              .addHeader("X-Test", "req")
              .setBody("payload".getBytes(StandardCharsets.UTF_8))
              .build();

      final TransportResponse response = executor.execute(request).get(5, TimeUnit.SECONDS);
      assertEquals(201, response.statusCode());
      assertEquals("hello", new String(response.body(), StandardCharsets.UTF_8));
      assertArrayEquals(new String[] {"ok"}, response.headers().get("X-Test").toArray());
    }
  }

  @Test
  public void timesOutWhenCallExceedsTimeout() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse().setBody("slow").setBodyDelay(200, TimeUnit.MILLISECONDS));
      server.start();

      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/slow").uri())
              .setTimeout(Duration.ofMillis(10))
              .build();

      final ExecutionException error =
          assertThrows(
              ExecutionException.class, () -> executor.execute(request).get(2, TimeUnit.SECONDS));
      assertNotNull(error.getCause());
    }
  }

  @Test
  public void invalidateAndCancelShutsDownDispatcher() {
    final OkHttpClient client = new OkHttpClient();
    final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(client);
    executor.invalidateAndCancel();
    assertTrue(client.dispatcher().executorService().isShutdown());
  }
}
