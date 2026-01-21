package org.hyperledger.iroha.android.client.mock;

import java.net.HttpURLConnection;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.client.transport.UrlConnectionTransportExecutor;
import org.hyperledger.iroha.android.client.stream.ServerSentEvent;
import org.hyperledger.iroha.android.client.stream.ToriiEventStream;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamClient;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamListener;
import org.hyperledger.iroha.android.client.stream.ToriiEventStreamOptions;

/** Unit tests covering the Torii mock server harness. */
public final class ToriiMockServerTests {

  private ToriiMockServerTests() {}

  public static void main(final String[] args) throws Exception {
    if (!ToriiMockServer.isSupported()) {
      System.out.println(
          "[IrohaAndroid] Torii mock server tests skipped (mock server cannot bind in this environment).");
      return;
    }
    recordsSubmitRequests();
    servesQueuedStatusResponsesPerHash();
    servesEventStreams();
    System.out.println("[IrohaAndroid] Torii mock server tests passed.");
  }

  private static void recordsSubmitRequests() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println(
          "[IrohaAndroid] Torii mock server tests skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      final URI submitUri = server.baseUri().resolve("/transaction");
      server.enqueueSubmitResponse(
          ToriiMockServer.MockResponse.bytes(
              202,
              new byte[] {0x01},
              Map.of("Content-Type", "application/x-norito")));

      final HttpResponseData response =
          sendRequest(
              submitUri,
              "POST",
              new byte[] {0x01, 0x02, 0x03},
              Map.of(
                  "Content-Type", "application/x-norito",
                  "X-Test", "harness"));
      assert response.statusCode() == 202 : "Mock server must honour queued response";

      final var submissions = server.submittedTransactions();
      assert submissions.size() == 1 : "Submission should be recorded";
      final var submission = submissions.get(0);
      assert "/transaction".equals(submission.path())
          : "Submission path should match Torii endpoint";
      assert Arrays.equals(submission.body(), new byte[] {0x01, 0x02, 0x03})
          : "Submission body should be captured";
    }
  }

  private static void servesQueuedStatusResponsesPerHash() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println(
          "[IrohaAndroid] Torii mock server tests skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      final String hashHex = "deadbeef";
      final URI statusUri =
          server.baseUri().resolve("/v1/pipeline/transactions/status?hash=" + hashHex);

      server.enqueueStatusResponse(
          hashHex, ToriiMockServer.MockResponse.json(202, statusPayload("Pending")));
      server.enqueueStatusResponse(
          hashHex, ToriiMockServer.MockResponse.json(200, statusPayload("Committed")));

      final HttpResponseData first = sendRequest(statusUri, "GET", null, Map.of());
      assert first.statusCode() == 202 : "First queued status should be returned";
      assert first.body().contains("Pending") : "Expected pending payload";

      final HttpResponseData second = sendRequest(statusUri, "GET", null, Map.of());
      assert second.statusCode() == 200 : "Second queued status should be returned";
      assert second.body().contains("Committed") : "Expected committed payload";

      final HttpResponseData third = sendRequest(statusUri, "GET", null, Map.of());
      assert third.statusCode() == 404 : "Default status should be returned when queue is empty";

      final var requests = server.statusRequests();
      assert requests.size() == 3 : "All polls should be recorded";
      assert hashHex.equals(requests.get(0).hashHex())
          : "Recorded hash hex should match query parameter";
    }
  }

  private static void servesEventStreams() throws Exception {
    final var maybeServer = ToriiMockServer.tryCreate();
    if (maybeServer.isEmpty()) {
      System.out.println(
          "[IrohaAndroid] Torii mock server tests skipped (mock server unavailable).");
      return;
    }
    try (ToriiMockServer server = maybeServer.get()) {
      server.enqueueEventStream(
          ToriiMockServer.MockEventStream.builder()
              .addFrame(
                  ToriiMockServer.MockSseFrame.builder()
                      .event("transactions")
                      .id("evt-1")
                      .data("{\"status\":\"Committed\"}")
                      .build())
              .build());
      final ToriiEventStreamClient client =
          ToriiEventStreamClient.builder()
              .setBaseUri(server.baseUri())
              .setTransportExecutor(new UrlConnectionTransportExecutor())
              .build();
      final RecordingListener listener = new RecordingListener();
      try (ToriiEventStream stream =
          client.openSseStream("/v1/events/sse", ToriiEventStreamOptions.defaultOptions(), listener)) {
        stream.completion().join();
      }
      final List<ServerSentEvent> events = listener.events();
      assert listener.opened() : "SSE stream should open successfully";
      assert listener.closed() : "SSE stream should close when script completes";
      assert !listener.failed() : "SSE listener should not observe failures";
      assert events.size() == 1 : "Exactly one SSE frame should be delivered";
      assert events.get(0).data().contains("Committed")
          : "Event payload should match queued script";
      final var requests = server.eventStreamRequests();
      assert requests.size() == 1 : "SSE request should be recorded";
      assert requests.get(0).path().contains("/v1/events/sse")
          : "Recorded SSE path should match endpoint";
    }
  }

  private static String statusPayload(final String kind) {
    return "{\"kind\":\"Transaction\",\"content\":{\"status\":{\"kind\":\""
        + kind + "\"}}}";
  }

  private static HttpResponseData sendRequest(
      final URI uri,
      final String method,
      final byte[] body,
      final Map<String, String> headers)
      throws Exception {
    final HttpURLConnection connection = (HttpURLConnection) uri.toURL().openConnection();
    connection.setInstanceFollowRedirects(false);
    connection.setRequestMethod(method);
    if (headers != null) {
      headers.forEach(connection::setRequestProperty);
    }
    if (body != null) {
      connection.setDoOutput(true);
      try (var output = connection.getOutputStream()) {
        output.write(body);
      }
    }
    final int status = connection.getResponseCode();
    final String responseBody = readBody(connection);
    connection.disconnect();
    return new HttpResponseData(status, responseBody);
  }

  private static String readBody(final HttpURLConnection connection) throws Exception {
    final var stream =
        connection.getErrorStream() != null ? connection.getErrorStream() : connection.getInputStream();
    if (stream == null) {
      return "";
    }
    try (stream) {
      return new String(stream.readAllBytes(), StandardCharsets.UTF_8);
    }
  }

  private static final class HttpResponseData {
    private final int statusCode;
    private final String body;

    private HttpResponseData(final int statusCode, final String body) {
      this.statusCode = statusCode;
      this.body = body == null ? "" : body;
    }

    int statusCode() {
      return statusCode;
    }

    String body() {
      return body;
    }
  }

  private static final class RecordingListener implements ToriiEventStreamListener {
    private final List<ServerSentEvent> events = new CopyOnWriteArrayList<>();
    private final AtomicBoolean opened = new AtomicBoolean(false);
    private final AtomicBoolean closed = new AtomicBoolean(false);
    private final AtomicBoolean failed = new AtomicBoolean(false);

    @Override
    public void onOpen() {
      opened.set(true);
    }

    @Override
    public void onEvent(final ServerSentEvent event) {
      events.add(event);
    }

    @Override
    public void onRetryHint(final Duration duration) {
      // no-op for tests
    }

    @Override
    public void onClosed() {
      closed.set(true);
    }

    @Override
    public void onError(final Throwable error) {
      failed.set(true);
    }

    public boolean opened() {
      return opened.get();
    }

    public boolean closed() {
      return closed.get();
    }

    public boolean failed() {
      return failed.get();
    }

    public List<ServerSentEvent> events() {
      return List.copyOf(events);
    }
  }
}
