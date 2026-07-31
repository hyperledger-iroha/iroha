package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Focused transaction-submission wire and capability-probe contract tests. */
public final class HttpClientTransportSubmissionContractTests {
  private HttpClientTransportSubmissionContractTests() {}

  /** Runs the submission contract checks from the Gradle main-test harness. */
  public static void main(final String[] args) {
    submitTransactionJsonBuildsJsonIngressRequest();
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
}
