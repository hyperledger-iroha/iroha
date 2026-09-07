package org.hyperledger.iroha.sdk.client.transport;

import static org.junit.jupiter.api.Assertions.*;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.Executor;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.Cache;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import okhttp3.mockwebserver.SocketPolicy;
import okio.Buffer;
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner;
import org.hyperledger.iroha.sdk.client.ClientConfig;
import org.hyperledger.iroha.sdk.client.HttpClientTransport;
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor;
import org.hyperledger.iroha.sdk.client.RequestSigner;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/** Java applications use the Kotlin-owned adapter with explicit resource lifetimes. */
final class OkHttpTransportJavaConsumerTest {
  private static final long TIMESTAMP = 1_717_171_717_001L;
  private static final String NONCE = "java-http-consumer-1";

  @TempDir Path temporary;

  @Test
  void signedPostPreservesExactBytesAndHeadersWithoutFollowingRedirects() throws Exception {
    assertSignedDispatch("POST", 307);
  }

  @Test
  void signedGetReturnsRetryAfterWithoutASecondDispatch() throws Exception {
    assertSignedDispatch("GET", 503);
  }

  private static void assertSignedDispatch(String method, int status) throws Exception {
    try (MockWebServer server = new MockWebServer();
         OkHttpTransportExecutor transport = OkHttpTransportExecutor.create()) {
      server.enqueue(new MockResponse().setResponseCode(status)
          .setHeader("Location", "/must-not-follow").setHeader("Retry-After", "0")
          .setBody("original response"));
      server.enqueue(new MockResponse().setBody("a replay would be wrong"));
      final URI uri = server.url("/v1/signed?b=two%20words&a=1").uri();
      final byte[] body = method.equals("POST") ? new byte[] {0, 42, -1, 10} : new byte[0];
      final byte[] expectedBody = body.clone();
      final KeyPair keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      final Map<String, String> signed = CanonicalRequestSigner.buildHeaders(
          network(), method, uri, body, "alice@universal", RequestSigner.ed25519(keys.getPrivate()),
          TIMESTAMP, NONCE);
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      signed.forEach((name, value) -> headers.put(name, new ArrayList<>(Collections.singletonList(value))));
      headers.put("Content-Type", new ArrayList<>(Collections.singletonList("application/x-norito")));
      headers.put("X-Java-Order", new ArrayList<>(Arrays.asList("first", "second")));
      final TransportRequest request = new TransportRequest(method, uri, headers, body);
      Arrays.fill(body, (byte) 9);
      headers.values().forEach(List::clear);
      headers.clear();
      Arrays.fill(request.getBody(), (byte) 8);

      final TransportResponse response = transport.execute(request).get(5, TimeUnit.SECONDS);
      assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy);
      assertEquals(status, response.statusCode);
      assertEquals(uri, response.finalUri);
      assertFalse(response.redirected);
      assertEquals(Collections.singletonList("0"), response.getHeaders().get("Retry-After"));
      assertEquals("original response", new String(response.getBody(), StandardCharsets.UTF_8));
      final RecordedRequest observed = server.takeRequest(2, TimeUnit.SECONDS);
      assertNotNull(observed);
      assertEquals(method, observed.getMethod());
      assertEquals(uri.getRawPath() + "?" + uri.getRawQuery(), observed.getPath());
      final byte[] observedBody = observed.getBody().readByteArray();
      assertArrayEquals(expectedBody, observedBody);
      signed.forEach((name, value) -> assertEquals(value, observed.getHeader(name)));
      assertEquals(Arrays.asList("first", "second"), observed.getHeaders().values("X-Java-Order"));
      final Signature verifier = Signature.getInstance("Ed25519");
      verifier.initVerify(keys.getPublic());
      verifier.update(CanonicalRequestSigner.canonicalRequestSignatureMessage(
          network(), observed.getMethod(), observed.getRequestUrl().uri(), observedBody, TIMESTAMP, NONCE));
      assertTrue(verifier.verify(Base64.getDecoder().decode(
          observed.getHeader(CanonicalRequestSigner.HEADER_SIGNATURE))));
      assertNull(server.takeRequest(150, TimeUnit.MILLISECONDS), "signed bytes must never replay");
      assertEquals(1, server.getRequestCount());
    }
  }

  @Test
  void signedConnectionLossNeverReplaysTheRequest() throws Exception {
    try (MockWebServer server = new MockWebServer();
         OkHttpTransportExecutor transport = OkHttpTransportExecutor.create()) {
      server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AFTER_REQUEST));
      server.enqueue(new MockResponse().setResponseCode(202));
      final TransportRequest request = TransportRequest.builder().setMethod("POST")
          .setUri(server.url("/v1/submit").uri()).setBody(new byte[] {0, 1, -1})
          .addHeader("X-Iroha-Signature", "one-shot-signature-fixture").build();
      final ExecutionException failure = assertThrows(ExecutionException.class,
          () -> transport.execute(request).get(5, TimeUnit.SECONDS));
      assertTrue(failure.getCause() instanceof IOException);
      final RecordedRequest observed = server.takeRequest(2, TimeUnit.SECONDS);
      assertNotNull(observed);
      assertArrayEquals(request.getBody(), observed.getBody().readByteArray());
      assertNull(server.takeRequest(150, TimeUnit.MILLISECONDS));
      assertEquals(1, server.getRequestCount());
    }
  }

  @Test
  void borrowedClientRetainsItsOtherCallsPoolDispatcherAndCache() throws Exception {
    final Cache cache = new Cache(temporary.resolve("http-cache").toFile(), 1024L * 1024L);
    final OkHttpClient shared = new OkHttpClient.Builder().cache(cache).build();
    try {
      try (MockWebServer server = new MockWebServer();
           OkHttpTransportExecutor first = new OkHttpTransportExecutor(shared);
           OkHttpTransportExecutor second = new OkHttpTransportExecutor(shared)) {
        server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE));
        server.enqueue(new MockResponse().setBody("unrelated").setBodyDelay(250, TimeUnit.MILLISECONDS));
        server.enqueue(new MockResponse().setBody("shared client remains usable"));
        final CompletableFuture<TransportResponse> cancelled = first.execute(request(server, "/first"));
        assertNotNull(server.takeRequest(2, TimeUnit.SECONDS));
        final CompletableFuture<TransportResponse> retained = second.execute(request(server, "/second"));
        assertNotNull(server.takeRequest(2, TimeUnit.SECONDS));
        first.close();
        first.close();
        assertTrue(cancelled.isCancelled());
        assertEquals("unrelated", new String(retained.get(5, TimeUnit.SECONDS).getBody(), StandardCharsets.UTF_8));
        assertFalse(shared.dispatcher().executorService().isShutdown());
        assertFalse(cache.isClosed());
        try (Response direct = shared.newCall(new Request.Builder().url(server.url("/direct")).build()).execute()) {
          assertEquals("shared client remains usable", direct.body().string());
        }
        final RecordedRequest reused = server.takeRequest(2, TimeUnit.SECONDS);
        assertNotNull(reused);
        assertEquals(1, reused.getSequenceNumber(), "closing a borrowed adapter must not evict the shared connection");
      }
    } finally {
      closeClient(shared);
    }
  }

  @Test
  void futureCancellationStopsIoAndCloseRejectsBothJavaEntryPoints() throws Exception {
    final OkHttpClient shared = new OkHttpClient();
    try {
      try (MockWebServer server = new MockWebServer();
           OkHttpTransportExecutor transport = new OkHttpTransportExecutor(shared)) {
        server.enqueue(new MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE));
        final TransportRequest request = request(server, "/cancel");
        final CompletableFuture<TransportResponse> pending = transport.execute(request);
        assertNotNull(server.takeRequest(2, TimeUnit.SECONDS));
        assertTrue(pending.cancel(true));
        awaitIdle(shared);
        assertTrue(pending.isCancelled());
        transport.close();
        transport.close();
        assertClosed(transport.execute(request));
        assertClosed(transport.openStream(request));
        assertEquals(1, server.getRequestCount());
        assertFalse(shared.dispatcher().executorService().isShutdown());
      }
    } finally {
      closeClient(shared);
    }
  }

  @Test
  void cancellingAJavaScheduledRequestPreventsNetworkAdmission() throws Exception {
    final List<Runnable> scheduled = new ArrayList<>();
    try (MockWebServer server = new MockWebServer();
         OkHttpTransportExecutor transport = OkHttpTransportExecutor.create(null, null, scheduled::add)) {
      server.start();
      final CompletableFuture<TransportResponse> pending = transport.execute(request(server, "/queued"));
      assertEquals(1, scheduled.size());
      assertTrue(pending.cancel(false));
      scheduled.get(0).run();
      assertTrue(pending.isCancelled());
      assertEquals(0, server.getRequestCount());
    }
  }

  @Test
  void javaInputStreamCloseOwnsSseDisposalAndHeadersAreImmutable() throws Exception {
    final OkHttpClient shared = new OkHttpClient();
    try {
      try (MockWebServer server = new MockWebServer();
           OkHttpTransportExecutor transport = new OkHttpTransportExecutor(shared)) {
        server.enqueue(new MockResponse().setHeader("Content-Type", "text/event-stream")
            .setHeader("X-Stream", "owned").setBody("data: ready\n\n"));
        final TransportStreamResponse response = transport.openStream(request(server, "/events"))
            .get(5, TimeUnit.SECONDS);
        assertEquals(200, response.statusCode);
        assertEquals("data: ready", new BufferedReader(new InputStreamReader(
            response.getBody(), StandardCharsets.UTF_8)).readLine());
        assertThrows(UnsupportedOperationException.class, () -> response.getHeaders().clear());
        assertThrows(UnsupportedOperationException.class, () -> response.getHeaders().get("x-stream").clear());
        response.getBody().close();
        response.close();
        response.close();
        assertThrows(IOException.class, () -> response.getBody().read());
        awaitIdle(shared);
        assertFalse(shared.dispatcher().executorService().isShutdown());
      }
    } finally {
      closeClient(shared);
    }
  }

  @Test
  void ownedFactoriesAndExplicitClientInjectionAreJavaCallable() throws Exception {
    final ExecutorService scheduling = Executors.newSingleThreadExecutor(r -> new Thread(r, "java-http-consumer"));
    final AtomicReference<String> worker = new AtomicReference<>();
    final Executor executor = task -> scheduling.execute(() -> {
      worker.set(Thread.currentThread().getName());
      task.run();
    });
    final byte[] bytes = new byte[] {1, 2, 3};
    try {
      try (MockWebServer server = new MockWebServer();
           OkHttpTransportExecutor adapter = OkHttpTransportExecutor.create(
               Duration.ofSeconds(2), Duration.ofSeconds(3), executor, 64L)) {
        server.enqueue(new MockResponse().setHeader("Content-Type", "application/x-norito")
            .setBody(new Buffer().write(bytes)));
        server.enqueue(new MockResponse().setHeader("Content-Type", "application/x-norito")
            .setBody(new Buffer().write(bytes)));
        final AtomicInteger executions = new AtomicInteger();
        final HttpTransportExecutor injected = new HttpTransportExecutor() {
          @Override public CompletableFuture<TransportResponse> execute(TransportRequest request) {
            executions.incrementAndGet();
            return adapter.execute(request);
          }
          @Override public void close() { adapter.close(); }
        };
        final ClientConfig config = ClientConfig.builder().setBaseUri(server.url("/").uri()).build();
        try (HttpClientTransport client = new HttpClientTransport(injected, config)) {
          assertArrayEquals(bytes, client.getLedgerExecutedBlockWire(1L).get(5, TimeUnit.SECONDS));
          assertEquals(1, executions.get());
          assertEquals("/v1/ledger/block/1", server.takeRequest(2, TimeUnit.SECONDS).getPath());
          assertEquals("java-http-consumer", worker.get());
        }
        try (HttpClientTransport owned = HttpClientTransport.createDefault(config, executor)) {
          assertArrayEquals(bytes, owned.getLedgerExecutedBlockWire(2L).get(5, TimeUnit.SECONDS));
          assertEquals("/v1/ledger/block/2", server.takeRequest(2, TimeUnit.SECONDS).getPath());
        }
      }
      assertFalse(scheduling.isShutdown());
      assertEquals(42, scheduling.submit(() -> 42).get(2, TimeUnit.SECONDS).intValue());
    } finally {
      scheduling.shutdownNow();
    }
  }

  private static NetworkId network() {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) 1);
    return NetworkId.fromBytes(bytes);
  }

  private static TransportRequest request(MockWebServer server, String path) {
    return TransportRequest.builder().setUri(server.url(path).uri()).build();
  }

  private static void assertClosed(CompletableFuture<?> future) {
    final ExecutionException failure = assertThrows(ExecutionException.class, () -> future.get(2, TimeUnit.SECONDS));
    assertTrue(failure.getCause() instanceof IllegalStateException);
    assertEquals("HTTP transport is closed", failure.getCause().getMessage());
  }

  private static void awaitIdle(OkHttpClient client) throws InterruptedException {
    final long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
    while (client.dispatcher().runningCallsCount() != 0 && System.nanoTime() < deadline) {
      Thread.sleep(10);
    }
    assertEquals(0, client.dispatcher().runningCallsCount());
  }

  private static void closeClient(OkHttpClient client) throws IOException {
    client.dispatcher().cancelAll();
    client.connectionPool().evictAll();
    if (client.cache() != null) client.cache().close();
    client.dispatcher().executorService().shutdownNow();
  }
}
