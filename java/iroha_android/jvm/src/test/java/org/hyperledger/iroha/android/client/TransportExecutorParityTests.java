package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.io.IOException;
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
import org.junit.Test;

/** Parity tests covering OkHttp vs JDK HTTP executors on the JVM. */
public final class TransportExecutorParityTests {

  @Test
  public void shouldMatchOnSimpleGet() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse().setResponseCode(202).setHeader("x-test", "ok").setBody("pong"));
      server.start();

      final URI uri = new URI(server.url("/ping").toString());
      final TransportRequest request =
          TransportRequest.builder().setMethod("GET").setUri(uri).setHeaders(Map.of()).build();

      final HttpTransportExecutor okHttp = new OkHttpTestExecutor();
      final HttpTransportExecutor jdk =
          new JavaHttpExecutor(java.net.http.HttpClient.newHttpClient());

      final TransportResponse okHttpResponse = okHttp.execute(request).join();
      final TransportResponse jdkResponse = jdk.execute(request).join();

      assert okHttpResponse.statusCode() == jdkResponse.statusCode() : "status codes should match";
      assert Arrays.equals(okHttpResponse.body(), jdkResponse.body()) : "bodies should match";
      assert "pong".equals(new String(okHttpResponse.body(), StandardCharsets.UTF_8));
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
  }
}
