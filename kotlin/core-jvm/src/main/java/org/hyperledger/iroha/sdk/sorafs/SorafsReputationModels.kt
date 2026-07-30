package org.hyperledger.iroha.sdk.sorafs

import java.util.ArrayList
import java.util.Collections
import java.util.Objects

/** Canonical V1 reputation scoring weights. */
class SorafsReputationWeightsV1(
    @JvmField val version: Int,
    @JvmField val porSuccessBps: Int,
    @JvmField val pdpSuccessBps: Int,
    @JvmField val potrSuccessBps: Int,
    @JvmField val latencyBps: Int,
    @JvmField val disputeBps: Int,
    @JvmField val tokenViolationBps: Int,
    @JvmField val repairBreachBps: Int,
) {
    override fun equals(other: Any?): Boolean =
        other is SorafsReputationWeightsV1 &&
            version == other.version &&
            porSuccessBps == other.porSuccessBps &&
            pdpSuccessBps == other.pdpSuccessBps &&
            potrSuccessBps == other.potrSuccessBps &&
            latencyBps == other.latencyBps &&
            disputeBps == other.disputeBps &&
            tokenViolationBps == other.tokenViolationBps &&
            repairBreachBps == other.repairBreachBps

    override fun hashCode(): Int =
        Objects.hash(
            version,
            porSuccessBps,
            pdpSuccessBps,
            potrSuccessBps,
            latencyBps,
            disputeBps,
            tokenViolationBps,
            repairBreachBps,
        )
}

/** Canonical V1 metrics committed for one provider. */
class SorafsReputationProviderMetricsV1(
    @JvmField val version: Int,
    @JvmField val porSuccessBps: Int,
    @JvmField val pdpSuccessBps: Int,
    @JvmField val potrSuccessBps: Int,
    @JvmField val latencyHealthBps: Int,
    @JvmField val disputeRateBps: Int,
    @JvmField val tokenViolationRateBps: Int,
    @JvmField val repairBreachRateBps: Int,
) {
    override fun equals(other: Any?): Boolean =
        other is SorafsReputationProviderMetricsV1 &&
            version == other.version &&
            porSuccessBps == other.porSuccessBps &&
            pdpSuccessBps == other.pdpSuccessBps &&
            potrSuccessBps == other.potrSuccessBps &&
            latencyHealthBps == other.latencyHealthBps &&
            disputeRateBps == other.disputeRateBps &&
            tokenViolationRateBps == other.tokenViolationRateBps &&
            repairBreachRateBps == other.repairBreachRateBps

    override fun hashCode(): Int =
        Objects.hash(
            version,
            porSuccessBps,
            pdpSuccessBps,
            potrSuccessBps,
            latencyHealthBps,
            disputeRateBps,
            tokenViolationRateBps,
            repairBreachRateBps,
        )
}

/** One provider entry in an immutable reputation snapshot. */
class SorafsReputationProviderV1(
    @JvmField val providerId: String,
    @JvmField val scoreBps: Int,
    degradationFlags: List<String>,
    @JvmField val rawMetrics: SorafsReputationProviderMetricsV1,
    @JvmField val rawMetricsHashHex: String,
) {
    private val _degradationFlags: List<String> =
        Collections.unmodifiableList(ArrayList(degradationFlags))

    /** Canonical sorted unique degradation-flag labels. */
    val degradationFlags: List<String> get() = _degradationFlags

    override fun equals(other: Any?): Boolean =
        other is SorafsReputationProviderV1 &&
            providerId == other.providerId &&
            scoreBps == other.scoreBps &&
            degradationFlags == other.degradationFlags &&
            rawMetrics == other.rawMetrics &&
            rawMetricsHashHex == other.rawMetricsHashHex

    override fun hashCode(): Int =
        Objects.hash(providerId, scoreBps, degradationFlags, rawMetrics, rawMetricsHashHex)
}

/** Bounded view of one immutable committed reputation snapshot. */
class SorafsReputationSnapshotSummaryV1(
    @JvmField val snapshotIdHex: String,
    /** Canonical unsigned decimal Unix timestamp. */
    @JvmField val generatedAtUnix: String,
    @JvmField val previousSnapshotIdHex: String?,
    @JvmField val merkleRootHex: String,
    @JvmField val providerCount: Int,
    @JvmField val returnedProviderCount: Int,
    @JvmField val limit: Int,
    @JvmField val truncatedProviders: Boolean,
    @JvmField val alphaBps: Int,
    @JvmField val currentScoreWeightBps: Int,
    @JvmField val weights: SorafsReputationWeightsV1,
    providers: List<SorafsReputationProviderV1>,
) {
    private val _providers: List<SorafsReputationProviderV1> =
        Collections.unmodifiableList(ArrayList(providers))

    /** Returned canonical provider prefix. */
    val providers: List<SorafsReputationProviderV1> get() = _providers

    override fun equals(other: Any?): Boolean =
        other is SorafsReputationSnapshotSummaryV1 &&
            snapshotIdHex == other.snapshotIdHex &&
            generatedAtUnix == other.generatedAtUnix &&
            previousSnapshotIdHex == other.previousSnapshotIdHex &&
            merkleRootHex == other.merkleRootHex &&
            providerCount == other.providerCount &&
            returnedProviderCount == other.returnedProviderCount &&
            limit == other.limit &&
            truncatedProviders == other.truncatedProviders &&
            alphaBps == other.alphaBps &&
            currentScoreWeightBps == other.currentScoreWeightBps &&
            weights == other.weights &&
            providers == other.providers

    override fun hashCode(): Int =
        Objects.hash(
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
            providers,
        )
}

/** Complete V1 Merkle inclusion proof for one provider. */
class SorafsReputationMerkleProofV1(
    @JvmField val providerId: String,
    @JvmField val leafIndex: Int,
    @JvmField val leafCount: Int,
    siblingsHex: List<String>,
) {
    private val _siblingsHex: List<String> =
        Collections.unmodifiableList(ArrayList(siblingsHex))

    /** Canonical lowercase sibling digests. */
    val siblingsHex: List<String> get() = _siblingsHex

    override fun equals(other: Any?): Boolean =
        other is SorafsReputationMerkleProofV1 &&
            providerId == other.providerId &&
            leafIndex == other.leafIndex &&
            leafCount == other.leafCount &&
            siblingsHex == other.siblingsHex

    override fun hashCode(): Int = Objects.hash(providerId, leafIndex, leafCount, siblingsHex)
}

/** Provider readback plus its proof against the returned snapshot root. */
class SorafsReputationProviderResponseV1(
    @JvmField val snapshotIdHex: String,
    /** Canonical unsigned decimal Unix timestamp. */
    @JvmField val generatedAtUnix: String,
    @JvmField val merkleRootHex: String,
    @JvmField val provider: SorafsReputationProviderV1,
    @JvmField val proof: SorafsReputationMerkleProofV1,
) {
    override fun equals(other: Any?): Boolean =
        other is SorafsReputationProviderResponseV1 &&
            snapshotIdHex == other.snapshotIdHex &&
            generatedAtUnix == other.generatedAtUnix &&
            merkleRootHex == other.merkleRootHex &&
            provider == other.provider &&
            proof == other.proof

    override fun hashCode(): Int =
        Objects.hash(snapshotIdHex, generatedAtUnix, merkleRootHex, provider, proof)
}

/** Active scoring weights read from the latest committed snapshot. */
class SorafsReputationWeightsResponseV1(
    @JvmField val snapshotIdHex: String,
    /** Canonical unsigned decimal Unix timestamp. */
    @JvmField val generatedAtUnix: String,
    @JvmField val alphaBps: Int,
    @JvmField val currentScoreWeightBps: Int,
    @JvmField val weights: SorafsReputationWeightsV1,
) {
    override fun equals(other: Any?): Boolean =
        other is SorafsReputationWeightsResponseV1 &&
            snapshotIdHex == other.snapshotIdHex &&
            generatedAtUnix == other.generatedAtUnix &&
            alphaBps == other.alphaBps &&
            currentScoreWeightBps == other.currentScoreWeightBps &&
            weights == other.weights

    override fun hashCode(): Int =
        Objects.hash(
            snapshotIdHex,
            generatedAtUnix,
            alphaBps,
            currentScoreWeightBps,
            weights,
        )
}

/** One finalized reputation snapshot event. */
class SorafsReputationSnapshotEventV1(
    @JvmField val version: Int,
    /** Canonical positive unsigned decimal sequence. */
    @JvmField val sequence: String,
    @JvmField val snapshotIdHex: String,
    /** Canonical positive unsigned decimal Unix timestamp. */
    @JvmField val generatedAtUnix: String,
    @JvmField val merkleRootHex: String,
    @JvmField val providerCount: Int,
    @JvmField val previousSnapshotIdHex: String?,
) {
    override fun equals(other: Any?): Boolean =
        other is SorafsReputationSnapshotEventV1 &&
            version == other.version &&
            sequence == other.sequence &&
            snapshotIdHex == other.snapshotIdHex &&
            generatedAtUnix == other.generatedAtUnix &&
            merkleRootHex == other.merkleRootHex &&
            providerCount == other.providerCount &&
            previousSnapshotIdHex == other.previousSnapshotIdHex

    override fun hashCode(): Int =
        Objects.hash(
            version,
            sequence,
            snapshotIdHex,
            generatedAtUnix,
            merkleRootHex,
            providerCount,
            previousSnapshotIdHex,
        )
}

/** Bounded committed reputation event page. */
class SorafsReputationEventsResponseV1(
    /** Canonical unsigned decimal request cursor, or `null` when omitted. */
    @JvmField val since: String?,
    @JvmField val limit: Int,
    @JvmField val count: Int,
    /** Canonical positive unsigned decimal last sequence, or `null` for an empty page. */
    @JvmField val nextSince: String?,
    events: List<SorafsReputationSnapshotEventV1>,
) {
    private val _events: List<SorafsReputationSnapshotEventV1> =
        Collections.unmodifiableList(ArrayList(events))

    /** Finalized snapshot events in sequence order. */
    val events: List<SorafsReputationSnapshotEventV1> get() = _events

    override fun equals(other: Any?): Boolean =
        other is SorafsReputationEventsResponseV1 &&
            since == other.since &&
            limit == other.limit &&
            count == other.count &&
            nextSince == other.nextSince &&
            events == other.events

    override fun hashCode(): Int = Objects.hash(since, limit, count, nextSince, events)
}

/** Typed callbacks for the authenticated reputation SSE route. */
interface SorafsReputationEventStreamListener {
    /** Invoked after Torii accepts the stream. */
    fun onOpen() {}

    /** Invoked for a finalized snapshot event. */
    fun onSnapshot(event: SorafsReputationSnapshotEventV1)

    /** Invoked when the retained journal skipped a positive number of events. */
    fun onLagged(skipped: String) {}

    /** Invoked after normal stream termination. */
    fun onClosed() {}

    /** Invoked for transport, status, framing, or projection failures. */
    fun onError(error: Throwable) {}
}
