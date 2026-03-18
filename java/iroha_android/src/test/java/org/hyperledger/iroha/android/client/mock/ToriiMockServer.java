package org.hyperledger.iroha.android.client.mock;

import com.sun.net.httpserver.Headers;
import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.UncheckedIOException;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Lightweight HTTP server that mimics Torii's `/transaction` routes for tests.
 *
 * <p>The harness records requests for assertions and allows tests to enqueue deterministic responses
 * for both submissions and status polling, enabling end-to-end tests without depending on a real
 * Torii instance.
 */
public final class ToriiMockServer implements AutoCloseable {

  private static final MockResponse DEFAULT_SUBMIT_RESPONSE =
      MockResponse.bytes(
          202,
          new byte[] {0x01},
          Map.of("Content-Type", "application/x-norito"));
  private static final MockResponse DEFAULT_STATUS_RESPONSE = MockResponse.empty(404);

  private final HttpServer server;
  private final ExecutorService executor;
  private final URI baseUri;
  private final ConcurrentLinkedQueue<MockResponse> submitResponses = new ConcurrentLinkedQueue<>();
  private final ConcurrentHashMap<String, ConcurrentLinkedQueue<MockResponse>> statusResponses =
      new ConcurrentHashMap<>();
  private final CopyOnWriteArrayList<SubmitRequest> submissions = new CopyOnWriteArrayList<>();
  private final CopyOnWriteArrayList<StatusRequest> statusRequests = new CopyOnWriteArrayList<>();
  private final ConcurrentLinkedQueue<MockEventStream> eventStreams = new ConcurrentLinkedQueue<>();
  private final CopyOnWriteArrayList<EventStreamRequest> eventStreamRequests =
      new CopyOnWriteArrayList<>();

  private volatile MockResponse defaultSubmitResponse = DEFAULT_SUBMIT_RESPONSE;
  private volatile MockResponse defaultStatusResponse = DEFAULT_STATUS_RESPONSE;
  private volatile MockEventStream defaultEventStream = null;

  private ToriiMockServer() throws IOException {
    final int port = PortAllocator
        .findAvailablePort(0)
        .orElseThrow(() -> new IOException("no loopback port available"));
    this.server = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 0);
    this.executor = Executors.newCachedThreadPool();
    server.setExecutor(executor);
    server.createContext("/transaction", this::handleSubmit);
    server.createContext("/v1/pipeline/transactions/status", this::handleStatus);
    server.createContext("/v1/events/sse", this::handleEventsSse);
    server.start();
    final InetSocketAddress address = server.getAddress();
    this.baseUri =
        URI.create("http://" + address.getHostString() + ":" + Integer.toUnsignedString(address.getPort()));
  }

  private static int allocatePort() throws IOException {
    return PortAllocator
        .findAvailablePort(0)
        .orElseThrow(() -> new IOException("no loopback port available"));
  }

  /** Starts a new mock server instance bound to localhost on a random port. */
  public static ToriiMockServer create() throws IOException {
    return new ToriiMockServer();
  }

  /**
   * Attempts to create a mock server and returns an {@link Optional} that is empty when binding a
   * loopback port is not permitted in the current environment.
   */
  public static Optional<ToriiMockServer> tryCreate() {
    try {
      return Optional.of(new ToriiMockServer());
    } catch (final IOException | SecurityException ex) {
      return Optional.empty();
    }
  }

  /**
   * Returns {@code true} when the current environment allows binding localhost sockets required by
   * the mock server.
   */
  public static boolean isSupported() {
    try {
      final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
      server.stop(0);
      return true;
    } catch (final IOException ex) {
      return false;
    }
  }

  /** Returns the base URI (scheme + host + port) clients should use. */
  public URI baseUri() {
    return baseUri;
  }

  /** Overrides the default submission response used when no queued response exists. */
  public void setDefaultSubmitResponse(final MockResponse response) {
    this.defaultSubmitResponse = Objects.requireNonNull(response, "response");
  }

  /** Overrides the default status response used when no queued response exists for a hash. */
  public void setDefaultStatusResponse(final MockResponse response) {
    this.defaultStatusResponse = Objects.requireNonNull(response, "response");
  }

  /** Overrides the default SSE event stream served when the queue is empty. */
  public void setDefaultEventStream(final MockEventStream stream) {
    this.defaultEventStream = stream;
  }

  /** Queues a response that will be returned for the next transaction submission. */
  public void enqueueSubmitResponse(final MockResponse response) {
    submitResponses.add(Objects.requireNonNull(response, "response"));
  }

  /**
   * Queues a response for {@code hashHex}. Responses are delivered in FIFO order per hash when the
   * status endpoint is polled.
   */
  public void enqueueStatusResponse(final String hashHex, final MockResponse response) {
    Objects.requireNonNull(hashHex, "hashHex");
    Objects.requireNonNull(response, "response");
    statusResponses
        .computeIfAbsent(hashHex, key -> new ConcurrentLinkedQueue<>())
        .add(response);
  }

  /** Queues an SSE script used by the next `/v1/events/sse` connection. */
  public void enqueueEventStream(final MockEventStream stream) {
    eventStreams.add(Objects.requireNonNull(stream, "stream"));
  }

  /** Returns an immutable snapshot of recorded submission requests. */
  public List<SubmitRequest> submittedTransactions() {
    return List.copyOf(submissions);
  }

  /** Returns an immutable snapshot of recorded status requests. */
  public List<StatusRequest> statusRequests() {
    return List.copyOf(statusRequests);
  }

  /** Returns SSE requests observed by the mock server. */
  public List<EventStreamRequest> eventStreamRequests() {
    return List.copyOf(eventStreamRequests);
  }

  @Override
  public void close() {
    server.stop(0);
    executor.shutdownNow();
  }

  private void handleSubmit(final HttpExchange exchange) {
    try {
      if (!"POST".equalsIgnoreCase(exchange.getRequestMethod())) {
        respond(exchange, MockResponse.empty(405));
        return;
      }
      final byte[] body = exchange.getRequestBody().readAllBytes();
      submissions.add(
          new SubmitRequest(
              exchange.getRequestURI().getPath(), copyHeaders(exchange.getRequestHeaders()), body));
      final MockResponse response = submitResponses.poll();
      respond(exchange, response != null ? response : defaultSubmitResponse);
    } catch (final IOException ex) {
      throw new UncheckedIOException(ex);
    }
  }

  private void handleStatus(final HttpExchange exchange) {
    try {
      if (!"GET".equalsIgnoreCase(exchange.getRequestMethod())) {
        respond(exchange, MockResponse.empty(405));
        return;
      }
      final String hash = extractHashParameter(exchange);
      if (hash.isEmpty()) {
        respond(exchange, MockResponse.empty(400));
        return;
      }
      statusRequests.add(
          new StatusRequest(
              hash, exchange.getRequestURI().getPath(), copyHeaders(exchange.getRequestHeaders())));
      final MockResponse response = pollStatusResponse(hash);
      respond(exchange, response != null ? response : defaultStatusResponse);
    } catch (final IOException ex) {
      throw new UncheckedIOException(ex);
    }
  }

  private void handleEventsSse(final HttpExchange exchange) {
    try {
      if (!"GET".equalsIgnoreCase(exchange.getRequestMethod())) {
        respond(exchange, MockResponse.empty(405));
        return;
      }
      final MockEventStream streamPlan = nextEventStream();
      if (streamPlan == null) {
        respond(exchange, MockResponse.empty(503));
        return;
      }
      eventStreamRequests.add(
          new EventStreamRequest(
              exchange.getRequestURI().toString(), copyHeaders(exchange.getRequestHeaders())));
      final Headers headers = exchange.getResponseHeaders();
      headers.set("Content-Type", "text/event-stream");
      headers.set("Cache-Control", "no-cache");
      headers.set("Connection", "keep-alive");
      exchange.sendResponseHeaders(streamPlan.statusCode(), 0);
      try (OutputStream os = exchange.getResponseBody()) {
        streamPlan.replay(os);
      } catch (final InterruptedException interrupted) {
        Thread.currentThread().interrupt();
      }
    } catch (final IOException ex) {
      throw new UncheckedIOException(ex);
    }
  }

  private MockEventStream nextEventStream() {
    final MockEventStream script = eventStreams.poll();
    if (script != null) {
      return script;
    }
    return defaultEventStream;
  }

  private MockResponse pollStatusResponse(final String hash) {
    final ConcurrentLinkedQueue<MockResponse> queue = statusResponses.get(hash);
    if (queue == null) {
      return null;
    }
    final MockResponse response = queue.poll();
    if (queue.isEmpty()) {
      statusResponses.remove(hash, queue);
    }
    return response;
  }

  private static Map<String, List<String>> copyHeaders(final Headers headers) {
    final Map<String, List<String>> copy = new ConcurrentHashMap<>();
    headers.forEach((key, values) -> copy.put(key, List.copyOf(values)));
    return Collections.unmodifiableMap(copy);
  }

  private static String extractHashParameter(final HttpExchange exchange) {
    final String query = exchange.getRequestURI().getRawQuery();
    if (query == null || query.isEmpty()) {
      return "";
    }
    final String[] segments = query.split("&");
    for (final String segment : segments) {
      final int delimiter = segment.indexOf('=');
      if (delimiter <= 0) {
        continue;
      }
      final String key =
          URLDecoder.decode(segment.substring(0, delimiter), StandardCharsets.UTF_8);
      if (!"hash".equals(key)) {
        continue;
      }
      return URLDecoder.decode(segment.substring(delimiter + 1), StandardCharsets.UTF_8);
    }
    return "";
  }

  private static void respond(final HttpExchange exchange, final MockResponse response)
      throws IOException {
    final MockResponse resolved = response != null ? response : MockResponse.empty(500);
    resolved.headers().forEach((key, value) -> exchange.getResponseHeaders().set(key, value));
    final byte[] body = resolved.body();
    exchange.sendResponseHeaders(resolved.statusCode(), body.length);
    try (OutputStream os = exchange.getResponseBody()) {
      os.write(body);
    }
  }

  /** Recorded submission request. */
  public static final class SubmitRequest {
    private final String path;
    private final Map<String, List<String>> headers;
    private final byte[] body;

    private SubmitRequest(
        final String path, final Map<String, List<String>> headers, final byte[] body) {
      this.path = Objects.requireNonNull(path, "path");
      this.headers = Objects.requireNonNull(headers, "headers");
      this.body = body == null ? new byte[0] : body.clone();
    }

    public String path() {
      return path;
    }

    public Optional<String> header(final String name) {
      final List<String> values = headers.get(name);
      if (values == null || values.isEmpty()) {
        return Optional.empty();
      }
      return Optional.ofNullable(values.get(0));
    }

    public byte[] body() {
      return body.clone();
    }

    public String bodyUtf8() {
      return new String(body, StandardCharsets.UTF_8);
    }
  }

  /** Recorded status polling request. */
  public static final class StatusRequest {
    private final String hashHex;
    private final String path;
    private final Map<String, List<String>> headers;

    private StatusRequest(
        final String hashHex, final String path, final Map<String, List<String>> headers) {
      this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
      this.path = Objects.requireNonNull(path, "path");
      this.headers = Objects.requireNonNull(headers, "headers");
    }

    public String hashHex() {
      return hashHex;
    }

    public String path() {
      return path;
    }

    public Optional<String> header(final String name) {
      final List<String> values = headers.get(name);
      if (values == null || values.isEmpty()) {
        return Optional.empty();
      }
      return Optional.ofNullable(values.get(0));
    }
  }

  /** Recorded SSE request. */
  public static final class EventStreamRequest {
    private final String path;
    private final Map<String, List<String>> headers;

    private EventStreamRequest(final String path, final Map<String, List<String>> headers) {
      this.path = Objects.requireNonNull(path, "path");
      this.headers = Objects.requireNonNull(headers, "headers");
    }

    public String path() {
      return path;
    }

    public Optional<String> header(final String name) {
      final List<String> values = headers.get(name);
      if (values == null || values.isEmpty()) {
        return Optional.empty();
      }
      return Optional.ofNullable(values.get(0));
    }
  }

  /** Script describing an SSE response served by the mock server. */
  public static final class MockEventStream {
    private final int statusCode;
    private final List<MockSseFrame> frames;
    private final long tailDelayMs;

    private MockEventStream(
        final int statusCode, final List<MockSseFrame> frames, final long tailDelayMs) {
      this.statusCode = statusCode;
      this.frames = List.copyOf(frames);
      this.tailDelayMs = Math.max(0L, tailDelayMs);
    }

    public int statusCode() {
      return statusCode;
    }

    void replay(final OutputStream output) throws IOException, InterruptedException {
      final BufferedWriter writer =
          new BufferedWriter(new OutputStreamWriter(output, StandardCharsets.UTF_8));
      for (final MockSseFrame frame : frames) {
        frame.writeTo(writer);
        writer.flush();
        if (frame.delayMs() > 0) {
          Thread.sleep(frame.delayMs());
        }
      }
      if (tailDelayMs > 0) {
        Thread.sleep(tailDelayMs);
      }
      writer.flush();
    }

    public static Builder builder() {
      return new Builder();
    }

    public static MockEventStream empty() {
      return builder().build();
    }

    /** Builder for {@link MockEventStream}. */
    public static final class Builder {
      private int statusCode = 200;
      private final List<MockSseFrame> frames = new ArrayList<>();
      private long tailDelayMs = 0L;

      private Builder() {}

      public Builder statusCode(final int statusCode) {
        this.statusCode = statusCode;
        return this;
      }

      public Builder addFrame(final MockSseFrame frame) {
        frames.add(Objects.requireNonNull(frame, "frame"));
        return this;
      }

      public Builder addData(final String data) {
        return addFrame(MockSseFrame.builder().data(data).build());
      }

      public Builder closeDelay(final Duration duration) {
        if (duration != null) {
          this.tailDelayMs = Math.max(0L, duration.toMillis());
        }
        return this;
      }

      public MockEventStream build() {
        return new MockEventStream(statusCode, frames, tailDelayMs);
      }
    }
  }

  /** Single SSE frame emitted by {@link MockEventStream}. */
  public static final class MockSseFrame {
    private final String event;
    private final String data;
    private final boolean hasData;
    private final String id;
    private final String comment;
    private final Long retryMs;
    private final long delayMs;

    private MockSseFrame(final Builder builder) {
      this.event = builder.event;
      this.data = builder.data;
      this.hasData = builder.data != null;
      this.id = builder.id;
      this.comment = builder.comment;
      this.retryMs = builder.retryMs;
      this.delayMs = Math.max(0L, builder.delayMs);
    }

    long delayMs() {
      return delayMs;
    }

    void writeTo(final BufferedWriter writer) throws IOException {
      if (comment != null) {
        writer.write(": ");
        writer.write(comment);
        writer.newLine();
      }
      if (retryMs != null) {
        writer.write("retry: ");
        writer.write(Long.toString(retryMs));
        writer.newLine();
      }
      if (id != null) {
        writer.write("id: ");
        writer.write(id);
        writer.newLine();
      }
      if (event != null) {
        writer.write("event: ");
        writer.write(event);
        writer.newLine();
      }
      if (hasData) {
        final String[] lines = data.split("\\r?\\n", -1);
        for (final String line : lines) {
          writer.write("data");
          if (!line.isEmpty()) {
            writer.write(": ");
            writer.write(line);
          } else {
            writer.write(":");
          }
          writer.newLine();
        }
      }
      writer.newLine();
    }

    public static Builder builder() {
      return new Builder();
    }

    /** Builder for {@link MockSseFrame}. */
    public static final class Builder {
      private String event;
      private String data;
      private String id;
      private String comment;
      private Long retryMs;
      private long delayMs;

      private Builder() {}

      public Builder event(final String event) {
        this.event = event;
        return this;
      }

      public Builder data(final String data) {
        this.data = data;
        return this;
      }

      public Builder id(final String id) {
        this.id = id;
        return this;
      }

      public Builder comment(final String comment) {
        this.comment = comment;
        return this;
      }

      public Builder retry(final Duration duration) {
        if (duration != null) {
          this.retryMs = Math.max(0L, duration.toMillis());
        }
        return this;
      }

      public Builder delay(final Duration duration) {
        if (duration != null) {
          this.delayMs = Math.max(0L, duration.toMillis());
        }
        return this;
      }

      public MockSseFrame build() {
        return new MockSseFrame(this);
      }
    }
  }

  /** Simple value object describing HTTP responses returned by the mock server. */
  public static final class MockResponse {
    private final int statusCode;
    private final byte[] body;
    private final Map<String, String> headers;

    private MockResponse(final int statusCode, final byte[] body, final Map<String, String> headers) {
      this.statusCode = statusCode;
      this.body = body == null ? new byte[0] : body.clone();
      this.headers = Map.copyOf(headers);
    }

    public int statusCode() {
      return statusCode;
    }

    public byte[] body() {
      return body.clone();
    }

    public Map<String, String> headers() {
      return headers;
    }

    /** Convenience helper for JSON responses using UTF-8 encoding. */
    public static MockResponse json(final int statusCode, final String jsonBody) {
      Objects.requireNonNull(jsonBody, "jsonBody");
      return new MockResponse(
          statusCode,
          jsonBody.getBytes(StandardCharsets.UTF_8),
          Map.of("Content-Type", "application/json"));
    }

    /** Convenience helper for responses without a body. */
    public static MockResponse empty(final int statusCode) {
      return new MockResponse(statusCode, new byte[0], Map.of());
    }

    /** Creates a response with arbitrary bytes and optional headers. */
    public static MockResponse bytes(
        final int statusCode, final byte[] body, final Map<String, String> headers) {
      return new MockResponse(statusCode, body, headers == null ? Map.of() : headers);
    }
  }
}
