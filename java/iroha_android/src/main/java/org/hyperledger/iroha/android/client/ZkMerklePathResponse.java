package org.hyperledger.iroha.android.client;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Response body emitted by {@code POST /v1/zk/merkle-path}. */
public final class ZkMerklePathResponse {
  private final String root;
  private final int frontierLen;
  private final int treeDepth;
  private final List<Entry> paths;

  public ZkMerklePathResponse(
      final String root, final int frontierLen, final int treeDepth, final List<Entry> paths) {
    this.root = ZkRootsResponse.normalizeRootHex(root, "root");
    if (frontierLen < 0) {
      throw new IllegalArgumentException("frontier_len must be non-negative");
    }
    if (treeDepth < 0) {
      throw new IllegalArgumentException("tree_depth must be non-negative");
    }
    final List<Entry> checkedPaths = Objects.requireNonNull(paths, "paths");
    final ArrayList<Entry> copied = new ArrayList<>(checkedPaths.size());
    for (int i = 0; i < checkedPaths.size(); i++) {
      final Entry entry = Objects.requireNonNull(checkedPaths.get(i), "paths[" + i + "]");
      if (!entry.root().equals(this.root)) {
        throw new IllegalArgumentException("paths[" + i + "].root must match response root");
      }
      if (entry.leafIndex() >= frontierLen) {
        throw new IllegalArgumentException("paths[" + i + "].leaf_index must be below frontier_len");
      }
      if (entry.siblings().size() != treeDepth) {
        throw new IllegalArgumentException("paths[" + i + "].siblings size must match tree_depth");
      }
      copied.add(entry);
    }
    this.frontierLen = frontierLen;
    this.treeDepth = treeDepth;
    this.paths = Collections.unmodifiableList(copied);
  }

  public String root() {
    return root;
  }

  public int frontierLen() {
    return frontierLen;
  }

  public int treeDepth() {
    return treeDepth;
  }

  public List<Entry> paths() {
    return paths;
  }

  public byte[] rootBytes() {
    return ZkRootsResponse.decodeHex32(root, "root");
  }

  static ZkMerklePathResponse parse(final byte[] payload) {
    return ZkMerklePathJson.parseResponse(payload);
  }

  /** One path entry returned by {@code POST /v1/zk/merkle-path}. */
  public static final class Entry {
    private final String commitment;
    private final int leafIndex;
    private final List<String> siblings;
    private final byte[] directions;
    private final List<String> witnessNodes;
    private final String root;

    public Entry(
        final String commitment,
        final int leafIndex,
        final List<String> siblings,
        final byte[] directions,
        final List<String> witnessNodes,
        final String root) {
      this.commitment = ZkRootsResponse.normalizeRootHex(commitment, "commitment");
      if (leafIndex < 0) {
        throw new IllegalArgumentException("leaf_index must be non-negative");
      }
      this.leafIndex = leafIndex;
      this.siblings = normalizeHexList(siblings, "siblings");
      this.directions = Objects.requireNonNull(directions, "directions").clone();
      this.witnessNodes = normalizeHexList(witnessNodes, "witness_nodes");
      this.root = ZkRootsResponse.normalizeRootHex(root, "root");
      if (this.directions.length != this.siblings.size()) {
        throw new IllegalArgumentException("directions size must match siblings size");
      }
      if (this.witnessNodes.size() != this.siblings.size()) {
        throw new IllegalArgumentException("witness_nodes size must match siblings size");
      }
      if (this.directions.length >= Integer.SIZE) {
        throw new IllegalArgumentException("path depth must fit in leaf_index bits");
      }
      if ((leafIndex >>> this.directions.length) != 0) {
        throw new IllegalArgumentException("leaf_index must fit within path depth");
      }
      for (int i = 0; i < this.directions.length; i++) {
        if (this.directions[i] != 0 && this.directions[i] != 1) {
          throw new IllegalArgumentException("directions[" + i + "] must be 0 or 1");
        }
        final int expectedDirection = (leafIndex >>> i) & 1;
        if (this.directions[i] != expectedDirection) {
          throw new IllegalArgumentException("directions[" + i + "] must match leaf_index bit " + i);
        }
      }
    }

    public String commitment() {
      return commitment;
    }

    public int leafIndex() {
      return leafIndex;
    }

    public List<String> siblings() {
      return siblings;
    }

    public byte[] directions() {
      return directions.clone();
    }

    public List<String> witnessNodes() {
      return witnessNodes;
    }

    public String root() {
      return root;
    }

    public byte[] commitmentBytes() {
      return ZkRootsResponse.decodeHex32(commitment, "commitment");
    }

    public List<byte[]> siblingBytes() {
      final ArrayList<byte[]> out = new ArrayList<>(siblings.size());
      for (int i = 0; i < siblings.size(); i++) {
        out.add(ZkRootsResponse.decodeHex32(siblings.get(i), "siblings[" + i + "]"));
      }
      return out;
    }

    public byte[] rootBytes() {
      return ZkRootsResponse.decodeHex32(root, "root");
    }

    private static List<String> normalizeHexList(final List<String> values, final String field) {
      final List<String> checkedValues = Objects.requireNonNull(values, field);
      final ArrayList<String> out = new ArrayList<>(checkedValues.size());
      for (int i = 0; i < checkedValues.size(); i++) {
        out.add(ZkRootsResponse.normalizeRootHex(checkedValues.get(i), field + "[" + i + "]"));
      }
      return Collections.unmodifiableList(out);
    }
  }
}
