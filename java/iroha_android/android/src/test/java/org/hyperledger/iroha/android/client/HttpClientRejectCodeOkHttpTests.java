package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;

import java.net.URI;
import java.time.Duration;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.junit.Test;

/** Android/OkHttp regression to ensure reject headers propagate through the transport stack. */
public final class HttpClientRejectCodeOkHttpTests {

  @Test
  public void okHttpTransportSurfacesRejectHeader() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(400)
              .addHeader("Content-Type", "application/json")
              .addHeader("x-iroha-reject-code", "PRTRY:TX_SIGNATURE_MISSING")
              .setBody("{\"error\":\"rejected\"}"));
      server.start();

      final URI baseUri = server.url("/").uri();
      final AndroidClientFactory factory = AndroidClientFactory.withDefaultClient();
      final HttpClientTransport transport =
          factory.createHttpClientTransport(
              ClientConfig.builder()
                  .setBaseUri(baseUri)
                  .setRequestTimeout(Duration.ofSeconds(5))
                  .build());

      final SignedTransaction transaction = sampleTransaction((byte) 0x01);

      final ClientResponse response = transport.submitTransaction(transaction).join();

      assertEquals(400, response.statusCode());
      assertEquals("PRTRY:TX_SIGNATURE_MISSING", response.rejectCode().orElse(null));

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull("mock server must observe submission", recorded);
      assertEquals("/v1/pipeline/transactions", recorded.getPath());
    }
  }

  private static SignedTransaction sampleTransaction(final byte seed) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(String.format("%08x", seed))
            .setAuthority("reject@wonderland")
            .setCreationTimeMs(1_700_000_000_000L + (seed & 0xFF))
            .setInstructionBytes(new byte[] {seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce((seed & 0xFF) + 1)
            .setMetadata(Map.of("note", "tx-" + seed))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encoded;
    try {
      encoded = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }
}
