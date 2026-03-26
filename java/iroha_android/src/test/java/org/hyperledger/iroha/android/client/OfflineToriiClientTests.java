package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CancellationException;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.offline.OfflineAllowanceCommitment;
import org.hyperledger.iroha.android.offline.OfflineAllowanceList;
import org.hyperledger.iroha.android.offline.OfflineAllowanceRegisterResponse;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueRequest;
import org.hyperledger.iroha.android.offline.OfflineBuildClaimIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineBundleProofStatus;
import org.hyperledger.iroha.android.offline.OfflineCertificateIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.hyperledger.iroha.android.offline.OfflineProofRequestKind;
import org.hyperledger.iroha.android.offline.OfflineProofRequestParams;
import org.hyperledger.iroha.android.offline.OfflineProofRequestResult;
import org.hyperledger.iroha.android.offline.OfflineQueryEnvelope;
import org.hyperledger.iroha.android.offline.OfflineSettlementBuildClaimOverride;
import org.hyperledger.iroha.android.offline.OfflineSettlementSubmitResponse;
import org.hyperledger.iroha.android.offline.OfflineTopUpResponse;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineTransferList;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificate;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificateDraft;
import org.hyperledger.iroha.android.offline.OfflineWalletPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class OfflineToriiClientTests {

  private OfflineToriiClientTests() {}

  private static void assertServerSideSigningRemoved(
      final String endpoint, final Runnable action) {
    try {
      action.run();
    } catch (final UnsupportedOperationException expected) {
      assert expected.getMessage().contains(endpoint) : "missing endpoint in error message";
      assert expected.getMessage().contains("locally signed transaction")
          : "missing remediation in error message";
      return;
    }
    throw new AssertionError("Expected " + endpoint + " to reject server-side signing");
  }

  public static void main(final String[] args) {
    listAllowancesParsesResponse();
    propagatesNon2xxResponses();
    propagatesRejectCodeFromNon2xxResponses();
    propagatesNestedJsonMessageFromNon2xxResponses();
    propagatesCaseInsensitiveNestedJsonMessageFromNon2xxResponses();
    propagatesCompactJsonFallbackFromNon2xxResponses();
    queryTransfersUsesPostBody();
    submitSettlementRejectsInsecureTransportForPrivateKeyBody();
    queryEnvelopeFromParamsParsesJson();
    builderRejectsInvalidVerdictFilters();
    buildProofRequestPostsBody();
    submitSettlementPostsBodyAndParsesResponse();
    submitSettlementSupportsBuildClaimOverridesAndRepairFlag();
    submitSettlementRejectsInvalidBuildClaimOverrideTxId();
    submitSettlementAndWaitUsesDefaultStatusOptionsWhenOmitted();
    submitSettlementAndWaitPollsTransactionStatus();
    submitSettlementAndWaitPropagatesTransactionStatusFailure();
    submitSettlementAndWaitPropagatesCancellation();
    getSettlementFetchesDetail();
    getBundleProofStatusParsesResponse();
    proofRequestBuilderValidation();
    issueCertificatePostsDraft();
    issueBuildClaimPostsBodyAndParsesResponse();
    issueBuildClaimRejectsUnsupportedPlatform();
    issueCertificateRenewalUsesPath();
    registerAllowancePostsCertificate();
    registerAllowanceParsesResponse();
    topUpAllowanceChainsIssueAndRegister();
    topUpAllowanceRenewalChainsIssueAndRegister();
    System.out.println("[IrohaAndroid] OfflineToriiClientTests passed.");
  }

  private static void listAllowancesParsesResponse() {
    final String assetDefinitionId = AssetDefinitionIdEncoder.encode("usd", "wonderland");
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "total": 1,
              "items": [
                {
                  "certificate_id_hex": "deadbeef",
                  "controller_id": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                  "controller_display": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                  "asset_id": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                  "asset_definition_id": "%s",
                  "asset_definition_name": "USD",
                  "asset_definition_alias": null,
                  "registered_at_ms": 1,
                  "expires_at_ms": 1700000000000,
                  "policy_expires_at_ms": 1710000000000,
                  "refresh_at_ms": 1710500000000,
                  "verdict_id_hex": "feedface",
                  "attestation_nonce_hex": "abcd1234",
                  "remaining_amount": "42.0",
                  "record": {}
                }
              ]
            }
            """
                .formatted(assetDefinitionId));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .addHeader("X-Test", "1")
            .build();
    final OfflineAllowanceList list =
        client
            .listAllowances(
                OfflineListParams.builder()
                    .limit(5L)
                    .offset(10L)
                    .assetId("usd##soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ")
                    .certificateExpiresBeforeMs(1_000L)
                    .certificateExpiresAfterMs(500L)
                    .policyExpiresBeforeMs(2_000L)
                    .policyExpiresAfterMs(750L)
                    .verdictIdHex("DEADBEEF")
                    .requireVerdict(true)
                    .build())
            .join();
    assert list.total() == 1 : "allowance total mismatch";
    assert "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ".equals(list.items().get(0).controllerId())
        : "allowance controller mismatch";
    assert assetDefinitionId.equals(list.items().get(0).assetDefinitionId())
        : "allowance asset definition id mismatch";
    assert "USD".equals(list.items().get(0).assetDefinitionName())
        : "allowance asset definition name mismatch";
    assert list.items().get(0).certificateExpiresAtMs() == 1_700_000_000_000L
        : "certificate expiry mismatch";
    assert "42.0".equals(list.items().get(0).remainingAmount())
        : "remaining amount mismatch";
    assert executor.lastRequest
        .uri()
        .toString()
        .contains("limit=5") : "limit query missing";
    final String query = executor.lastRequest.uri().getQuery();
    assert query.contains("offset=10") : "offset query missing";
    assert query.contains("asset_id=usd##soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ") : "asset_id missing";
    assert query.contains("certificate_expires_before_ms=1000")
        : "certificate_expires_before_ms missing";
    assert query.contains("certificate_expires_after_ms=500")
        : "certificate_expires_after_ms missing";
    assert query.contains("policy_expires_before_ms=2000")
        : "policy_expires_before_ms missing";
    assert query.contains("policy_expires_after_ms=750")
        : "policy_expires_after_ms missing";
    assert query.contains("verdict_id_hex=deadbeef") : "verdict_id_hex missing";
    assert query.contains("require_verdict=true") : "require_verdict missing";
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
      client.submitSettlement(Map.of("bundle_id", "deadbeef"), "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef")
          .join();
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
    throw new AssertionError("Expected CompletionException for non-2xx responses");
  }

  private static void propagatesNestedJsonMessageFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(
            422,
            "{\"error\":{\"detail\":\"offline settlement policy validation failed\"}}");
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
      assert "offline settlement policy validation failed".equals(error.responseBody().orElse(null))
          : "expected nested message to be extracted";
      assert error.getMessage().contains("offline settlement policy validation failed")
          : "nested message missing from exception";
      return;
    }
    throw new AssertionError("Expected CompletionException for nested non-2xx errors");
  }

  private static void propagatesCaseInsensitiveNestedJsonMessageFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(
            422,
            "{\"Error\":{\"Detail\":\"offline settlement policy validation failed\"}}");
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
      assert "offline settlement policy validation failed".equals(error.responseBody().orElse(null))
          : "expected case-insensitive nested message to be extracted";
      assert error.getMessage().contains("offline settlement policy validation failed")
          : "case-insensitive nested message missing from exception";
      return;
    }
    throw new AssertionError("Expected CompletionException for case-insensitive nested non-2xx errors");
  }

  private static void propagatesCompactJsonFallbackFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(422, "{\"status\":\"invalid\",\"code\":\"E123\"}");
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
      assert "{\"code\":\"E123\",\"status\":\"invalid\"}".equals(error.responseBody().orElse(null))
          : "expected compact sorted JSON fallback";
      assert error.getMessage().contains("{\"code\":\"E123\",\"status\":\"invalid\"}")
          : "compact fallback missing from exception";
      return;
    }
    throw new AssertionError("Expected CompletionException for compact fallback non-2xx errors");
  }

  private static void submitSettlementRejectsInsecureTransportForPrivateKeyBody() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("http://example.com"))
            .build();
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlement(Map.of("bundle_id", "deadbeef"), "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef")
                .join());
  }

  private static void queryTransfersUsesPostBody() {
    final StubExecutor executor = new StubExecutor(200, "{\"total\":0,\"items\":[]}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineQueryEnvelope envelope =
        OfflineQueryEnvelope.builder()
            .filterJson("{\"op\":\"eq\",\"args\":[\"receiver_id\",\"soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ\"]}")
            .setLimit(5L)
            .build();
    client.queryTransfers(envelope).join();
    assert "POST".equals(executor.lastRequest.method()) : "expected POST";
    assert executor.lastRequest.uri().getPath().endsWith("/query") : "query path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content type missing";
    assert executor.lastBody.contains("\"limit\":5") : "limit missing in body";
    assert executor.lastBody.contains("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ") : "filter missing in body";
  }

  private static void queryEnvelopeFromParamsParsesJson() {
    final OfflineListParams params =
        OfflineListParams.builder()
            .filter("{\"op\":\"eq\",\"args\":[\"controller_id\",\"soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ\"]}")
            .limit(10L)
            .build();
    final OfflineQueryEnvelope envelope = OfflineQueryEnvelope.fromListParams(params);
    final String json = new String(envelope.toJsonBytes(), StandardCharsets.UTF_8);
    assert json.contains("\"limit\":10") : "limit missing";
    assert json.contains("controller_id") : "filter not parsed";
  }

  private static void builderRejectsInvalidVerdictFilters() {
    try {
      OfflineListParams.builder().verdictIdHex("deadbeef").onlyMissingVerdict(true).build();
      throw new AssertionError("should reject verdictIdHex + onlyMissingVerdict");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    try {
      OfflineListParams.builder().requireVerdict(true).onlyMissingVerdict(true).build();
      throw new AssertionError("should reject requireVerdict + onlyMissingVerdict");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static void buildProofRequestPostsBody() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "header": { "version": 1, "bundle_id": "deadbeef" },
              "counters": [4096, 4097]
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final Map<String, Object> transferPayload = Map.of("bundle_id", "deadbeef");
    final OfflineProofRequestParams params =
        OfflineProofRequestParams.builder()
            .transferPayload(transferPayload)
            .kind(OfflineProofRequestKind.COUNTER)
            .counterCheckpoint(4095L)
            .build();
    final OfflineProofRequestResult result = client.buildProofRequest(params).join();
    assert result.kind() == OfflineProofRequestKind.COUNTER : "kind mismatch";
    assert result.json().contains("\"version\":1") : "missing version";
    assert executor.lastRequest.uri().getPath().endsWith("/proof") : "path mismatch";
    assert executor.lastBody.contains("\"transfer\"") : "transfer not in body";
    assert executor.lastBody.contains("\"bundle_id\":\"deadbeef\"")
        : "bundle id not in body";
    assert executor.lastBody.contains("\"counter_checkpoint\":4095")
        : "checkpoint missing";
  }

  private static void submitSettlementPostsBodyAndParsesResponse() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlement(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef")
                .join());
  }

  private static void submitSettlementSupportsBuildClaimOverridesAndRepairFlag() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineSettlementBuildClaimOverride claimOverride =
        OfflineSettlementBuildClaimOverride.builder()
            .txIdHex("ab".repeat(32))
            .appId("com.example.android")
            .buildNumber(77L)
            .issuedAtMs(1_700_000_000_000L)
            .expiresAtMs(1_700_000_100_000L)
            .build();

    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlement(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef",
                    List.of(claimOverride),
                    true)
                .join());
  }

  private static void submitSettlementRejectsInvalidBuildClaimOverrideTxId() {
    try {
      OfflineSettlementBuildClaimOverride.builder()
          .txIdHex("not-a-hash")
          .build();
      throw new AssertionError("expected txIdHex validation failure");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("txIdHex") : "unexpected error message";
    }
  }

  private static void submitSettlementAndWaitPollsTransactionStatus() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final StubStatusClient statusClient = StubStatusClient.success(Map.of("status", "Committed"));
    final PipelineStatusOptions options =
        PipelineStatusOptions.builder().intervalMillis(50L).maxAttempts(5).build();
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlementAndWait(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef",
                    statusClient,
                    options)
                .join());
    assert statusClient.waitCalls == 0 : "status polling should not start";
  }

  private static void submitSettlementAndWaitUsesDefaultStatusOptionsWhenOmitted() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final StubStatusClient statusClient = StubStatusClient.success(Map.of("status", "Committed"));
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlementAndWait(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef",
                    statusClient)
                .join());
    assert statusClient.waitCalls == 0 : "status polling should not start";
  }

  private static void submitSettlementAndWaitPropagatesTransactionStatusFailure() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final TransactionStatusException statusFailure =
        new TransactionStatusException(
            "cb".repeat(32), "Rejected", "build_claim_missing", Map.of("kind", "Transaction"));
    final StubStatusClient statusClient = StubStatusClient.failure(statusFailure);
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlementAndWait(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef",
                    statusClient,
                    PipelineStatusOptions.builder().intervalMillis(1L).maxAttempts(2).build())
                .join());
    assert statusClient.waitCalls == 0 : "status polling should not start";
  }

  private static void submitSettlementAndWaitPropagatesCancellation() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final StubStatusClient statusClient =
        StubStatusClient.failure(new CancellationException("poll cancelled"));
    assertServerSideSigningRemoved(
        "/v1/offline/settlements",
        () ->
            client
                .submitSettlementAndWait(
                    Map.of("bundle_id", "deadbeef", "receipts", List.of()),
                    "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
                    "deadbeef",
                    statusClient)
                .join());
    assert statusClient.waitCalls == 0 : "status polling should not start";
  }

  private static void getSettlementFetchesDetail() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "bundle_id_hex": "deadbeef",
              "receiver_id": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
              "receiver_display": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
              "deposit_account_id": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
              "deposit_account_display": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
              "asset_id": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
              "receipt_count": 1,
              "total_amount": "5",
              "claimed_delta": "5",
              "status": "settled",
              "recorded_at_ms": 1700000000000,
              "recorded_at_height": 42,
              "status_transitions": [
                {"status":"settled","transitioned_at_ms":1700000000000}
              ],
              "transfer": {}
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineTransferList.OfflineTransferItem item = client.getSettlement("deadbeef").join();
    assert "deadbeef".equals(item.bundleIdHex()) : "bundle id mismatch";
    assert "settled".equals(item.status()) : "status mismatch";
    assert Long.valueOf(1_700_000_000_000L).equals(item.recordedAtMs())
        : "recordedAtMs mismatch";
    assert Long.valueOf(42L).equals(item.recordedAtHeight()) : "recordedAtHeight mismatch";
    assert item.statusTransitionsJson() != null && item.statusTransitionsJson().contains("settled")
        : "status transitions missing";
    assert "GET".equals(executor.lastRequest.method()) : "expected GET";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/settlements/deadbeef")
        : "settlement detail path mismatch";
  }

  private static void getBundleProofStatusParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "bundle_id_hex": "deadbeef",
              "receipts_root_hex": "aa",
              "aggregate_proof_root_hex": "aa",
              "receipts_root_matches": true,
              "proof_status": "match",
              "proof_summary": {
                "version": 1,
                "proof_sum_bytes": 32,
                "proof_counter_bytes": 64,
                "proof_replay_bytes": 96,
                "metadata_keys": ["alpha", "beta"]
              }
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineBundleProofStatus status = client.getBundleProofStatus("deadbeef").join();
    assert "deadbeef".equals(status.bundleIdHex()) : "bundle id mismatch";
    assert "match".equals(status.proofStatus()) : "proof status mismatch";
    assert Boolean.TRUE.equals(status.receiptsRootMatches()) : "root match mismatch";
    assert status.proofSummary() != null : "proof summary missing";
    assert status.proofSummary().version() == 1 : "version mismatch";
    assert status.proofSummary().metadataKeys().size() == 2 : "metadata keys mismatch";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/bundle/proof_status")
        : "proof status path mismatch";
    assert executor.lastRequest.uri().getQuery().contains("bundle_id_hex=deadbeef")
        : "bundle_id_hex query missing";
  }

  private static void proofRequestBuilderValidation() {
    final Map<String, Object> transferPayload = Map.of("bundle_id", "deadbeef");
    try {
      OfflineProofRequestParams.builder()
          .kind(OfflineProofRequestKind.SUM)
          .build();
      throw new AssertionError("transfer payload is required");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    try {
      OfflineProofRequestParams.builder()
          .transferPayload(transferPayload)
          .kind(OfflineProofRequestKind.REPLAY)
          .build();
      throw new AssertionError("replay proofs require head/tail");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
    try {
      OfflineProofRequestParams.builder()
          .transferPayload(transferPayload)
          .kind(OfflineProofRequestKind.SUM)
          .replayLogHeadHex("aa")
          .replayLogTailHex("bb")
          .build();
      throw new AssertionError("non-replay proofs must not set head/tail");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static void issueCertificatePostsDraft() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "certificate_id_hex": "deadbeef",
              "certificate": {
                "controller": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                "operator": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                "allowance": { "asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "amount": "10", "commitment": [1, 2] },
                "spend_public_key": "ed0120deadbeef",
                "attestation_report": [3, 4],
                "issued_at_ms": 100,
                "expires_at_ms": 200,
                "policy": { "max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200 },
                "operator_signature": "AA",
                "metadata": {},
                "verdict_id": null,
                "attestation_nonce": null,
                "refresh_at_ms": null
              }
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            null,
            null,
            null,
            null);
    final OfflineCertificateIssueResponse response = client.issueCertificate(draft).join();
    assert response.certificateIdHex().equals("deadbeef") : "certificate id mismatch";
    assert response.certificate().controller().equals("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ")
        : "certificate controller mismatch";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/certificates/issue")
        : "certificate issue path mismatch";
    assert executor.lastBody.contains("\"certificate\"") : "certificate missing from body";
    assert !executor.lastBody.contains("\"operator\"")
        : "draft request must omit operator";
  }

  private static void issueBuildClaimPostsBodyAndParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "claim_id_hex": "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface",
              "build_claim": {
                "claim_id": "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
                "nonce": "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
                "platform": "Apple",
                "app_id": "com.example.ios",
                "build_number": 77,
                "issued_at_ms": 1700000000000,
                "expires_at_ms": 1700000100000,
                "operator_signature": "AA"
              }
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();

    final OfflineBuildClaimIssueRequest request =
        OfflineBuildClaimIssueRequest.builder()
            .certificateIdHex("ab".repeat(32))
            .txIdHex("cd".repeat(32))
            .platform("apple")
            .appId("com.example.ios")
            .buildNumber(77L)
            .issuedAtMs(1_700_000_000_000L)
            .expiresAtMs(1_700_000_100_000L)
            .build();
    final OfflineBuildClaimIssueResponse response = client.issueBuildClaim(request).join();

    assert response.claimIdHex().equals("feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface")
        : "claim id mismatch";
    assert "Apple".equals(response.buildClaim().get("platform"))
        : "build claim platform mismatch";
    assert "Apple".equals(response.typedBuildClaim().platform())
        : "typed build claim platform mismatch";
    assert "com.example.ios".equals(response.typedBuildClaim().appId())
        : "typed build claim app id mismatch";
    assert response.typedBuildClaim().buildNumber() == 77L
        : "typed build claim build number mismatch";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/build-claims/issue")
        : "build claim issue path mismatch";
    assert executor.lastBody.contains("\"certificate_id_hex\":\"" + "ab".repeat(32) + "\"")
        : "certificate_id_hex missing";
    assert executor.lastBody.contains("\"tx_id_hex\":\"" + "cd".repeat(32) + "\"")
        : "tx_id_hex missing";
    assert executor.lastBody.contains("\"platform\":\"apple\"")
        : "platform missing";
  }

  private static void issueBuildClaimRejectsUnsupportedPlatform() {
    try {
      OfflineBuildClaimIssueRequest.builder()
          .certificateIdHex("ab".repeat(32))
          .txIdHex("cd".repeat(32))
          .platform("windows-phone")
          .build();
      throw new AssertionError("expected platform validation failure");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("platform") : "unexpected error message";
    }
  }

  private static void issueCertificateRenewalUsesPath() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "certificate_id_hex": "deadbeef",
              "certificate": {
                "controller": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                "operator": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
                "allowance": { "asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "amount": "10", "commitment": [1, 2] },
                "spend_public_key": "ed0120deadbeef",
                "attestation_report": [3, 4],
                "issued_at_ms": 100,
                "expires_at_ms": 200,
                "policy": { "max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200 },
                "operator_signature": "AA",
                "metadata": {},
                "verdict_id": null,
                "attestation_nonce": null,
                "refresh_at_ms": null
              }
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            null,
            null,
            null,
            null);
    client.issueCertificateRenewal("DEADBEEF", draft).join();
    assert executor.lastRequest.uri().getPath().contains("/v1/offline/certificates/")
        : "renewal path missing";
    assert executor.lastRequest.uri().getPath().endsWith("/renew/issue")
        : "renewal path mismatch";
    assert !executor.lastBody.contains("\"operator\"")
        : "renew draft request must omit operator";
  }

  private static void registerAllowancePostsCertificate() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final String verdictId =
        "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    final String attestationNonce =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    final OfflineWalletCertificate certificate =
        new OfflineWalletCertificate(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            "AA",
            Map.of("note", "value"),
            verdictId,
            attestationNonce,
            null);
    assertServerSideSigningRemoved(
        "/v1/offline/allowances",
        () -> client.registerAllowance(certificate, "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef").join());
  }

  private static void registerAllowanceParsesResponse() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificate certificate =
        new OfflineWalletCertificate(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            "AA",
            Map.of(),
            null,
            null,
            null);
    assertServerSideSigningRemoved(
        "/v1/offline/allowances",
        () ->
            client
                .registerAllowanceDetailed(certificate, "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef")
                .join());
  }

  private static void topUpAllowanceChainsIssueAndRegister() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new SequencedExecutor(List.of()))
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            null,
            null,
            null,
            null);
    assertServerSideSigningRemoved(
        "/v1/offline/allowances",
        () -> client.topUpAllowance(draft, "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef").join());
  }

  private static void topUpAllowanceRenewalChainsIssueAndRegister() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new SequencedExecutor(List.of()))
            .baseUri(URI.create("https://example.com"))
            .build();
    final OfflineAllowanceCommitment allowance =
        new OfflineAllowanceCommitment("7EAD8EFYUx1aVKZPUU1fyKvr8dF1#soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            allowance,
            "ed0120deadbeef",
            new byte[] {3, 4},
            100L,
            200L,
            policy,
            null,
            null,
            null,
            null);
    assertServerSideSigningRemoved(
        "/v1/offline/allowances/deadbeef/renew",
        () ->
            client
                .topUpAllowanceRenewal("deadbeef", draft, "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", "deadbeef")
                .join());
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

  private static final class StubResponse {
    private final int status;
    private final byte[] body;

    private StubResponse(final int status, final String body) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
    }
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private final List<StubResponse> queue;
    private final java.util.List<TransportRequest> requests = new java.util.ArrayList<>();
    private final java.util.List<String> bodies = new java.util.ArrayList<>();

    private SequencedExecutor(final List<StubResponse> responses) {
      this.queue = new java.util.ArrayList<>(responses);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.requests.add(request);
      this.bodies.add(new String(request.body(), StandardCharsets.UTF_8));
      if (queue.isEmpty()) {
        final TransportResponse response =
            new TransportResponse(500, "{}".getBytes(StandardCharsets.UTF_8), "", java.util.Map.of());
        return CompletableFuture.completedFuture(response);
      }
      final StubResponse next = queue.remove(0);
      final TransportResponse response =
          new TransportResponse(next.status, next.body, "", java.util.Map.of());
      return CompletableFuture.completedFuture(response);
    }
  }

  private static final class StubStatusClient implements IrohaClient {
    private final CompletableFuture<Map<String, Object>> nextResult;
    private String lastHashHex;
    private PipelineStatusOptions lastOptions;
    private int waitCalls;

    private StubStatusClient(final CompletableFuture<Map<String, Object>> nextResult) {
      this.nextResult = nextResult;
    }

    private static StubStatusClient success(final Map<String, Object> payload) {
      return new StubStatusClient(CompletableFuture.completedFuture(payload));
    }

    private static StubStatusClient failure(final Throwable throwable) {
      final CompletableFuture<Map<String, Object>> failed = new CompletableFuture<>();
      failed.completeExceptionally(throwable);
      return new StubStatusClient(failed);
    }

    @Override
    public CompletableFuture<ClientResponse> submitTransaction(final SignedTransaction transaction) {
      final CompletableFuture<ClientResponse> future = new CompletableFuture<>();
      future.completeExceptionally(new UnsupportedOperationException("submitTransaction not used"));
      return future;
    }

    @Override
    public CompletableFuture<Map<String, Object>> waitForTransactionStatus(
        final String hashHex, final PipelineStatusOptions options) {
      this.waitCalls += 1;
      this.lastHashHex = hashHex;
      this.lastOptions = options;
      return nextResult;
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
