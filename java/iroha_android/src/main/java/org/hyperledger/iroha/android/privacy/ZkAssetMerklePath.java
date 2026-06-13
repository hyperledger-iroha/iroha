package org.hyperledger.iroha.android.privacy;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/** Direction bytes and sibling hashes for one zk_assets inclusion path. */
public final class ZkAssetMerklePath {
  private final long leafIndex;
  private final List<byte[]> siblings;
  private final byte[] directions;
  private final byte[] rootAtHeight;
  private final long heightOrIndex;

  public ZkAssetMerklePath(
      final long leafIndex,
      final List<byte[]> siblings,
      final byte[] directions,
      final byte[] rootAtHeight,
      final long heightOrIndex) {
    if (leafIndex < 0) {
      throw new IllegalArgumentException("leafIndex must be non-negative");
    }
    if (heightOrIndex < 0) {
      throw new IllegalArgumentException("heightOrIndex must be non-negative");
    }
    if (rootAtHeight == null || rootAtHeight.length != 32) {
      throw new IllegalArgumentException("rootAtHeight must be 32 bytes");
    }
    final List<byte[]> copied = new ArrayList<>(siblings == null ? 0 : siblings.size());
    if (siblings != null) {
      for (int i = 0; i < siblings.size(); i++) {
        final byte[] sibling = siblings.get(i);
        if (sibling == null || sibling.length != 32) {
          throw new IllegalArgumentException("siblings[" + i + "] must be 32 bytes");
        }
        copied.add(sibling.clone());
      }
    }
    final byte[] dirs = directions == null ? new byte[0] : directions.clone();
    if (dirs.length != copied.size()) {
      throw new IllegalArgumentException("directions size must match siblings size");
    }
    if (dirs.length >= Long.SIZE) {
      throw new IllegalArgumentException("path depth must fit in leafIndex bits");
    }
    if ((leafIndex >>> dirs.length) != 0L) {
      throw new IllegalArgumentException("leafIndex must fit within path depth");
    }
    for (int i = 0; i < dirs.length; i++) {
      if (dirs[i] != 0 && dirs[i] != 1) {
        throw new IllegalArgumentException("directions[" + i + "] must be 0 or 1");
      }
      final int expectedDirection = (int) ((leafIndex >>> i) & 1L);
      if (dirs[i] != expectedDirection) {
        throw new IllegalArgumentException("directions[" + i + "] must match leafIndex bit " + i);
      }
    }
    this.leafIndex = leafIndex;
    this.siblings = Collections.unmodifiableList(copied);
    this.directions = dirs;
    this.rootAtHeight = rootAtHeight.clone();
    this.heightOrIndex = heightOrIndex;
  }

  public long leafIndex() {
    return leafIndex;
  }

  public List<byte[]> siblings() {
    final ArrayList<byte[]> out = new ArrayList<>(siblings.size());
    for (final byte[] sibling : siblings) {
      out.add(sibling.clone());
    }
    return out;
  }

  public byte[] directions() {
    return directions.clone();
  }

  public byte[] rootAtHeight() {
    return rootAtHeight.clone();
  }

  public long heightOrIndex() {
    return heightOrIndex;
  }

  public boolean verify(
      final byte[] commitment, final byte[] expectedRoot, final ZkAssetMerkleHasher hasher) {
    if (commitment == null
        || commitment.length != 32
        || expectedRoot == null
        || expectedRoot.length != 32
        || !Arrays.equals(rootAtHeight, expectedRoot)) {
      return false;
    }
    byte[] current = commitment.clone();
    for (int i = 0; i < siblings.size(); i++) {
      current =
          directions[i] == 0
              ? hasher.hashPair(current, siblings.get(i))
              : hasher.hashPair(siblings.get(i), current);
    }
    return Arrays.equals(current, expectedRoot);
  }
}
