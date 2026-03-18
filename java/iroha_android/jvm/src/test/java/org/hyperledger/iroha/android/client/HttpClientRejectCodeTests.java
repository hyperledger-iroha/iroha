package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
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

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000001")
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1L)
            .setExecutable(Executable.ivm(new byte[] {0x01}))
            .build();
    final byte[] encodedPayload;
    try {
      encodedPayload = new NoritoJavaCodecAdapter().encodeTransaction(payload);
    } catch (final NoritoException ex) {
      throw new IllegalStateException("Failed to encode payload fixture", ex);
    }
    final SignedTransaction tx =
        new SignedTransaction(
            encodedPayload,
            new byte[64],
            new byte[32],
            "iroha.android.transaction.Payload.v1");

    final ClientResponse response = transport.submitTransaction(tx).join();
    assert response.statusCode() == 400 : "status should propagate from executor";
    assert "PRTRY:TX_SIGNATURE_MISSING".equals(response.rejectCode().orElse(null))
        : "reject header must propagate to ClientResponse";
  }
}
