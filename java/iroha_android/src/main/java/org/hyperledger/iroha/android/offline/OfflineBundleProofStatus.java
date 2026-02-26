package org.hyperledger.iroha.android.offline;

import java.util.List;
import java.util.Objects;

/** Parsed response from `/v1/offline/bundle/proof_status`. */
public final class OfflineBundleProofStatus {
  private final String bundleIdHex;
  private final String receiptsRootHex;
  private final String aggregateProofRootHex;
  private final Boolean receiptsRootMatches;
  private final String proofStatus;
  private final ProofSummary proofSummary;

  public OfflineBundleProofStatus(
      final String bundleIdHex,
      final String receiptsRootHex,
      final String aggregateProofRootHex,
      final Boolean receiptsRootMatches,
      final String proofStatus,
      final ProofSummary proofSummary) {
    this.bundleIdHex = Objects.requireNonNull(bundleIdHex, "bundleIdHex");
    this.receiptsRootHex = Objects.requireNonNull(receiptsRootHex, "receiptsRootHex");
    this.aggregateProofRootHex = aggregateProofRootHex;
    this.receiptsRootMatches = receiptsRootMatches;
    this.proofStatus = Objects.requireNonNull(proofStatus, "proofStatus");
    this.proofSummary = proofSummary;
  }

  public String bundleIdHex() {
    return bundleIdHex;
  }

  public String receiptsRootHex() {
    return receiptsRootHex;
  }

  public String aggregateProofRootHex() {
    return aggregateProofRootHex;
  }

  public Boolean receiptsRootMatches() {
    return receiptsRootMatches;
  }

  public String proofStatus() {
    return proofStatus;
  }

  public ProofSummary proofSummary() {
    return proofSummary;
  }

  /** Aggregate proof metadata summary. */
  public static final class ProofSummary {
    private final int version;
    private final Long proofSumBytes;
    private final Long proofCounterBytes;
    private final Long proofReplayBytes;
    private final List<String> metadataKeys;

    public ProofSummary(
        final int version,
        final Long proofSumBytes,
        final Long proofCounterBytes,
        final Long proofReplayBytes,
        final List<String> metadataKeys) {
      this.version = version;
      this.proofSumBytes = proofSumBytes;
      this.proofCounterBytes = proofCounterBytes;
      this.proofReplayBytes = proofReplayBytes;
      this.metadataKeys = List.copyOf(Objects.requireNonNull(metadataKeys, "metadataKeys"));
    }

    public int version() {
      return version;
    }

    public Long proofSumBytes() {
      return proofSumBytes;
    }

    public Long proofCounterBytes() {
      return proofCounterBytes;
    }

    public Long proofReplayBytes() {
      return proofReplayBytes;
    }

    public List<String> metadataKeys() {
      return metadataKeys;
    }
  }
}
