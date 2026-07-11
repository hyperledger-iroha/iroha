package org.hyperledger.iroha.android.sorafs;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.atomic.AtomicBoolean;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;

public final class SorafsGatewayClientTests {

  private SorafsGatewayClientTests() {}

  public static void main(final String[] args) {
    fetchPostsJsonAndAppliesHeaders();
    failureStatusTriggersException();
    transportErrorPropagatesException();
    fetchSummaryParsesResponse();
    fetchSummaryInvalidJsonFails();
    fetchSummaryRejectsFractionalCounts();
    fetchSummaryRejectsChunkIndexOverflow();
    fetchSummaryRejectsNonCanonicalManifestId();
    fetchSummaryRejectsNonCanonicalChunkerHandle();
    negativeTimeoutIsRejectedBeforeDispatch();
    unsafeOriginsAndFetchPathsAreRejectedBeforeDispatch();
    boundedStreamingResponsesRejectDeclaredAndChunkedOverflow();
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
            "  \"manifest_id_hex\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",",
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
    assert "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        .equals(summary.manifestIdHex());
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
            "  \"manifest_id_hex\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",",
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

  private static void fetchSummaryRejectsChunkIndexOverflow() {
    final GatewayFetchRequest request = sampleRequest();
    final String json =
        String.join(
            "\n",
            "{",
            "  \"manifest_id_hex\": \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",",
            "  \"chunker_handle\": \"sorafs.sf1@1.0.0\",",
            "  \"client_id\": \"android-sdk\",",
            "  \"chunk_count\": 2,",
            "  \"content_length\": 1024,",
            "  \"assembled_bytes\": 1024,",
            "  \"provider_reports\": [",
            "    {\"provider\":\"alpha\",\"successes\":2,\"failures\":1,\"disabled\":false}",
            "  ],",
            "  \"chunk_receipts\": [",
            "    {\"chunk_index\":2147483648,\"provider\":\"alpha\",\"attempts\":1}",
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

  private static void fetchSummaryRejectsNonCanonicalManifestId() {
    final String manifestId =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    assertIllegalArgument(
        () ->
            GatewayFetchSummary.fromJsonBytes(
                summaryJson(
                        manifestId.toUpperCase(java.util.Locale.ROOT), "sorafs.sf1@1.0.0")
                    .getBytes(StandardCharsets.UTF_8)),
        "summary must reject uppercase manifest ids");
    assertIllegalArgument(
        () ->
            GatewayFetchSummary.fromJsonBytes(
                summaryJson("0x" + manifestId, "sorafs.sf1@1.0.0")
                    .getBytes(StandardCharsets.UTF_8)),
        "summary must reject prefixed manifest ids");
  }

  private static void fetchSummaryRejectsNonCanonicalChunkerHandle() {
    final String manifestId =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    final String[] values = {
      " sorafs.sf1@1.0.0", "sorafs/sf1@1.0.0", "sorafs-sf1", "sorafs.sf1@01.0.0"
    };
    for (final String value : values) {
      assertIllegalArgument(
          () ->
              GatewayFetchSummary.fromJsonBytes(
                  summaryJson(manifestId, value).getBytes(StandardCharsets.UTF_8)),
          "summary must reject noncanonical chunker handle: " + value);
    }
  }

  private static void negativeTimeoutIsRejectedBeforeDispatch() {
    assertIllegalArgument(
        () -> SorafsGatewayClient.builder().setTimeout(Duration.ofNanos(-1)),
        "negative timeout must be rejected");

    final RecordingExecutor executor =
        new RecordingExecutor(
            new TransportResponse(
                200,
                "{}".getBytes(StandardCharsets.UTF_8),
                "OK",
                java.util.Collections.emptyMap()));
    final SorafsGatewayClient client =
        SorafsGatewayClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://gateway.example/"))
            .setTimeout(Duration.ZERO)
            .build();
    client.fetch(sampleRequest()).join();
    final TransportRequest request =
        Objects.requireNonNull(executor.lastRequest, "zero-timeout request must be recorded");
    assert Duration.ZERO.equals(request.timeout()) : "zero timeout must be preserved";
  }

  private static void unsafeOriginsAndFetchPathsAreRejectedBeforeDispatch() {
    final String[] unsafeOrigins = {
      "http://gateway.example/",
      "https://gateway.example:443/",
      "https://gateway.example/path",
      "https://localhost/",
      "https://127.0.0.1/",
      "https://10.0.0.1/",
      "https://169.254.169.254/",
      "https://192.0.2.1/",
      "https://[::1]/",
      "https://[fc00::1]/",
      "https://[2001:db8::1]/"
    };
    for (final String origin : unsafeOrigins) {
      assertIllegalArgument(
          () -> SorafsGatewayClient.builder().setBaseUri(URI.create(origin)).build(),
          "unsafe gateway origin must be rejected: " + origin);
    }
    assertIllegalState(
        () -> SorafsGatewayClient.builder().build(),
        "gateway base URI must be explicitly configured");

    final String[] unsafePaths = {
      "",
      "v1/sorafs/gateway/fetch",
      "/",
      "/v1/sorafs/gateway/fetch/",
      "/v1//sorafs/gateway/fetch",
      "/v1/./sorafs/gateway/fetch",
      "/v1/../sorafs/gateway/fetch",
      "/v1/sorafs/gateway/%66etch",
      "/v1/sorafs/gateway/fetch?target=https://evil.example/",
      "/v1/sorafs/gateway/fetch#fragment",
      "http://evil.example/v1/sorafs/gateway/fetch",
      "https://evil.example/v1/sorafs/gateway/fetch"
    };
    for (final String path : unsafePaths) {
      assertIllegalArgument(
          () -> SorafsGatewayClient.builder().setFetchPath(path),
          "unsafe gateway fetch path must be rejected: " + path);
    }
  }

  private static void boundedStreamingResponsesRejectDeclaredAndChunkedOverflow() {
    final AtomicBoolean declaredClosed = new AtomicBoolean(false);
    final BoundedStreamingExecutor declared =
        new BoundedStreamingExecutor(
            new RepeatingInputStream(0),
            java.util.Collections.singletonMap(
                "Content-Length",
                java.util.Collections.singletonList(
                    Integer.toString(16 * 1024 * 1024 + 1))),
            declaredClosed);
    final SorafsGatewayClient declaredClient =
        SorafsGatewayClient.builder()
            .setBaseUri(URI.create("https://gateway.example/"))
            .setExecutor(declared)
            .build();
    assertCompletionFailure(
        () -> declaredClient.fetch(sampleRequest()).join(),
        "oversized declared response must fail");
    assert declaredClosed.get() : "declared-length response must be closed";

    final AtomicBoolean chunkedClosed = new AtomicBoolean(false);
    final BoundedStreamingExecutor chunked =
        new BoundedStreamingExecutor(
            new RepeatingInputStream(16 * 1024 * 1024 + 1),
            java.util.Collections.emptyMap(),
            chunkedClosed);
    final SorafsGatewayClient chunkedClient =
        SorafsGatewayClient.builder()
            .setBaseUri(URI.create("https://gateway.example/"))
            .setExecutor(chunked)
            .build();
    assertCompletionFailure(
        () -> chunkedClient.fetch(sampleRequest()).join(),
        "oversized chunked response must fail");
    assert chunkedClosed.get() : "chunked response must be closed";

    final Map<String, List<String>> ambiguousHeaders = new LinkedHashMap<>();
    ambiguousHeaders.put("Content-Length", java.util.Collections.singletonList("1"));
    ambiguousHeaders.put("content-length", java.util.Collections.singletonList("1"));
    final AtomicBoolean ambiguousClosed = new AtomicBoolean(false);
    final BoundedStreamingExecutor ambiguous =
        new BoundedStreamingExecutor(
            new RepeatingInputStream(1), ambiguousHeaders, ambiguousClosed);
    final SorafsGatewayClient ambiguousClient =
        SorafsGatewayClient.builder()
            .setBaseUri(URI.create("https://gateway.example/"))
            .setExecutor(ambiguous)
            .build();
    assertCompletionFailure(
        () -> ambiguousClient.fetch(sampleRequest()).join(),
        "case-variant duplicate Content-Length headers must fail");
    assert ambiguousClosed.get() : "ambiguous-length response must be closed";

    final AtomicBoolean stalledClosed = new AtomicBoolean(false);
    final BoundedStreamingExecutor stalled =
        new BoundedStreamingExecutor(
            new ZeroProgressInputStream(), java.util.Collections.emptyMap(), stalledClosed);
    final SorafsGatewayClient stalledClient =
        SorafsGatewayClient.builder()
            .setBaseUri(URI.create("https://gateway.example/"))
            .setExecutor(stalled)
            .build();
    assertCompletionFailure(
        () -> stalledClient.fetch(sampleRequest()).join(),
        "zero-progress response streams must fail closed");
    assert stalledClosed.get() : "zero-progress response must be closed";
  }

  private static GatewayFetchRequest sampleRequest() {
    final GatewayProvider provider =
        GatewayProvider.builder()
            .setName("alpha")
            .setProviderIdHex(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .setGatewayPublicKeyHex(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
            .setBaseUrl("https://provider.example/")
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

  private static String summaryJson(final String manifestIdHex, final String chunkerHandle) {
    return "{\"manifest_id_hex\":\""
        + manifestIdHex
        + "\",\"chunker_handle\":\""
        + chunkerHandle
        + "\",\"client_id\":null,\"chunk_count\":0,\"content_length\":0,"
        + "\"assembled_bytes\":0,\"provider_reports\":[],\"chunk_receipts\":[],"
        + "\"anonymity_policy\":\"anon-guard-pq\",\"anonymity_status\":\"met\","
        + "\"anonymity_reason\":null,\"anonymity_soranet_selected\":0,"
        + "\"anonymity_pq_selected\":0,\"anonymity_classical_selected\":0,"
        + "\"anonymity_classical_ratio\":0.0,\"anonymity_pq_ratio\":1.0,"
        + "\"anonymity_candidate_ratio\":1.0,\"anonymity_deficit_ratio\":0.0,"
        + "\"anonymity_supply_delta\":0.0,\"anonymity_brownout\":false,"
        + "\"anonymity_brownout_effective\":false,\"anonymity_uses_classical\":false}";
  }

  private static void assertIllegalArgument(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertIllegalState(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalStateException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static void assertCompletionFailure(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final CompletionException expected) {
      return;
    }
    throw new AssertionError(message);
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

  private static final class BoundedStreamingExecutor
      implements HttpTransportExecutor, StreamingTransportExecutor {
    private final InputStream body;
    private final Map<String, List<String>> headers;
    private final AtomicBoolean closed;

    private BoundedStreamingExecutor(
        final InputStream body,
        final Map<String, List<String>> headers,
        final AtomicBoolean closed) {
      this.body = body;
      this.headers = headers;
      this.closed = closed;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      final CompletableFuture<TransportResponse> result = new CompletableFuture<>();
      result.completeExceptionally(
          new AssertionError("buffering execute path must not be used"));
      return result;
    }

    @Override
    public CompletableFuture<TransportStreamResponse> openStream(
        final TransportRequest request) {
      return CompletableFuture.completedFuture(
          new TransportStreamResponse(
              200, body, "OK", headers, () -> closed.set(true)));
    }
  }

  private static final class RepeatingInputStream extends InputStream {
    private int remaining;

    private RepeatingInputStream(final int remaining) {
      this.remaining = remaining;
    }

    @Override
    public int read() {
      if (remaining == 0) {
        return -1;
      }
      remaining--;
      return 0;
    }

    @Override
    public int read(final byte[] buffer, final int offset, final int length) {
      if (remaining == 0) {
        return -1;
      }
      final int count = Math.min(length, remaining);
      java.util.Arrays.fill(buffer, offset, offset + count, (byte) 0);
      remaining -= count;
      return count;
    }
  }

  private static final class ZeroProgressInputStream extends InputStream {
    @Override
    public int read() {
      return 0;
    }

    @Override
    public int read(final byte[] buffer, final int offset, final int length) {
      return 0;
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
