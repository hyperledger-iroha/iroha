package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.InetAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.Headers;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.junit.AfterClass;
import org.junit.Test;

/** Parity tests covering OkHttp vs JDK HTTP executors on the JVM. */
public final class TransportExecutorParityTests {

  private static final ExecutorService JDK_EXECUTOR =
      Executors.newCachedThreadPool(
          runnable -> {
            final Thread thread = new Thread(runnable, "jvm-http-executor");
            thread.setDaemon(true);
            return thread;
          });
  private static final java.net.http.HttpClient JDK_CLIENT =
      java.net.http.HttpClient.newBuilder()
          .executor(JDK_EXECUTOR)
          .connectTimeout(Duration.ofSeconds(5))
          .version(java.net.http.HttpClient.Version.HTTP_1_1)
          .build();

  @AfterClass
  public static void shutdownJdkExecutor() {
    JDK_EXECUTOR.shutdownNow();
  }

  @Test
  public void shouldMatchOnSimpleGet() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      final MockResponse response =
          new MockResponse().setResponseCode(202).setHeader("x-test", "ok").setBody("pong");
      server.enqueue(response);
      server.enqueue(response);
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final URI uri = new URI(server.url("/ping").toString());
      final TransportRequest request =
          TransportRequest.builder().setMethod("GET").setUri(uri).setHeaders(Map.of()).build();

      final OkHttpTestExecutor okHttp = new OkHttpTestExecutor();
      final HttpTransportExecutor jdk = new JavaHttpExecutor(JDK_CLIENT);

      try {
        final TransportResponse okHttpResponse = okHttp.execute(request).get(5, TimeUnit.SECONDS);
        final TransportResponse jdkResponse = jdk.execute(request).get(5, TimeUnit.SECONDS);

        assert okHttpResponse.statusCode() == jdkResponse.statusCode() : "status codes should match";
        assert Arrays.equals(okHttpResponse.body(), jdkResponse.body()) : "bodies should match";
        assert "pong".equals(new String(okHttpResponse.body(), StandardCharsets.UTF_8));
      } finally {
        okHttp.shutdown();
      }
    }
  }

  @Test
  public void openStreamReturnsResponseBody() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("data: ok\n\n"));
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final URI uri = new URI(server.url("/stream").toString());
      final TransportRequest request =
          TransportRequest.builder().setMethod("GET").setUri(uri).setHeaders(Map.of()).build();
      final JavaHttpExecutor executor = new JavaHttpExecutor(JDK_CLIENT);

      final TransportStreamResponse response = executor.openStream(request).get(5, TimeUnit.SECONDS);
      final String body = readBody(response.body());
      response.close();

      assert response.statusCode() == 200 : "expected 200";
      assert body.contains("data: ok") : "expected streaming body";
    }
  }

  private static final class OkHttpTestExecutor implements HttpTransportExecutor {
    private final OkHttpClient client = new OkHttpClient.Builder().connectTimeout(5, TimeUnit.SECONDS).build();

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final Request.Builder builder = new Request.Builder().url(request.uri().toString());
      final Headers.Builder headers = new Headers.Builder();
      request
          .headers()
          .forEach(
              (name, values) -> {
                for (final String value : values) {
                  headers.add(name, value);
                }
              });
      builder.headers(headers.build());
      final RequestBody body = buildRequestBody(request);
      builder.method(request.method(), body);

      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      final Call call = client.newCall(builder.build());
      call.enqueue(
          new Callback() {
            @Override
            public void onFailure(final Call call, final IOException e) {
              future.completeExceptionally(e);
            }

            @Override
            public void onResponse(final Call call, final Response response) {
              try (response) {
                final byte[] responseBody =
                    response.body() == null ? new byte[0] : response.body().bytes();
                future.complete(
                    new TransportResponse(
                        response.code(),
                        responseBody,
                        response.message(),
                        response.headers().toMultimap()));
              } catch (final IOException e) {
                future.completeExceptionally(e);
              }
            }
          });
      future.whenComplete(
          (ignored, throwable) -> {
            if (future.isCancelled()) {
              call.cancel();
            }
          });
      return future;
    }

    private static RequestBody buildRequestBody(final TransportRequest request) {
      final byte[] payload = request.body();
      if (payload == null || payload.length == 0) {
        final String method = request.method() == null ? "" : request.method().trim().toUpperCase();
        return ("GET".equals(method) || "HEAD".equals(method)) ? null : RequestBody.create(new byte[0], null);
      }
      return RequestBody.create(payload, null);
    }

    private void shutdown() {
      client.dispatcher().executorService().shutdownNow();
      client.connectionPool().evictAll();
      if (client.cache() != null) {
        try {
          client.cache().close();
        } catch (final IOException ignored) {
          // Best-effort cleanup for test shutdown.
        }
      }
    }
  }

  private static String readBody(final InputStream input) throws Exception {
    try (InputStream stream = input; ByteArrayOutputStream output = new ByteArrayOutputStream()) {
      final byte[] buffer = new byte[4096];
      int read;
      while ((read = stream.read(buffer)) != -1) {
        output.write(buffer, 0, read);
      }
      return output.toString(StandardCharsets.UTF_8);
    }
  }
}
