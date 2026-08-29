package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNotSame;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.zip.GZIPOutputStream;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.SocketPolicy;
import okio.Buffer;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
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
      assertEquals(request.uri(), response.finalUri());
      assertFalse(response.redirected());
    }
  }

  @Test
  public void oneShotBodiesNeverFollow307Or308WithUnsafeInjectedClient() throws Exception {
    for (final int status : new int[] {307, 308}) {
      try (MockWebServer server = new MockWebServer()) {
        server.start();
        server.enqueue(
            new MockResponse()
                .setResponseCode(status)
                .setHeader("Location", server.url("/redirected")));
        server.enqueue(new MockResponse().setResponseCode(202));
        final OkHttpClient unsafeClient =
            new OkHttpClient.Builder()
                .followRedirects(true)
                .followSslRedirects(true)
                .retryOnConnectionFailure(true)
                .build();
        final TransportRequest request =
            TransportRequest.builder()
                .setMethod("POST")
                .setUri(server.url("/signed").uri())
                .setBody("signed-bytes".getBytes(StandardCharsets.UTF_8))
                .build();

        final TransportResponse response =
            new OkHttpTransportExecutor(unsafeClient).execute(request).get(5, TimeUnit.SECONDS);

        assertEquals(status, response.statusCode());
        assertEquals("one-shot request must not reach redirect target", 1, server.getRequestCount());
      }
    }
  }

  @Test
  public void oneShotBodiesNeverRetryAfterConnectionFailure() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start();
      server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AT_START));
      server.enqueue(new MockResponse().setResponseCode(202));
      final OkHttpClient unsafeClient =
          new OkHttpClient.Builder().retryOnConnectionFailure(true).build();
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("POST")
              .setUri(server.url("/signed").uri())
              .setBody("signed-bytes".getBytes(StandardCharsets.UTF_8))
              .build();

      assertThrows(
          ExecutionException.class,
          () ->
              new OkHttpTransportExecutor(unsafeClient)
                  .execute(request)
                  .get(5, TimeUnit.SECONDS));
      assertEquals("one-shot request must not be retried", 1, server.getRequestCount());
    }
  }

  @Test
  public void oneShotBodiesNeverRetryRetryAfterZeroStatus() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start();
      server.enqueue(
          new MockResponse().setResponseCode(503).setHeader("Retry-After", "0"));
      server.enqueue(new MockResponse().setResponseCode(202));
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("POST")
              .setUri(server.url("/signed").uri())
              .setBody("signed-bytes".getBytes(StandardCharsets.UTF_8))
              .build();

      final TransportResponse response =
          new OkHttpTransportExecutor(new OkHttpClient.Builder().build())
              .execute(request)
              .get(5, TimeUnit.SECONDS);

      assertEquals(503, response.statusCode());
      assertEquals("one-shot request must not honor Retry-After", 1, server.getRequestCount());
    }
  }

  @Test
  public void acceptsResponseAtConfiguredBufferedLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("12345678"));
      server.start();

      final OkHttpTransportExecutor executor =
          new OkHttpTransportExecutor(new OkHttpClient());
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/exact").uri())
              .setMaximumResponseBytes(8L)
              .build();

      final TransportResponse response = executor.execute(request).get(5, TimeUnit.SECONDS);
      assertEquals("12345678", new String(response.body(), StandardCharsets.UTF_8));
    }
  }

  @Test
  public void rejectsChunkedResponseAboveConfiguredBufferedLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse().setResponseCode(500).setChunkedBody("123456789", 3));
      server.start();

      final OkHttpTransportExecutor executor =
          new OkHttpTransportExecutor(new OkHttpClient());
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/overflow").uri())
              .setMaximumResponseBytes(8L)
              .build();

      final ExecutionException failure =
          assertThrows(
              ExecutionException.class,
              () -> executor.execute(request).get(5, TimeUnit.SECONDS));
      assertTrue(hasCauseMessage(failure, "body exceeds the 8-byte limit"));
    }
  }

  @Test
  public void rejectsDeclaredNonSuccessResponseAbovePerRequestLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(500).setBody("123456789"));
      server.start();

      final OkHttpTransportExecutor executor =
          new OkHttpTransportExecutor(new OkHttpClient());
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/declared-overflow").uri())
              .setMaximumResponseBytes(8L)
              .build();

      final ExecutionException failure =
          assertThrows(
              ExecutionException.class,
              () -> executor.execute(request).get(5, TimeUnit.SECONDS));
      assertTrue(hasCauseMessage(failure, "Content-Length 9 exceeds the 8-byte limit"));
    }
  }

  @Test
  public void rejectsTransparentGzipExpansionAbovePerRequestLimit() throws Exception {
    final byte[] decoded = new byte[1024];
    java.util.Arrays.fill(decoded, (byte) 'a');
    final byte[] encoded = gzip(decoded);
    assertTrue("gzip fixture must fit below the decoded cap", encoded.length < 64);
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setHeader("Content-Encoding", "gzip")
              .setBody(new Buffer().write(encoded)));
      server.start();

      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/gzip-expansion").uri())
              .setMaximumResponseBytes(64L)
              .build();
      final ExecutionException failure =
          assertThrows(
              ExecutionException.class,
              () ->
                  new OkHttpTransportExecutor(new OkHttpClient())
                      .execute(request)
                      .get(5, TimeUnit.SECONDS));

      assertTrue(hasCauseMessage(failure, "body exceeds the 64-byte limit"));
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

  @Test
  public void defaultFactoryInvalidationDoesNotShutdownSharedDispatcher() {
    final OkHttpClient client = new OkHttpClient();
    OkHttpClientProvider.installForTests(client);
    try {
      OkHttpTransportExecutorFactory.createDefault().invalidateAndCancel();
      assertFalse(client.dispatcher().executorService().isShutdown());
    } finally {
      client.dispatcher().executorService().shutdownNow();
      OkHttpClientProvider.resetForTests();
    }
  }

  @Test
  public void defaultFactoryInvalidationCancelsOnlyExecutorCalls() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setBody("slow").setBodyDelay(5, TimeUnit.SECONDS));
      server.start();

      final OkHttpClient client = new OkHttpClient();
      OkHttpClientProvider.installForTests(client);
      try {
        final HttpTransportExecutor executor = OkHttpTransportExecutorFactory.createDefault();
        final TransportRequest request =
            TransportRequest.builder()
                .setMethod("GET")
                .setUri(server.url("/slow").uri())
                .build();

        final CompletableFuture<TransportResponse> future = executor.execute(request);
        executor.invalidateAndCancel();

        final ExecutionException error =
            assertThrows(ExecutionException.class, () -> future.get(2, TimeUnit.SECONDS));
        assertNotNull(error.getCause());
        assertFalse(client.dispatcher().executorService().isShutdown());
      } finally {
        client.dispatcher().executorService().shutdownNow();
        OkHttpClientProvider.resetForTests();
      }
    }
  }

  @Test
  public void sharedProviderReplacesShutdownClient() {
    final OkHttpClient client = new OkHttpClient();
    OkHttpClientProvider.installForTests(client);
    client.dispatcher().executorService().shutdown();
    try {
      final OkHttpClient replacement = OkHttpClientProvider.shared();
      assertNotSame(client, replacement);
      assertFalse(replacement.dispatcher().executorService().isShutdown());
      replacement.dispatcher().executorService().shutdownNow();
    } finally {
      OkHttpClientProvider.resetForTests();
    }
  }

  private static boolean hasCauseMessage(final Throwable failure, final String fragment) {
    Throwable current = failure;
    while (current != null) {
      if (current.getMessage() != null && current.getMessage().contains(fragment)) {
        return true;
      }
      current = current.getCause();
    }
    return false;
  }

  private static byte[] gzip(final byte[] bytes) throws IOException {
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    try (GZIPOutputStream gzip = new GZIPOutputStream(output)) {
      gzip.write(bytes);
    }
    return output.toByteArray();
  }
}
