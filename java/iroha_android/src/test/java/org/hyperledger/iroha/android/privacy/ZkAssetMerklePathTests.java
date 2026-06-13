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
    toriiProviderRejectsPathCountDriftAndReorderedNodeResponses();
    toriiProviderRejectsMismatchedNodeCommitment();
    toriiProviderRejectsNonVerifyingNodePath();
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

    assertThrows(
        () -> new ZkAssetMerklePath(1, List.of(new byte[32]), new byte[] {0}, new byte[32], 1));
    assertThrows(
        () -> new ZkAssetMerklePath(2, List.of(new byte[32]), new byte[] {0}, new byte[32], 1));
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

  private static void toriiProviderRejectsPathCountDriftAndReorderedNodeResponses() {
    final List<byte[]> commitments = List.of(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final LocalZkAssetMerklePathProvider localProvider =
        new LocalZkAssetMerklePathProvider(List.of(root), commitments);
    final ZkAssetMerklePath firstPath =
        localProvider.getMerklePathForCommitment("usd#bank", commitments.get(0)).join();
    final ZkAssetMerklePath secondPath =
        localProvider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();

    final ToriiZkAssetMerklePathProvider shortProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                List.of(new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      shortProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected short path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("Torii returned 1 Merkle paths for 2 commitments")
          : "wrong message";
    }

    final ToriiZkAssetMerklePathProvider longProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                List.of(
                    new MerklePathResponseEntry(commitments.get(0), firstPath),
                    new MerklePathResponseEntry(commitments.get(1), secondPath),
                    new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      longProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected long path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("Torii returned 3 Merkle paths for 2 commitments")
          : "wrong message";
    }

    final ToriiZkAssetMerklePathProvider reorderedProvider =
        toriiProviderWithResponse(
            merklePathResponse(
                root,
                List.of(
                    new MerklePathResponseEntry(commitments.get(1), secondPath),
                    new MerklePathResponseEntry(commitments.get(0), firstPath))));
    try {
      reorderedProvider.getMerklePaths("usd#bank", commitments).join();
      throw new AssertionError("expected reordered path response rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("commitment mismatch at index 0")
          : "wrong message";
    }
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

  private static void toriiProviderRejectsNonVerifyingNodePath() {
    final List<byte[]> commitments = List.of(scalarBytes(1), scalarBytes(2));
    final byte[] root = computeRoot(commitments);
    final ZkAssetMerklePath localPath =
        new LocalZkAssetMerklePathProvider(List.of(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments.get(1))
            .join();
    final List<byte[]> badSiblings = localPath.siblings();
    badSiblings.set(0, scalarBytes(9));
    final CapturingExecutor executor =
        new CapturingExecutor(merklePathResponse(root, commitments.get(1), localPath, badSiblings));
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    final ToriiZkAssetMerklePathProvider provider = new ToriiZkAssetMerklePathProvider(client);
    try {
      provider.getMerklePathForCommitment("usd#bank", commitments.get(1)).join();
      throw new AssertionError("expected non-verifying path rejection");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof IllegalArgumentException : "wrong error type";
      assert expected.getCause().getMessage().contains("does not verify")
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

  private static ToriiZkAssetMerklePathProvider toriiProviderWithResponse(final String responseBody) {
    final ConfidentialAssetToriiClient client =
        ConfidentialAssetToriiClient.builder()
            .executor(new CapturingExecutor(responseBody))
            .baseUri(URI.create("https://example.com"))
            .build();
    return new ToriiZkAssetMerklePathProvider(client);
  }

  private static String merklePathResponse(
      final byte[] root, final byte[] commitment, final ZkAssetMerklePath path) {
    return merklePathResponse(root, commitment, path, path.siblings());
  }

  private static String merklePathResponse(
      final byte[] root,
      final byte[] commitment,
      final ZkAssetMerklePath path,
      final List<byte[]> siblingsOverride) {
    return merklePathResponse(root, List.of(new MerklePathResponseEntry(commitment, path, siblingsOverride)));
  }

  private static String merklePathResponse(
      final byte[] root, final List<MerklePathResponseEntry> entries) {
    final int treeDepth = entries.isEmpty() ? 0 : entries.get(0).path.siblings().size();
    final ArrayList<String> paths = new ArrayList<>(entries.size());
    for (final MerklePathResponseEntry entry : entries) {
      final String siblings = quotedHexList(entry.siblings);
      final String directions = directionList(entry.path.directions());
      final String witnessNodes = quotedHexList(entry.path.siblings());
      paths.add(
          """
                  {
                    "commitment": "%s",
                    "leaf_index": %d,
                    "siblings": [%s],
                    "directions": [%s],
                    "witness_nodes": [%s],
                    "root": "%s"
                  }
              """
              .formatted(hex(entry.commitment), entry.path.leafIndex(), siblings, directions, witnessNodes, hex(root)));
    }
    return """
            {
              "root": "%s",
              "frontier_len": 2,
              "tree_depth": %d,
              "paths": [%s]
            }
        """
        .formatted(hex(root), treeDepth, String.join(",", paths));
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

  private static void assertThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected path.
    }
  }

  private static final class MerklePathResponseEntry {
    private final byte[] commitment;
    private final ZkAssetMerklePath path;
    private final List<byte[]> siblings;

    private MerklePathResponseEntry(final byte[] commitment, final ZkAssetMerklePath path) {
      this(commitment, path, path.siblings());
    }

    private MerklePathResponseEntry(
        final byte[] commitment, final ZkAssetMerklePath path, final List<byte[]> siblings) {
      this.commitment = commitment.clone();
      this.path = path;
      this.siblings = new ArrayList<>(siblings);
    }
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
