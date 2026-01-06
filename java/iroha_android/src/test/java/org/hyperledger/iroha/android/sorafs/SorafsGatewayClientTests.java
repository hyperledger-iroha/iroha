package org.hyperledger.iroha.android.sorafs;

import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class SorafsGatewayClientTests {

  private SorafsGatewayClientTests() {}

  public static void main(final String[] args) {
    fetchPostsJsonAndAppliesHeaders();
    failureStatusTriggersException();
    transportErrorPropagatesException();
    fetchSummaryParsesResponse();
    fetchSummaryInvalidJsonFails();
    fetchSummaryRejectsFractionalCounts();
    customEncoderOverridesDefault();
    System.out.println("[IrohaAndroid] SoraFS gateway client tests passed.");
  }

  private static void fetchPostsJsonAndAppliesHeaders() {
    final GatewayFetchRequest request = sampleRequest();
    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(
                200, "{\"status\":\"ok\"}".getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
    final CountingObserver observer = new CountingObserver();
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .setTimeout(Duration.ofSeconds(5))
            .putDefaultHeader("Authorization", "Bearer sample")
            .addObserver(observer)
            .build();

    final ClientResponse response = client.fetch(request).join();
    assert response.statusCode() == 200 : "successful response expected";
    assert Arrays.equals(
        "{\"status\":\"ok\"}".getBytes(StandardCharsets.UTF_8), response.body());

    final TransportRequest recorded =
        Objects.requireNonNull(executor.lastRequest, "executor must capture request");
    assert "POST".equals(recorded.method()) : "client must issue POST";
    assert "https://gateway.example/v1/sorafs/gateway/fetch"
            .equals(recorded.uri().toString())
        : "fetch path should resolve relative to base URI";
    assert header(recorded, "Authorization")
        .map("Bearer sample"::equals)
        .orElse(false) : "default headers propagated";
    assert header(recorded, "Content-Type")
        .map("application/json"::equals)
        .orElse(false) : "JSON content type expected";
    assert header(recorded, "Accept")
        .map("application/json"::equals)
        .orElse(false) : "Accept header should default to JSON";
    assert Arrays.equals(request.toJsonBytes(), bodyBytes(recorded))
        : "request body should match JSON payload";
    assert observer.requestCount == 1 : "observer should see request";
    assert observer.responseCount == 1 : "observer should see response";
    assert observer.failureCount == 0 : "observer must not see failure";
  }

  private static void failureStatusTriggersException() {
    final GatewayFetchRequest request = sampleRequest();
    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(
                502,
                "{\"error\":\"bad_gateway\"}".getBytes(StandardCharsets.UTF_8),
                "Bad Gateway",
                Map.of()));
    final CountingObserver observer = new CountingObserver();
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .addObserver(observer)
            .build();

    try {
      client.fetch(request).join();
      throw new AssertionError("expected SorafsStorageException");
    } catch (final CompletionException ex) {
      final Throwable cause = ex.getCause();
      assert cause instanceof SorafsStorageException : "SorafsStorageException expected";
      final String message = cause.getMessage();
      assert message != null && message.contains("502") : "should include HTTP status";
    }

    assert observer.requestCount == 1 : "observer should see request before failure";
    assert observer.responseCount == 0 : "observer should not see successful response";
    assert observer.failureCount == 1 : "observer should see failure callback";
  }

  private static void transportErrorPropagatesException() {
    final GatewayFetchRequest request = sampleRequest();
    final HttpTransportExecutor executor =
        httpRequest -> {
          final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
          future.completeExceptionally(new IOException("network down"));
          return future;
        };
    final CountingObserver observer = new CountingObserver();
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .addObserver(observer)
            .build();

    try {
      client.fetch(request).join();
      throw new AssertionError("expected transport failure to propagate");
    } catch (final CompletionException ex) {
      final Throwable cause = ex.getCause();
      assert cause instanceof SorafsStorageException : "cause should wrap SorafsStorageException";
      assert cause.getCause() instanceof IOException
          : "original transport exception should be retained";
    }
    assert observer.failureCount == 1 : "failure should notify observer";
  }

  private static void fetchSummaryParsesResponse() {
    final GatewayFetchRequest request = sampleRequest();
    final String json =
        String.join(
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
    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(
                200, json.getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .build();

    final GatewayFetchSummary summary = client.fetchSummary(request).join();
    assert "0123".equals(summary.manifestIdHex());
    assert "sorafs.sf1@1.0.0".equals(summary.chunkerHandle());
    assert "android-sdk".equals(summary.clientId());
    assert summary.chunkCount() == 2;
    assert summary.contentLength() == 1024;
    assert summary.assembledBytes() == 1024;
    assert summary.providerReports().size() == 2;
    final GatewayFetchSummary.ProviderReport alpha = summary.providerReports().get(0);
    assert "alpha".equals(alpha.provider());
    assert alpha.successes() == 2;
    assert alpha.failures() == 1;
    assert !alpha.disabled();
    final GatewayFetchSummary.ProviderReport beta = summary.providerReports().get(1);
    assert beta.disabled();
    assert summary.chunkReceipts().size() == 2;
    final GatewayFetchSummary.ChunkReceipt receipt = summary.chunkReceipts().get(1);
    assert receipt.chunkIndex() == 1;
    assert "beta".equals(receipt.provider());
    assert receipt.attempts() == 2;
    assert summary.anonymityPqRatio() == 1.0;
    assert summary.anonymityClassicalRatio() == 0.0;
    assert summary.anonymityCandidateRatio() == 0.5;
    assert summary.anonymitySupplyDelta() == -0.5;
    assert !summary.anonymityBrownout();
  }

  private static void fetchSummaryInvalidJsonFails() {
    final GatewayFetchRequest request = sampleRequest();
    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(200, "[]".getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .build();
    try {
      client.fetchSummary(request).join();
      throw new AssertionError("expected summary parsing to fail");
    } catch (final CompletionException ex) {
      final Throwable cause = ex.getCause();
      assert cause instanceof SorafsStorageException : "expected SorafsStorageException";
    }
  }

  private static void fetchSummaryRejectsFractionalCounts() {
    final GatewayFetchRequest request = sampleRequest();
    final String json =
        String.join(
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
            "    {\"chunk_index\":1.5,\"provider\":\"beta\",\"attempts\":2}",
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
    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(
                200, json.getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example"))
            .build();

    try {
      client.fetchSummary(request).join();
      throw new AssertionError("expected summary parsing to fail");
    } catch (final CompletionException ex) {
      final Throwable cause = ex.getCause();
      assert cause instanceof SorafsStorageException : "expected SorafsStorageException";
    }
  }

  private static void customEncoderOverridesDefault() {
    final GatewayFetchRequest request = sampleRequest();
    final byte[] bridgeBytes = "{\"bridge\":true}".getBytes(StandardCharsets.UTF_8);
    final SorafsGatewayClient.RequestPayloadEncoder original =
        SorafsGatewayClient.requestPayloadEncoder();
    SorafsGatewayClient.setRequestPayloadEncoder(
        req -> {
          assert req == request : "encoder should receive original request";
          return bridgeBytes.clone();
        });
    try {
      assert Arrays.equals(bridgeBytes, request.toJsonBytes())
          : "GatewayFetchRequest should delegate encoding to gateway client";

      final RecordingExecutor executor =
          new RecordingExecutor(
              new TransportResponse(
                  200, "{\"status\":\"ok\"}".getBytes(StandardCharsets.UTF_8), "OK", Map.of()));
      final SorafsGatewayClient client =
          SorafsGatewayClient.builder()
              .setExecutor(executor)
              .setBaseUri(URI.create("https://gateway.example"))
              .build();
      client.fetch(request).join();

      final TransportRequest recorded =
          Objects.requireNonNull(executor.lastRequest, "request must be recorded");
      assert Arrays.equals(bridgeBytes, bodyBytes(recorded))
          : "custom encoder should control payload bytes";
    } finally {
      SorafsGatewayClient.setRequestPayloadEncoder(original);
    }
  }

  private static GatewayFetchRequest sampleRequest() {
    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("alpha")
            .setProviderIdHex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .setBaseUrl("https://provider.example")
            .setStreamTokenBase64("c3RyZWFtLXRva2Vu")
            .build();
    return GatewayFetchRequest.builder()
        .setManifestIdHex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        .setChunkerHandle("sorafs.sf1@1.0.0")
        .setOptions(
            GatewayFetchOptions.builder()
                .setClientId("android-sdk")
                .setTelemetryRegion("ap-northeast-1")
                .build())
        .addProvider(provider)
        .build();
  }

  private static byte[] bodyBytes(final TransportRequest request) {
    return request.body();
  }

  private static Optional<String> header(final TransportRequest request, final String name) {
    final var values = request.headers().get(name);
    if (values == null || values.isEmpty()) {
      return Optional.empty();
    }
    return Optional.ofNullable(values.get(0));
  }

  private static final class RecordingExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;

    private RecordingExecutor(final TransportResponse response) {
      this.response = response;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(
        final TransportRequest request) {
      this.lastRequest = request;
      return CompletableFuture.completedFuture(response);
    }
  }

  private static final class CountingObserver implements ClientObserver {
    private int requestCount;
    private int responseCount;
    private int failureCount;

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
