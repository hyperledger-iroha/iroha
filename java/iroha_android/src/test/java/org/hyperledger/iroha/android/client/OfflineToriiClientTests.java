package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineReadiness;

public final class OfflineToriiClientTests {

  private OfflineToriiClientTests() {}

  public static void main(final String[] args) {
    readinessUsesCanonicalGetPathAndParsesResponse();
    propagatesNon2xxResponses();
    propagatesRejectCodeFromNon2xxResponses();
    rejectsInsecureAuthorizationHeader();
    System.out.println("[IrohaAndroid] OfflineToriiClientTests passed.");
  }

  private static void readinessUsesCanonicalGetPathAndParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "offline_note": true,
              "offline_one_use_keys": true,
              "offline_recursive_note_proof": false,
              "offline_fountain_qr": true,
              "offline_sync_optional": true,
              "offline_telemetry": true
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .addHeader("X-Test", "1")
            .build();

    final OfflineReadiness readiness = client.getOfflineReadiness().join();

    assert "GET".equals(executor.lastRequest.method()) : "readiness must use GET";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/readiness")
        : "readiness path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
    assert readiness.offlineNote() : "offline_note mismatch";
    assert readiness.offlineOneUseKeys() : "offline_one_use_keys mismatch";
    assert !readiness.offlineRecursiveNoteProof() : "offline_recursive_note_proof mismatch";
    assert readiness.offlineFountainQr() : "offline_fountain_qr mismatch";
    assert readiness.offlineSyncOptional() : "offline_sync_optional mismatch";
    assert readiness.offlineTelemetry() : "offline_telemetry mismatch";
  }

  private static void propagatesNon2xxResponses() {
    final StubExecutor executor = new StubExecutor(500, "{\"error\":\"boom\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.getOfflineReadiness().join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("500") : "status missing from message";
      assert ex.getCause().getMessage().contains("boom") : "body missing from message";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(500).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert error.responseBody().orElse("").contains("boom")
          : "response body not surfaced";
      assert error.rejectCode().isEmpty() : "unexpected reject code";
      return;
    }
    throw new AssertionError("Expected CompletionException for non-2xx responses");
  }

  private static void propagatesRejectCodeFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(
            400,
            "{\"error\":\"not ready\"}",
            "Bad Request",
            Map.of("X-IrOhA-ReJeCt-CoDe", List.of("offline_unavailable")));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.getOfflineReadiness().join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(400).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert "offline_unavailable".equals(error.rejectCode().orElse(null))
          : "reject code not surfaced";
      assert error.getMessage().contains("reject_code=offline_unavailable")
          : "reject code missing from message";
      return;
    }
    throw new AssertionError("Expected CompletionException for reject code propagation");
  }

  private static void rejectsInsecureAuthorizationHeader() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("http://example.com"))
            .addHeader("Authorization", "Bearer secret")
            .build();
    try {
      client.getOfflineReadiness();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("insecure transport over http")
          : "security message mismatch";
      return;
    }
    throw new AssertionError("Expected insecure credentialed HTTP request to fail");
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private final int status;
    private final byte[] body;
    private final String message;
    private final Map<String, List<String>> headers;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private StubExecutor(final int status, final String body) {
      this(status, body, "", Map.of());
    }

    private StubExecutor(
        final int status,
        final String body,
        final String message,
        final Map<String, List<String>> headers) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
      this.message = message;
      this.headers = headers;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(
          new TransportResponse(status, body, message, headers));
    }
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    for (final var entry : request.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name)) {
        final List<String> values = entry.getValue();
        if (!values.isEmpty()) {
          return values.get(0);
        }
      }
    }
    return "";
  }
}
