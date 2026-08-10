package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.testing.TestNetworkIds;

/** Focused transaction-submission wire and capability-probe contract tests. */
public final class HttpClientTransportSubmissionContractTests {
  private HttpClientTransportSubmissionContractTests() {}

  /** Runs the submission contract checks from the Gradle main-test harness. */
  public static void main(final String[] args) {
    submitTransactionJsonBuildsJsonIngressRequest();
    replayPolicyIsSafeOnlyForUnsignedBodylessReads();
    canonicalAuthRedirectStatusAndNetworkFailuresAreOneShot();
    System.out.println("[IrohaAndroid] HTTP submission contract tests passed.");
  }

  private static void submitTransactionJsonBuildsJsonIngressRequest() {
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080"))
            .setWireFormatPreference(WireFormatPreference.JSON_PREFERRED)
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);
    final byte[] body = "{\"version\":1,\"content\":{}}".getBytes(StandardCharsets.UTF_8);

    final ClientResponse response = transport.submitTransactionJson(body).join();

    assert response.statusCode() == 202 : "Expected JSON submit to be accepted";
    final TransportRequest request = executor.lastRequest;
    assert "POST".equals(request.method()) : "JSON submit must use POST";
    assert request.uri().toString().equals("https://127.0.0.1:8080/v1/pipeline/transactions")
        : "JSON submit endpoint must target Torii pipeline route";
    assert request.headers().get("Content-Type").contains("application/json")
        : "JSON submit Content-Type must be application/json";
    assert request.headers().get("Accept").contains(WireFormatPreference.JSON_PREFERRED.acceptHeader())
        : "JSON submit Accept header must use configured wire preference";
    assert java.util.Arrays.equals(body, request.body()) : "JSON submit body must be preserved";
    assert request.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "transaction bodies must be one-shot";
  }

  private static void replayPolicyIsSafeOnlyForUnsignedBodylessReads() {
    final TransportRequest unsignedRead =
        TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/v1/status"))
            .build();
    final TransportRequest signedQuery =
        TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/v1/query"))
            .addHeader("X-Iroha-Signature", "signature")
            .addHeader("X-Iroha-Nonce", "nonce")
            .build();

    assert unsignedRead.replayPolicy() == RequestReplayPolicy.RETRY_SAFE
        : "unsigned bodyless reads may be retried";
    assert signedQuery.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "nonce-bearing signed queries must be one-shot";
  }

  private static void canonicalAuthRedirectStatusAndNetworkFailuresAreOneShot() {
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice", message -> Arrays.copyOf(message, 64), 1_717_171_717_000L,
            "canonical-one-shot-nonce");
    for (final int status : new int[] {307, 308, 503}) {
      assertCanonicalAliasFailsOnce(auth, OutcomeExecutor.forStatus(status));
    }
    assertCanonicalAliasFailsOnce(
        auth, OutcomeExecutor.forFailure(new RuntimeException("ambiguous network failure")));
  }

  private static void assertCanonicalAliasFailsOnce(
      final ToriiCanonicalRequestAuth auth, final OutcomeExecutor executor) {
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://127.0.0.1:8080"))
            .setLocalSigningContext(new LocalSigningContext(TestNetworkIds.canonical()))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    try {
      transport.resolveAccountAlias("merchant@private", auth).join();
      throw new AssertionError("canonical-auth failure must be surfaced");
    } catch (final CompletionException expected) {
      // Expected: redirects, retryable statuses, and transport failures all terminate the call.
    }
    assert executor.callCount == 1 : "canonical-auth request must be dispatched once";
    assert executor.lastRequest.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "canonical-auth request must carry the one-shot replay policy";
  }

  static boolean isCapabilitiesRequest(final TransportRequest request) {
    return "GET".equals(request.method())
        && request.uri().getPath().endsWith("/v1/node/capabilities");
  }

  static TransportResponse compatibleCapabilitiesResponse() {
    return new TransportResponse(
        200,
        ("{\"data_model_version\":4,"
                + "\"signed_transaction_schema_hash_hex\":"
                + "\"7ab5ff9c572efb316deac478f19209c5\"}")
            .getBytes(StandardCharsets.UTF_8),
        "",
        Map.of());
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private TransportRequest lastRequest;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      if (isCapabilitiesRequest(request)) {
        return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(202, new byte[0], "accepted", Map.of()));
    }
  }

  private static final class OutcomeExecutor implements HttpTransportExecutor {
    private final Integer status;
    private final RuntimeException failure;
    private int callCount;
    private TransportRequest lastRequest;

    private OutcomeExecutor(final Integer status, final RuntimeException failure) {
      this.status = status;
      this.failure = failure;
    }

    private static OutcomeExecutor forStatus(final int status) {
      return new OutcomeExecutor(status, null);
    }

    private static OutcomeExecutor forFailure(final RuntimeException failure) {
      return new OutcomeExecutor(null, failure);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      callCount++;
      lastRequest = request;
      final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
      if (failure != null) {
        future.completeExceptionally(failure);
      } else {
        future.complete(new TransportResponse(status, new byte[0], "failure", Map.of()));
      }
      return future;
    }
  }
}
