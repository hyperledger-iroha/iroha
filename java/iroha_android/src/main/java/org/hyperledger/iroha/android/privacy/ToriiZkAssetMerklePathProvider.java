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

  public ToriiZkAssetMerklePathProvider() {
    this(ConfidentialAssetToriiClient.builder().build());
  }

  public ToriiZkAssetMerklePathProvider(final ConfidentialAssetToriiClient client) {
    this.client = Objects.requireNonNull(client, "client");
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
      final ArrayList<byte[]> copied = new ArrayList<>(commitments == null ? 0 : commitments.size());
      if (commitments != null) {
        for (int i = 0; i < commitments.size(); i++) {
          final byte[] commitment = commitments.get(i);
          if (commitment == null || commitment.length != 32) {
            throw new IllegalArgumentException("commitments[" + i + "] must be 32 bytes");
          }
          copied.add(commitment.clone());
        }
      }
      if (copied.isEmpty()) {
        return CompletableFuture.completedFuture(Collections.emptyList());
      }
      return client
          .getZkAssetMerklePaths(new ZkMerklePathRequest(asset, copied))
          .thenApply(response -> toPaths(response, copied));
    } catch (final RuntimeException ex) {
      return failedFuture(ex);
    }
  }

  private static List<ZkAssetMerklePath> toPaths(
      final ZkMerklePathResponse response, final List<byte[]> requestedCommitments) {
    if (response.paths().size() != requestedCommitments.size()) {
      throw new IllegalArgumentException(
          "Torii returned "
              + response.paths().size()
              + " Merkle paths for "
              + requestedCommitments.size()
              + " commitments");
    }
    final ArrayList<ZkAssetMerklePath> out = new ArrayList<>(response.paths().size());
    for (int i = 0; i < response.paths().size(); i++) {
      final ZkMerklePathResponse.Entry entry = response.paths().get(i);
      if (!Arrays.equals(entry.commitmentBytes(), requestedCommitments.get(i))) {
        throw new IllegalArgumentException("Torii Merkle path commitment mismatch at index " + i);
      }
      if (entry.siblings().size() != response.treeDepth()) {
        throw new IllegalArgumentException("Torii Merkle path sibling depth mismatch at index " + i);
      }
      out.add(
          new ZkAssetMerklePath(
              entry.leafIndex(),
              entry.siblingBytes(),
              entry.directions(),
              response.rootBytes(),
              response.frontierLen()));
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
