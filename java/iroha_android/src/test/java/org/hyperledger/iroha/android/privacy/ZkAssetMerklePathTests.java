package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CompletionException;

public final class ZkAssetMerklePathTests {
  private ZkAssetMerklePathTests() {}

  public static void main(final String[] args) {
    localProviderComputesAndVerifiesCurrentFrontierPath();
    localProviderRejectsAmbiguousOrMismatchedFrontiers();
    toriiProviderFailsClosedUntilNodeEndpointExists();
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

  private static void toriiProviderFailsClosedUntilNodeEndpointExists() {
    final ToriiZkAssetMerklePathProvider provider = new ToriiZkAssetMerklePathProvider();
    try {
      provider.getMerklePathForCommitment("usd#bank", scalarBytes(1)).join();
      throw new AssertionError("expected unsupported provider failure");
    } catch (final CompletionException expected) {
      assert expected.getCause() instanceof UnsupportedOperationException : "wrong error type";
      assert expected.getCause().getMessage().contains("does not expose") : "wrong message";
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
