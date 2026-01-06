package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.hyperledger.iroha.android.client.okhttp.OkHttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.offline.attestation.HttpSafetyDetectService;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectAttestation;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectOptions;
import org.hyperledger.iroha.android.offline.attestation.SafetyDetectRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchOptions;
import org.hyperledger.iroha.android.sorafs.GatewayFetchRequest;
import org.hyperledger.iroha.android.sorafs.GatewayFetchSummary;
import org.hyperledger.iroha.android.sorafs.GatewayProvider;
import org.hyperledger.iroha.android.sorafs.SorafsGatewayClient;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.junit.Test;

/** End-to-end coverage for OkHttp-backed transports across Android SDK clients. */
public final class OkHttpClientIntegrationTests {

  @Test
  public void okhttpExecutorSupportsRestAndRpcClients() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(202).setBody("accepted"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("rpc-ok"));
      server.enqueue(
          new MockResponse()
              .setResponseCode(200)
              .setBody(sampleGatewaySummaryJson()));
      server.start();

      final URI baseUri = server.url("/").uri();
      final OkHttpClient client = new OkHttpClient();
      final HttpTransportExecutor executor = new OkHttpTransportExecutor(client);
      final RecordingTelemetrySink telemetry = new RecordingTelemetrySink();
      final RecordingObserver observer = new RecordingObserver();

      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(baseUri)
              .setRequestTimeout(Duration.ofSeconds(5))
              .setTelemetryOptions(TelemetryOptions.builder().build())
              .setTelemetrySink(telemetry)
              .addObserver(observer)
              .build();
      final HttpClientTransport transport = HttpClientTransport.withExecutor(executor, config);

      final SignedTransaction transaction =
          new SignedTransaction(
              new byte[] {0x01, 0x02},
              new byte[] {0x03, 0x04},
              new byte[] {0x05, 0x06},
              "iroha.android.transaction.Payload.v1");

      final ClientResponse submitResponse = transport.submitTransaction(transaction).get(2, TimeUnit.SECONDS);
      assertEquals(202, submitResponse.statusCode());
      assertEquals(SignedTransactionHasher.hashHex(transaction), submitResponse.hashHex().orElse(null));
      final RecordedRequest submit = server.takeRequest();
      assertEquals("/v1/pipeline/transactions", submit.getPath());
      assertEquals("POST", submit.getMethod());
      assertEquals("application/json", submit.getHeader("Content-Type"));
      final String submitBody = submit.getBody().readUtf8();
      assertTrue(submitBody.contains(Base64.getEncoder().encodeToString(transaction.encodedPayload())));

      final NoritoRpcClient rpcClient =
          NoritoRpcClient.builder()
              .setBaseUri(baseUri)
              .setTransportExecutor(executor)
              .setTelemetryOptions(TelemetryOptions.builder().build())
              .setTelemetrySink(telemetry)
              .build();
      final byte[] payload = "payload-bytes".getBytes(StandardCharsets.UTF_8);
      final byte[] rpcResponse = rpcClient.call("/rpc", payload, null);
      assertArrayEquals("rpc-ok".getBytes(StandardCharsets.UTF_8), rpcResponse);
      final RecordedRequest rpcRequest = server.takeRequest();
      assertEquals("/rpc", rpcRequest.getPath());
      assertEquals("POST", rpcRequest.getMethod());
      assertEquals("application/x-norito", rpcRequest.getHeader("Content-Type"));
      assertArrayEquals(payload, rpcRequest.getBody().readByteArray());

      final GatewayFetchRequest fetchRequest =
          GatewayFetchRequest.builder()
              .setManifestIdHex("01".repeat(32))
              .setChunkerHandle("sorafs.sf1@1.0.0")
              .setOptions(GatewayFetchOptions.builder().build())
              .addProvider(
                  GatewayProvider.builder()
                      .setName("alpha")
                      .setProviderIdHex("aa".repeat(32))
                      .setBaseUrl("https://provider.example")
                      .setStreamTokenBase64("dG9rZW4=")
                      .build())
              .build();
      final SorafsGatewayClient gatewayClient =
          SorafsGatewayClient.builder()
              .setExecutor(executor)
              .setBaseUri(baseUri)
              .setTimeout(Duration.ofSeconds(5))
              .addObserver(observer)
              .build();
      final GatewayFetchSummary summary = gatewayClient.fetchSummary(fetchRequest).get(2, TimeUnit.SECONDS);
      assertEquals("0123", summary.manifestIdHex());
      assertEquals("sorafs.sf1@1.0.0", summary.chunkerHandle());
      final RecordedRequest fetch = server.takeRequest();
      assertEquals("/v1/sorafs/gateway/fetch", fetch.getPath());
      assertEquals("POST", fetch.getMethod());
      assertEquals("application/json", fetch.getHeader("Content-Type"));

      assertEquals(2, telemetry.requests.size());
      assertEquals(2, telemetry.responses.size());
      assertEquals(2, observer.requestCount);
      assertEquals(2, observer.responseCount);
    }
  }

  @Test
  public void okhttpExecutorHandlesSafetyDetectFlow() throws Exception {
    try (MockWebServer server = new MockWebServer()) {
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"access_token\":\"token-abc\",\"expires_in\":3600}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"attestation-1\"}"));
      server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"token\":\"attestation-2\"}"));
      server.start();

      final OkHttpClient client = new OkHttpClient();
      final HttpTransportExecutor executor = new OkHttpTransportExecutor(client);
      final SafetyDetectOptions options =
          SafetyDetectOptions.builder()
              .setOauthEndpoint(server.url("/oauth").uri())
              .setAttestationEndpoint(server.url("/attest").uri())
              .setClientId("client")
              .setClientSecret("secret")
              .setPackageName("pkg")
              .setSigningDigestSha256("abcd")
              .build();
      final HttpSafetyDetectService service = new HttpSafetyDetectService(executor, options);
      final SafetyDetectRequest request =
          SafetyDetectRequest.builder()
              .setCertificateIdHex("deadbeef")
              .setAppId("app")
              .setNonceHex("00ff")
              .setPackageName("pkg")
              .setSigningDigestSha256("abcd")
              .build();

      final SafetyDetectAttestation first = service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attestation-1", first.token());
      final SafetyDetectAttestation second = service.fetch(request).get(2, TimeUnit.SECONDS);
      assertEquals("attestation-2", second.token());

      final RecordedRequest oauth = server.takeRequest();
      assertEquals("/oauth", oauth.getPath());
      assertEquals("POST", oauth.getMethod());
      final String oauthBody = oauth.getBody().readUtf8();
      assertTrue(oauthBody.contains("grant_type=client_credentials"));
      assertTrue(oauthBody.contains("client_id=client"));

      final RecordedRequest attest = server.takeRequest();
      assertEquals("/attest", attest.getPath());
      assertEquals("POST", attest.getMethod());
      assertEquals("Bearer token-abc", attest.getHeader("Authorization"));
      final String attestBody = attest.getBody().readUtf8();
      assertTrue(attestBody.contains("\"certificate_id_hex\":\"deadbeef\""));
      assertTrue(attestBody.contains(Base64.getEncoder().encodeToString(new byte[] {0x00, (byte) 0xFF})));

      final RecordedRequest attestCached = server.takeRequest();
      assertEquals("/attest", attestCached.getPath());
      assertEquals("POST", attestCached.getMethod());
      assertEquals("Bearer token-abc", attestCached.getHeader("Authorization"));
    }
  }

  private static String sampleGatewaySummaryJson() {
    return String.join(
        "\n",
        "{",
        "  \"manifest_id_hex\": \"0123\",",
        "  \"chunker_handle\": \"sorafs.sf1@1.0.0\",",
        "  \"client_id\": \"android-sdk\",",
        "  \"chunk_count\": 2,",
        "  \"content_length\": 1024,",
        "  \"assembled_bytes\": 1024,",
        "  \"provider_reports\": [",
        "    {\"provider\":\"alpha\",\"successes\":2,\"failures\":1,\"disabled\":false},",
        "    {\"provider\":\"beta\",\"successes\":0,\"failures\":3,\"disabled\":true}",
        "  ],",
        "  \"chunk_receipts\": [",
        "    {\"chunk_index\":0,\"provider\":\"alpha\",\"attempts\":1},",
        "    {\"chunk_index\":1,\"provider\":\"beta\",\"attempts\":2}",
        "  ],",
        "  \"anonymity_policy\": \"anon-guard-pq\",",
        "  \"anonymity_status\": \"met\",",
        "  \"anonymity_reason\": \"satisfied\",",
        "  \"anonymity_soranet_selected\": 1,",
        "  \"anonymity_pq_selected\": 1,",
        "  \"anonymity_classical_selected\": 0,",
        "  \"anonymity_classical_ratio\": 0.0,",
        "  \"anonymity_pq_ratio\": 1.0,",
        "  \"anonymity_candidate_ratio\": 0.5,",
        "  \"anonymity_deficit_ratio\": 0.0,",
        "  \"anonymity_supply_delta\": -0.5,",
        "  \"anonymity_brownout\": false,",
        "  \"anonymity_brownout_effective\": false,",
        "  \"anonymity_uses_classical\": false",
        "}");
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    final List<TelemetryRecord> requests = new ArrayList<>();
    final List<TelemetryRecord> responses = new ArrayList<>();
    final List<TelemetryRecord> failures = new ArrayList<>();

    @Override
    public void onRequest(final TelemetryRecord record) {
      requests.add(record);
    }

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {
      responses.add(record);
    }

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {
      failures.add(record);
    }
  }

  private static final class RecordingObserver implements ClientObserver {
    int requestCount = 0;
    int responseCount = 0;
    int failureCount = 0;

    @Override
    public void onRequest(final TransportRequest request) {
      requestCount++;
    }

    @Override
    public void onResponse(final TransportRequest request, final ClientResponse response) {
      responseCount++;
    }

    @Override
    public void onFailure(final TransportRequest request, final Throwable error) {
      failureCount++;
    }
  }
}
