package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
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
    final List<String> requests = new ArrayList<>();
    final HttpTransportExecutor executor =
        request -> {
          requests.add(request.method() + " " + request.uri().getPath());
          if ("GET".equals(request.method())
              && "/v1/node/capabilities".equals(request.uri().getPath())) {
            return CompletableFuture.completedFuture(compatibleCapabilitiesResponse());
          }
          assert "POST".equals(request.method()) : "submission must use POST";
          assert "/v1/pipeline/transactions".equals(request.uri().getPath())
              : "submission must target the transaction endpoint";
          final byte[] body = "{\"error\":\"rejected\"}".getBytes(StandardCharsets.UTF_8);
          final TransportResponse response =
              new TransportResponse(
                  400,
                  body,
                  "bad_request",
                  Map.of(
                      "x-iroha-reject-code", List.of("PRTRY:TX_SIGNATURE_MISSING")));
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
            .setFeePayment(
                org.hyperledger.iroha.android.model.FeePaymentIntent.authority(
                    java.util.Collections.emptyList(), 1L))
            .setChainId("00000001")
            .setAuthority("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            .setCreationTimeMs(1L)
            .setExecutable(Executable.ivm(new byte[] {0x01}))
            .build();
    final byte[] encodedPayload;
    try {
      encodedPayload =
          new NoritoJavaCodecAdapter(
                  org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT)
              .encodeTransaction(payload);
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
    assert requests.equals(
            List.of(
                "GET /v1/node/capabilities", "POST /v1/pipeline/transactions"))
        : "capabilities probe must immediately precede transaction submission: " + requests;
  }

  private static TransportResponse compatibleCapabilitiesResponse() {
    final byte[] body =
        ("{\"data_model_version\":"
                + ToriiTransactionCompatibility.EXPECTED_DATA_MODEL_VERSION
                + ",\"signed_transaction_schema_hash_hex\":\""
                + ToriiTransactionCompatibility.EXPECTED_SIGNED_TRANSACTION_SCHEMA_HASH_HEX
                + "\"}")
            .getBytes(StandardCharsets.UTF_8);
    return new TransportResponse(200, body, "ok", Map.of());
  }
}
