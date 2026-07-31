package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Complete Merkle witness used by a Nexus lane privacy proof. */
public final class LanePrivacyMerkleWitness {
  public static final int MAX_DEPTH = 255;

  private final byte[] leaf;
  private final long leafIndex;
  private final List<byte[]> auditPath;

  public LanePrivacyMerkleWitness(
      final byte[] leaf, final long leafIndex, final List<byte[]> auditPath) {
    this.leaf = ZkInstructionUtils.fixedBytes(leaf, 32, "leaf");
    if (leafIndex < 0L || leafIndex > 0xffff_ffffL) {
      throw new IllegalArgumentException("leafIndex must fit in u32");
    }
    if (auditPath == null) {
      throw new IllegalArgumentException("auditPath must be provided");
    }
    if (auditPath.isEmpty() || auditPath.size() > MAX_DEPTH) {
      throw new IllegalArgumentException(
          "auditPath must contain between 1 and " + MAX_DEPTH + " siblings");
    }
    if (auditPath.size() < 32 && leafIndex >= (1L << auditPath.size())) {
      throw new IllegalArgumentException("leafIndex cannot fit the supplied Merkle path depth");
    }
    this.leafIndex = leafIndex;
    final List<byte[]> canonicalPath = new ArrayList<>(auditPath.size());
    for (int index = 0; index < auditPath.size(); index++) {
      final byte[] canonical =
          ZkInstructionUtils.fixedBytes(auditPath.get(index), 32, "auditPath[" + index + "]");
      canonical[canonical.length - 1] = (byte) (canonical[canonical.length - 1] | 1);
      canonicalPath.add(canonical);
    }
    this.auditPath = Collections.unmodifiableList(canonicalPath);
  }

  public byte[] leaf() {
    return leaf.clone();
  }

  public long leafIndex() {
    return leafIndex;
  }

  public List<byte[]> auditPath() {
    final List<byte[]> copy = new ArrayList<>(auditPath.size());
    for (final byte[] sibling : auditPath) {
      copy.add(sibling.clone());
    }
    return Collections.unmodifiableList(copy);
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof LanePrivacyMerkleWitness other)
        || leafIndex != other.leafIndex
        || !java.util.Arrays.equals(leaf, other.leaf)
        || auditPath.size() != other.auditPath.size()) {
      return false;
    }
    for (int index = 0; index < auditPath.size(); index++) {
      if (!java.util.Arrays.equals(auditPath.get(index), other.auditPath.get(index))) {
        return false;
      }
    }
    return true;
  }

  @Override
  public int hashCode() {
    int result = java.util.Arrays.hashCode(leaf);
    result = 31 * result + Long.hashCode(leafIndex);
    for (final byte[] sibling : auditPath) {
      result = 31 * result + java.util.Arrays.hashCode(sibling);
    }
    return result;
  }
}
