package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Tests for the typed Torii DA proof client. */
public final class DaToriiClientTests {
  private static final String HASH =
      "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB";

  private DaToriiClientTests() {}

  public static void main(final String[] args) {
    listCommitmentsUsesSnapshotCursorAndTypedResponse();
    proveAndVerifyPinIntentPreserveUnsignedIntegersAndProofShape();
    proveReturnsNullForMissingRecord();
    listPinIntentsUsesLocationCursorAndRequiresResponseEnvelope();
    proofParserRejectsLegacyLocationOnlyPayloadAndMalformedTags();
    pinIntentAliasesUseServerUtf8ByteBound();
    verifyResponseRejectsContradictoryValidityAndError();
    observerErrorsCompleteTheReturnedFuture();
    System.out.println("[IrohaAndroid] DaToriiClientTests passed.");
  }

  private static void listCommitmentsUsesSnapshotCursorAndTypedResponse() {
    final CapturingDaExecutor executor =
        new CapturingDaExecutor(
            "{\"policies\":{\"version\":1,\"policy_hash\":\""
                + HASH
                + "\",\"policies\":[]},\"commitments\":[],"
                + "\"next_cursor\":{\"snapshot\":{\"block_height\":"
                + "18446744073709551615,\"block_hash\":\""
                + HASH
                + "\"},\"after\":{\"lane_id\":7,\"epoch\":"
                + "18446744073709551615,\"sequence\":9}}}");
    final DaModels.ListSnapshot snapshot =
        new DaModels.ListSnapshot(
            new BigInteger("18446744073709551615"), HASH);
    final DaModels.CommitmentListRequest requestModel =
        new DaModels.CommitmentListRequest(
            new BigInteger("18446744073709551615"),
            new DaModels.CommitmentListCursor(
                snapshot,
                new DaModels.CommitmentKey(
                    7L,
                    new BigInteger("18446744073709551615"),
                    BigInteger.valueOf(9))));

    final DaModels.CommitmentListResponse response =
        client(executor).listCommitments(requestModel).join();

    assert "POST".equals(executor.lastRequest.method()) : "commitment query must use POST";
    assert "/v1/da/commitments".equals(executor.lastRequest.uri().getPath())
        : "commitment path mismatch";
    assert Long.valueOf(8L * 1024L * 1024L)
        .equals(executor.lastRequest.maximumResponseBytes())
        : "DA response limit must be enforced by the executor";
    final Map<String, Object> request =
        object(DaJson.parse(executor.lastRequest.body(), "request"));
    assert request.size() == 2
            && request.containsKey("cursor")
            && request.containsKey("limit")
        : "commitment list must only send cursor fields";
    assert new BigInteger("18446744073709551615").equals(request.get("limit"))
        : "u64::MAX list limit must remain exact";
    final Map<String, Object> cursor = object(request.get("cursor"));
    final Map<String, Object> encodedSnapshot = object(cursor.get("snapshot"));
    assert new BigInteger("18446744073709551615")
        .equals(encodedSnapshot.get("block_height")) : "snapshot height mismatch";
    assert HASH.equals(encodedSnapshot.get("block_hash")) : "snapshot hash mismatch";
    assert new BigInteger("18446744073709551615")
        .equals(object(cursor.get("after")).get("epoch")) : "cursor epoch mismatch";
    assert response.policies().version() == 1 : "policy version mismatch";
    assert HASH.equals(response.policies().policyHash()) : "policy hash mismatch";
    assert response.commitments().isEmpty() : "commitment list must be empty";
    assert response.nextCursor() != null : "continuation cursor must decode";
    assert snapshot.blockHeight().equals(response.nextCursor().snapshot().blockHeight())
        : "response snapshot mismatch";
    assert new BigInteger("18446744073709551615")
        .equals(response.nextCursor().after().epoch()) : "response cursor epoch mismatch";
  }

  private static void proveAndVerifyPinIntentPreserveUnsignedIntegersAndProofShape() {
    final CapturingDaExecutor proveExecutor =
        new CapturingDaExecutor(pinIntentProofJson());
    final DaModels.PinIntentProof proof =
        client(proveExecutor)
            .provePinIntent(
                new DaModels.PinIntentQueryRequest(
                    null,
                    DaModels.Digest32.fromHex("22".repeat(32)),
                    null,
                    null,
                    null,
                    null))
            .join();

    assert proof != null : "proof must be present";
    final Map<String, Object> proveRequest =
        object(DaJson.parse(proveExecutor.lastRequest.body(), "request"));
    assert proveRequest.size() == 1 && proveRequest.containsKey("storage_ticket")
        : "pin proof request must contain selector fields only";
    assert new BigInteger("18446744073709551615").equals(proof.intent().epoch())
        : "proof epoch must remain exact";
    assert proof.bundleLength() == 2 : "bundle length mismatch";
    assert proof.path().get(0).direction() == DaModels.MerkleDirection.RIGHT
        : "Merkle direction mismatch";

    final CapturingDaExecutor verifyExecutor =
        new CapturingDaExecutor("{\"valid\":true,\"error\":null}");
    final DaModels.VerifyResponse response =
        client(verifyExecutor).verifyPinIntent(proof).join();
    assert response.valid() : "proof must be valid";
    assert response.error() == null : "valid proof must omit an error";
    assert "/v1/da/pin-intents/verify".equals(verifyExecutor.lastRequest.uri().getPath())
        : "pin-intent verify path mismatch";
    final Map<String, Object> posted =
        object(DaJson.parse(verifyExecutor.lastRequest.body(), "request"));
    final Map<String, Object> intent = object(posted.get("intent"));
    assert new BigInteger("18446744073709551615").equals(intent.get("epoch"))
        : "verified proof must preserve u64::MAX";
    assert intent.containsKey("alias") && intent.get("alias") == null
        : "optional pin alias must be explicit null";
    assert intent.containsKey("owner") && intent.get("owner") == null
        : "optional pin owner must be explicit null";
    final Map<String, Object> pathItem = object(list(posted.get("path")).get(0));
    final Map<String, Object> direction = object(pathItem.get("direction"));
    assert "Right".equals(direction.get("direction")) : "direction tag mismatch";
    assert direction.containsKey("value") && direction.get("value") == null
        : "unit direction must carry a null value";
  }

  private static void proveReturnsNullForMissingRecord() {
    final CapturingDaExecutor executor = new CapturingDaExecutor("null");
    assert client(executor)
            .proveCommitment(
                new DaModels.CommitmentProofRequest(
                    DaModels.Digest32.fromHex("11".repeat(32)),
                    7L,
                    new BigInteger("18446744073709551615"),
                    BigInteger.ONE))
            .join()
        == null
        : "missing proof must decode as null";
    assert "/v1/da/commitments/prove".equals(executor.lastRequest.uri().getPath())
        : "commitment prove path mismatch";
    final Map<String, Object> request =
        object(DaJson.parse(executor.lastRequest.body(), "request"));
    assert request.size() == 4
            && request.containsKey("manifest_hash")
            && request.containsKey("lane_id")
            && request.containsKey("epoch")
            && request.containsKey("sequence")
        : "commitment proof request must contain selectors only";
    assert !request.containsKey("limit")
            && !request.containsKey("cursor")
            && !request.containsKey("pagination")
        : "commitment proof request must not carry list pagination";
  }

  private static void listPinIntentsUsesLocationCursorAndRequiresResponseEnvelope() {
    final CapturingDaExecutor executor =
        new CapturingDaExecutor(
            "{\"intents\":[],\"next_cursor\":{"
                + "\"snapshot\":{\"block_height\":10,\"block_hash\":\""
                + HASH
                + "\"},\"after\":{\"block_height\":9,"
                + "\"index_in_bundle\":4294967295}}}");
    final DaModels.PinIntentListRequest request =
        new DaModels.PinIntentListRequest(
            BigInteger.valueOf(5),
            new DaModels.PinIntentListCursor(
                new DaModels.ListSnapshot(BigInteger.TEN, HASH),
                new DaModels.Location(BigInteger.valueOf(9), 4_294_967_295L)));

    final DaModels.PinIntentListResponse response =
        client(executor).listPinIntents(request).join();

    assert response.intents().isEmpty() : "pin list must decode its envelope";
    assert response.nextCursor() != null : "pin continuation cursor must decode";
    assert BigInteger.TEN.equals(response.nextCursor().snapshot().blockHeight())
        : "pin snapshot height mismatch";
    assert response.nextCursor().after().indexInBundle() == 4_294_967_295L
        : "pin cursor location mismatch";
    final Map<String, Object> posted =
        object(DaJson.parse(executor.lastRequest.body(), "request"));
    assert posted.size() == 2
            && posted.containsKey("cursor")
            && posted.containsKey("limit")
        : "pin list request must carry cursor fields only";
    assert Long.valueOf(9)
        .equals(object(object(posted.get("cursor")).get("after")).get("block_height"))
        : "pin cursor location must round-trip";

    final DaModels.PinIntentListResponse finalPage =
        client(new CapturingDaExecutor("{\"intents\":[],\"next_cursor\":null}"))
            .listPinIntents()
            .join();
    assert finalPage.intents().isEmpty() : "final pin page must decode";
    assert finalPage.nextCursor() == null : "final pin cursor must be explicit null";

    boolean rejected = false;
    try {
      client(new CapturingDaExecutor("[]")).listPinIntents().join();
    } catch (final CompletionException error) {
      rejected = error.getCause() instanceof DaToriiException;
    }
    assert rejected : "legacy bare-array pin lists must be rejected";
  }

  private static void proofParserRejectsLegacyLocationOnlyPayloadAndMalformedTags() {
    boolean rejected = false;
    try {
      DaModels.ProofScheme.fromJson(
          DaJson.parse(
              "{\"type\":\"KzgBls12_381\",\"value\":null}"
                  .getBytes(StandardCharsets.UTF_8),
              "proof scheme"),
          "proof_scheme");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "removed KZG proof schemes must be rejected";

    rejected = false;
    try {
      DaJson.parsePinIntentProof(
          DaJson.parse(
              "{\"intent\":{},\"location\":{\"block_height\":1,\"index_in_bundle\":0}}"
                  .getBytes(StandardCharsets.UTF_8),
              "proof"),
          "proof");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "legacy location-only proof must be rejected";

    final String malformed =
        pinIntentProofJson()
            .replace(
                "\"direction\":{\"direction\":\"Right\",\"value\":null}",
                "\"direction\":\"Right\"");
    rejected = false;
    try {
      DaJson.parsePinIntentProof(
          DaJson.parse(malformed.getBytes(StandardCharsets.UTF_8), "proof"), "proof");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "string Merkle directions must be rejected";

    final String inconsistentPath =
        pinIntentProofJson().replace("\"bundle_len\":2", "\"bundle_len\":1");
    rejected = false;
    try {
      DaJson.parsePinIntentProof(
          DaJson.parse(
              inconsistentPath.getBytes(StandardCharsets.UTF_8), "proof"),
          "proof");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "proof path must match its bundle geometry";

    final String badChecksum =
        pinIntentProofJson().replace(HASH, HASH.substring(0, HASH.length() - 1) + "A");
    rejected = false;
    try {
      DaJson.parsePinIntentProof(
          DaJson.parse(badChecksum.getBytes(StandardCharsets.UTF_8), "proof"),
          "proof");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "non-canonical hash checksums must be rejected";

    rejected = false;
    try {
      DaJson.parseCommitmentList(
          DaJson.parse(
              ("{\"policies\":{\"version\":1,\"policy_hash\":\""
                      + HASH
                      + "\",\"policies\":[]},\"commitments\":[]}")
                  .getBytes(StandardCharsets.UTF_8),
              "response"),
          "response");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "commitment list responses must carry explicit next_cursor";

    rejected = false;
    try {
      DaJson.parsePinIntentList(
          DaJson.parse(
              ("{\"intents\":[],\"next_cursor\":{"
                      + "\"snapshot\":{\"block_height\":10},"
                      + "\"after\":{\"block_height\":9,\"index_in_bundle\":0}}}")
                  .getBytes(StandardCharsets.UTF_8),
              "response"),
          "response");
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "cursor snapshots must carry explicit block_hash";
  }

  private static void pinIntentAliasesUseServerUtf8ByteBound() {
    new DaModels.PinIntentQueryRequest(null, null, "", null, null, null);
    new DaModels.PinIntentQueryRequest(
        null, null, "é".repeat(128), null, null, null);
    new DaModels.RetentionPolicy(
        BigInteger.ZERO,
        BigInteger.ZERO,
        0,
        DaModels.StorageClass.HOT,
        "");
    boolean rejected = false;
    try {
      new DaModels.PinIntentQueryRequest(
          null, null, "é".repeat(129), null, null, null);
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "pin-intent aliases must be bounded by UTF-8 bytes";

    rejected = false;
    try {
      new DaModels.CommitmentListRequest(BigInteger.ZERO, null);
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "list limits must be nonzero";

    rejected = false;
    try {
      new DaModels.PinIntentListCursor(
          new DaModels.ListSnapshot(BigInteger.ONE, null),
          new DaModels.Location(BigInteger.ONE, 0));
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected : "non-empty snapshots must bind a block hash";
  }

  private static void verifyResponseRejectsContradictoryValidityAndError() {
    final CapturingDaExecutor executor =
        new CapturingDaExecutor("{\"valid\":true,\"error\":\"forged\"}");
    final DaModels.PinIntentProof proof =
        DaJson.parsePinIntentProof(
            DaJson.parse(
                pinIntentProofJson().getBytes(StandardCharsets.UTF_8), "proof"),
            "proof");
    boolean rejected = false;
    try {
      client(executor).verifyPinIntent(proof).join();
    } catch (final CompletionException error) {
      rejected = error.getCause() instanceof DaToriiException;
    }
    assert rejected : "contradictory verify response must be rejected";
  }

  private static void observerErrorsCompleteTheReturnedFuture() {
    final CapturingDaExecutor executor =
        new CapturingDaExecutor(
            "{\"version\":1,\"policy_hash\":\""
                + HASH
                + "\",\"policies\":[]}");
    final DaToriiClient client =
        DaToriiClient.builder()
            .setExecutor(executor)
            .setBaseUri(URI.create("https://example.com"))
            .addObserver(
                new ClientObserver() {
                  @Override
                  public void onResponse(
                      final TransportRequest request,
                      final ClientResponse response) {
                    throw new AssertionError("observer failed");
                  }
                })
            .build();
    boolean completedExceptionally = false;
    try {
      client.getProofPolicies().join();
    } catch (final CompletionException error) {
      completedExceptionally = error.getCause() instanceof AssertionError;
    }
    assert completedExceptionally
        : "response-stage errors must complete the returned future exceptionally";
  }

  private static DaToriiClient client(final HttpTransportExecutor executor) {
    return DaToriiClient.builder()
        .setExecutor(executor)
        .setBaseUri(URI.create("https://example.com"))
        .build();
  }

  private static String pinIntentProofJson() {
    final String digest = digestJson(0x22);
    return "{"
        + "\"intent\":{"
        + "\"lane_id\":7,"
        + "\"epoch\":18446744073709551615,"
        + "\"sequence\":9,"
        + "\"storage_ticket\":"
        + digest
        + ",\"manifest_hash\":"
        + digest
        + ",\"alias\":null,\"owner\":null},"
        + "\"location\":{\"block_height\":10,\"index_in_bundle\":0},"
        + "\"bundle_hash\":\""
        + HASH
        + "\",\"bundle_len\":2,\"root\":\""
        + HASH
        + "\",\"path\":[{\"sibling\":\""
        + HASH
        + "\",\"direction\":{\"direction\":\"Right\",\"value\":null}}]}";
  }

  private static String digestJson(final int value) {
    final StringBuilder output = new StringBuilder("[[");
    for (int index = 0; index < 32; index++) {
      if (index != 0) {
        output.append(',');
      }
      output.append(value);
    }
    return output.append("]]").toString();
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> object(final Object value) {
    return (Map<String, Object>) value;
  }

  @SuppressWarnings("unchecked")
  private static List<Object> list(final Object value) {
    return (List<Object>) value;
  }

  private static final class CapturingDaExecutor implements HttpTransportExecutor {
    private final String responseJson;
    private TransportRequest lastRequest;

    private CapturingDaExecutor(final String responseJson) {
      this.responseJson = responseJson;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = request;
      return CompletableFuture.completedFuture(
          TransportResponse.builder()
              .setStatusCode(200)
              .addHeader("Content-Type", "application/json; charset=utf-8")
              .setBody(responseJson.getBytes(StandardCharsets.UTF_8))
              .build());
    }
  }
}
