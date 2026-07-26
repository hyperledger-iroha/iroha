package org.hyperledger.iroha.android.privacy;

/** Pair-compression function used by the zk_assets commitment tree. */
@FunctionalInterface
public interface ZkAssetMerkleHasher {
  byte[] hashPair(byte[] left, byte[] right);
}
