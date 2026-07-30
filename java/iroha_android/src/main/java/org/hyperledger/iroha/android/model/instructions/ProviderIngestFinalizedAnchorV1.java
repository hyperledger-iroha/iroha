package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Exact finalized committed-chain prefix used to prepare a provider completion. */
public final class ProviderIngestFinalizedAnchorV1 {

  private final long height;
  private final String blockHash;

  /**
   * Creates a finalized anchor.
   *
   * @param height one-based committed block height
   * @param blockHash exact non-zero block hash
   */
  public ProviderIngestFinalizedAnchorV1(final long height, final String blockHash) {
    this.height =
        ReplicationOrderInstructionValidation.requirePositiveRevision(height, "height");
    this.blockHash =
        ReplicationOrderInstructionValidation.requireDigest(blockHash, "blockHash");
  }

  public long height() {
    return height;
  }

  public String blockHash() {
    return blockHash;
  }

  String canonicalJson() {
    return "{\"height\":" + height + ",\"block_hash\":\"" + blockHash + "\"}";
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProviderIngestFinalizedAnchorV1)) {
      return false;
    }
    final ProviderIngestFinalizedAnchorV1 other =
        (ProviderIngestFinalizedAnchorV1) obj;
    return height == other.height && Objects.equals(blockHash, other.blockHash);
  }

  @Override
  public int hashCode() {
    return Objects.hash(height, blockHash);
  }
}
