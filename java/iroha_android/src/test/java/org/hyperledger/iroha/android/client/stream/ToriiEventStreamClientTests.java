package org.hyperledger.iroha.android.client.stream;

import com.sun.net.httpserver.Headers;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Smoke tests for {@link ToriiEventStreamClient}. */
public final class ToriiEventStreamClientTests {

  private ToriiEventStreamClientTests() {}

  public static void main(final String[] args) throws Exception {
    if (!SseServer.isSupported()) {
      System.out.println(
          "[IrohaAndroid] Torii SSE client tests skipped (loopback sockets disabled).");
      return;
    }
    sseStreamDeliversEvents();
    sseStreamEmitsRetryHints();
    sseStreamStopsAfterClose();
    closingBeforeResponseDoesNotEmitError();
    unwrapsCompletionExceptionFailures();
    observersReceiveLifecycleCallbacks();
    sseRejectsInsecureAuthorizationHeader();
    sseRequestPropagatesTimeout();
    System.out.println("[IrohaAndroid] Torii SSE client tests passed.");
  }

  private static void sseStreamDeliversEvents() throws Exception {
    try (SseServer server =
        SseServer.start(
            emitter -> {
              emitter.send("event: block\nid: abc123\ndata: line-1\ndata: line-2\n\n");
              Thread.sleep(25);
              emitter.send("data: keepalive\n\n");
            })) {
      final RecordingListener listener = new RecordingListener(2);
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);
      if (!listener.await(2, TimeUnit.SECONDS)) {
        throw new AssertionError("did not receive expected events");
      }
      stream.completion().get(1, TimeUnit.SECONDS);
      assertEquals(2, listener.events.size(), "expected two events");
      final ServerSentEvent first = listener.events.get(0);
      assertEquals("block", first.event(), "unexpected event name");
      assertEquals("abc123", first.id(), "unexpected event id");
      assertEquals("line-1\nline-2", first.data(), "unexpected event data");
      final ServerSentEvent second = listener.events.get(1);
      assertEquals("message", second.event(), "default event name should be message");
      assertEquals("keepalive", second.data(), "second event payload mismatch");
    }
  }

  private static void sseStreamEmitsRetryHints() throws Exception {
    try (SseServer server =
        SseServer.start(
            emitter -> {
              emitter.send("retry: 2500\n\n");
              emitter.send("data: done\n\n");
            })) {
      final RecordingListener listener = new RecordingListener(1);
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);
      if (!listener.await(2, TimeUnit.SECONDS)) {
        throw new AssertionError("retry test did not receive events");
      }
      stream.completion().get(1, TimeUnit.SECONDS);
      assertEquals(1, listener.retryHints.size(), "expected a retry hint");
      assertEquals(
          Duration.ofMillis(2500), listener.retryHints.get(0), "retry hint duration mismatch");
    }
  }

  private static void sseStreamStopsAfterClose() throws Exception {
    try (SseServer server =
        SseServer.start(
            emitter -> {
              for (int i = 0; i < 5; i++) {
                emitter.send("data: chunk-" + i + "\n\n");
                Thread.sleep(10);
              }
            })) {
      final RecordingListener listener = new RecordingListener(1);
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final ToriiEventStream stream =
          client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);
      if (!listener.await(2, TimeUnit.SECONDS)) {
        throw new AssertionError("close test did not receive initial event");
      }
      stream.close();
      stream.completion().get(1, TimeUnit.SECONDS);
      if (!listener.errors.isEmpty()) {
        throw new AssertionError("expected no errors when closing stream");
      }
    }
  }

  private static void closingBeforeResponseDoesNotEmitError() throws Exception {
    final CompletableFuture<TransportResponse> pending = new CompletableFuture<>();
    final RecordingObserver observer = new RecordingObserver();
    final TransportExecutor executor = request -> pending;
    final RecordingListener listener = new RecordingListener(0);
    final ToriiEventStreamClient client =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://example.com/base"))
            .setTransportExecutor(executor)
            .addObserver(observer)
            .build();
    final ToriiEventStream stream =
        client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);
    stream.close();
    stream.completion().get(1, TimeUnit.SECONDS);
    if (!pending.isCancelled()) {
      throw new AssertionError("expected pending SSE request to be cancelled");
    }
    if (!listener.errors.isEmpty()) {
      throw new AssertionError("unexpected errors after closing SSE stream");
    }
    if (observer.failureCount != 0) {
      throw new AssertionError("observer should not record failures after cancel");
    }
  }

  private static void unwrapsCompletionExceptionFailures() throws Exception {
    final IOException boom = new IOException("boom");
    final CompletableFuture<TransportResponse> failed = new CompletableFuture<>();
    failed.completeExceptionally(new java.util.concurrent.CompletionException(boom));
    final RecordingObserver observer = new RecordingObserver();
    final TransportExecutor executor = request -> failed;
    final RecordingListener listener = new RecordingListener(0);
    final ToriiEventStreamClient client =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://example.com/base"))
            .setTransportExecutor(executor)
            .addObserver(observer)
            .build();
    final ToriiEventStream stream =
        client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), listener);
    try {
      stream.completion().get(1, TimeUnit.SECONDS);
    } catch (Exception ignored) {
      // Completion is expected to fail.
    }
    if (listener.errors.size() != 1 || listener.errors.get(0) != boom) {
      throw new AssertionError("expected unwrapped error for listener");
    }
    if (observer.failureCount != 1 || observer.lastFailure.orElse(null) != boom) {
      throw new AssertionError("expected unwrapped error for observer");
    }
  }

  private static void observersReceiveLifecycleCallbacks() throws Exception {
    final RecordingObserver observer = new RecordingObserver();
    final TransportExecutor executor =
        request -> {
          final TransportResponse response =
              TransportResponse.builder()
                  .setStatusCode(200)
                  .setBody("data: ok\n\n".getBytes(StandardCharsets.UTF_8))
                  .setMessage("OK")
                  .addHeader("Content-Type", "text/event-stream")
                  .build();
          return CompletableFuture.completedFuture(response);
        };

    final RecordingListener listener = new RecordingListener(1);
    final ToriiEventStreamClient client =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://example.com/base"))
            .setTransportExecutor(executor)
            .putDefaultHeader("X-Test", "true")
            .addObserver(observer)
            .build();
    final ToriiEventStream stream =
        client.openSseStream(
            "/events?kind=blocks", ToriiEventStreamOptions.defaultOptions(), listener);
    stream.completion().get(1, TimeUnit.SECONDS);

    if (observer.requestCount != 1 || observer.responseCount != 1 || observer.failureCount != 0) {
      throw new AssertionError("observer counts mismatch: " + observer);
    }
    final TransportRequest recorded =
        observer.lastRequest.orElseThrow(() -> new AssertionError("missing observed request"));
    final List<String> acceptHeader = recorded.headers().get("Accept");
    if (acceptHeader == null || !acceptHeader.contains("text/event-stream")) {
      throw new AssertionError("missing Accept: text/event-stream header");
    }
    final List<String> customHeader = recorded.headers().get("X-Test");
    if (customHeader == null || !customHeader.contains("true")) {
      throw new AssertionError("default headers not propagated to request");
    }
    if (!recorded.uri().toString().equals("http://example.com/base/events?kind=blocks")) {
      throw new AssertionError("unexpected SSE target URI: " + recorded.uri());
    }
    if (listener.events.isEmpty() || !"ok".equals(listener.events.get(0).data())) {
      throw new AssertionError("expected SSE payload to be parsed");
    }
  }

  private static void sseRejectsInsecureAuthorizationHeader() {
    final TransportExecutor executor =
        request ->
            CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(200).setBody(new byte[0]).build());
    final ToriiEventStreamClient client =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://example.com/base"))
            .setTransportExecutor(executor)
            .putDefaultHeader("Authorization", "Bearer token")
            .build();
    try {
      client.openSseStream("/events", ToriiEventStreamOptions.defaultOptions(), event -> {});
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("refuses insecure transport")
          : "expected insecure transport rejection";
      return;
    }
    throw new AssertionError("Expected insecure credentialed SSE request to be rejected");
  }

  private static void sseRequestPropagatesTimeout() throws Exception {
    final RecordingObserver observer = new RecordingObserver();
    final TransportExecutor executor =
        request -> {
          final TransportResponse response =
              TransportResponse.builder()
                  .setStatusCode(200)
                  .setBody("data: ok\n\n".getBytes(StandardCharsets.UTF_8))
                  .setMessage("OK")
                  .addHeader("Content-Type", "text/event-stream")
                  .build();
          return CompletableFuture.completedFuture(response);
        };

    final RecordingListener listener = new RecordingListener(1);
    final ToriiEventStreamClient client =
        ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("http://example.com/base"))
            .setTransportExecutor(executor)
            .addObserver(observer)
            .build();
    final Duration timeout = Duration.ofSeconds(5);
    final ToriiEventStreamOptions options =
        ToriiEventStreamOptions.builder().setTimeout(timeout).build();
    final ToriiEventStream stream = client.openSseStream("/events", options, listener);
    stream.completion().get(1, TimeUnit.SECONDS);

    final TransportRequest recorded =
        observer.lastRequest.orElseThrow(() -> new AssertionError("missing observed request"));
    assertEquals(timeout, recorded.timeout(), "timeout should propagate to request");
  }

  private static void assertEquals(
      final Object expected, final Object actual, final String message) {
    if (expected == null ? actual != null : !expected.equals(actual)) {
      throw new AssertionError(message + " (expected=" + expected + ", actual=" + actual + ")");
    }
  }

  private interface SseHandler {
    void handle(OutputEmitter emitter) throws Exception;
  }

  private static final class SseServer implements AutoCloseable {
    private final HttpServer server;
    private final URI baseUri;
    private final SseHandler handler;

    private SseServer(final HttpServer server, final URI baseUri, final SseHandler handler) {
      this.server = server;
      this.baseUri = baseUri;
      this.handler = handler;
    }

    static SseServer start(final SseHandler handler) throws IOException {
      final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
      final InetSocketAddress address = server.getAddress();
      final URI baseUri =
          URI.create(
              "http://" + address.getHostString() + ":" + Integer.toUnsignedString(address.getPort()));
      final SseServer wrapper = new SseServer(server, baseUri, handler);
      server.createContext("/events", wrapper::handleExchange);
      server.start();
      return wrapper;
    }

    static boolean isSupported() {
      try {
        final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        server.stop(0);
        return true;
      } catch (final IOException | SecurityException ex) {
        return false;
      }
    }

    URI baseUri() {
      return baseUri;
    }

    private void handleExchange(final HttpExchange exchange) throws IOException {
      final Headers headers = exchange.getResponseHeaders();
      headers.set("Content-Type", "text/event-stream");
      headers.set("Cache-Control", "no-cache");
      exchange.sendResponseHeaders(200, 0);
      try (OutputStream output = exchange.getResponseBody()) {
        handler.handle(new OutputEmitter(output));
      } catch (final Exception ex) {
        throw new IOException(ex);
      }
    }

    @Override
    public void close() {
      server.stop(0);
    }
  }

  private static final class OutputEmitter {
    private final OutputStream output;

    OutputEmitter(final OutputStream output) {
      this.output = output;
    }

    void send(final String payload) throws IOException {
      output.write(payload.getBytes(StandardCharsets.UTF_8));
      output.flush();
    }
  }

  private static final class RecordingListener implements ToriiEventStreamListener {
    final List<ServerSentEvent> events = new ArrayList<>();
    final List<Duration> retryHints = new ArrayList<>();
    final List<Throwable> errors = new ArrayList<>();
    private final CountDownLatch latch;
    private final AtomicBoolean closed = new AtomicBoolean(false);

    RecordingListener(final int expectedEvents) {
      this.latch = new CountDownLatch(expectedEvents);
    }

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
      latch.countDown();
    }

    @Override
    public void onRetryHint(final Duration duration) {
      retryHints.add(duration);
    }

    @Override
    public void onClosed() {
      closed.set(true);
    }

    @Override
    public void onError(final Throwable error) {
      errors.add(error);
    }

    boolean await(final long timeout, final TimeUnit unit) throws InterruptedException {
      return latch.await(timeout, unit);
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    int requestCount = 0;
    int responseCount = 0;
    int failureCount = 0;
    java.util.Optional<TransportRequest> lastRequest = java.util.Optional.empty();
    java.util.Optional<ClientResponse> lastResponse = java.util.Optional.empty();
    java.util.Optional<Throwable> lastFailure = java.util.Optional.empty();

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount++;
      lastRequest = java.util.Optional.of(request);
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responseCount++;
      lastResponse = java.util.Optional.of(response);
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount++;
      lastFailure = java.util.Optional.ofNullable(error);
    }

    @Override
    public String toString() {
      return "RecordingObserver{"
          + "requestCount="
          + requestCount
          + ", responseCount="
          + responseCount
          + ", failureCount="
          + failureCount
          + '}';
    }
  }
}
