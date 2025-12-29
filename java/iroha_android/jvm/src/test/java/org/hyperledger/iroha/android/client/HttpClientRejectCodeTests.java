package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.junit.Test;

/** JVM regression ensuring reject headers propagate into ClientResponse. */
public final class HttpClientRejectCodeTests {

  @Test
  public void submitSurfacesRejectHeader() {
    final HttpTransportExecutor executor =
        request -> {
          final byte[] body = "{\"error\":\"rejected\"}".getBytes(StandardCharsets.UTF_8);
          final TransportResponse response =
              new TransportResponse(
                  400, body, "bad_request", Map.of("x-iroha-reject-code", java.util.List.of("PRTRY:TX_SIGNATURE_MISSING")));
          return CompletableFuture.completedFuture(response);
        };

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("https://torii.example"))
            .setRequestTimeout(Duration.ofSeconds(5))
            .build();
    final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

    final SignedTransaction tx =
        new SignedTransaction(
            new byte[] {0x01},
            new byte[] {0x02},
            new byte[] {0x03},
            "iroha.android.transaction.Payload.v1");

    final ClientResponse response = transport.submitTransaction(tx).join();
    assert response.statusCode() == 400 : "status should propagate from executor";
    assert "PRTRY:TX_SIGNATURE_MISSING".equals(response.rejectCode().orElse(null))
        : "reject header must propagate to ClientResponse";
  }
}
