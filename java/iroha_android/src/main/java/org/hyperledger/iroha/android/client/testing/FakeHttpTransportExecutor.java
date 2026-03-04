package org.hyperledger.iroha.android.client.testing;

import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/**
 * In-memory {@link HttpTransportExecutor} that returns pre-seeded responses without performing
 * network I/O. Useful for deterministic tests on both Android and JVM targets.
 */
public final class FakeHttpTransportExecutor implements HttpTransportExecutor {

  private final ConcurrentLinkedQueue<TransportResponse> globalResponses =
      new ConcurrentLinkedQueue<>();
  private final ConcurrentHashMap<String, ConcurrentLinkedQueue<TransportResponse>> pathResponses =
      new ConcurrentHashMap<>();

  private volatile TransportResponse defaultResponse =
      new TransportResponse(200, new byte[0], "", Map.of());

  /** Enqueue a response that will be returned for any request when no path-specific response exists. */
  public void enqueueResponse(final TransportResponse response) {
    globalResponses.add(Objects.requireNonNull(response, "response"));
  }

  /** Enqueue a response for the given request path (e.g., {@code /transaction}). */
  public void enqueueResponse(final String path, final TransportResponse response) {
    Objects.requireNonNull(path, "path");
    pathResponses
        .computeIfAbsent(path, ignored -> new ConcurrentLinkedQueue<>())
        .add(Objects.requireNonNull(response, "response"));
  }

  /** Sets the fallback response used when no queued response is available. */
  public void setDefaultResponse(final TransportResponse response) {
    this.defaultResponse = Objects.requireNonNull(response, "response");
  }

  /** Clears queued responses and restores the queues to an empty state. */
  public void clear() {
    globalResponses.clear();
    pathResponses.clear();
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    final String path = request.uri().getPath();
    TransportResponse response = poll(pathResponses.get(path));
    if (response == null) {
      response = globalResponses.poll();
    }
    if (response == null) {
      response = defaultResponse;
    }
    return CompletableFuture.completedFuture(
        new TransportResponse(
            response.statusCode(), response.body(), response.message(), response.headers()));
  }

  private static TransportResponse poll(final ConcurrentLinkedQueue<TransportResponse> queue) {
    return queue == null ? null : queue.poll();
  }
}
