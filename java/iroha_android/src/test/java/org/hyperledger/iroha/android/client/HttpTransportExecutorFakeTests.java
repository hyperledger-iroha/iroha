package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import org.hyperledger.iroha.android.client.testing.FakeHttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class HttpTransportExecutorFakeTests {

  private HttpTransportExecutorFakeTests() {}

  public static void main(final String[] args) {
    shouldReturnPathSpecificResponses();
    shouldFallBackToGlobalQueue();
    shouldUseDefaultResponseWhenQueuesEmpty();
    shouldResetQueues();
    System.out.println("[IrohaAndroid] FakeHttpTransportExecutor tests passed.");
  }

  private static void shouldReturnPathSpecificResponses() {
    final FakeHttpTransportExecutor executor = new FakeHttpTransportExecutor();
    final TransportResponse pathResponse =
        new TransportResponse(202, "ok".getBytes(StandardCharsets.UTF_8), "", Map.of());
    executor.enqueueResponse("/transaction", pathResponse);

    final TransportRequest request =
        TransportRequest.builder()
            .setUri(URI.create("https://example.test/transaction"))
            .setMethod("POST")
            .build();

    final TransportResponse response = executor.execute(request).join();
    assert response.statusCode() == 202 : "path-specific response should be used";
    assert new String(response.body(), StandardCharsets.UTF_8).equals("ok")
        : "path-specific body should be preserved";
  }

  private static void shouldFallBackToGlobalQueue() {
    final FakeHttpTransportExecutor executor = new FakeHttpTransportExecutor();
    final TransportResponse globalResponse =
        new TransportResponse(201, "global".getBytes(StandardCharsets.UTF_8), "", Map.of());
    executor.enqueueResponse(globalResponse);

    final TransportRequest request =
        TransportRequest.builder()
            .setUri(URI.create("https://example.test/unused-path"))
            .setMethod("POST")
            .build();

    final TransportResponse response = executor.execute(request).join();
    assert response.statusCode() == 201 : "global queue response should be used when no path match";
  }

  private static void shouldUseDefaultResponseWhenQueuesEmpty() {
    final FakeHttpTransportExecutor executor = new FakeHttpTransportExecutor();
    executor.setDefaultResponse(new TransportResponse(503, new byte[0], "", Map.of()));

    final TransportRequest request =
        TransportRequest.builder()
            .setUri(URI.create("https://example.test/empty"))
            .setMethod("GET")
            .build();
    final TransportResponse response = executor.execute(request).join();
    assert response.statusCode() == 503 : "default response should be used when queues are empty";
  }

  private static void shouldResetQueues() {
    final FakeHttpTransportExecutor executor = new FakeHttpTransportExecutor();
    executor.enqueueResponse("/foo", new TransportResponse(200, new byte[0], "", Map.of()));
    executor.enqueueResponse(new TransportResponse(201, new byte[0], "", Map.of()));
    executor.clear();
    executor.setDefaultResponse(new TransportResponse(404, new byte[0], "", Map.of()));

    final TransportRequest request =
        TransportRequest.builder()
            .setUri(URI.create("https://example.test/after-clear"))
            .setMethod("GET")
            .build();
    final TransportResponse response = executor.execute(request).join();
    assert response.statusCode() == 404 : "queues should be empty after clear()";
  }
}
