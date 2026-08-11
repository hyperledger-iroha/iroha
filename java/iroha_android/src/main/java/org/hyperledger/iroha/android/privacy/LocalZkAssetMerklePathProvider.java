package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;

/** Computes inclusion paths from a caller-supplied zk_assets commitment frontier. */
public final class LocalZkAssetMerklePathProvider implements ZkAssetMerklePathProvider {
  public static final int CONFIDENTIAL_TREE_DEPTH_V2 = 16;
  public static final int CONFIDENTIAL_TREE_CAPACITY_V2 = 1 << CONFIDENTIAL_TREE_DEPTH_V2;

  private final List<byte[]> roots;
  private final List<byte[]> commitments;

  public LocalZkAssetMerklePathProvider(
      final List<byte[]> rootHistory, final List<byte[]> commitmentHistory) {
    this.roots = copyFixed32List(rootHistory, "rootHistory");
    this.commitments = copyFixed32List(commitmentHistory, "commitmentHistory");
    if (this.commitments.size() > CONFIDENTIAL_TREE_CAPACITY_V2) {
      throw new IllegalArgumentException("commitmentHistory exceeds confidential v2 tree capacity");
    }
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
      final List<byte[]> checkedCommitments =
          Objects.requireNonNull(requestedCommitments, "requestedCommitments");
      final ArrayList<ZkAssetMerklePath> out =
          new ArrayList<>(checkedCommitments.size());
      for (final byte[] commitment : checkedCommitments) {
        out.add(getMerklePathForCommitment(asset, commitment).join());
      }
      return CompletableFuture.completedFuture(out);
    } catch (final RuntimeException ex) {
      return ToriiZkAssetMerklePathProvider.failedFuture(ex);
    }
  }

  private ZkAssetMerklePath computePath(final int leafIndex) {
    final byte[] encoded =
        PrivacyNativeBridge.deriveConfidentialMerklePathV3(commitments, leafIndex);
    final byte[] root = Arrays.copyOfRange(encoded, 0, 32);
    final ArrayList<byte[]> siblings = new ArrayList<>(CONFIDENTIAL_TREE_DEPTH_V2);
    for (int level = 0; level < CONFIDENTIAL_TREE_DEPTH_V2; level++) {
      final int offset = 32 + level * 32;
      siblings.add(Arrays.copyOfRange(encoded, offset, offset + 32));
    }
    final byte[] directions =
        Arrays.copyOfRange(
            encoded, 32 + CONFIDENTIAL_TREE_DEPTH_V2 * 32, encoded.length);
    if (!roots.isEmpty() && !Arrays.equals(roots.get(roots.size() - 1), root)) {
      throw new IllegalArgumentException(
          "latest rootHistory entry does not match computed commitment frontier root");
    }
    return new ZkAssetMerklePath(
        leafIndex, siblings, directions, root, commitments.size());
  }

  private static List<byte[]> copyFixed32List(final List<byte[]> source, final String field) {
    final List<byte[]> checkedSource = Objects.requireNonNull(source, field);
    final ArrayList<byte[]> out = new ArrayList<>(checkedSource.size());
    for (int i = 0; i < checkedSource.size(); i++) {
      final byte[] value = checkedSource.get(i);
      if (value == null || value.length != 32) {
        throw new IllegalArgumentException(field + "[" + i + "] must be 32 bytes");
      }
      out.add(value.clone());
    }
    return java.util.Collections.unmodifiableList(out);
  }
}
