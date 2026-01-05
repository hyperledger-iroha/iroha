package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.offline.OfflineAllowanceCommitment;
import org.hyperledger.iroha.android.offline.OfflineAllowanceList;
import org.hyperledger.iroha.android.offline.OfflineCertificateIssueResponse;
import org.hyperledger.iroha.android.offline.OfflineListParams;
import org.hyperledger.iroha.android.offline.OfflineProofRequestKind;
import org.hyperledger.iroha.android.offline.OfflineProofRequestParams;
import org.hyperledger.iroha.android.offline.OfflineProofRequestResult;
import org.hyperledger.iroha.android.offline.OfflineQueryEnvelope;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineWalletCertificateDraft;
import org.hyperledger.iroha.android.offline.OfflineWalletPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class OfflineToriiClientTests {

  private OfflineToriiClientTests() {}

  public static void main(final String[] args) {
    listAllowancesParsesResponse();
    propagatesNon2xxResponses();
    queryTransfersUsesPostBody();
    queryEnvelopeFromParamsParsesJson();
    builderRejectsInvalidVerdictFilters();
    buildProofRequestPostsBody();
    proofRequestBuilderValidation();
    issueCertificatePostsDraft();
    issueCertificateRenewalUsesPath();
    System.out.println("[IrohaAndroid] OfflineToriiClientTests passed.");
  }

  private static void listAllowancesParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "total": 1,
              "items": [
                {
                  "certificate_id_hex": "deadbeef",
                  "controller_id": "alice@wonderland",
                  "controller_display": "alice@wonderland",
                  "asset_id": "usd#wonderland",
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
            """);
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
                    .certificateExpiresBeforeMs(1_000L)
                    .certificateExpiresAfterMs(500L)
                    .policyExpiresBeforeMs(2_000L)
                    .policyExpiresAfterMs(750L)
                    .verdictIdHex("DEADBEEF")
                    .requireVerdict(true)
                    .build())
            .join();
    assert list.total() == 1 : "allowance total mismatch";
    assert "alice@wonderland".equals(list.items().get(0).controllerId())
        : "allowance controller mismatch";
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
      return;
    }
    throw new AssertionError("Expected CompletionException for non-2xx responses");
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
            .filterJson("{\"op\":\"eq\",\"args\":[\"receiver_id\",\"merchant@wonderland\"]}")
            .setLimit(5L)
            .build();
    client.queryTransfers(envelope).join();
    assert "POST".equals(executor.lastRequest.method()) : "expected POST";
    assert executor.lastRequest.uri().getPath().endsWith("/query") : "query path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content type missing";
    assert executor.lastBody.contains("\"limit\":5") : "limit missing in body";
    assert executor.lastBody.contains("merchant@wonderland") : "filter missing in body";
  }

  private static void queryEnvelopeFromParamsParsesJson() {
    final OfflineListParams params =
        OfflineListParams.builder()
            .filter("{\"op\":\"eq\",\"args\":[\"controller_id\",\"merchant@wonderland\"]}")
            .limit(10L)
            .addressFormat("canonical")
            .build();
    final OfflineQueryEnvelope envelope = OfflineQueryEnvelope.fromListParams(params);
    final String json = new String(envelope.toJsonBytes(), StandardCharsets.UTF_8);
    assert json.contains("\"limit\":10") : "limit missing";
    assert json.contains("controller_id") : "filter not parsed";
    assert json.contains("canonical") : "address format missing";
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
                "controller": "alice@wonderland",
                "allowance": { "asset": "usd#wonderland", "amount": "10", "commitment": [1, 2] },
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
        new OfflineAllowanceCommitment("usd#wonderland", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "alice@wonderland",
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
    assert response.certificate().controller().equals("alice@wonderland")
        : "certificate controller mismatch";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/certificates/issue")
        : "certificate issue path mismatch";
    assert executor.lastBody.contains("\"certificate\"") : "certificate missing from body";
  }

  private static void issueCertificateRenewalUsesPath() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "certificate_id_hex": "deadbeef",
              "certificate": {
                "controller": "alice@wonderland",
                "allowance": { "asset": "usd#wonderland", "amount": "10", "commitment": [1, 2] },
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
        new OfflineAllowanceCommitment("usd#wonderland", "10", new byte[] {1, 2});
    final OfflineWalletPolicy policy = new OfflineWalletPolicy("10", "5", 200L);
    final OfflineWalletCertificateDraft draft =
        new OfflineWalletCertificateDraft(
            "alice@wonderland",
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
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private final int status;
    private final byte[] body;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private StubExecutor(final int status, final String body) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(new TransportResponse(status, body, "", java.util.Map.of()));
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
