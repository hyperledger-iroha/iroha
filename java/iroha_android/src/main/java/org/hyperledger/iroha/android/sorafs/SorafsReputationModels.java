package org.hyperledger.iroha.android.sorafs;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Closed typed response models for committed SoraFS reputation V1 projections. */
public final class SorafsReputationModels {

  private SorafsReputationModels() {}

  /** Canonical V1 reputation scoring weights. */
  public static final class WeightsV1 {
    private final int version;
    private final int porSuccessBps;
    private final int pdpSuccessBps;
    private final int potrSuccessBps;
    private final int latencyBps;
    private final int disputeBps;
    private final int tokenViolationBps;
    private final int repairBreachBps;

    public WeightsV1(
        final int version,
        final int porSuccessBps,
        final int pdpSuccessBps,
        final int potrSuccessBps,
        final int latencyBps,
        final int disputeBps,
        final int tokenViolationBps,
        final int repairBreachBps) {
      this.version = version;
      this.porSuccessBps = porSuccessBps;
      this.pdpSuccessBps = pdpSuccessBps;
      this.potrSuccessBps = potrSuccessBps;
      this.latencyBps = latencyBps;
      this.disputeBps = disputeBps;
      this.tokenViolationBps = tokenViolationBps;
      this.repairBreachBps = repairBreachBps;
    }

    public int version() {
      return version;
    }

    public int porSuccessBps() {
      return porSuccessBps;
    }

    public int pdpSuccessBps() {
      return pdpSuccessBps;
    }

    public int potrSuccessBps() {
      return potrSuccessBps;
    }

    public int latencyBps() {
      return latencyBps;
    }

    public int disputeBps() {
      return disputeBps;
    }

    public int tokenViolationBps() {
      return tokenViolationBps;
    }

    public int repairBreachBps() {
      return repairBreachBps;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof WeightsV1)) {
        return false;
      }
      final WeightsV1 rhs = (WeightsV1) other;
      return version == rhs.version
          && porSuccessBps == rhs.porSuccessBps
          && pdpSuccessBps == rhs.pdpSuccessBps
          && potrSuccessBps == rhs.potrSuccessBps
          && latencyBps == rhs.latencyBps
          && disputeBps == rhs.disputeBps
          && tokenViolationBps == rhs.tokenViolationBps
          && repairBreachBps == rhs.repairBreachBps;
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          version,
          porSuccessBps,
          pdpSuccessBps,
          potrSuccessBps,
          latencyBps,
          disputeBps,
          tokenViolationBps,
          repairBreachBps);
    }
  }

  /** Canonical V1 metrics committed for one provider. */
  public static final class ProviderMetricsV1 {
    private final int version;
    private final int porSuccessBps;
    private final int pdpSuccessBps;
    private final int potrSuccessBps;
    private final int latencyHealthBps;
    private final int disputeRateBps;
    private final int tokenViolationRateBps;
    private final int repairBreachRateBps;

    public ProviderMetricsV1(
        final int version,
        final int porSuccessBps,
        final int pdpSuccessBps,
        final int potrSuccessBps,
        final int latencyHealthBps,
        final int disputeRateBps,
        final int tokenViolationRateBps,
        final int repairBreachRateBps) {
      this.version = version;
      this.porSuccessBps = porSuccessBps;
      this.pdpSuccessBps = pdpSuccessBps;
      this.potrSuccessBps = potrSuccessBps;
      this.latencyHealthBps = latencyHealthBps;
      this.disputeRateBps = disputeRateBps;
      this.tokenViolationRateBps = tokenViolationRateBps;
      this.repairBreachRateBps = repairBreachRateBps;
    }

    public int version() {
      return version;
    }

    public int porSuccessBps() {
      return porSuccessBps;
    }

    public int pdpSuccessBps() {
      return pdpSuccessBps;
    }

    public int potrSuccessBps() {
      return potrSuccessBps;
    }

    public int latencyHealthBps() {
      return latencyHealthBps;
    }

    public int disputeRateBps() {
      return disputeRateBps;
    }

    public int tokenViolationRateBps() {
      return tokenViolationRateBps;
    }

    public int repairBreachRateBps() {
      return repairBreachRateBps;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof ProviderMetricsV1)) {
        return false;
      }
      final ProviderMetricsV1 rhs = (ProviderMetricsV1) other;
      return version == rhs.version
          && porSuccessBps == rhs.porSuccessBps
          && pdpSuccessBps == rhs.pdpSuccessBps
          && potrSuccessBps == rhs.potrSuccessBps
          && latencyHealthBps == rhs.latencyHealthBps
          && disputeRateBps == rhs.disputeRateBps
          && tokenViolationRateBps == rhs.tokenViolationRateBps
          && repairBreachRateBps == rhs.repairBreachRateBps;
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          version,
          porSuccessBps,
          pdpSuccessBps,
          potrSuccessBps,
          latencyHealthBps,
          disputeRateBps,
          tokenViolationRateBps,
          repairBreachRateBps);
    }
  }

  /** One provider entry in an immutable reputation snapshot. */
  public static final class ProviderV1 {
    private final String providerId;
    private final int scoreBps;
    private final List<String> degradationFlags;
    private final ProviderMetricsV1 rawMetrics;
    private final String rawMetricsHashHex;

    public ProviderV1(
        final String providerId,
        final int scoreBps,
        final List<String> degradationFlags,
        final ProviderMetricsV1 rawMetrics,
        final String rawMetricsHashHex) {
      this.providerId = Objects.requireNonNull(providerId, "providerId");
      this.scoreBps = scoreBps;
      this.degradationFlags =
          Collections.unmodifiableList(new ArrayList<>(degradationFlags));
      this.rawMetrics = Objects.requireNonNull(rawMetrics, "rawMetrics");
      this.rawMetricsHashHex =
          Objects.requireNonNull(rawMetricsHashHex, "rawMetricsHashHex");
    }

    public String providerId() {
      return providerId;
    }

    public int scoreBps() {
      return scoreBps;
    }

    public List<String> degradationFlags() {
      return degradationFlags;
    }

    public ProviderMetricsV1 rawMetrics() {
      return rawMetrics;
    }

    public String rawMetricsHashHex() {
      return rawMetricsHashHex;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof ProviderV1)) {
        return false;
      }
      final ProviderV1 rhs = (ProviderV1) other;
      return scoreBps == rhs.scoreBps
          && providerId.equals(rhs.providerId)
          && degradationFlags.equals(rhs.degradationFlags)
          && rawMetrics.equals(rhs.rawMetrics)
          && rawMetricsHashHex.equals(rhs.rawMetricsHashHex);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          providerId, scoreBps, degradationFlags, rawMetrics, rawMetricsHashHex);
    }
  }

  /** Bounded view of one immutable committed reputation snapshot. */
  public static final class SnapshotSummaryV1 {
    private final String snapshotIdHex;
    private final String generatedAtUnix;
    private final String previousSnapshotIdHex;
    private final String merkleRootHex;
    private final int providerCount;
    private final int returnedProviderCount;
    private final int limit;
    private final boolean truncatedProviders;
    private final int alphaBps;
    private final int currentScoreWeightBps;
    private final WeightsV1 weights;
    private final List<ProviderV1> providers;

    public SnapshotSummaryV1(
        final String snapshotIdHex,
        final String generatedAtUnix,
        final String previousSnapshotIdHex,
        final String merkleRootHex,
        final int providerCount,
        final int returnedProviderCount,
        final int limit,
        final boolean truncatedProviders,
        final int alphaBps,
        final int currentScoreWeightBps,
        final WeightsV1 weights,
        final List<ProviderV1> providers) {
      this.snapshotIdHex = Objects.requireNonNull(snapshotIdHex, "snapshotIdHex");
      this.generatedAtUnix = Objects.requireNonNull(generatedAtUnix, "generatedAtUnix");
      this.previousSnapshotIdHex = previousSnapshotIdHex;
      this.merkleRootHex = Objects.requireNonNull(merkleRootHex, "merkleRootHex");
      this.providerCount = providerCount;
      this.returnedProviderCount = returnedProviderCount;
      this.limit = limit;
      this.truncatedProviders = truncatedProviders;
      this.alphaBps = alphaBps;
      this.currentScoreWeightBps = currentScoreWeightBps;
      this.weights = Objects.requireNonNull(weights, "weights");
      this.providers = Collections.unmodifiableList(new ArrayList<>(providers));
    }

    public String snapshotIdHex() {
      return snapshotIdHex;
    }

    public String generatedAtUnix() {
      return generatedAtUnix;
    }

    public String previousSnapshotIdHex() {
      return previousSnapshotIdHex;
    }

    public String merkleRootHex() {
      return merkleRootHex;
    }

    public int providerCount() {
      return providerCount;
    }

    public int returnedProviderCount() {
      return returnedProviderCount;
    }

    public int limit() {
      return limit;
    }

    public boolean truncatedProviders() {
      return truncatedProviders;
    }

    public int alphaBps() {
      return alphaBps;
    }

    public int currentScoreWeightBps() {
      return currentScoreWeightBps;
    }

    public WeightsV1 weights() {
      return weights;
    }

    public List<ProviderV1> providers() {
      return providers;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof SnapshotSummaryV1)) {
        return false;
      }
      final SnapshotSummaryV1 rhs = (SnapshotSummaryV1) other;
      return providerCount == rhs.providerCount
          && returnedProviderCount == rhs.returnedProviderCount
          && limit == rhs.limit
          && truncatedProviders == rhs.truncatedProviders
          && alphaBps == rhs.alphaBps
          && currentScoreWeightBps == rhs.currentScoreWeightBps
          && snapshotIdHex.equals(rhs.snapshotIdHex)
          && generatedAtUnix.equals(rhs.generatedAtUnix)
          && Objects.equals(previousSnapshotIdHex, rhs.previousSnapshotIdHex)
          && merkleRootHex.equals(rhs.merkleRootHex)
          && weights.equals(rhs.weights)
          && providers.equals(rhs.providers);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          snapshotIdHex,
          generatedAtUnix,
          previousSnapshotIdHex,
          merkleRootHex,
          providerCount,
          returnedProviderCount,
          limit,
          truncatedProviders,
          alphaBps,
          currentScoreWeightBps,
          weights,
          providers);
    }
  }

  /** Complete V1 Merkle inclusion proof for one provider. */
  public static final class MerkleProofV1 {
    private final String providerId;
    private final int leafIndex;
    private final int leafCount;
    private final List<String> siblingsHex;

    public MerkleProofV1(
        final String providerId,
        final int leafIndex,
        final int leafCount,
        final List<String> siblingsHex) {
      this.providerId = Objects.requireNonNull(providerId, "providerId");
      this.leafIndex = leafIndex;
      this.leafCount = leafCount;
      this.siblingsHex = Collections.unmodifiableList(new ArrayList<>(siblingsHex));
    }

    public String providerId() {
      return providerId;
    }

    public int leafIndex() {
      return leafIndex;
    }

    public int leafCount() {
      return leafCount;
    }

    public List<String> siblingsHex() {
      return siblingsHex;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof MerkleProofV1)) {
        return false;
      }
      final MerkleProofV1 rhs = (MerkleProofV1) other;
      return leafIndex == rhs.leafIndex
          && leafCount == rhs.leafCount
          && providerId.equals(rhs.providerId)
          && siblingsHex.equals(rhs.siblingsHex);
    }

    @Override
    public int hashCode() {
      return Objects.hash(providerId, leafIndex, leafCount, siblingsHex);
    }
  }

  /** Provider readback plus its proof against the returned snapshot root. */
  public static final class ProviderResponseV1 {
    private final String snapshotIdHex;
    private final String generatedAtUnix;
    private final String merkleRootHex;
    private final ProviderV1 provider;
    private final MerkleProofV1 proof;

    public ProviderResponseV1(
        final String snapshotIdHex,
        final String generatedAtUnix,
        final String merkleRootHex,
        final ProviderV1 provider,
        final MerkleProofV1 proof) {
      this.snapshotIdHex = Objects.requireNonNull(snapshotIdHex, "snapshotIdHex");
      this.generatedAtUnix = Objects.requireNonNull(generatedAtUnix, "generatedAtUnix");
      this.merkleRootHex = Objects.requireNonNull(merkleRootHex, "merkleRootHex");
      this.provider = Objects.requireNonNull(provider, "provider");
      this.proof = Objects.requireNonNull(proof, "proof");
    }

    public String snapshotIdHex() {
      return snapshotIdHex;
    }

    public String generatedAtUnix() {
      return generatedAtUnix;
    }

    public String merkleRootHex() {
      return merkleRootHex;
    }

    public ProviderV1 provider() {
      return provider;
    }

    public MerkleProofV1 proof() {
      return proof;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof ProviderResponseV1)) {
        return false;
      }
      final ProviderResponseV1 rhs = (ProviderResponseV1) other;
      return snapshotIdHex.equals(rhs.snapshotIdHex)
          && generatedAtUnix.equals(rhs.generatedAtUnix)
          && merkleRootHex.equals(rhs.merkleRootHex)
          && provider.equals(rhs.provider)
          && proof.equals(rhs.proof);
    }

    @Override
    public int hashCode() {
      return Objects.hash(snapshotIdHex, generatedAtUnix, merkleRootHex, provider, proof);
    }
  }

  /** Active scoring weights read from the latest committed snapshot. */
  public static final class WeightsResponseV1 {
    private final String snapshotIdHex;
    private final String generatedAtUnix;
    private final int alphaBps;
    private final int currentScoreWeightBps;
    private final WeightsV1 weights;

    public WeightsResponseV1(
        final String snapshotIdHex,
        final String generatedAtUnix,
        final int alphaBps,
        final int currentScoreWeightBps,
        final WeightsV1 weights) {
      this.snapshotIdHex = Objects.requireNonNull(snapshotIdHex, "snapshotIdHex");
      this.generatedAtUnix = Objects.requireNonNull(generatedAtUnix, "generatedAtUnix");
      this.alphaBps = alphaBps;
      this.currentScoreWeightBps = currentScoreWeightBps;
      this.weights = Objects.requireNonNull(weights, "weights");
    }

    public String snapshotIdHex() {
      return snapshotIdHex;
    }

    public String generatedAtUnix() {
      return generatedAtUnix;
    }

    public int alphaBps() {
      return alphaBps;
    }

    public int currentScoreWeightBps() {
      return currentScoreWeightBps;
    }

    public WeightsV1 weights() {
      return weights;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof WeightsResponseV1)) {
        return false;
      }
      final WeightsResponseV1 rhs = (WeightsResponseV1) other;
      return alphaBps == rhs.alphaBps
          && currentScoreWeightBps == rhs.currentScoreWeightBps
          && snapshotIdHex.equals(rhs.snapshotIdHex)
          && generatedAtUnix.equals(rhs.generatedAtUnix)
          && weights.equals(rhs.weights);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          snapshotIdHex, generatedAtUnix, alphaBps, currentScoreWeightBps, weights);
    }
  }

  /** One finalized reputation snapshot event. */
  public static final class SnapshotEventV1 {
    private final int version;
    private final String sequence;
    private final String snapshotIdHex;
    private final String generatedAtUnix;
    private final String merkleRootHex;
    private final int providerCount;
    private final String previousSnapshotIdHex;

    public SnapshotEventV1(
        final int version,
        final String sequence,
        final String snapshotIdHex,
        final String generatedAtUnix,
        final String merkleRootHex,
        final int providerCount,
        final String previousSnapshotIdHex) {
      this.version = version;
      this.sequence = Objects.requireNonNull(sequence, "sequence");
      this.snapshotIdHex = Objects.requireNonNull(snapshotIdHex, "snapshotIdHex");
      this.generatedAtUnix = Objects.requireNonNull(generatedAtUnix, "generatedAtUnix");
      this.merkleRootHex = Objects.requireNonNull(merkleRootHex, "merkleRootHex");
      this.providerCount = providerCount;
      this.previousSnapshotIdHex = previousSnapshotIdHex;
    }

    public int version() {
      return version;
    }

    public String sequence() {
      return sequence;
    }

    public String snapshotIdHex() {
      return snapshotIdHex;
    }

    public String generatedAtUnix() {
      return generatedAtUnix;
    }

    public String merkleRootHex() {
      return merkleRootHex;
    }

    public int providerCount() {
      return providerCount;
    }

    public String previousSnapshotIdHex() {
      return previousSnapshotIdHex;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof SnapshotEventV1)) {
        return false;
      }
      final SnapshotEventV1 rhs = (SnapshotEventV1) other;
      return version == rhs.version
          && providerCount == rhs.providerCount
          && sequence.equals(rhs.sequence)
          && snapshotIdHex.equals(rhs.snapshotIdHex)
          && generatedAtUnix.equals(rhs.generatedAtUnix)
          && merkleRootHex.equals(rhs.merkleRootHex)
          && Objects.equals(previousSnapshotIdHex, rhs.previousSnapshotIdHex);
    }

    @Override
    public int hashCode() {
      return Objects.hash(
          version,
          sequence,
          snapshotIdHex,
          generatedAtUnix,
          merkleRootHex,
          providerCount,
          previousSnapshotIdHex);
    }
  }

  /** Bounded committed reputation event page. */
  public static final class EventsResponseV1 {
    private final String since;
    private final int limit;
    private final int count;
    private final String nextSince;
    private final List<SnapshotEventV1> events;

    public EventsResponseV1(
        final String since,
        final int limit,
        final int count,
        final String nextSince,
        final List<SnapshotEventV1> events) {
      this.since = since;
      this.limit = limit;
      this.count = count;
      this.nextSince = nextSince;
      this.events = Collections.unmodifiableList(new ArrayList<>(events));
    }

    public String since() {
      return since;
    }

    public int limit() {
      return limit;
    }

    public int count() {
      return count;
    }

    public String nextSince() {
      return nextSince;
    }

    public List<SnapshotEventV1> events() {
      return events;
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof EventsResponseV1)) {
        return false;
      }
      final EventsResponseV1 rhs = (EventsResponseV1) other;
      return limit == rhs.limit
          && count == rhs.count
          && Objects.equals(since, rhs.since)
          && Objects.equals(nextSince, rhs.nextSince)
          && events.equals(rhs.events);
    }

    @Override
    public int hashCode() {
      return Objects.hash(since, limit, count, nextSince, events);
    }
  }

  /** Typed callbacks for the authenticated reputation SSE route. */
  public interface EventStreamListener {
    /** Invoked after Torii accepts the stream. */
    default void onOpen() {}

    /** Invoked for a finalized snapshot event. */
    void onSnapshot(SnapshotEventV1 event);

    /** Invoked when the retained journal skipped a positive number of events. */
    default void onLagged(final String skipped) {}

    /** Invoked after normal stream termination. */
    default void onClosed() {}

    /** Invoked for transport, status, framing, or projection failures. */
    default void onError(final Throwable error) {}
  }
}
