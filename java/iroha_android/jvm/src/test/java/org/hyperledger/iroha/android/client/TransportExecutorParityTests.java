package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.Authenticator;
import java.net.CookieHandler;
import java.net.CookieManager;
import java.net.InetAddress;
import java.net.PasswordAuthentication;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
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

  @Test
  public void jdkExecutorAcceptsBodyAtConfiguredLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("12345678"));
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/exact").uri())
              .setHeaders(Map.of())
              .setMaximumResponseBytes(8L)
              .build();
      final TransportResponse response =
          new JavaHttpExecutor(JDK_CLIENT).execute(request).get(5, TimeUnit.SECONDS);

      assert Arrays.equals("12345678".getBytes(StandardCharsets.UTF_8), response.body())
          : "exact-limit response should be accepted";
    }
  }

  @Test
  public void jdkExecutorRejectsRedirectFollowingClientBeforeOneShotDispatch() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start(InetAddress.getByName("127.0.0.1"), 0);
      final java.net.http.HttpClient unsafeClient =
          java.net.http.HttpClient.newBuilder()
              .followRedirects(java.net.http.HttpClient.Redirect.ALWAYS)
              .build();
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("POST")
              .setUri(server.url("/signed").uri())
              .setBody("signed-bytes".getBytes(StandardCharsets.UTF_8))
              .build();

      try {
        new JavaHttpExecutor(unsafeClient).execute(request);
        throw new AssertionError("redirect-following client must be rejected");
      } catch (final IllegalArgumentException expected) {
        assert expected.getMessage().contains("redirects disabled");
      }
      assert server.getRequestCount() == 0 : "unsafe client must fail before network dispatch";
    }
  }

  @Test
  public void jdkExecutorRejectsAmbientCredentialProvidersBeforeDispatch() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start(InetAddress.getByName("127.0.0.1"), 0);
      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/v1/zk/vk").uri())
              .setAllowAmbientCredentials(false)
              .build();
      final java.net.http.HttpClient cookieClient =
          java.net.http.HttpClient.newBuilder().cookieHandler(new CookieManager()).build();
      final java.net.http.HttpClient authenticatorClient =
          java.net.http.HttpClient.newBuilder()
              .authenticator(
                  new Authenticator() {
                    @Override
                    protected PasswordAuthentication getPasswordAuthentication() {
                      return new PasswordAuthentication(
                          "ambient", "must-not-leak".toCharArray());
                    }
                  })
              .build();
      final java.net.http.HttpClient redirectClient =
          java.net.http.HttpClient.newBuilder()
              .followRedirects(java.net.http.HttpClient.Redirect.ALWAYS)
              .build();

      for (final JavaHttpExecutor executor :
          List.of(
              new JavaHttpExecutor(cookieClient),
              new JavaHttpExecutor(authenticatorClient),
              new JavaHttpExecutor(redirectClient))) {
        try {
          executor.execute(request);
          throw new AssertionError("ambient credential provider must fail before dispatch");
        } catch (final IllegalArgumentException expected) {
          assert expected.getMessage().contains("credential-free requests require");
        }
      }
      assert server.getRequestCount() == 0 : "unsafe JDK clients must fail before network dispatch";
    }
  }

  @Test
  public void jdkDefaultExecutorDoesNotInheritJvmCookieHandlerOrAuthenticator() throws Exception {
    final CookieHandler previousCookieHandler = CookieHandler.getDefault();
    final Authenticator previousAuthenticator = Authenticator.getDefault();
    final java.util.concurrent.atomic.AtomicInteger cookieHandlerCalls =
        new java.util.concurrent.atomic.AtomicInteger();
    final java.util.concurrent.atomic.AtomicInteger authenticatorCalls =
        new java.util.concurrent.atomic.AtomicInteger();
    try {
      CookieHandler.setDefault(
          new CookieHandler() {
            @Override
            public Map<String, List<String>> get(
                final URI uri, final Map<String, List<String>> requestHeaders) {
              cookieHandlerCalls.incrementAndGet();
              return Map.of("Cookie", List.of("ambient-session=must-not-leak"));
            }

            @Override
            public void put(
                final URI uri, final Map<String, List<String>> responseHeaders) {}
          });
      Authenticator.setDefault(
          new Authenticator() {
            @Override
            protected PasswordAuthentication getPasswordAuthentication() {
              authenticatorCalls.incrementAndGet();
              return new PasswordAuthentication(
                  "ambient", "must-not-leak".toCharArray());
            }
          });
      try (MockWebServer server = new MockWebServer()) {
        server.enqueue(
            new MockResponse()
                .setResponseCode(401)
                .setHeader("WWW-Authenticate", "Basic realm=ambient"));
        server.start(InetAddress.getByName("127.0.0.1"), 0);
        final TransportRequest request =
            TransportRequest.builder()
                .setMethod("GET")
                .setUri(server.url("/v1/zk/vk").uri())
                .setAllowAmbientCredentials(false)
                .build();

        final TransportResponse response =
            JavaHttpExecutorFactory.createDefault()
                .execute(request)
                .get(5, TimeUnit.SECONDS);

        assert response.statusCode() == 401 : "expected the original authentication challenge";
        final okhttp3.mockwebserver.RecordedRequest received =
            server.takeRequest(5, TimeUnit.SECONDS);
        assert received != null : "expected one credential-free request";
        assert received.getHeader("Authorization") == null : "origin credentials must not leak";
        assert received.getHeader("Proxy-Authorization") == null
            : "proxy credentials must not leak";
        assert received.getHeader("Cookie") == null : "ambient cookies must not leak";
        assert server.getRequestCount() == 1 : "401 challenge must not trigger an authenticated retry";
        assert cookieHandlerCalls.get() == 0 : "JVM-global CookieHandler must not be consulted";
        assert authenticatorCalls.get() == 0 : "JVM-global Authenticator must not be consulted";
      }
    } finally {
      Authenticator.setDefault(previousAuthenticator);
      CookieHandler.setDefault(previousCookieHandler);
    }
  }

  @Test
  public void jdkCredentialFreeStreamingAndUriUserInfoFailBeforeDispatch() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.start(InetAddress.getByName("127.0.0.1"), 0);
      final JavaHttpExecutor executor = new JavaHttpExecutor(JDK_CLIENT);
      final TransportRequest streaming =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/stream").uri())
              .setAllowAmbientCredentials(false)
              .build();
      try {
        executor.openStream(streaming);
        throw new AssertionError("credential-free streaming must be rejected");
      } catch (final IllegalArgumentException expected) {
        assert expected.getMessage().contains("streaming is unsupported");
      }

      final URI endpoint = server.url("/v1/zk/vk").uri();
      final TransportRequest userInfo =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(
                  new URI(
                      endpoint.getScheme(),
                      "ambient:must-not-leak",
                      endpoint.getHost(),
                      endpoint.getPort(),
                      endpoint.getPath(),
                      null,
                      null))
              .setAllowAmbientCredentials(false)
              .build();
      try {
        executor.execute(userInfo);
        throw new AssertionError("credential-free URI user-info must be rejected");
      } catch (final IllegalArgumentException expected) {
        assert expected.getMessage().contains("reject URI user-info");
      }
      assert server.getRequestCount() == 0 : "policy rejection must happen before dispatch";
    }
  }

  @Test
  public void jdkExecutorRejectsChunkedBodyAboveConfiguredLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse().setResponseCode(500).setChunkedBody("123456789", 3));
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/overflow").uri())
              .setHeaders(Map.of())
              .setMaximumResponseBytes(8L)
              .build();
      try {
        new JavaHttpExecutor(JDK_CLIENT).execute(request).get(5, TimeUnit.SECONDS);
        throw new AssertionError("oversized chunked response should be rejected");
      } catch (final ExecutionException expected) {
        assert hasCauseMessage(expected, "body exceeds the 8-byte limit")
            : "expected bounded-reader failure";
      }
    }
  }

  @Test
  public void jdkExecutorRejectsDeclaredBodyAbovePerRequestLimit() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(500).setBody("123456789"));
      server.start(InetAddress.getByName("127.0.0.1"), 0);

      final TransportRequest request =
          TransportRequest.builder()
              .setMethod("GET")
              .setUri(server.url("/declared-overflow").uri())
              .setHeaders(Map.of())
              .setMaximumResponseBytes(8L)
              .build();
      try {
        new JavaHttpExecutor(JDK_CLIENT).execute(request).get(5, TimeUnit.SECONDS);
        throw new AssertionError("declared oversized response should be rejected");
      } catch (final ExecutionException expected) {
        assert hasCauseMessage(expected, "Content-Length 9 exceeds the 8-byte limit")
            : "expected declared-length precheck failure";
      }
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
}
