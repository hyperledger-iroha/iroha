package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.ConfidentialAssetToriiClient;
import org.hyperledger.iroha.android.client.ZkMerklePathRequest;
import org.hyperledger.iroha.android.client.ZkMerklePathResponse;

/** Fetches current confidential-v2 commitment inclusion paths from Torii. */
public final class ToriiZkAssetMerklePathProvider implements ZkAssetMerklePathProvider {
  private final ConfidentialAssetToriiClient client;
  private final ZkAssetMerkleHasher hasher;

  public ToriiZkAssetMerklePathProvider() {
    this(ConfidentialAssetToriiClient.builder().build());
  }

  public ToriiZkAssetMerklePathProvider(final ConfidentialAssetToriiClient client) {
    this(client, PastaPoseidonNodeHasher.instance());
  }

  public ToriiZkAssetMerklePathProvider(
      final ConfidentialAssetToriiClient client, final ZkAssetMerkleHasher hasher) {
    this.client = Objects.requireNonNull(client, "client");
    this.hasher = Objects.requireNonNull(hasher, "hasher");
  }

  @Override
  public CompletableFuture<ZkAssetMerklePath> getMerklePathForCommitment(
      final String asset, final byte[] commitment) {
    return getMerklePaths(asset, Collections.singletonList(commitment))
        .thenApply(paths -> paths.get(0));
  }

  @Override
  public CompletableFuture<List<ZkAssetMerklePath>> getMerklePaths(
      final String asset, final List<byte[]> commitments) {
    try {
      if (asset == null || asset.trim().isEmpty()) {
        throw new IllegalArgumentException("asset must not be blank");
      }
      final List<byte[]> checkedCommitments = Objects.requireNonNull(commitments, "commitments");
      final ArrayList<byte[]> copied = new ArrayList<>(checkedCommitments.size());
      for (int i = 0; i < checkedCommitments.size(); i++) {
        final byte[] commitment = checkedCommitments.get(i);
        if (commitment == null || commitment.length != 32) {
          throw new IllegalArgumentException("commitments[" + i + "] must be 32 bytes");
        }
        copied.add(commitment.clone());
      }
      if (copied.isEmpty()) {
        return CompletableFuture.completedFuture(Collections.emptyList());
      }
      return client
          .getZkAssetMerklePaths(new ZkMerklePathRequest(asset, copied))
          .thenApply(response -> toPaths(response, copied, hasher));
    } catch (final RuntimeException ex) {
      return failedFuture(ex);
    }
  }

  private static List<ZkAssetMerklePath> toPaths(
      final ZkMerklePathResponse response,
      final List<byte[]> requestedCommitments,
      final ZkAssetMerkleHasher hasher) {
    if (response.paths().size() != requestedCommitments.size()) {
      throw new IllegalArgumentException(
          "Torii returned "
              + response.paths().size()
              + " Merkle paths for "
              + requestedCommitments.size()
              + " commitments");
    }
    final byte[] root = response.rootBytes();
    final ArrayList<ZkAssetMerklePath> out = new ArrayList<>(response.paths().size());
    for (int i = 0; i < response.paths().size(); i++) {
      final ZkMerklePathResponse.Entry entry = response.paths().get(i);
      if (!Arrays.equals(entry.commitmentBytes(), requestedCommitments.get(i))) {
        throw new IllegalArgumentException("Torii Merkle path commitment mismatch at index " + i);
      }
      if (entry.siblings().size() != response.treeDepth()) {
        throw new IllegalArgumentException("Torii Merkle path sibling depth mismatch at index " + i);
      }
      final ZkAssetMerklePath path =
          new ZkAssetMerklePath(
              entry.leafIndex(),
              entry.siblingBytes(),
              entry.directions(),
              root,
              response.frontierLen());
      if (!path.verify(requestedCommitments.get(i), root, hasher)) {
        throw new IllegalArgumentException("Torii Merkle path does not verify at index " + i);
      }
      out.add(path);
    }
    return Collections.unmodifiableList(out);
  }

  static void validateAssetAndCommitment(final String asset, final byte[] commitment) {
    if (asset == null || asset.trim().isEmpty()) {
      throw new IllegalArgumentException("asset must not be blank");
    }
    if (commitment == null || commitment.length != 32) {
      throw new IllegalArgumentException("commitment must be 32 bytes");
    }
  }

  static <T> CompletableFuture<T> failedFuture(final Throwable error) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    future.completeExceptionally(error);
    return future;
  }
}
