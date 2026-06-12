package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CompletableFuture;

/** Computes inclusion paths from a caller-supplied zk_assets commitment frontier. */
public final class LocalZkAssetMerklePathProvider implements ZkAssetMerklePathProvider {
  public static final int CONFIDENTIAL_TREE_DEPTH_V2 = 16;
  public static final int CONFIDENTIAL_TREE_CAPACITY_V2 = 1 << CONFIDENTIAL_TREE_DEPTH_V2;

  private final List<byte[]> roots;
  private final List<byte[]> commitments;
  private final ZkAssetMerkleHasher hasher;

  public LocalZkAssetMerklePathProvider(
      final List<byte[]> rootHistory, final List<byte[]> commitmentHistory) {
    this(rootHistory, commitmentHistory, PastaPoseidonNodeHasher.instance());
  }

  public LocalZkAssetMerklePathProvider(
      final List<byte[]> rootHistory,
      final List<byte[]> commitmentHistory,
      final ZkAssetMerkleHasher hasher) {
    this.roots = copyFixed32List(rootHistory, "rootHistory");
    this.commitments = copyFixed32List(commitmentHistory, "commitmentHistory");
    if (this.commitments.size() > CONFIDENTIAL_TREE_CAPACITY_V2) {
      throw new IllegalArgumentException("commitmentHistory exceeds confidential v2 tree capacity");
    }
    this.hasher = java.util.Objects.requireNonNull(hasher, "hasher");
  }

  @Override
  public CompletableFuture<ZkAssetMerklePath> getMerklePathForCommitment(
      final String asset, final byte[] commitment) {
    try {
      ToriiZkAssetMerklePathProvider.validateAssetAndCommitment(asset, commitment);
      int match = -1;
      int count = 0;
      for (int i = 0; i < commitments.size(); i++) {
        if (Arrays.equals(commitments.get(i), commitment)) {
          match = i;
          count++;
        }
      }
      if (count != 1) {
        throw new IllegalArgumentException(
            "commitment must appear exactly once in commitmentHistory");
      }
      return CompletableFuture.completedFuture(computePath(match));
    } catch (final RuntimeException ex) {
      return ToriiZkAssetMerklePathProvider.failedFuture(ex);
    }
  }

  @Override
  public CompletableFuture<List<ZkAssetMerklePath>> getMerklePaths(
      final String asset, final List<byte[]> requestedCommitments) {
    try {
      if (asset == null || asset.trim().isEmpty()) {
        throw new IllegalArgumentException("asset must not be blank");
      }
      final ArrayList<ZkAssetMerklePath> out =
          new ArrayList<>(requestedCommitments == null ? 0 : requestedCommitments.size());
      if (requestedCommitments != null) {
        for (final byte[] commitment : requestedCommitments) {
          out.add(getMerklePathForCommitment(asset, commitment).join());
        }
      }
      return CompletableFuture.completedFuture(out);
    } catch (final RuntimeException ex) {
      return ToriiZkAssetMerklePathProvider.failedFuture(ex);
    }
  }

  private ZkAssetMerklePath computePath(final int leafIndex) {
    ArrayList<byte[]> layer = new ArrayList<>(CONFIDENTIAL_TREE_CAPACITY_V2);
    for (final byte[] commitment : commitments) {
      layer.add(commitment.clone());
    }
    while (layer.size() < CONFIDENTIAL_TREE_CAPACITY_V2) {
      layer.add(new byte[32]);
    }
    final ArrayList<byte[]> siblings = new ArrayList<>(CONFIDENTIAL_TREE_DEPTH_V2);
    final byte[] directions = new byte[CONFIDENTIAL_TREE_DEPTH_V2];
    int currentIndex = leafIndex;
    for (int level = 0; level < CONFIDENTIAL_TREE_DEPTH_V2; level++) {
      final boolean isRight = (currentIndex & 1) == 1;
      final int siblingIndex = isRight ? currentIndex - 1 : currentIndex + 1;
      directions[level] = (byte) (isRight ? 1 : 0);
      siblings.add(layer.get(siblingIndex).clone());
      final ArrayList<byte[]> next = new ArrayList<>(layer.size() / 2);
      for (int i = 0; i < layer.size(); i += 2) {
        next.add(hasher.hashPair(layer.get(i), layer.get(i + 1)));
      }
      currentIndex /= 2;
      layer = next;
    }
    final byte[] root = layer.get(0);
    if (!roots.isEmpty() && !Arrays.equals(roots.get(roots.size() - 1), root)) {
      throw new IllegalArgumentException(
          "latest rootHistory entry does not match computed commitment frontier root");
    }
    return new ZkAssetMerklePath(
        leafIndex, siblings, directions, root, commitments.size());
  }

  private static List<byte[]> copyFixed32List(final List<byte[]> source, final String field) {
    final ArrayList<byte[]> out = new ArrayList<>(source == null ? 0 : source.size());
    if (source != null) {
      for (int i = 0; i < source.size(); i++) {
        final byte[] value = source.get(i);
        if (value == null || value.length != 32) {
          throw new IllegalArgumentException(field + "[" + i + "] must be 32 bytes");
        }
        out.add(value.clone());
      }
    }
    return java.util.Collections.unmodifiableList(out);
  }
}
