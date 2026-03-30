package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueRequest;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.hyperledger.iroha.android.offline.OfflineQueryEnvelope;
import org.hyperledger.iroha.android.offline.OfflineRevocationList;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineTransferList;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class OfflineToriiClientTests {

  private OfflineToriiClientTests() {}

  public static void main(final String[] args) {
    listRevocationsParsesResponse();
    propagatesNon2xxResponses();
    propagatesRejectCodeFromNon2xxResponses();
    propagatesNestedJsonMessageFromNon2xxResponses();
    propagatesCaseInsensitiveNestedJsonMessageFromNon2xxResponses();
    propagatesCompactJsonFallbackFromNon2xxResponses();
    listTransfersRejectsInsecureAuthorizationHeader();
    queryTransfersUsesPostBody();
    queryEnvelopeFromParamsParsesJson();
    builderRejectsInvalidVerdictFilters();
    getTransferFetchesDetail();
    cashEndpointsPostCanonicalPathsAndBodies();
    getRevocationBundleJsonUsesCanonicalPath();
    issueBuildClaimPostsBodyAndParsesResponse();
    issueBuildClaimRejectsUnsupportedPlatform();
    System.out.println("[IrohaAndroid] OfflineToriiClientTests passed.");
  }

  private static void listRevocationsParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "total": 1,
              "items": [
                {
                  "verdict_id_hex": "DEADBEEF",
                  "issuer_id": "issuer@sbp",
                  "issuer_display": "issuer@sbp",
                  "revoked_at_ms": 1234,
                  "reason": "compromised_device",
                  "note": "rotated",
                  "metadata": { "foo": "bar" },
                  "record": { "verdict_id_hex": "deadbeef" }
                }
              ]
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .addHeader("X-Test", "1")
            .build();
    final OfflineRevocationList list =
        client
            .listRevocations(
                OfflineListParams.builder()
                    .limit(5L)
                    .offset(2L)
                    .sort("revoked_at_ms:desc")
                    .build())
            .join();
    assert list.total() == 1 : "revocation total mismatch";
    assert list.items().size() == 1 : "revocation item mismatch";
    final OfflineRevocationList.OfflineRevocationItem item = list.items().get(0);
    assert "DEADBEEF".equals(item.verdictIdHex()) : "verdict id mismatch";
    assert "issuer@sbp".equals(item.issuerId()) : "issuer id mismatch";
    assert "issuer@sbp".equals(item.issuerDisplay()) : "issuer display mismatch";
    assert item.revokedAtMs() == 1234L : "revoked_at mismatch";
    assert "compromised_device".equals(item.reason()) : "reason mismatch";
    assert "rotated".equals(item.note()) : "note mismatch";
    assert "bar".equals(item.metadataAsMap().get("foo")) : "metadata mismatch";
    final String query = executor.lastRequest.uri().getQuery();
    assert query.contains("limit=5") : "limit query missing";
    assert query.contains("offset=2") : "offset query missing";
    assert query.contains("sort=revoked_at_ms:desc") : "sort query missing";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
  }

  private static void propagatesNon2xxResponses() {
    final StubExecutor executor = new StubExecutor(500, "{\"error\":\"boom\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.listTransfers(null).join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("500") : "status missing from message";
      assert ex.getCause().getMessage().contains("boom") : "body missing from message";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(500).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert error.responseBody().orElse("").contains("boom")
          : "response body not surfaced";
      assert error.rejectCode().isEmpty() : "unexpected reject code";
      return;
    }
    throw new AssertionError("Expected CompletionException for non-2xx responses");
  }

  private static void propagatesRejectCodeFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(
            400,
            "{\"error\":\"missing build claim\"}",
            "Bad Request",
            Map.of("X-IrOhA-ReJeCt-CoDe", List.of("build_claim_missing")));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.listTransfers(null).join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(400).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert "build_claim_missing".equals(error.rejectCode().orElse(null))
          : "reject code not surfaced";
      assert error.responseBody().orElse("").contains("missing build claim")
          : "response body not surfaced";
      assert error.getMessage().contains("reject_code=build_claim_missing")
          : "reject code missing from message";
      return;
    }
    throw new AssertionError("Expected CompletionException for reject code propagation");
  }

  private static void propagatesNestedJsonMessageFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(503, "{\"message\":{\"error\":\"ingress unavailable\"}}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.listTransfers(null).join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("ingress unavailable")
          : "nested JSON message missing";
      return;
    }
    throw new AssertionError("Expected CompletionException for nested JSON message");
  }

  private static void propagatesCaseInsensitiveNestedJsonMessageFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(503, "{\"Message\":{\"Error\":\"route unavailable\"}}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.listTransfers(null).join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("route unavailable")
          : "case-insensitive message missing";
      return;
    }
    throw new AssertionError("Expected CompletionException for nested JSON message");
  }

  private static void propagatesCompactJsonFallbackFromNon2xxResponses() {
    final StubExecutor executor = new StubExecutor(503, "{\"error\":\"temporary_outage\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.listTransfers(null).join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("temporary_outage")
          : "compact JSON fallback missing";
      return;
    }
    throw new AssertionError("Expected CompletionException for compact JSON fallback");
  }

  private static void listTransfersRejectsInsecureAuthorizationHeader() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{\"items\":[],\"total\":0}"))
            .baseUri(URI.create("http://example.com"))
            .addHeader("Authorization", "Bearer secret")
            .build();
    try {
      client.listTransfers(null);
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("insecure transport over http")
          : "security message mismatch";
      return;
    }
    throw new AssertionError("Expected insecure credentialed HTTP request to fail");
  }

  private static void queryTransfersUsesPostBody() {
    final StubExecutor executor = new StubExecutor(200, "{\"items\":[],\"total\":0}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .build();
    final OfflineQueryEnvelope envelope =
        OfflineQueryEnvelope.builder()
            .filterJson("{\"Eq\":[\"status\",\"pending\"]}")
            .sortJson("[{\"key\":\"recorded_at_ms\",\"order\":\"desc\"}]")
            .setLimit(10L)
            .setOffset(5L)
            .build();
    client.queryTransfers(envelope).join();
    assert "POST".equals(executor.lastRequest.method()) : "query must use POST";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/transfers/query")
        : "query path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content-type mismatch";
    assert executor.lastBody.contains("\"filter\":{\"Eq\":[\"status\",\"pending\"]}")
        : "filter body missing";
    assert executor.lastBody.contains("\"sort\":[{\"key\":\"recorded_at_ms\",\"order\":\"desc\"}]")
        : "sort body missing";
    assert executor.lastBody.contains("\"limit\":10") : "limit body missing";
    assert executor.lastBody.contains("\"offset\":5") : "offset body missing";
  }

  private static void queryEnvelopeFromParamsParsesJson() {
    final OfflineQueryEnvelope envelope =
        OfflineQueryEnvelope.fromListParams(
            OfflineListParams.builder()
                .filter("{\"Eq\":[\"status\",\"pending\"]}")
                .sort("[{\"key\":\"recorded_at_ms\",\"order\":\"desc\"}]")
                .limit(3L)
                .offset(4L)
                .build());
    final String json = new String(envelope.toJsonBytes(), StandardCharsets.UTF_8);
    assert json.contains("\"filter\":{\"Eq\":[\"status\",\"pending\"]}") : "filter missing";
    assert json.contains("\"sort\":[{\"key\":\"recorded_at_ms\",\"order\":\"desc\"}]")
        : "sort missing";
    assert json.contains("\"limit\":3") : "limit missing";
    assert json.contains("\"offset\":4") : "offset missing";
  }

  private static void builderRejectsInvalidVerdictFilters() {
    try {
      OfflineListParams.builder()
          .requireVerdict(true)
          .onlyMissingVerdict(true)
          .build();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("requireVerdict") : "missing conflict explanation";
      return;
    }
    throw new AssertionError("Expected invalid verdict filter combination to fail");
  }

  private static void getTransferFetchesDetail() {
    final String assetDefinitionId = TestAssetDefinitionIds.SECONDARY;
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "bundle_id_hex": "deadbeef",
              "receiver_id": "receiver@sbp",
              "receiver_display": "receiver@sbp",
              "deposit_account_id": "vault@sbp",
              "deposit_account_display": "vault@sbp",
              "asset_id": "%s",
              "receipt_count": 2,
              "total_amount": "42",
              "claimed_delta": "17",
              "status": "pending",
              "recorded_at_ms": 1000,
              "recorded_at_height": 25,
              "status_transitions": [],
              "platform_policy": "play_integrity",
              "platform_token_snapshot": {
                "policy": "play_integrity",
                "attestation_jws_b64": "dG9rZW4="
              },
              "transfer": {
                "receipts": [],
                "balance_proof": { "claimed_delta": "17" }
              }
            }
            """
                .formatted(assetDefinitionId));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineTransferList.OfflineTransferItem item = client.getTransfer("deadbeef").join();
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/transfers/deadbeef")
        : "transfer detail path mismatch";
    assert "deadbeef".equals(item.bundleIdHex()) : "bundle id mismatch";
    assert "receiver@sbp".equals(item.receiverId()) : "receiver mismatch";
    assert assetDefinitionId.equals(item.assetId()) : "asset id mismatch";
    assert item.receiptCount() == 2L : "receipt count mismatch";
    assert "42".equals(item.totalAmount()) : "total amount mismatch";
    assert "17".equals(item.claimedDelta()) : "claimed delta mismatch";
    assert "pending".equals(item.status()) : "status mismatch";
    assert item.recordedAtMs().equals(1000L) : "recorded_at_ms mismatch";
    assert item.recordedAtHeight().equals(25L) : "recorded_at_height mismatch";
    assert "play_integrity".equals(item.platformPolicy()) : "platform policy mismatch";
    assert item.platformTokenSnapshot() != null : "platform token snapshot missing";
  }

  private static void cashEndpointsPostCanonicalPathsAndBodies() {
    assertCashPostPath("/v1/offline/cash/setup", client -> client.setupCash("{\"device_id\":\"setup\"}"));
    assertCashPostPath("/v1/offline/cash/load", client -> client.loadCash("{\"device_id\":\"load\"}"));
    assertCashPostPath(
        "/v1/offline/cash/refresh", client -> client.refreshCash("{\"device_id\":\"refresh\"}"));
    assertCashPostPath("/v1/offline/cash/sync", client -> client.syncCash("{\"device_id\":\"sync\"}"));
    assertCashPostPath(
        "/v1/offline/cash/redeem", client -> client.redeemCash("{\"device_id\":\"redeem\"}"));
  }

  private static void getRevocationBundleJsonUsesCanonicalPath() {
    final StubExecutor executor = new StubExecutor(200, "{\"bundle_id_hex\":\"deadbeef\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final String bundleJson = client.getRevocationBundleJson().join();
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/revocations/bundle")
        : "revocation bundle path mismatch";
    assert "{\"bundle_id_hex\":\"deadbeef\"}".equals(bundleJson) : "bundle payload mismatch";
  }

  private static void issueBuildClaimPostsBodyAndParsesResponse() {
    final String certificateIdHex = "ab".repeat(32);
    final String txIdHex = "cd".repeat(32);
    final String claimIdHex = "ef".repeat(32);
    final String nonceHex = "12".repeat(32);
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "claim_id_hex": "%s",
              "build_claim": {
                "claim_id": "%s",
                "nonce": "%s",
                "platform": "Android",
                "app_id": "com.example.wallet",
                "build_number": 77,
                "issued_at_ms": 1700000000000,
                "expires_at_ms": 1700000100000,
                "lineage_scope": "production",
                "operator_signature": "AA"
              }
            }
            """
                .formatted(claimIdHex, claimIdHex, nonceHex));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineBuildClaimIssueRequest request =
        OfflineBuildClaimIssueRequest.builder()
            .certificateIdHex(certificateIdHex)
            .txIdHex(txIdHex)
            .platform("ANDROID")
            .appId("com.example.wallet")
            .buildNumber(77L)
            .issuedAtMs(1_700_000_000_000L)
            .expiresAtMs(1_700_000_100_000L)
            .build();
    final OfflineBuildClaimIssueResponse response = client.issueBuildClaim(request).join();
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/build-claims/issue")
        : "build claim path mismatch";
    assert "POST".equals(executor.lastRequest.method()) : "build claim must use POST";
    assert executor.lastBody.contains("\"certificate_id_hex\":\"" + certificateIdHex + "\"")
        : "certificate id missing";
    assert executor.lastBody.contains("\"tx_id_hex\":\"" + txIdHex + "\"")
        : "tx id missing";
    assert executor.lastBody.contains("\"platform\":\"android\"") : "platform missing";
    assert executor.lastBody.contains("\"app_id\":\"com.example.wallet\"") : "app id missing";
    assert claimIdHex.equals(response.claimIdHex()) : "claim id mismatch";
    assert "Android".equals(response.typedBuildClaim().platform()) : "typed platform mismatch";
    assert "com.example.wallet".equals(response.typedBuildClaim().appId()) : "typed app id mismatch";
    assert response.typedBuildClaim().buildNumber() == 77L : "typed build number mismatch";
  }

  private static void issueBuildClaimRejectsUnsupportedPlatform() {
    try {
      OfflineBuildClaimIssueRequest.builder()
          .certificateIdHex("ab".repeat(32))
          .txIdHex("cd".repeat(32))
          .platform("windows")
          .build();
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("either \"apple\" or \"android\"")
          : "platform validation mismatch";
      return;
    }
    throw new AssertionError("Expected unsupported platform to fail");
  }

  private static void assertCashPostPath(
      final String expectedPath, final CashInvoker action) {
    final StubExecutor executor = new StubExecutor(200, "{\"state\":\"ok\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final String responseBody = action.invoke(client).join();
    assert expectedPath.equals(executor.lastRequest.uri().getPath())
        : "cash path mismatch for " + expectedPath;
    assert "POST".equals(executor.lastRequest.method()) : "cash endpoint must use POST";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "cash content-type mismatch";
    assert responseBody.contains("\"state\":\"ok\"") : "cash response mismatch";
    assert executor.lastBody.contains("\"device_id\"") : "cash request body missing";
  }

  @FunctionalInterface
  private interface CashInvoker {
    CompletableFuture<String> invoke(OfflineToriiClient client);
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private final int status;
    private final byte[] body;
    private final String message;
    private final Map<String, List<String>> headers;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private StubExecutor(final int status, final String body) {
      this(status, body, "", Map.of());
    }

    private StubExecutor(
        final int status,
        final String body,
        final String message,
        final Map<String, List<String>> headers) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
      this.message = message;
      this.headers = headers;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(
          new TransportResponse(status, body, message, headers));
    }
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    for (final var entry : request.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name)) {
        final List<String> values = entry.getValue();
        if (!values.isEmpty()) {
          return values.get(0);
        }
      }
    }
    return "";
  }
}
