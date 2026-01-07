package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Base64;
import java.util.Map;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.junit.Test;

/** OkHttp-backed submission/parity tests for {@link org.hyperledger.iroha.android.client.HttpClientTransport}. */
public final class HttpClientTransportOkHttpTests {

  @Test
  public void submitsTransactionWithOkHttpExecutorAndNotifiesObservers() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(202).setBody("{\"status\":\"accepted\"}"));
      server.start();

      final RecordingObserver observer = new RecordingObserver();
      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(server.url("/").uri())
              .setRequestTimeout(Duration.ofSeconds(2))
              .addObserver(observer)
              .putDefaultHeader("X-Test", "ok")
              .build();

      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final org.hyperledger.iroha.android.client.HttpClientTransport transport =
          new org.hyperledger.iroha.android.client.HttpClientTransport(executor, config);

      final TransactionPayload payload =
          TransactionPayload.builder()
              .setChainId("00000001")
              .setAuthority("okhttp@wonderland")
              .setCreationTimeMs(1_700_000_000_000L)
              .setInstructionBytes("payload".getBytes(StandardCharsets.UTF_8))
              .setTimeToLiveMs(5_000L)
              .setNonce(1)
              .setMetadata(java.util.Map.of("note", "okhttp"))
              .build();
      final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
      final byte[] encodedPayload;
      try {
        encodedPayload = codec.encodeTransaction(payload);
      } catch (final Exception ex) {
        throw new IllegalStateException("Failed to encode transaction payload", ex);
      }
      final byte[] signature = "signature".getBytes(StandardCharsets.UTF_8);
      final byte[] publicKey = "public-key".getBytes(StandardCharsets.UTF_8);
      final SignedTransaction tx =
          SignedTransaction.builder()
              .setEncodedPayload(encodedPayload)
              .setSignature(signature)
              .setPublicKey(publicKey)
              .setSchemaName(codec.schemaName())
              .build();

      final ClientResponse response = transport.submitTransaction(tx).get(2, TimeUnit.SECONDS);
      assertEquals(202, response.statusCode());
      assertEquals(SignedTransactionHasher.hashHex(tx), response.hashHex().orElse(null));
      observer.assertNoFailure();
      assertEquals(1, observer.requestsCount());
      assertEquals(1, observer.responsesCount());

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(recorded);
      assertEquals("/v1/pipeline/transactions", recorded.getPath());
      assertEquals("POST", recorded.getMethod());
      assertEquals("application/json", recorded.getHeader("Content-Type"));
      assertEquals("application/json", recorded.getHeader("Accept"));
      assertEquals("ok", recorded.getHeader("X-Test"));

      final Object parsed = JsonParser.parse(recorded.getBody().readUtf8());
      assertTrue(parsed instanceof Map);
      @SuppressWarnings("unchecked")
      final Map<String, Object> map = (Map<String, Object>) parsed;
      assertEquals(Base64.getEncoder().encodeToString(encodedPayload), map.get("payload"));
      assertEquals(Base64.getEncoder().encodeToString(signature), map.get("signature"));
      assertEquals(Base64.getEncoder().encodeToString(publicKey), map.get("public_key"));
      assertEquals(tx.schemaName(), map.get("schema"));
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    private final AtomicReference<Throwable> failure = new AtomicReference<>(null);
    private int requests;
    private int responses;

    @Override
    public void onRequest(final TransportRequest request) {
      requests++;
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responses++;
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failure.compareAndSet(null, error);
    }

    int requestsCount() {
      return requests;
    }

    int responsesCount() {
      return responses;
    }

    void assertNoFailure() {
      assertNull("unexpected observer failure", failure.get());
    }
  }
}
