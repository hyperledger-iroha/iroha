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
import org.hyperledger.iroha.android.offline.OfflineToriiException;

public final class ConfidentialAssetToriiClientTests {
  private ConfidentialAssetToriiClientTests() {}

  public static void main(final String[] args) {
    rootsUsesCanonicalPostPathAndParsesBody();
    merklePathsUsesCanonicalPostPathAndParsesBody();
    emptyLatestRootIsNotNullableOnWireButNullInByteHelper();
    rootsRejectNonCanonicalHexAndNullLatest();
    rootsAndMerklePathsRejectNumericStrings();
    nonSuccessResponsesSurfaceOfflineToriiException();
    System.out.println("[IrohaAndroid] ConfidentialAssetToriiClientTests passed.");
  }

  private static void rootsUsesCanonicalPostPathAndParsesBody() {
    final String root = "01".repeat(32);
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "latest": "%s",
              "roots": ["%s"],
              "height": 1
            }
            """
                .formatted(root, root));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .build();

    final ZkRootsResponse response = client.getZkAssetRoots(new ZkRootsRequest("usd#bank", 7)).join();

    assert "POST".equals(executor.lastRequest.method()) : "roots must use POST";
    assert "/v1/zk/roots".equals(executor.lastRequest.uri().getPath()) : "roots path mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "content-type header mismatch";
    assert "{\"asset_id\":\"usd#bank\",\"max\":7}".equals(executor.lastBody)
        : "request body mismatch: " + executor.lastBody;
    assert root.equals(response.latest()) : "latest mismatch";
    assert response.roots().equals(List.of(root)) : "roots mismatch";
    assert response.height() == 1 : "height mismatch";
    assert java.util.Arrays.equals(filled((byte) 1), response.latestRootBytes())
        : "latest bytes mismatch";
    assert java.util.Arrays.equals(filled((byte) 1), response.rootBytes(0))
        : "root bytes mismatch";
  }

  private static void merklePathsUsesCanonicalPostPathAndParsesBody() {
    final String commitment = "02".repeat(32);
    final String sibling = "00".repeat(32);
    final String root = "03".repeat(32);
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "root": "%s",
              "frontier_len": 3,
              "tree_depth": 1,
              "paths": [{
                "commitment": "%s",
                "leaf_index": 2,
                "siblings": ["%s"],
                "directions": [0],
                "witness_nodes": ["%s"],
                "root": "%s"
              }]
            }
            """
                .formatted(root, commitment, sibling, root, root));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .build();

    final ZkMerklePathResponse response =
        client.getZkAssetMerklePaths(new ZkMerklePathRequest("usd#bank", List.of(filled((byte) 2)))).join();

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
    assert response.frontierLen() == 3 : "frontier length mismatch";
    assert response.treeDepth() == 1 : "tree depth mismatch";
    assert response.paths().size() == 1 : "path count mismatch";
    assert response.paths().get(0).leafIndex() == 2 : "leaf index mismatch";
    assert response.paths().get(0).siblings().equals(List.of(sibling)) : "siblings mismatch";
    assert java.util.Arrays.equals(new byte[] {0}, response.paths().get(0).directions())
        : "directions mismatch";
  }

  private static void emptyLatestRootIsNotNullableOnWireButNullInByteHelper() {
    final ZkRootsResponse response =
        ZkRootsResponse.parse("{\"latest\":\"\",\"roots\":[],\"height\":0}".getBytes(StandardCharsets.UTF_8));
    assert response.latest().isEmpty() : "latest should be empty";
    assert response.latestRootBytes() == null : "empty latest should map to null bytes";
  }

  private static void rootsRejectNonCanonicalHexAndNullLatest() {
    try {
      new ZkRootsResponse("AA".repeat(32), List.of(), 0);
      throw new AssertionError("expected uppercase latest rejection");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical lowercase") : "wrong message";
    }
    try {
      ZkRootsResponse.parse("{\"latest\":null,\"roots\":[],\"height\":0}".getBytes(StandardCharsets.UTF_8));
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

  private static void nonSuccessResponsesSurfaceOfflineToriiException() {
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(new StubExecutor(503, "{\"error\":\"not ready\"}", "Unavailable", Map.of()))
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.getZkAssetRoots(new ZkRootsRequest("usd#bank")).join();
      throw new AssertionError("expected roots request failure");
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(503).equals(error.statusCode().orElse(null))
          : "status code mismatch";
      assert error.getMessage().contains("/v1/zk/roots") : "path missing from message";
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
      return CompletableFuture.completedFuture(new TransportResponse(status, body, message, headers));
    }
  }
}
