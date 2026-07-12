package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.junit.Test;

/** Adversarial parity tests for the SCCP recent-message query window. */
public final class SccpRecentQueryValidationTests {

  @Test
  public void invalidWindowsAreRejectedBeforeHttpExecution() {
    final CountingExecutor executor = new CountingExecutor();
    final HttpClientTransport transport = transport(executor);

    for (final BigInteger from :
        List.of(
            BigInteger.valueOf(Long.MIN_VALUE),
            BigInteger.valueOf(-1),
            BigInteger.ZERO,
            BigInteger.ONE.shiftLeft(64))) {
      assertThrows(
          "from=" + from,
          IllegalArgumentException.class,
          () -> transport.getSccpRecentMessages(from, null, 1));
    }
    for (final int limit : new int[] {Integer.MIN_VALUE, -1, 0, 51, Integer.MAX_VALUE}) {
      assertThrows(
          "limit=" + limit,
          IllegalArgumentException.class,
          () -> transport.getSccpRecentMessages(BigInteger.ONE, null, limit));
    }
    for (final int afterIndex : new int[] {Integer.MIN_VALUE, -1, 512, Integer.MAX_VALUE}) {
      assertThrows(
          "afterIndex=" + afterIndex,
          IllegalArgumentException.class,
          () -> transport.getSccpRecentMessages(BigInteger.ONE, afterIndex, 1));
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> transport.getSccpRecentMessages(null, 0, 1));
    assertThrows(
        IllegalArgumentException.class,
        () -> new SccpModels.RecentCursor(BigInteger.ZERO, 0));
    assertThrows(
        IllegalArgumentException.class,
        () -> new SccpModels.RecentCursor(BigInteger.ONE.shiftLeft(64), 0));
    assertThrows(
        IllegalArgumentException.class,
        () -> new SccpModels.RecentCursor(BigInteger.ONE, 512));

    assertEquals("invalid queries must not reach HTTP execution", 0, executor.requests.size());
  }

  @Test
  public void exactWindowBoundariesReachHttpWithCanonicalQuery() {
    final CountingExecutor executor = new CountingExecutor();
    final HttpClientTransport transport = transport(executor);

    final BigInteger maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
    transport.getSccpRecentMessages(BigInteger.ONE, null, 1).join();
    transport.getSccpRecentMessages(maxU64, 0, 50).join();
    transport
        .getSccpRecentMessages(new SccpModels.RecentCursor(BigInteger.valueOf(7), 511), 1)
        .join();

    assertEquals(3, executor.requests.size());
    assertEquals("from=1&limit=1", executor.requests.get(0).uri().getRawQuery());
    assertEquals(
        "from=" + maxU64 + "&after_index=0&limit=50",
        executor.requests.get(1).uri().getRawQuery());
    assertEquals(
        "from=7&after_index=511&limit=1",
        executor.requests.get(2).uri().getRawQuery());
  }

  @Test
  public void sccpEndpointsCarryNarrowResponseLimits() {
    final CountingExecutor executor = new CountingExecutor();
    final HttpClientTransport transport = transport(executor);
    final String messageId = "11".repeat(32);

    transport.getSccpCapabilities();
    transport.getSccpRegistry();
    transport.getSccpMessageBundle(messageId);
    transport.getSccpProofRequest(messageId);
    transport.getSccpRecentMessages(BigInteger.ONE, null, 1);

    final List<Long> responseLimits = new ArrayList<>();
    for (final TransportRequest request : executor.requests) {
      responseLimits.add(request.maximumResponseBytes());
    }
    assertEquals(
        List.of(
            64L * 1024L,
            64L * 1024L * 1024L,
            64L * 1024L * 1024L,
            64L * 1024L * 1024L,
            8L * 1024L * 1024L),
        responseLimits);
  }

  @Test
  public void sccpJsonRequiresExact200AndUnambiguousCanonicalContentType() {
    final List<TransportResponse> invalidResponses =
        List.of(
            response(201, List.of("application/json"), null),
            response(204, List.of("application/json"), new byte[0]),
            response(200, List.of("text/html"), null),
            response(200, null, null),
            response(200, List.of("application/json", "application/json"), null),
            response(200, List.of("application/json; charset=utf-8"), null));
    for (final TransportResponse response : invalidResponses) {
      final CountingExecutor executor = new CountingExecutor(response);
      assertThrows(
          CompletionException.class,
          () ->
              transport(executor)
                  .getSccpRecentMessages(BigInteger.ONE, null, 1)
                  .join());
      assertEquals(1, executor.requests.size());
    }
  }

  private static HttpClientTransport transport(final CountingExecutor executor) {
    return HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .build());
  }

  private static final class CountingExecutor implements HttpTransportExecutor {
    private final List<TransportRequest> requests = new ArrayList<>();
    private final TransportResponse response;

    private CountingExecutor() {
      this(response(200, List.of("application/json"), null));
    }

    private CountingExecutor(final TransportResponse response) {
      this.response = response;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(request);
      return CompletableFuture.completedFuture(response);
    }
  }

  private static TransportResponse response(
      final int status, final List<String> contentTypes, final byte[] body) {
    final TransportResponse.Builder builder =
        TransportResponse.builder()
            .setStatusCode(status)
            .setBody(
                body == null
                    ? "{\"items\":[]}".getBytes(StandardCharsets.UTF_8)
                    : body);
    if (contentTypes != null) {
      for (final String contentType : contentTypes) {
        builder.addHeader("Content-Type", contentType);
      }
    }
    return builder.build();
  }
}
