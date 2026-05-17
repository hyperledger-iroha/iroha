package org.hyperledger.iroha.android.client.okhttp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Optional;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.AccountAliasResolution;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
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
              .setAuthority("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB")
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
      assertEquals("application/x-norito", recorded.getHeader("Content-Type"));
      assertEquals("application/x-norito, application/json", recorded.getHeader("Accept"));
      assertEquals("ok", recorded.getHeader("X-Test"));
      assertArrayEquals(SignedTransactionEncoder.encodeVersioned(tx), recorded.getBody().readByteArray());
    }
  }

  @Test
  public void resolveAccountAliasParsesSuccessResponse() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody(
                  "{\"alias\":\"alice@universal\","
                      + "\"account_id\":\"sorauaccount\","
                      + "\"index\":3,"
                      + "\"source\":\"directory\"}"));
      server.start();

      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(server.url("/").uri())
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final org.hyperledger.iroha.android.client.HttpClientTransport transport =
          new org.hyperledger.iroha.android.client.HttpClientTransport(executor, config);

      final Optional<AccountAliasResolution> response =
          transport.resolveAccountAlias("alice@universal").get(2, TimeUnit.SECONDS);
      assertTrue("account alias resolution must be present", response.isPresent());
      final AccountAliasResolution resolution = response.orElseThrow();
      assertEquals("alice@universal", resolution.alias());
      assertEquals("sorauaccount", resolution.accountId());
      assertEquals(Long.valueOf(3L), resolution.index());
      assertEquals("directory", resolution.source());

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(recorded);
      assertEquals("POST", recorded.getMethod());
      assertEquals("/v1/aliases/resolve", recorded.getPath());
      final String body = recorded.getBody().readString(StandardCharsets.UTF_8);
      assertTrue(
          "request body must carry the alias literal: " + body,
          body.contains("\"alias\":\"alice@universal\""));
    }
  }

  @Test
  public void resolveAccountAliasMapsNotFoundToEmptyOptional() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(404));
      server.start();

      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(server.url("/").uri())
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final org.hyperledger.iroha.android.client.HttpClientTransport transport =
          new org.hyperledger.iroha.android.client.HttpClientTransport(executor, config);

      final Optional<AccountAliasResolution> response =
          transport.resolveAccountAlias("missing@universal").get(2, TimeUnit.SECONDS);
      assertFalse("404 must map to Optional.empty", response.isPresent());

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(recorded);
      assertEquals("POST", recorded.getMethod());
      assertEquals("/v1/aliases/resolve", recorded.getPath());
    }
  }

  @Test
  public void resolveAccountAliasParsesSuccessResponseWithoutIndex() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody(
                  "{\"alias\":\"banking@centralbank.universal\","
                      + "\"account_id\":\"aid:banking-123\","
                      + "\"source\":\"rekey_record\"}"));
      server.start();

      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(server.url("/").uri())
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final org.hyperledger.iroha.android.client.HttpClientTransport transport =
          new org.hyperledger.iroha.android.client.HttpClientTransport(executor, config);

      final Optional<AccountAliasResolution> response =
          transport.resolveAccountAlias("banking@centralbank.universal")
              .get(2, TimeUnit.SECONDS);
      assertTrue("account alias resolution must be present", response.isPresent());
      final AccountAliasResolution resolution = response.orElseThrow();
      assertEquals("banking@centralbank.universal", resolution.alias());
      assertEquals("aid:banking-123", resolution.accountId());
      assertNull("index must be null when omitted", resolution.index());
      assertEquals("rekey_record", resolution.source());

      final RecordedRequest recorded = server.takeRequest(1, TimeUnit.SECONDS);
      assertNotNull(recorded);
      assertEquals("POST", recorded.getMethod());
      assertEquals("/v1/aliases/resolve", recorded.getPath());
      final String body = recorded.getBody().readString(StandardCharsets.UTF_8);
      assertTrue(
          "request body must carry the alias literal: " + body,
          body.contains("\"alias\":\"banking@centralbank.universal\""));
    }
  }

  @Test
  public void resolveAccountAliasFailsOnMalformedJson() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("not json"));
      server.start();

      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(server.url("/").uri())
              .setRequestTimeout(Duration.ofSeconds(2))
              .build();
      final OkHttpTransportExecutor executor = new OkHttpTransportExecutor(new OkHttpClient());
      final org.hyperledger.iroha.android.client.HttpClientTransport transport =
          new org.hyperledger.iroha.android.client.HttpClientTransport(executor, config);

      final ExecutionException ex =
          assertThrows(
              ExecutionException.class,
              () -> transport.resolveAccountAlias("alice@universal").get(2, TimeUnit.SECONDS));
      assertNotNull(ex.getCause());
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
