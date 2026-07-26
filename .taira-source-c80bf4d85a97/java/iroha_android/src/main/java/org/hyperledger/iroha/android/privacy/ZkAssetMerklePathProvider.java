package org.hyperledger.iroha.android.privacy;

import java.util.List;
import java.util.concurrent.CompletableFuture;

/** Source of zk_assets Merkle inclusion paths. */
public interface ZkAssetMerklePathProvider {
  CompletableFuture<ZkAssetMerklePath> getMerklePathForCommitment(
      String asset, byte[] commitment);

  CompletableFuture<List<ZkAssetMerklePath>> getMerklePaths(
      String asset, List<byte[]> commitments);
}
