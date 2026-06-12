package org.hyperledger.iroha.android.privacy;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.ConfidentialAssetToriiClient;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.ZkRootsResponse;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

public final class ZkAssetMerklePathTests {
  private ZkAssetMerklePathTests() {}

  public static void main(final String[] args) {
    localProviderComputesAndVerifiesCurrentFrontierPath();
    localProviderRejectsAmbiguousOrMismatchedFrontiers();
    toriiProviderFetchesAndValidatesNodeEndpointPaths();
    toriiProviderRejectsMismatchedNodeCommitment();
    pathAccessorsReturnDefensiveCopies();
    System.out.println("[IrohaAndroid] ZkAssetMerklePathTests passed.");
  }

  private static void localProviderComputesAndVerifiesCurrentFrontierPath() {
    final List<byte[]> commitments = List.of(scalarBytes(1), scalarBytes(2), scalarBytes(3));
    final byte[] root = computeRoot(commitments);
    final LocalZkAssetMerklePathProvider provider =
        new LocalZkAssetMerklePathProvider(List.of(root), commitments);

    final ZkAssetMerklePath path =
        provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    assert path.leafIndex() == 1L : "leaf index mismatch";
    assert path.siblings().size() == LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2
        : "sibling count mismatch";
    assert path.directions().length == LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2
        : "direction count mismatch";
    assert Arrays.equals(root, path.rootAtHeight()) : "root mismatch";
    assert path.verify(commitments.get(1), root, PastaPoseidonNodeHasher.instance())
        : "path must verify";
    assert !path.verify(commitments.get(1), scalarBytes(9), PastaPoseidonNodeHasher.instance())
        : "path must reject wrong root";
  }

  private static void localProviderRejectsAmbiguousOrMismatchedFrontiers() {
    final byte[] repeated = scalarBytes(4);
    final LocalZkAssetMerklePathProvider duplicateProvider =
        new LocalZkAssetMerklePathProvider(List.of(), List.of(repeated, repeated));
    try {
      duplicateProvider.getMerklePathForCommitment("usd#bank", repeated).join();
      throw new AssertionError("expected duplicate commitment rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
    }

    final LocalZkAssetMerklePathProvider mismatchProvider =
        new LocalZkAssetMerklePathProvider(List.of(scalarBytes(9)), List.of(scalarBytes(1)));
    try {
      mismatchProvider.getMerklePathForCommitment("usd#bank", scalarBytes(1)).join();
      throw new AssertionError("expected root mismatch rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
    }
  }

  private static void toriiProviderFetchesAndValidatesNodeEndpointPaths() {
    final List<byte[]> commitments = List.of(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(List.of(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments.get(1))
            .join();
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, commitments.get(1), localPath));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final ToriiZkAssetMerklePathProvider provider = new ToriiZkAssetMerklePathProvider(client);

    final ZkAssetMerklePath path =
        provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    assert "/v1/zk/merkle-path".equals(executor.lastRequest.uri().getPath())
        : "path mismatch";
    assert ("{\"asset_id\":\"usd#bank\",\"commitments\":[\"" + hex(commitments.get(1)) + "\"]}")
            .equals(executor.lastBody)
        : "request body mismatch: " + executor.lastBody;
    assert path.leafIndex() == 1L : "leaf index mismatch";
    assert Arrays.equals(root, path.rootAtHeight()) : "root mismatch";
    assert path.verify(commitments.get(1), root, PastaPoseidonNodeHasher.instance())
        : "path must verify";
  }

  private static void toriiProviderRejectsMismatchedNodeCommitment() {
    final byte[] requested = scalarBytes(1);
    final byte[] root = computeRoot(List.of(requested));
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(List.of(root), List.of(requested))
            .getMerklePathForCommitment("usd#bank", requested)
            .join();
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, scalarBytes(2), localPath));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final ToriiZkAssetMerklePathProvider provider = new ToriiZkAssetMerklePathProvider(client);
    try {
      provider.getMerklePathForCommitment("usd#bank", requested).join();
      throw new AssertionError("expected commitment mismatch");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("commitment mismatch")
          : "wrong message";
    }
  }

  private static void pathAccessorsReturnDefensiveCopies() {
    final byte[] commitment = scalarBytes(1);
    final LocalZkAssetMerklePathProvider provider =
        new LocalZkAssetMerklePathProvider(List.of(), List.of(commitment));
    final ZkAssetMerklePath path = provider.getMerklePathForCommitment("usd#bank", commitment).join();
    final byte[] root = path.rootAtHeight();
    root[0] = 99;
    final byte[] sibling = path.siblings().get(0);
    sibling[0] = 88;
    final byte[] directions = path.directions();
    directions[0] = 1;

    assert path.verify(commitment, path.rootAtHeight(), PastaPoseidonNodeHasher.instance())
        : "defensive copies were not preserved";
  }

  private static byte[] scalarBytes(final int value) {
    final byte[] out = new byte[32];
    out[0] = (byte) value;
    return out;
  }

  private static String hex(final byte[] bytes) {
    return ZkRootsResponse.encodeHex(bytes);
  }

  private static String merklePathResponse(
      final byte[] root, final byte[] commitment, final ZkAssetMerklePath path) {
    final String siblings = quotedHexList(path.siblings());
    final String directions = directionList(path.directions());
    final String witnessNodes = quotedHexList(path.siblings());
    return """
            {
              "root": "%s",
              "frontier_len": 2,
              "tree_depth": %d,
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
            hex(root),
            path.siblings().size(),
            hex(commitment),
            path.leafIndex(),
            siblings,
            directions,
            witnessNodes,
            hex(root));
  }

  private static String quotedHexList(final List<byte[]> values) {
    final ArrayList<String> out = new ArrayList<>(values.size());
    for (final byte[] value : values) {
      out.add("\"" + hex(value) + "\"");
    }
    return String.join(",", out);
  }

  private static String directionList(final byte[] directions) {
    final ArrayList<String> out = new ArrayList<>(directions.length);
    for (final byte direction : directions) {
      out.add(Integer.toString(direction & 0xff));
    }
    return String.join(",", out);
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final String responseBody;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private CapturingExecutor(final String responseBody) {
      this.responseBody = responseBody;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(
          new TransportResponse(
              200, responseBody.getBytes(StandardCharsets.UTF_8), "", java.util.Map.of()));
    }
  }

  private static byte[] computeRoot(final List<byte[]> commitments) {
    ArrayList<byte[]> layer =
        new ArrayList<>(LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_CAPACITY_V2);
    for (final byte[] commitment : commitments) {
      layer.add(commitment.clone());
    }
    while (layer.size() < LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_CAPACITY_V2) {
      layer.add(new byte[32]);
    }
    for (int level = 0; level < LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2; level++) {
      final ArrayList<byte[]> next = new ArrayList<>(layer.size() / 2);
      for (int i = 0; i < layer.size(); i += 2) {
        next.add(PastaPoseidonNodeHasher.instance().hashPair(layer.get(i), layer.get(i + 1)));
      }
      layer = next;
    }
    return layer.get(0);
  }
}
