package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.time.Duration;
import java.util.Base64;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.android.model.NetworkId;

public final class ConfidentialAssetToriiClientTests {
  private static final NetworkId NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
  private static final NetworkId OTHER_NETWORK_ID =
      NetworkId.parse(
          "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22");
  private static final KeyPair KEY_PAIR = generateKeyPair();

  private ConfidentialAssetToriiClientTests() {}

  public static void main(final String[] args) {
    rootsUsesCanonicalPostPathAndParsesBody();
    merklePathsUsesCanonicalPostPathAndParsesBody();
    emptyLatestRootIsNotNullableOnWireButNullInByteHelper();
    rootsRejectNonCanonicalHexAndNullLatest();
    rootsAndMerklePathsRejectNumericStrings();
    rootsAndMerklePathsRejectOverflowDuplicateKeysAndInconsistentShape();
    merklePathParserRejectsDuplicateKeysBeforeLastValueWins();
    nonSuccessResponsesSurfaceConfidentialAssetToriiException();
    canonicalAuthConfigurationFailsClosedBeforeDispatch();
    System.out.println("[IrohaAndroid] ConfidentialAssetToriiClientTests passed.");
  }

  private static void rootsUsesCanonicalPostPathAndParsesBody() {
    final String root = "01".repeat(32);
    final String blockHash = "0a".repeat(32);
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "latest": "%s",
              "roots": ["%s"],
              "evaluated_block_height": 7,
              "evaluated_block_hash": "%s"
            }
            """
                .formatted(root, root, blockHash));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .timeout(Duration.ofSeconds(5))
            .build();

    final ZkRootsResponse response =
        client.getZkAssetRoots(new ZkRootsRequest("usd#bank", 7), canonicalAuth("zk-roots-1"))
            .join();

    assert "POST".equals(executor.lastRequest.method()) : "roots must use POST";
    assert "/v1/zk/roots".equals(executor.lastRequest.uri().getPath()) : "roots path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content-type header mismatch";
    assert "{\"asset_id\":\"usd#bank\",\"max\":7}".equals(executor.lastBody)
        : "request body mismatch: " + executor.lastBody;
    assert executor.lastRequest.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "signed roots request must be one-shot";
    assert executor.requestCount == 1 : "signed roots request must dispatch exactly once";
    assertCanonicalSignature(executor.lastRequest, NETWORK_ID, "zk-roots-1", true);
    assertCanonicalSignature(executor.lastRequest, OTHER_NETWORK_ID, "zk-roots-1", false);
    assert root.equals(response.latest()) : "latest mismatch";
    assert response.roots().equals(List.of(root)) : "roots mismatch";
    assert response.evaluatedBlockHeight() == 7 : "height mismatch";
    assert blockHash.equals(response.evaluatedBlockHash()) : "block hash mismatch";
    assert java.util.Arrays.equals(filled((byte) 1), response.latestRootBytes())
        : "latest bytes mismatch";
    assert java.util.Arrays.equals(filled((byte) 1), response.rootBytes(0))
        : "root bytes mismatch";
  }

  private static void merklePathsUsesCanonicalPostPathAndParsesBody() {
    final String commitment = "02".repeat(32);
    final String sibling = "00".repeat(32);
    final String root = "03".repeat(32);
    final String blockHash = "0a".repeat(32);
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "%s",
              "root": "%s",
              "frontier_len": 1,
              "tree_depth": 1,
              "next_zero_path": {
                "commitment": "%s",
                "leaf_index": 1,
                "siblings": ["%s"],
                "directions": [1],
                "witness_nodes": ["%s"],
                "root": "%s"
              },
              "paths": [{
                "commitment": "%s",
                "leaf_index": 0,
                "siblings": ["%s"],
                "directions": [0],
                "witness_nodes": ["%s"],
                "root": "%s"
              }]
            }
            """
                .formatted(blockHash, root, "00".repeat(32), sibling, root, root, commitment, sibling, root, root));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .timeout(Duration.ofSeconds(5))
            .build();

    final ZkMerklePathResponse response =
        client
            .getZkAssetMerklePaths(
                new ZkMerklePathRequest("usd#bank", List.of(filled((byte) 2))),
                canonicalAuth("zk-paths-1"))
            .join();

    assert "POST".equals(executor.lastRequest.method()) : "merkle path must use POST";
    assert "/v1/zk/merkle-path".equals(executor.lastRequest.uri().getPath())
        : "merkle path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content-type header mismatch";
    assert ("{\"asset_id\":\"usd#bank\",\"commitments\":[\"" + commitment + "\"]}")
            .equals(executor.lastBody)
        : "request body mismatch: " + executor.lastBody;
    assert root.equals(response.root()) : "root mismatch";
    assert response.evaluatedBlockHeight() == 7 : "snapshot height mismatch";
    assert blockHash.equals(response.evaluatedBlockHash()) : "snapshot hash mismatch";
    response.requireEvaluatedSnapshot(7, filled((byte) 0x0a));
    expectIllegalArgument(() -> response.requireEvaluatedSnapshot(8, filled((byte) 0x0a)));
    expectIllegalArgument(() -> response.requireEvaluatedSnapshot(7, filled((byte) 0x0b)));
    assert response.frontierLen() == 1 : "frontier length mismatch";
    assert response.treeDepth() == 1 : "tree depth mismatch";
    assert response.paths().size() == 1 : "path count mismatch";
    assert response.paths().get(0).leafIndex() == 0 : "leaf index mismatch";
    assert response.paths().get(0).siblings().equals(List.of(sibling)) : "siblings mismatch";
    assert java.util.Arrays.equals(new byte[] {0}, response.paths().get(0).directions())
        : "directions mismatch";
  }

  private static void emptyLatestRootIsNotNullableOnWireButNullInByteHelper() {
    final ZkRootsResponse response =
        ZkRootsResponse.parse(("{\"latest\":\"\",\"roots\":[],\"evaluated_block_height\":0,\"evaluated_block_hash\":\"" + "00".repeat(32) + "\"}").getBytes(StandardCharsets.UTF_8));
    assert response.latest().isEmpty() : "latest should be empty";
    assert response.latestRootBytes() == null : "empty latest should map to null bytes";
  }

  private static void rootsRejectNonCanonicalHexAndNullLatest() {
    try {
      new ZkRootsResponse("AA".repeat(32), List.of(), 0, "00".repeat(32));
      throw new AssertionError("expected uppercase latest rejection");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical lowercase") : "wrong message";
    }
    try {
      ZkRootsResponse.parse(("{\"latest\":null,\"roots\":[],\"evaluated_block_height\":0,\"evaluated_block_hash\":\"" + "00".repeat(32) + "\"}").getBytes(StandardCharsets.UTF_8));
      throw new AssertionError("expected null latest rejection");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("latest") : "wrong message";
    }
  }

  private static void rootsAndMerklePathsRejectNumericStrings() {
    final String commitment = "02".repeat(32);
    final String sibling = "00".repeat(32);
    final String root = "03".repeat(32);
    expectIllegalArgument(
        () -> ZkRootsResponse.parse("{\"latest\":\"\",\"roots\":[],\"height\":\"0\"}"
            .getBytes(StandardCharsets.UTF_8)));
    expectIllegalArgument(
        () -> ZkRootsResponse.parse("{\"latest\":\"\",\"roots\":[],\"height\":0.0}"
            .getBytes(StandardCharsets.UTF_8)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
              "root": "%s",
                  "frontier_len": "3",
                  "tree_depth": 1,
                  "paths": []
                }
                """
                    .formatted(root)
                    .getBytes(StandardCharsets.UTF_8)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "%s",
                  "frontier_len": 3,
                  "tree_depth": 1,
                  "paths": [{
                    "commitment": "%s",
                    "leaf_index": 2,
                    "siblings": ["%s"],
                    "directions": ["0"],
                    "witness_nodes": ["%s"],
                    "root": "%s"
                  }]
                }
                """
                    .formatted(root, commitment, sibling, root, root)
                    .getBytes(StandardCharsets.UTF_8)));
  }

  private static void rootsAndMerklePathsRejectOverflowDuplicateKeysAndInconsistentShape() {
    final String commitment = "02".repeat(32);
    final String sibling = "00".repeat(32);
    final String root = "03".repeat(32);
    final String otherRoot = "04".repeat(32);
    expectIllegalArgument(
        () ->
            ZkRootsResponse.parse(
                "{\"latest\":\"\",\"roots\":[],\"height\":2147483648}"
                    .getBytes(StandardCharsets.UTF_8)));
    expectIllegalState(
        () ->
            ZkRootsResponse.parse(
                ("{\"latest\":\"\",\"latest\":\"" + root + "\",\"roots\":[],\"height\":0}")
                    .getBytes(StandardCharsets.UTF_8)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "%s",
                  "frontier_len": 2147483648,
                  "tree_depth": 1,
                  "paths": []
                }
                """
                    .formatted(root)
                    .getBytes(StandardCharsets.UTF_8)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, List.of(sibling), List.of(0), List.of(root), otherRoot, 3, 1)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 1, List.of(sibling), List.of(0), List.of(root), root, 3, 1)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 1, List.of(sibling), List.of(1), List.of(root), root, 1, 1)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, List.of(sibling), List.of(0), List.of(root), root, 3, 2)));
    expectIllegalArgument(
        () ->
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, List.of(sibling), List.of(0), List.of(), root, 3, 1)));
  }

  private static void merklePathParserRejectsDuplicateKeysBeforeLastValueWins() {
    final String commitment = "02".repeat(32);
    final String sibling = "00".repeat(32);
    final String root = "03".repeat(32);
    final String otherRoot = "04".repeat(32);
    expectIllegalState(
        () ->
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "%s",
                  "frontier_len": 3,
                  "frontier_len": 1,
                  "tree_depth": 1,
                  "paths": []
                }
                """
                    .formatted(root)
                    .getBytes(StandardCharsets.UTF_8)));
    expectIllegalState(
        () ->
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "%s",
                  "frontier_len": 3,
                  "tree_depth": 1,
                  "paths": [{
                    "commitment": "%s",
                    "commitment": "%s",
                    "leaf_index": 0,
                    "siblings": ["%s"],
                    "directions": [0],
                    "witness_nodes": ["%s"],
                    "root": "%s"
                  }]
                }
                """
                    .formatted(root, commitment, otherRoot, sibling, root, root)
                    .getBytes(StandardCharsets.UTF_8)));
  }

  private static void nonSuccessResponsesSurfaceConfidentialAssetToriiException() {
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(new StubExecutor(503, "{\"error\":\"not ready\"}", "Unavailable", Map.of()))
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .build();
    try {
      client
          .getZkAssetRoots(new ZkRootsRequest("usd#bank"), canonicalAuth("zk-failure-1"))
          .join();
      throw new AssertionError("expected roots request failure");
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof ConfidentialAssetToriiException
          : "expected ConfidentialAssetToriiException";
      final ConfidentialAssetToriiException error =
          (ConfidentialAssetToriiException) ex.getCause();
      assert Integer.valueOf(503).equals(error.getStatusCode()) : "status code mismatch";
      assert error.getMessage().contains("/v1/zk/roots") : "path missing from message";
    }
  }

  private static void canonicalAuthConfigurationFailsClosedBeforeDispatch() {
    final StubExecutor executor = new StubExecutor(200, "{}");
    expectIllegalState(
        () ->
            ConfidentialAssetToriiClient.builder()
                .executor(executor)
                .baseUri(URI.create("https://example.com"))
                .build());
    assert executor.requestCount == 0 : "missing network context must not dispatch";

    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(new LocalSigningContext(NETWORK_ID))
            .addHeader("x-IROHA-signature", "forged")
            .build();
    expectIllegalArgument(
        () ->
            client.getZkAssetRoots(
                new ZkRootsRequest("usd#bank"), canonicalAuth("zk-header-1")));
    assert executor.requestCount == 0 : "ambiguous canonical headers must not dispatch";
  }

  private static ToriiCanonicalRequestAuth canonicalAuth(final String nonce) {
    return new ToriiCanonicalRequestAuth(
        "alice",
        message -> sign(message),
        Long.valueOf(1_700_000_000_000L),
        nonce);
  }

  private static KeyPair generateKeyPair() {
    try {
      return KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create signing key fixture", ex);
    }
  }

  private static byte[] sign(final byte[] message) {
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(KEY_PAIR.getPrivate());
      signer.update(message);
      return signer.sign();
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to sign request fixture", ex);
    }
  }

  private static void assertCanonicalSignature(
      final TransportRequest request,
      final NetworkId expectedNetworkId,
      final String nonce,
      final boolean expected) {
    try {
      final byte[] signature =
          Base64.getDecoder()
              .decode(firstHeader(request, CanonicalRequestSigner.HEADER_SIGNATURE));
      final Signature verifier = Signature.getInstance("Ed25519");
      verifier.initVerify(KEY_PAIR.getPublic());
      verifier.update(
          CanonicalRequestSigner.canonicalRequestSignatureMessage(
              expectedNetworkId,
              request.method(),
              request.uri(),
              request.body(),
              1_700_000_000_000L,
              nonce));
      assert verifier.verify(signature) == expected : "canonical request signature mismatch";
    } catch (final Exception ex) {
      throw new AssertionError("failed to verify canonical request signature", ex);
    }
  }

  private static byte[] filled(final byte value) {
    final byte[] out = new byte[32];
    java.util.Arrays.fill(out, value);
    return out;
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    for (final var entry : request.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name)) {
        final List<String> values = entry.getValue();
        return values.isEmpty() ? "" : values.get(0);
      }
    }
    return "";
  }

  private static void expectIllegalArgument(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected path.
    }
  }

  private static void expectIllegalState(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // Expected path.
    }
  }

  private static byte[] merklePathPayload(
      final String root,
      final String commitment,
      final int leafIndex,
      final List<String> siblings,
      final List<Integer> directions,
      final List<String> witnessNodes,
      final String pathRoot,
      final int frontierLen,
      final int treeDepth) {
    final String siblingJson = quotedStrings(siblings);
    final String directionJson = joinInts(directions);
    final String witnessJson = quotedStrings(witnessNodes);
    return """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
              "root": "%s",
              "frontier_len": %d,
              "tree_depth": %d,
              "next_zero_path": null,
              "paths": [{
                "commitment": "%s",
                "leaf_index": %d,
                "siblings": [%s],
                "directions": [%s],
                "witness_nodes": [%s],
                "root": "%s"
              }]
            }
        """
        .formatted(
            root,
            frontierLen,
            treeDepth,
            commitment,
            leafIndex,
            siblingJson,
            directionJson,
            witnessJson,
            pathRoot)
        .getBytes(StandardCharsets.UTF_8);
  }

  private static String quotedStrings(final List<String> values) {
    final java.util.ArrayList<String> out = new java.util.ArrayList<>(values.size());
    for (final String value : values) {
      out.add("\"" + value + "\"");
    }
    return String.join(",", out);
  }

  private static String joinInts(final List<Integer> values) {
    final java.util.ArrayList<String> out = new java.util.ArrayList<>(values.size());
    for (final Integer value : values) {
      out.add(value.toString());
    }
    return String.join(",", out);
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private final int status;
    private final byte[] body;
    private final String message;
    private final Map<String, List<String>> headers;
    private TransportRequest lastRequest;
    private String lastBody = "";
    private int requestCount;

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
      requestCount++;
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(new TransportResponse(status, body, message, headers));
    }
  }
}
