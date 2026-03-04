package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;

/** Regression tests for {@link HttpSafetyDetectService}. */
public final class HttpSafetyDetectServiceTests {

  private HttpSafetyDetectServiceTests() {}

  public static void main(final String[] args) throws Exception {
    fetchesAttestationAndCachesToken();
    System.out.println("[IrohaAndroid] HttpSafetyDetectServiceTests passed.");
  }

  private static void fetchesAttestationAndCachesToken() {
    final RecordingExecutor executor = new RecordingExecutor();
    executor.enqueue(
        "/oauth/token",
        new RecordingExecutor.Response(
            200,
            """
            {
              "access_token": "token-123",
              "expires_in": 120
            }
            """
                .getBytes(StandardCharsets.UTF_8),
            "OK"));
    executor.enqueue(
        "/attest",
        new RecordingExecutor.Response(
            200, "{\"token\":\"attest-token\"}".getBytes(StandardCharsets.UTF_8), "OK"));
    executor.enqueue(
        "/attest",
        new RecordingExecutor.Response(
            200, "{\"token\":\"attest-token-2\"}".getBytes(StandardCharsets.UTF_8), "OK"));

    final SafetyDetectOptions options =
        SafetyDetectOptions.builder()
            .setEnabled(true)
            .setOauthEndpoint(URI.create("https://example.com/oauth/token"))
            .setAttestationEndpoint(URI.create("https://example.com/attest"))
            .setClientId("client-id")
            .setClientSecret("secret")
            .setPackageName("org.example.app")
            .setSigningDigestSha256("abcdef")
            .setRequestTimeout(Duration.ofSeconds(5))
            .build();
    final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);

    final SafetyDetectRequest request =
        SafetyDetectRequest.builder()
            .setCertificateIdHex("deadbeef")
            .setAppId("app-id")
            .setNonceHex("0011")
            .setPackageName("org.example.app")
            .setSigningDigestSha256("abcdef")
            .setRequiredEvaluations(List.of("strong_integrity"))
            .setMaxTokenAgeMs(1_000L)
            .build();

    final SafetyDetectAttestation attestation = service.fetch(request).join();
    assert "attest-token".equals(attestation.token()) : "attestation token must match response";

    // Second call should reuse the cached OAuth token and hit only the attestation endpoint.
    final SafetyDetectAttestation second = service.fetch(request).join();
    assert "attest-token-2".equals(second.token()) : "attestation token should update per call";

    final List<TransportRequest> observed = executor.requests();
    assert observed.size() == 3 : "expected one oauth + two attestation requests";

    final TransportRequest oauthRequest = observed.get(0);
    assert oauthRequest.uri().getPath().equals("/oauth/token") : "oauth path mismatch";
    final String oauthBody = new String(oauthRequest.body(), StandardCharsets.UTF_8);
    assert oauthBody.contains("grant_type=client_credentials") : "missing grant_type";
    assert oauthBody.contains("client_id=client-id") : "missing client_id";
    assert oauthBody.contains("client_secret=secret") : "missing client_secret";

    final TransportRequest firstAttest = observed.get(1);
    assert headerContains(firstAttest, "Authorization", "Bearer token-123")
        : "attestation should include cached bearer token";
    final String attestPayload = new String(firstAttest.body(), StandardCharsets.UTF_8);
    assert attestPayload.contains("\"required_evaluations\"")
        : "attestation payload should include required evaluations";
    assert attestPayload.contains("\"max_token_age_ms\"")
        : "attestation payload should include max_token_age_ms";
    assert firstAttest.method().equals("POST") : "attestation should use POST";
    assert firstAttest.uri().toString().equals("https://example.com/attest")
        : "attestation URI mismatch";
  }

  private static boolean headerContains(
      final TransportRequest request, final String name, final String expectedValue) {
    final List<String> values = request.headers().get(name);
    return values != null && values.contains(expectedValue);
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final Map<String, Deque<Response>> responses = new ConcurrentHashMap<>();
    private final List<TransportRequest> requests = new ArrayList<>();

    void enqueue(final String path, final Response response) {
      responses.computeIfAbsent(path, ignored -> new ArrayDeque<>()).add(response);
    }

    List<TransportRequest> requests() {
      return List.copyOf(requests);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      Objects.requireNonNull(request, "request");
      requests.add(request);
      final Deque<Response> queue =
          responses.getOrDefault(request.uri().getPath(), new ArrayDeque<>());
      final Response response = queue.poll();
      if (response == null) {
        final CompletableFuture<TransportResponse> failed = new CompletableFuture<>();
        failed.completeExceptionally(
            new IllegalStateException("No queued response for " + request.uri().getPath()));
        return failed;
      }
      return CompletableFuture.completedFuture(
          new TransportResponse(
              response.statusCode, response.body, response.message, Map.of()));
    }

    record Response(int statusCode, byte[] body, String message) {}
  }
}
