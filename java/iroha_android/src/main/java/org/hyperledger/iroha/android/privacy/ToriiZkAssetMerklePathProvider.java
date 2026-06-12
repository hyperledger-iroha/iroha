package org.hyperledger.iroha.android.privacy;

import java.util.Collections;
import java.util.List;
import java.util.concurrent.CompletableFuture;

/** Fails closed until Torii exposes a commitment-inclusion endpoint. */
public final class ToriiZkAssetMerklePathProvider implements ZkAssetMerklePathProvider {
  @Override
  public CompletableFuture<ZkAssetMerklePath> getMerklePathForCommitment(
      final String asset, final byte[] commitment) {
    validateAssetAndCommitment(asset, commitment);
    return failedFuture(unsupported());
  }

  @Override
  public CompletableFuture<List<ZkAssetMerklePath>> getMerklePaths(
      final String asset, final List<byte[]> commitments) {
    if (asset == null || asset.trim().isEmpty()) {
      throw new IllegalArgumentException("asset must not be blank");
    }
    if (commitments == null || commitments.isEmpty()) {
      return CompletableFuture.completedFuture(Collections.emptyList());
    }
    for (int i = 0; i < commitments.size(); i++) {
      final byte[] commitment = commitments.get(i);
      if (commitment == null || commitment.length != 32) {
        throw new IllegalArgumentException("commitments[" + i + "] must be 32 bytes");
      }
    }
    return failedFuture(unsupported());
  }

  private static UnsupportedOperationException unsupported() {
    return new UnsupportedOperationException(
        "Torii does not expose a zk_assets Merkle-path endpoint yet; use LocalZkAssetMerklePathProvider with audited frontier material");
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
