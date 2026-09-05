// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/** Strict immutable models for the authoritative {@code /v1/sumeragi/status} response. */
public final class SumeragiStatusModels {
  /** The only Sumeragi status protocol revision accepted by the first-release SDK. */
  public static final int STATUS_PROTOCOL_VERSION = 4;

  /** Maximum encoded JSON body accepted from {@code /v1/sumeragi/status}. */
  public static final long STATUS_JSON_MAX_BYTES = 1L * 1024L * 1024L;

  /** Maximum encoded JSON body accepted from {@code /v1/sumeragi/diagnostics}. */
  public static final long DIAGNOSTICS_JSON_MAX_BYTES = 16L * 1024L * 1024L;

  private static final int NATIVE_AMX_APPLICATION_MANIFEST_VERSION = 1;
  private static final int NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES = 1_024;
  private static final int LANE_FINALITY_MANIFEST_MAX_LEAVES = 1_024;
  private static final int MERGE_CARRIER_COMMITMENT_VERSION = 1;
  private static final String NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT =
      "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F";
  private static final BigInteger THIRTY_ONE = BigInteger.valueOf(31L);
  private static final BigInteger THREE = BigInteger.valueOf(3L);
  private static final BigInteger TWO = BigInteger.valueOf(2L);

  private SumeragiStatusModels() {}

  private interface WireEnum {
    String wireName();
  }

  /** One-element hash tuple identifying a frozen height context. */
  public static final class ContextId {
    private final String hash;

    private ContextId(final String hash) {
      this.hash = hash;
    }

    public String hash() { return hash; }
  }

  /** Consensus round in one frozen height context. */
  public static final class Round {
    private final ContextId contextId;
    private final BigInteger height;
    private final BigInteger view;

    private Round(final ContextId contextId, final BigInteger height, final BigInteger view) {
      this.contextId = contextId;
      this.height = height;
      this.view = view;
    }

    public ContextId contextId() { return contextId; }
    public BigInteger height() { return height; }
    public BigInteger view() { return view; }
  }

  /** Exact block and payload identity certified by Sumeragi. */
  public static final class BlockSubject {
    private final String parentBlockHash;
    private final String blockHash;
    private final String payloadHash;

    private BlockSubject(
        final String parentBlockHash, final String blockHash, final String payloadHash) {
      this.parentBlockHash = parentBlockHash;
      this.blockHash = blockHash;
      this.payloadHash = payloadHash;
    }

    public String parentBlockHash() { return parentBlockHash; }
    public String blockHash() { return blockHash; }
    public String payloadHash() { return payloadHash; }
  }

  /** Exact Merkle root and non-zero leaf count of canonical lane-finality statements. */
  public static final class LaneFinalityManifestCommitment {
    private final String root;
    private final BigInteger leafCount;

    private LaneFinalityManifestCommitment(final String root, final BigInteger leafCount) {
      this.root = root;
      this.leafCount = leafCount;
    }

    public String root() { return root; }
    public BigInteger leafCount() { return leafCount; }
  }

  /** Exact merge-ledger entry identity authenticated by a global certificate. */
  public static final class MergeCarrierCommitment {
    private final int version;
    private final String entryHash;

    private MergeCarrierCommitment(final int version, final String entryHash) {
      this.version = version;
      this.entryHash = entryHash;
    }

    public int version() { return version; }
    public String entryHash() { return entryHash; }
  }

  /** Deterministic execution commitment authenticated by a global certificate. */
  public static final class ExecutionCommitment {
    private final String parentStateRoot;
    private final String postStateRoot;
    private final String ordinaryWritesRoot;
    private final String kagemushaTopUpRoot;
    private final BigInteger kagemushaTopUpCount;
    private final int nativeAmxApplicationManifestVersion;
    private final String nativeAmxApplicationManifestRoot;
    private final BigInteger nativeAmxApplicationManifestCount;
    private final LaneFinalityManifestCommitment laneFinalityManifest;
    private final MergeCarrierCommitment mergeCarrier;
    private final BigInteger executedBlockWireLen;
    private final String executedBlockWireHash;

    private ExecutionCommitment(
        final String parentStateRoot,
        final String postStateRoot,
        final String ordinaryWritesRoot,
        final String kagemushaTopUpRoot,
        final BigInteger kagemushaTopUpCount,
        final int nativeAmxApplicationManifestVersion,
        final String nativeAmxApplicationManifestRoot,
        final BigInteger nativeAmxApplicationManifestCount,
        final LaneFinalityManifestCommitment laneFinalityManifest,
        final MergeCarrierCommitment mergeCarrier,
        final BigInteger executedBlockWireLen,
        final String executedBlockWireHash) {
      this.parentStateRoot = parentStateRoot;
      this.postStateRoot = postStateRoot;
      this.ordinaryWritesRoot = ordinaryWritesRoot;
      this.kagemushaTopUpRoot = kagemushaTopUpRoot;
      this.kagemushaTopUpCount = kagemushaTopUpCount;
      this.nativeAmxApplicationManifestVersion = nativeAmxApplicationManifestVersion;
      this.nativeAmxApplicationManifestRoot = nativeAmxApplicationManifestRoot;
      this.nativeAmxApplicationManifestCount = nativeAmxApplicationManifestCount;
      this.laneFinalityManifest = laneFinalityManifest;
      this.mergeCarrier = mergeCarrier;
      this.executedBlockWireLen = executedBlockWireLen;
      this.executedBlockWireHash = executedBlockWireHash;
    }

    public String parentStateRoot() { return parentStateRoot; }
    public String postStateRoot() { return postStateRoot; }
    public String ordinaryWritesRoot() { return ordinaryWritesRoot; }
    public String kagemushaTopUpRoot() { return kagemushaTopUpRoot; }
    public BigInteger kagemushaTopUpCount() { return kagemushaTopUpCount; }
    public int nativeAmxApplicationManifestVersion() {
      return nativeAmxApplicationManifestVersion;
    }
    public String nativeAmxApplicationManifestRoot() {
      return nativeAmxApplicationManifestRoot;
    }
    public BigInteger nativeAmxApplicationManifestCount() {
      return nativeAmxApplicationManifestCount;
    }
    public LaneFinalityManifestCommitment laneFinalityManifest() {
      return laneFinalityManifest;
    }
    public MergeCarrierCommitment mergeCarrier() { return mergeCarrier; }
    public BigInteger executedBlockWireLen() { return executedBlockWireLen; }
    public String executedBlockWireHash() { return executedBlockWireHash; }
  }

  /** Global voting phase carried by a quorum-certificate reference. */
  public enum GlobalPhase implements WireEnum {
    PREPARE("prepare"), COMMIT("commit");
    private final String wireName;
    GlobalPhase(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Stable semantic reference to one Sumeragi quorum certificate. */
  public static final class QcReference {
    private final Round round;
    private final Round proposalRound;
    private final GlobalPhase phase;
    private final BlockSubject subject;
    private final ExecutionCommitment executionCommitment;

    private QcReference(
        final Round round,
        final Round proposalRound,
        final GlobalPhase phase,
        final BlockSubject subject,
        final ExecutionCommitment executionCommitment) {
      this.round = round;
      this.proposalRound = proposalRound;
      this.phase = phase;
      this.subject = subject;
      this.executionCommitment = executionCommitment;
    }

    public Round round() { return round; }
    public Round proposalRound() { return proposalRound; }
    public GlobalPhase phase() { return phase; }
    public BlockSubject subject() { return subject; }
    public ExecutionCommitment executionCommitment() { return executionCommitment; }
  }

  /** Stable semantic reference to the latest installed timeout certificate. */
  public static final class TimeoutCertificate {
    private final Round round;
    private final QcReference highestPrepareQc;
    private final String certificateHash;

    private TimeoutCertificate(
        final Round round, final QcReference highestPrepareQc, final String certificateHash) {
      this.round = round;
      this.highestPrepareQc = highestPrepareQc;
      this.certificateHash = certificateHash;
    }

    public Round round() { return round; }
    public QcReference highestPrepareQc() { return highestPrepareQc; }
    public String certificateHash() { return certificateHash; }
  }

  /** Consensus mode frozen into the active height context. */
  public enum ConsensusMode implements WireEnum {
    PERMISSIONED("permissioned"), NPOS("npos");
    private final String wireName;
    ConsensusMode(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Equal-vote quorum inputs frozen for one height. */
  public static final class Quorum {
    private final BigInteger minSigners;
    private final BigInteger totalPower;

    private Quorum(final BigInteger minSigners, final BigInteger totalPower) {
      this.minSigners = minSigners;
      this.totalPower = totalPower;
    }

    public BigInteger minSigners() { return minSigners; }
    public BigInteger totalPower() { return totalPower; }
  }

  /** Frozen election and quorum inputs governing the active height. */
  public static final class HeightContext {
    private final BigInteger epoch;
    private final BigInteger epochEndHeight;
    private final ConsensusMode mode;
    private final String epochSeed;
    private final BigInteger validatorCount;
    private final Quorum quorum;

    private HeightContext(
        final BigInteger epoch,
        final BigInteger epochEndHeight,
        final ConsensusMode mode,
        final String epochSeed,
        final BigInteger validatorCount,
        final Quorum quorum) {
      this.epoch = epoch;
      this.epochEndHeight = epochEndHeight;
      this.mode = mode;
      this.epochSeed = epochSeed;
      this.validatorCount = validatorCount;
      this.quorum = quorum;
    }

    public BigInteger epoch() { return epoch; }
    public BigInteger epochEndHeight() { return epochEndHeight; }
    public ConsensusMode mode() { return mode; }
    public String epochSeed() { return epochSeed; }
    public BigInteger validatorCount() { return validatorCount; }
    public Quorum quorum() { return quorum; }
  }

  /** Latest durable CommitQC with exact count and voting-power totals. */
  public static final class CommitQc {
    private final QcReference certificate;
    private final BigInteger validatorCount;
    private final BigInteger signerCount;
    private final BigInteger minSigners;
    private final BigInteger signedPower;
    private final BigInteger totalPower;

    private CommitQc(
        final QcReference certificate,
        final BigInteger validatorCount,
        final BigInteger signerCount,
        final BigInteger minSigners,
        final BigInteger signedPower,
        final BigInteger totalPower) {
      this.certificate = certificate;
      this.validatorCount = validatorCount;
      this.signerCount = signerCount;
      this.minSigners = minSigners;
      this.signedPower = signedPower;
      this.totalPower = totalPower;
    }

    public QcReference certificate() { return certificate; }
    public BigInteger validatorCount() { return validatorCount; }
    public BigInteger signerCount() { return signerCount; }
    public BigInteger minSigners() { return minSigners; }
    public BigInteger signedPower() { return signedPower; }
    public BigInteger totalPower() { return totalPower; }
  }

  /** Partial dual quorum for one exact proposal round. */
  public static final class VoteQuorum {
    private final Round round;
    private final Round proposalRound;
    private final BlockSubject subject;
    private final ExecutionCommitment executionCommitment;
    private final BigInteger signerCount;
    private final BigInteger signedPower;
    private final BigInteger minSigners;
    private final BigInteger totalPower;

    private VoteQuorum(
        final Round round,
        final Round proposalRound,
        final BlockSubject subject,
        final ExecutionCommitment executionCommitment,
        final BigInteger signerCount,
        final BigInteger signedPower,
        final BigInteger minSigners,
        final BigInteger totalPower) {
      this.round = round;
      this.proposalRound = proposalRound;
      this.subject = subject;
      this.executionCommitment = executionCommitment;
      this.signerCount = signerCount;
      this.signedPower = signedPower;
      this.minSigners = minSigners;
      this.totalPower = totalPower;
    }

    public Round round() { return round; }
    public Round proposalRound() { return proposalRound; }
    public BlockSubject subject() { return subject; }
    public ExecutionCommitment executionCommitment() { return executionCommitment; }
    public BigInteger signerCount() { return signerCount; }
    public BigInteger signedPower() { return signedPower; }
    public BigInteger minSigners() { return minSigners; }
    public BigInteger totalPower() { return totalPower; }
  }

  /** Partial timeout quorum for one exact round. */
  public static final class TimeoutQuorum {
    private final Round round;
    private final BigInteger signerCount;
    private final BigInteger signedPower;
    private final BigInteger minSigners;
    private final BigInteger totalPower;
    private final boolean certificateFormed;

    private TimeoutQuorum(
        final Round round,
        final BigInteger signerCount,
        final BigInteger signedPower,
        final BigInteger minSigners,
        final BigInteger totalPower,
        final boolean certificateFormed) {
      this.round = round;
      this.signerCount = signerCount;
      this.signedPower = signedPower;
      this.minSigners = minSigners;
      this.totalPower = totalPower;
      this.certificateFormed = certificateFormed;
    }

    public Round round() { return round; }
    public BigInteger signerCount() { return signerCount; }
    public BigInteger signedPower() { return signedPower; }
    public BigInteger minSigners() { return minSigners; }
    public BigInteger totalPower() { return totalPower; }
    public boolean certificateFormed() { return certificateFormed; }
  }

  /** Durable outbound protocol intent kind. */
  public enum OutboundIntentKind implements WireEnum {
    PROPOSAL("proposal"),
    PREPARE_VOTE("prepare_vote"),
    COMMIT_VOTE("commit_vote"),
    TIMEOUT_VOTE("timeout_vote"),
    PREPARE_QC("prepare_qc"),
    COMMIT_QC("commit_qc"),
    TIMEOUT_CERTIFICATE("timeout_certificate");
    private final String wireName;
    OutboundIntentKind(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Durable delivery stage of an outbound protocol intent. */
  public enum OutboundIntentStage implements WireEnum {
    PENDING_PERSISTENCE("pending_persistence"),
    PENDING_SIGNATURE("pending_signature"),
    QUEUED("queued"),
    SENT("sent");
    private final String wireName;
    OutboundIntentStage(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Durable outbound protocol intent and its exact optional proposal identity. */
  public static final class OutboundIntent {
    private final OutboundIntentKind kind;
    private final Round round;
    private final Round proposalRound;
    private final BlockSubject subject;
    private final ExecutionCommitment executionCommitment;
    private final OutboundIntentStage stage;

    private OutboundIntent(
        final OutboundIntentKind kind,
        final Round round,
        final Round proposalRound,
        final BlockSubject subject,
        final ExecutionCommitment executionCommitment,
        final OutboundIntentStage stage) {
      this.kind = kind;
      this.round = round;
      this.proposalRound = proposalRound;
      this.subject = subject;
      this.executionCommitment = executionCommitment;
      this.stage = stage;
    }

    public OutboundIntentKind kind() { return kind; }
    public Round round() { return round; }
    public Round proposalRound() { return proposalRound; }
    public BlockSubject subject() { return subject; }
    public ExecutionCommitment executionCommitment() { return executionCommitment; }
    public OutboundIntentStage stage() { return stage; }
  }

  /** Local terminating-work stage. */
  public enum WorkStage implements WireEnum {
    IDLE("idle"), QUEUED("queued"), RUNNING("running"), COMPLETE("complete");
    private final String wireName;
    WorkStage(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Local terminating-work stages for the active height. */
  public static final class Work {
    private final WorkStage candidate;
    private final WorkStage bodyRecovery;
    private final WorkStage bodyStore;
    private final WorkStage validation;
    private final WorkStage application;
    private final WorkStage successorHeight;

    private Work(
        final WorkStage candidate,
        final WorkStage bodyRecovery,
        final WorkStage bodyStore,
        final WorkStage validation,
        final WorkStage application,
        final WorkStage successorHeight) {
      this.candidate = candidate;
      this.bodyRecovery = bodyRecovery;
      this.bodyStore = bodyStore;
      this.validation = validation;
      this.application = application;
      this.successorHeight = successorHeight;
    }

    public WorkStage candidate() { return candidate; }
    public WorkStage bodyRecovery() { return bodyRecovery; }
    public WorkStage bodyStore() { return bodyStore; }
    public WorkStage validation() { return validation; }
    public WorkStage application() { return application; }
    public WorkStage successorHeight() { return successorHeight; }
  }

  /** Identity of a bounded reducer or runtime progress queue. */
  public enum QueueKind implements WireEnum {
    INGRESS("ingress"),
    DEFERRED_NORMAL("deferred_normal"),
    DEFERRED_PROGRESS("deferred_progress"),
    DEFERRED_COMPLETION("deferred_completion"),
    RUNTIME_NORMAL("runtime_normal"),
    RUNTIME_PROGRESS("runtime_progress"),
    RUNTIME_COMPLETION("runtime_completion"),
    EFFECT_COMPLETION("effect_completion"),
    NETWORK_INGRESS("network_ingress"),
    EFFECT_DISPATCH("effect_dispatch");
    private final String wireName;
    QueueKind(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Occupancy and accumulated service debt for one bounded queue. */
  public static final class Queue {
    private final QueueKind queue;
    private final BigInteger depth;
    private final BigInteger capacity;
    private final BigInteger oldestAgeMs;
    private final BigInteger serviceDebt;

    private Queue(
        final QueueKind queue,
        final BigInteger depth,
        final BigInteger capacity,
        final BigInteger oldestAgeMs,
        final BigInteger serviceDebt) {
      this.queue = queue;
      this.depth = depth;
      this.capacity = capacity;
      this.oldestAgeMs = oldestAgeMs;
      this.serviceDebt = serviceDebt;
    }

    public QueueKind queue() { return queue; }
    public BigInteger depth() { return depth; }
    public BigInteger capacity() { return capacity; }
    public BigInteger oldestAgeMs() { return oldestAgeMs; }
    public BigInteger serviceDebt() { return serviceDebt; }
  }

  /** Reducer transition tracked as authoritative liveness progress. */
  public enum ProgressTransition implements WireEnum {
    PROPOSAL_ADMITTED("proposal_admitted"),
    BODY_AVAILABLE("body_available"),
    BODY_STORED("body_stored"),
    BODY_VALIDATED("body_validated"),
    PREPARE_VOTE_ADMITTED("prepare_vote_admitted"),
    COMMIT_VOTE_ADMITTED("commit_vote_admitted"),
    TIMEOUT_VOTE_ADMITTED("timeout_vote_admitted"),
    PREPARE_QUORUM("prepare_quorum"),
    LOCK_INSTALLED("lock_installed"),
    COMMIT_QUORUM("commit_quorum"),
    TIMEOUT_CERTIFICATE_INSTALLED("timeout_certificate_installed"),
    DECISION_PERSISTED("decision_persisted"),
    APPLIED("applied"),
    SUCCESSOR_HEIGHT_ACTIVATED("successor_height_activated"),
    RECOVERY_REPLAYED("recovery_replayed");
    private final String wireName;
    ProgressTransition(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Last tracked reducer transition and its local age. */
  public static final class Progress {
    private final BigInteger generation;
    private final Round round;
    private final ProgressTransition transition;
    private final BigInteger ageMs;

    private Progress(
        final BigInteger generation,
        final Round round,
        final ProgressTransition transition,
        final BigInteger ageMs) {
      this.generation = generation;
      this.round = round;
      this.transition = transition;
      this.ageMs = ageMs;
    }

    public BigInteger generation() { return generation; }
    public Round round() { return round; }
    public ProgressTransition transition() { return transition; }
    public BigInteger ageMs() { return ageMs; }
  }

  /** Classified cause of an active no-progress interval. */
  public enum LivenessBlocker implements WireEnum {
    MISSING_PROPOSAL("missing_proposal"),
    BODY_UNAVAILABLE("body_unavailable"),
    PREPARE_QUORUM_MISSING("prepare_quorum_missing"),
    COMMIT_QUORUM_MISSING("commit_quorum_missing"),
    TIMEOUT_CERTIFICATE_MISSING("timeout_certificate_missing"),
    SCHEDULER_STARVATION("scheduler_starvation"),
    APPLICATION_PENDING("application_pending"),
    SUCCESSOR_ACTIVATION_PENDING("successor_activation_pending"),
    LOCAL_CONTROL_PENDING("local_control_pending");
    private final String wireName;
    LivenessBlocker(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Closed reducer reason for safely ignoring an input. */
  public enum IgnoreReason implements WireEnum {
    WRONG_HEIGHT("wrong_height"),
    WRONG_VIEW("wrong_view"),
    STALE_GENERATION("stale_generation"),
    BUSY("busy"),
    DUPLICATE("duplicate"),
    NO_MATCHING_WORK("no_matching_work"),
    OBSERVER("observer"),
    VIEW_CLOSED("view_closed"),
    ALREADY_DECIDED("already_decided"),
    RECOVERY_PENDING("recovery_pending"),
    IRRELEVANT_VIEW("irrelevant_view"),
    UNSAFE_PROPOSAL("unsafe_proposal");
    private final String wireName;
    IgnoreReason(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Per-height count for one closed reducer ignore reason. */
  public static final class IgnoreCount {
    private final IgnoreReason reason;
    private final BigInteger count;

    private IgnoreCount(final IgnoreReason reason, final BigInteger count) {
      this.reason = reason;
      this.count = count;
    }

    public IgnoreReason reason() { return reason; }
    public BigInteger count() { return count; }
  }

  /** Authoritative progress diagnostics for the active height. */
  public static final class Liveness {
    private final BigInteger generation;
    private final List<VoteQuorum> prepareQuorums;
    private final List<VoteQuorum> commitQuorums;
    private final List<TimeoutQuorum> timeoutQuorums;
    private final List<OutboundIntent> outboundIntents;
    private final Work work;
    private final List<Queue> queues;
    private final Progress lastProgress;
    private final BigInteger noProgressAgeMs;
    private final LivenessBlocker blocker;
    private final List<IgnoreCount> ignoreCounts;

    private Liveness(
        final BigInteger generation,
        final List<VoteQuorum> prepareQuorums,
        final List<VoteQuorum> commitQuorums,
        final List<TimeoutQuorum> timeoutQuorums,
        final List<OutboundIntent> outboundIntents,
        final Work work,
        final List<Queue> queues,
        final Progress lastProgress,
        final BigInteger noProgressAgeMs,
        final LivenessBlocker blocker,
        final List<IgnoreCount> ignoreCounts) {
      this.generation = generation;
      this.prepareQuorums = immutableCopy(prepareQuorums);
      this.commitQuorums = immutableCopy(commitQuorums);
      this.timeoutQuorums = immutableCopy(timeoutQuorums);
      this.outboundIntents = immutableCopy(outboundIntents);
      this.work = work;
      this.queues = immutableCopy(queues);
      this.lastProgress = lastProgress;
      this.noProgressAgeMs = noProgressAgeMs;
      this.blocker = blocker;
      this.ignoreCounts = immutableCopy(ignoreCounts);
    }

    public BigInteger generation() { return generation; }
    public List<VoteQuorum> prepareQuorums() { return prepareQuorums; }
    public List<VoteQuorum> commitQuorums() { return commitQuorums; }
    public List<TimeoutQuorum> timeoutQuorums() { return timeoutQuorums; }
    public List<OutboundIntent> outboundIntents() { return outboundIntents; }
    public Work work() { return work; }
    public List<Queue> queues() { return queues; }
    public Progress lastProgress() { return lastProgress; }
    public BigInteger noProgressAgeMs() { return noProgressAgeMs; }
    public LivenessBlocker blocker() { return blocker; }
    public List<IgnoreCount> ignoreCounts() { return ignoreCounts; }
  }

  /** Active authoritative reducer phase. */
  public enum Phase implements WireEnum {
    AWAITING_PROPOSAL("awaiting_proposal"),
    RECONSTRUCTING_PAYLOAD("reconstructing_payload"),
    VALIDATING_PAYLOAD("validating_payload"),
    PREPARE("prepare"),
    COMMIT("commit"),
    PENDING_APPLY("pending_apply");
    private final String wireName;
    Phase(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Local body state paired with the authoritative reducer phase. */
  public enum BodyState implements WireEnum {
    MISSING("missing"),
    RECONSTRUCTING("reconstructing"),
    STORED("stored"),
    VALIDATED("validated"),
    PENDING_APPLY("pending_apply"),
    APPLIED("applied");
    private final String wireName;
    BodyState(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
  }

  /** Complete authoritative protocol-v4 status snapshot. */
  public static final class SumeragiV2Status {
    private final int protocolVersion;
    private final String nodeFingerprint;
    private final String buildFingerprint;
    private final String configFingerprint;
    private final boolean restartRequired;
    private final ContextId heightContextId;
    private final BigInteger height;
    private final BigInteger view;
    private final Phase phase;
    private final BigInteger leader;
    private final QcReference lockedPrepareQc;
    private final QcReference highestPrepareQc;
    private final TimeoutCertificate lastTimeoutCertificate;
    private final BodyState bodyState;
    private final BigInteger pendingPersistenceId;
    private final BigInteger lastCommittedHeight;
    private final BlockSubject lastCommittedSubject;
    private final HeightContext heightContext;
    private final CommitQc lastCommitQc;
    private final Liveness liveness;

    private SumeragiV2Status(
        final int protocolVersion,
        final String nodeFingerprint,
        final String buildFingerprint,
        final String configFingerprint,
        final boolean restartRequired,
        final ContextId heightContextId,
        final BigInteger height,
        final BigInteger view,
        final Phase phase,
        final BigInteger leader,
        final QcReference lockedPrepareQc,
        final QcReference highestPrepareQc,
        final TimeoutCertificate lastTimeoutCertificate,
        final BodyState bodyState,
        final BigInteger pendingPersistenceId,
        final BigInteger lastCommittedHeight,
        final BlockSubject lastCommittedSubject,
        final HeightContext heightContext,
        final CommitQc lastCommitQc,
        final Liveness liveness) {
      this.protocolVersion = protocolVersion;
      this.nodeFingerprint = nodeFingerprint;
      this.buildFingerprint = buildFingerprint;
      this.configFingerprint = configFingerprint;
      this.restartRequired = restartRequired;
      this.heightContextId = heightContextId;
      this.height = height;
      this.view = view;
      this.phase = phase;
      this.leader = leader;
      this.lockedPrepareQc = lockedPrepareQc;
      this.highestPrepareQc = highestPrepareQc;
      this.lastTimeoutCertificate = lastTimeoutCertificate;
      this.bodyState = bodyState;
      this.pendingPersistenceId = pendingPersistenceId;
      this.lastCommittedHeight = lastCommittedHeight;
      this.lastCommittedSubject = lastCommittedSubject;
      this.heightContext = heightContext;
      this.lastCommitQc = lastCommitQc;
      this.liveness = liveness;
    }

    public int protocolVersion() { return protocolVersion; }
    public String nodeFingerprint() { return nodeFingerprint; }
    public String buildFingerprint() { return buildFingerprint; }
    public String configFingerprint() { return configFingerprint; }
    public boolean restartRequired() { return restartRequired; }
    public ContextId heightContextId() { return heightContextId; }
    public BigInteger height() { return height; }
    public BigInteger view() { return view; }
    public Phase phase() { return phase; }
    public BigInteger leader() { return leader; }
    public QcReference lockedPrepareQc() { return lockedPrepareQc; }
    public QcReference highestPrepareQc() { return highestPrepareQc; }
    public TimeoutCertificate lastTimeoutCertificate() { return lastTimeoutCertificate; }
    public BodyState bodyState() { return bodyState; }
    public BigInteger pendingPersistenceId() { return pendingPersistenceId; }
    public BigInteger lastCommittedHeight() { return lastCommittedHeight; }
    public BlockSubject lastCommittedSubject() { return lastCommittedSubject; }
    public HeightContext heightContext() { return heightContext; }
    public CommitQc lastCommitQc() { return lastCommitQc; }
    public Liveness liveness() { return liveness; }

    /** Parse fatal UTF-8, duplicate-key rejecting authoritative status JSON. */
    public static SumeragiV2Status parseJson(final byte[] payload) {
      require(payload != null && payload.length > 0, "Sumeragi status response must not be empty");
      require(
          (long) payload.length <= STATUS_JSON_MAX_BYTES,
          "Sumeragi status response exceeds " + STATUS_JSON_MAX_BYTES + " bytes");
      return SumeragiStatusModels.parseStatus(
          SumeragiJsonSupport.decodeUtf8(payload, "Sumeragi status"));
    }

    /** Parse duplicate-key rejecting authoritative status JSON. */
    public static SumeragiV2Status parseJson(final String payload) {
      return SumeragiStatusModels.parseStatus(payload);
    }
  }

  /** Parse a strict authoritative Sumeragi status response. */
  public static SumeragiV2Status parseStatus(final byte[] payload) {
    return SumeragiV2Status.parseJson(payload);
  }

  /** Parse a strict authoritative Sumeragi status response. */
  public static SumeragiV2Status parseStatus(final String payload) {
    final String context = "Sumeragi status";
    final Map<String, Object> root = SumeragiJsonSupport.parseObject(payload, context);
    SumeragiJsonSupport.requireFields(
        root,
        Set.of(
            "protocol_version", "node_fingerprint", "build_fingerprint", "config_fingerprint",
            "restart_required", "height_context_id", "height", "view", "phase", "leader",
            "body_state", "last_committed_height", "height_context", "liveness"),
        Set.of(
            "locked_prepare_qc", "highest_prepare_qc", "last_timeout_certificate",
            "pending_persistence_id", "last_committed_subject", "last_commit_qc"),
        context);
    final int protocolVersion =
        SumeragiJsonSupport.u16(root.get("protocol_version"), context + ".protocol_version");
    require(
        protocolVersion == STATUS_PROTOCOL_VERSION,
        context + ".protocol_version must equal " + STATUS_PROTOCOL_VERSION);
    final ContextId heightContextId =
        parseContextId(root.get("height_context_id"), context + ".height_context_id");
    final BigInteger height =
        SumeragiJsonSupport.positiveU64(root.get("height"), context + ".height");
    final BigInteger view = SumeragiJsonSupport.u64(root.get("view"), context + ".view");
    final Phase phase = parseTagged(root.get("phase"), "phase", context + ".phase", Phase.values());
    final BigInteger leader = SumeragiJsonSupport.u32(root.get("leader"), context + ".leader");
    final BodyState bodyState =
        parseTagged(root.get("body_state"), "state", context + ".body_state", BodyState.values());
    final BigInteger pendingPersistenceId =
        root.get("pending_persistence_id") == null
            ? null
            : SumeragiJsonSupport.positiveU64(
                root.get("pending_persistence_id"), context + ".pending_persistence_id");
    final HeightContext heightContext =
        parseHeightContext(root.get("height_context"), context + ".height_context");
    require(
        heightContext.epochEndHeight().compareTo(height) >= 0,
        context + " height context must cover the active height");
    require(
        leader.compareTo(heightContext.validatorCount()) < 0,
        context + " leader must index the frozen validator roster");

    final QcReference lockedPrepareQc =
        root.get("locked_prepare_qc") == null
            ? null
            : parseQcReference(root.get("locked_prepare_qc"), context + ".locked_prepare_qc");
    final QcReference highestPrepareQc =
        root.get("highest_prepare_qc") == null
            ? null
            : parseQcReference(root.get("highest_prepare_qc"), context + ".highest_prepare_qc");
    final TimeoutCertificate lastTimeoutCertificate =
        root.get("last_timeout_certificate") == null
            ? null
            : parseTimeoutCertificate(
                root.get("last_timeout_certificate"), context + ".last_timeout_certificate");
    final BigInteger lastCommittedHeight =
        SumeragiJsonSupport.u64(
            root.get("last_committed_height"), context + ".last_committed_height");
    final BlockSubject lastCommittedSubject =
        root.get("last_committed_subject") == null
            ? null
            : parseBlockSubject(
                root.get("last_committed_subject"), context + ".last_committed_subject");
    final CommitQc lastCommitQc =
        root.get("last_commit_qc") == null
            ? null
            : parseCommitQc(root.get("last_commit_qc"), context + ".last_commit_qc");
    final Liveness liveness =
        parseLiveness(
            root.get("liveness"), height, view, heightContextId, heightContext,
            context + ".liveness");
    validatePhaseAndFrontier(
        phase, bodyState, height, lockedPrepareQc, highestPrepareQc, lastTimeoutCertificate,
        lastCommittedHeight, lastCommittedSubject, lastCommitQc, view, heightContextId,
        heightContext);

    return new SumeragiV2Status(
        protocolVersion,
        SumeragiJsonSupport.hash(root.get("node_fingerprint"), context + ".node_fingerprint"),
        SumeragiJsonSupport.hash(root.get("build_fingerprint"), context + ".build_fingerprint"),
        SumeragiJsonSupport.hash(root.get("config_fingerprint"), context + ".config_fingerprint"),
        SumeragiJsonSupport.bool(root.get("restart_required"), context + ".restart_required"),
        heightContextId,
        height,
        view,
        phase,
        leader,
        lockedPrepareQc,
        highestPrepareQc,
        lastTimeoutCertificate,
        bodyState,
        pendingPersistenceId,
        lastCommittedHeight,
        lastCommittedSubject,
        heightContext,
        lastCommitQc,
        liveness);
  }

  private static void validatePhaseAndFrontier(
      final Phase phase,
      final BodyState bodyState,
      final BigInteger height,
      final QcReference lockedPrepareQc,
      final QcReference highestPrepareQc,
      final TimeoutCertificate lastTimeoutCertificate,
      final BigInteger lastCommittedHeight,
      final BlockSubject lastCommittedSubject,
      final CommitQc lastCommitQc,
      final BigInteger view,
      final ContextId activeContextId,
      final HeightContext activeHeightContext) {
    final boolean phaseBodyValid;
    switch (phase) {
      case AWAITING_PROPOSAL:
        phaseBodyValid = bodyState == BodyState.MISSING;
        break;
      case RECONSTRUCTING_PAYLOAD:
        phaseBodyValid = bodyState == BodyState.RECONSTRUCTING;
        break;
      case VALIDATING_PAYLOAD:
        phaseBodyValid = bodyState == BodyState.STORED;
        break;
      case PREPARE:
      case COMMIT:
        phaseBodyValid = bodyState == BodyState.VALIDATED;
        break;
      case PENDING_APPLY:
        phaseBodyValid =
            bodyState == BodyState.PENDING_APPLY || bodyState == BodyState.APPLIED;
        break;
      default:
        throw new IllegalArgumentException("unsupported Sumeragi status phase");
    }
    require(phaseBodyValid, "Sumeragi status phase and body state are inconsistent");
    require(
        phase != Phase.COMMIT || lockedPrepareQc != null,
        "Sumeragi status commit phase requires a PrepareQC lock");
    require(
        phase != Phase.PREPARE || lockedPrepareQc == null,
        "Sumeragi status prepare phase cannot carry a PrepareQC lock");
    if (phase == Phase.PENDING_APPLY) {
      require(
          lastCommittedHeight.equals(height)
              && lastCommittedSubject != null
              && lastCommitQc != null,
          "pending-apply status must carry the current decided height and subject");
    } else {
      require(
          lastCommittedHeight.compareTo(height) < 0,
          "non-decided Sumeragi status must have a committed height below the active height");
    }
    require(
        lastCommittedHeight.signum() != 0
            || (lastCommittedSubject == null && lastCommitQc == null),
        "pre-genesis commit frontier cannot carry a subject or CommitQC");
    require(
        (lastCommittedSubject == null) == (lastCommitQc == null),
        "Sumeragi status committed subject and CommitQC must be paired");
    if (lastCommitQc != null) {
      final QcReference certificate = lastCommitQc.certificate();
      require(
          certificate.phase() == GlobalPhase.COMMIT
              && certificate.round().height().equals(lastCommittedHeight)
              && sameSubject(certificate.subject(), lastCommittedSubject),
          "Sumeragi status CommitQC does not certify the committed frontier");
      if (lastCommittedHeight.equals(height)) {
        require(
            sameContext(certificate.round().contextId(), activeContextId),
            "Sumeragi status CommitQC context does not match the active context");
        require(
            lastCommitQc.validatorCount().equals(activeHeightContext.validatorCount())
                && lastCommitQc.minSigners().equals(activeHeightContext.quorum().minSigners())
                && lastCommitQc.totalPower().equals(activeHeightContext.quorum().totalPower()),
            "Sumeragi status CommitQC quorum differs from the active height context");
      }
    }

    if (lockedPrepareQc != null) {
      validatePrepareReference(lockedPrepareQc, height, view, activeContextId);
    }
    if (highestPrepareQc != null) {
      validatePrepareReference(highestPrepareQc, height, view, activeContextId);
    }
    require(
        lockedPrepareQc == null || highestPrepareQc != null,
        "Sumeragi status lock requires a highest PrepareQC");
    if (lockedPrepareQc != null) {
      require(
          lockedPrepareQc.round().view().compareTo(highestPrepareQc.round().view()) <= 0,
          "Sumeragi status lock is above its highest PrepareQC");
      require(
          !lockedPrepareQc.round().view().equals(highestPrepareQc.round().view())
              || sameQc(lockedPrepareQc, highestPrepareQc),
          "Sumeragi status lock and highest PrepareQC conflict at the same view");
    }
    if (lastTimeoutCertificate != null) {
      final Round timeoutRound = lastTimeoutCertificate.round();
      require(
          sameContext(timeoutRound.contextId(), activeContextId),
          "Sumeragi status timeout context does not match the active context");
      require(
          timeoutRound.height().equals(height),
          "Sumeragi status timeout height does not match the active height");
      require(
          timeoutRound.view().compareTo(view) < 0,
          "Sumeragi status timeout certificate must precede the current view");
      if (lastTimeoutCertificate.highestPrepareQc() != null) {
        final QcReference highest = lastTimeoutCertificate.highestPrepareQc();
        validatePrepareReference(highest, height, view, activeContextId);
        require(
            highest.round().view().compareTo(timeoutRound.view()) <= 0,
            "Sumeragi status timeout certificate carries a future PrepareQC");
      }
    }
  }

  private static void validatePrepareReference(
      final QcReference reference,
      final BigInteger height,
      final BigInteger view,
      final ContextId activeContextId) {
    require(
        sameContext(reference.round().contextId(), activeContextId),
        "Sumeragi status certificate context does not match the active context");
    require(
        reference.round().height().equals(height),
        "Sumeragi status certificate height does not match the active height");
    require(
        reference.phase() == GlobalPhase.PREPARE,
        "Sumeragi status QC reference must be a PrepareQC");
    require(
        reference.round().view().compareTo(view) <= 0,
        "Sumeragi status QC reference is from a future view");
  }

  private static ContextId parseContextId(final Object value, final String context) {
    final List<?> tuple = SumeragiJsonSupport.array(value, context, 1);
    require(tuple.size() == 1, context + " must contain exactly one hash");
    return new ContextId(SumeragiJsonSupport.hash(tuple.get(0), context + "[0]"));
  }

  private static Round parseRound(final Object value, final String context) {
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(
            value, Set.of("context_id", "height", "view"), context);
    return new Round(
        parseContextId(record.get("context_id"), context + ".context_id"),
        SumeragiJsonSupport.u64(record.get("height"), context + ".height"),
        SumeragiJsonSupport.u64(record.get("view"), context + ".view"));
  }

  private static BlockSubject parseBlockSubject(final Object value, final String context) {
    final Map<String, Object> record = SumeragiJsonSupport.object(value, context);
    SumeragiJsonSupport.requireFields(
        record, Set.of("block_hash", "payload_hash"), Set.of("parent_block_hash"), context);
    return new BlockSubject(
        record.get("parent_block_hash") == null
            ? null
            : SumeragiJsonSupport.hash(
                record.get("parent_block_hash"), context + ".parent_block_hash"),
        SumeragiJsonSupport.hash(record.get("block_hash"), context + ".block_hash"),
        SumeragiJsonSupport.hash(record.get("payload_hash"), context + ".payload_hash"));
  }

  private static ExecutionCommitment parseExecutionCommitment(
      final Object value, final String context) {
    final Map<String, Object> record = SumeragiJsonSupport.object(value, context);
    SumeragiJsonSupport.requireFields(
        record,
        Set.of(
            "parent_state_root", "post_state_root", "ordinary_writes_root",
            "kagemusha_top_up_count", "native_amx_application_manifest_version",
            "native_amx_application_manifest_root", "native_amx_application_manifest_count",
            "lane_finality_manifest", "merge_carrier", "executed_block_wire_len",
            "executed_block_wire_hash"),
        Set.of("kagemusha_top_up_root"),
        context);
    final BigInteger kagemushaTopUpCount =
        SumeragiJsonSupport.u32(
            record.get("kagemusha_top_up_count"),
            context + ".kagemusha_top_up_count");
    final String kagemushaTopUpRoot =
        record.get("kagemusha_top_up_root") == null
            ? null
            : SumeragiJsonSupport.hash(
                record.get("kagemusha_top_up_root"),
                context + ".kagemusha_top_up_root");
    require(
        (kagemushaTopUpCount.signum() == 0) == (kagemushaTopUpRoot == null),
        context
            + ".kagemusha_top_up_root must be present exactly when "
            + "kagemusha_top_up_count is positive");
    final int manifestVersion =
        SumeragiJsonSupport.u16(
            record.get("native_amx_application_manifest_version"),
            context + ".native_amx_application_manifest_version");
    require(
        manifestVersion == NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        context + ".native_amx_application_manifest_version must equal "
            + NATIVE_AMX_APPLICATION_MANIFEST_VERSION);
    final String manifestRoot =
        SumeragiJsonSupport.hash(
            record.get("native_amx_application_manifest_root"),
            context + ".native_amx_application_manifest_root");
    final BigInteger manifestCount =
        SumeragiJsonSupport.unsigned(
            record.get("native_amx_application_manifest_count"),
            BigInteger.valueOf(NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES),
            context + ".native_amx_application_manifest_count");
    require(
        (manifestCount.signum() == 0)
            == NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT.equals(manifestRoot),
        context + ".native_amx_application_manifest_count must be zero exactly for the canonical empty root");

    final LaneFinalityManifestCommitment laneFinalityManifest;
    if (record.get("lane_finality_manifest") == null) {
      laneFinalityManifest = null;
    } else {
      final String laneContext = context + ".lane_finality_manifest";
      final Map<String, Object> lane =
          SumeragiJsonSupport.exactObject(
              record.get("lane_finality_manifest"), Set.of("root", "leaf_count"), laneContext);
      laneFinalityManifest =
          new LaneFinalityManifestCommitment(
              SumeragiJsonSupport.hash(lane.get("root"), laneContext + ".root"),
              SumeragiJsonSupport.unsigned(
                  lane.get("leaf_count"),
                  BigInteger.valueOf(LANE_FINALITY_MANIFEST_MAX_LEAVES),
                  laneContext + ".leaf_count",
                  true));
    }
    final MergeCarrierCommitment mergeCarrier;
    if (record.get("merge_carrier") == null) {
      mergeCarrier = null;
    } else {
      final String mergeContext = context + ".merge_carrier";
      final Map<String, Object> merge =
          SumeragiJsonSupport.exactObject(
              record.get("merge_carrier"), Set.of("version", "entry_hash"), mergeContext);
      final int version =
          SumeragiJsonSupport.u16(merge.get("version"), mergeContext + ".version");
      require(
          version == MERGE_CARRIER_COMMITMENT_VERSION,
          mergeContext + ".version must equal " + MERGE_CARRIER_COMMITMENT_VERSION);
      mergeCarrier =
          new MergeCarrierCommitment(
              version,
              SumeragiJsonSupport.hash(merge.get("entry_hash"), mergeContext + ".entry_hash"));
    }
    return new ExecutionCommitment(
        SumeragiJsonSupport.hash(record.get("parent_state_root"), context + ".parent_state_root"),
        SumeragiJsonSupport.hash(record.get("post_state_root"), context + ".post_state_root"),
        SumeragiJsonSupport.hash(
            record.get("ordinary_writes_root"), context + ".ordinary_writes_root"),
        kagemushaTopUpRoot,
        kagemushaTopUpCount,
        manifestVersion,
        manifestRoot,
        manifestCount,
        laneFinalityManifest,
        mergeCarrier,
        SumeragiJsonSupport.positiveU64(
            record.get("executed_block_wire_len"), context + ".executed_block_wire_len"),
        SumeragiJsonSupport.hash(
            record.get("executed_block_wire_hash"), context + ".executed_block_wire_hash"));
  }

  private static QcReference parseQcReference(final Object value, final String context) {
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(
            value,
            Set.of("round", "proposal_round", "phase", "subject", "execution_commitment"),
            context);
    final Round round = parseRound(record.get("round"), context + ".round");
    final Round proposalRound =
        parseRound(record.get("proposal_round"), context + ".proposal_round");
    require(sameRound(round, proposalRound), context + ".proposal_round must equal round");
    return new QcReference(
        round,
        proposalRound,
        parseTagged(record.get("phase"), "phase", context + ".phase", GlobalPhase.values()),
        parseBlockSubject(record.get("subject"), context + ".subject"),
        parseExecutionCommitment(
            record.get("execution_commitment"), context + ".execution_commitment"));
  }

  private static TimeoutCertificate parseTimeoutCertificate(
      final Object value, final String context) {
    final Map<String, Object> record = SumeragiJsonSupport.object(value, context);
    SumeragiJsonSupport.requireFields(
        record, Set.of("round", "certificate_hash"), Set.of("highest_prepare_qc"), context);
    return new TimeoutCertificate(
        parseRound(record.get("round"), context + ".round"),
        record.get("highest_prepare_qc") == null
            ? null
            : parseQcReference(
                record.get("highest_prepare_qc"), context + ".highest_prepare_qc"),
        SumeragiJsonSupport.hash(
            record.get("certificate_hash"), context + ".certificate_hash"));
  }

  private static HeightContext parseHeightContext(final Object value, final String context) {
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(
            value,
            Set.of("epoch", "epoch_end_height", "mode", "epoch_seed", "validator_count", "quorum"),
            context);
    final BigInteger validatorCount =
        SumeragiJsonSupport.unsigned(
            record.get("validator_count"), THIRTY_ONE, context + ".validator_count", true);
    require(
        validatorCount.compareTo(BigInteger.valueOf(4L)) >= 0
            && validatorCount.subtract(BigInteger.ONE).mod(THREE).signum() == 0,
        context + ".validator_count must have bounded 3f + 1 geometry");
    final Map<String, Object> quorum =
        SumeragiJsonSupport.exactObject(
            record.get("quorum"), Set.of("min_signers", "total_power"), context + ".quorum");
    final BigInteger minSigners =
        SumeragiJsonSupport.unsigned(
            quorum.get("min_signers"), THIRTY_ONE, context + ".quorum.min_signers", true);
    final BigInteger totalPower =
        SumeragiJsonSupport.positiveU64(
            quorum.get("total_power"), context + ".quorum.total_power");
    final BigInteger canonicalMinSigners =
        validatorCount.multiply(TWO).divide(THREE).add(BigInteger.ONE);
    require(
        minSigners.equals(canonicalMinSigners) && totalPower.equals(validatorCount),
        context + ".quorum is not canonical for validator_count");
    return new HeightContext(
        SumeragiJsonSupport.u64(record.get("epoch"), context + ".epoch"),
        SumeragiJsonSupport.u64(
            record.get("epoch_end_height"), context + ".epoch_end_height"),
        parseTagged(record.get("mode"), "mode", context + ".mode", ConsensusMode.values()),
        SumeragiJsonSupport.byte32(record.get("epoch_seed"), context + ".epoch_seed"),
        validatorCount,
        new Quorum(minSigners, totalPower));
  }

  private static CommitQc parseCommitQc(final Object value, final String context) {
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(
            value,
            Set.of(
                "certificate", "validator_count", "signer_count", "min_signers",
                "signed_power", "total_power"),
            context);
    final BigInteger validatorCount =
        SumeragiJsonSupport.unsigned(
            record.get("validator_count"), THIRTY_ONE, context + ".validator_count", true);
    require(
        validatorCount.compareTo(BigInteger.valueOf(4L)) >= 0
            && validatorCount.subtract(BigInteger.ONE).mod(THREE).signum() == 0,
        context + ".validator_count must have bounded 3f + 1 geometry");
    final BigInteger signerCount =
        SumeragiJsonSupport.unsigned(
            record.get("signer_count"), validatorCount, context + ".signer_count");
    final BigInteger minSigners =
        SumeragiJsonSupport.unsigned(
            record.get("min_signers"), THIRTY_ONE, context + ".min_signers", true);
    final BigInteger signedPower =
        SumeragiJsonSupport.u64(record.get("signed_power"), context + ".signed_power");
    final BigInteger totalPower =
        SumeragiJsonSupport.positiveU64(record.get("total_power"), context + ".total_power");
    final BigInteger canonicalMinSigners =
        validatorCount.multiply(TWO).divide(THREE).add(BigInteger.ONE);
    require(
        signerCount.equals(minSigners)
            && minSigners.equals(canonicalMinSigners)
            && signedPower.equals(signerCount)
            && totalPower.equals(validatorCount)
            && signedPower.multiply(THREE).compareTo(totalPower.multiply(TWO)) > 0,
        context + " does not satisfy its exact frozen certificate quorum");
    return new CommitQc(
        parseQcReference(record.get("certificate"), context + ".certificate"),
        validatorCount,
        signerCount,
        minSigners,
        signedPower,
        totalPower);
  }

  private static Liveness parseLiveness(
      final Object value,
      final BigInteger activeHeight,
      final BigInteger activeView,
      final ContextId activeContextId,
      final HeightContext heightContext,
      final String context) {
    final Map<String, Object> record = SumeragiJsonSupport.object(value, context);
    SumeragiJsonSupport.requireFields(
        record,
        Set.of(
            "generation", "prepare_quorums", "commit_quorums", "timeout_quorums",
            "outbound_intents", "work", "queues", "no_progress_age_ms", "ignore_counts"),
        Set.of("last_progress", "blocker"),
        context);
    final BigInteger generation =
        SumeragiJsonSupport.u64(record.get("generation"), context + ".generation");

    final List<VoteQuorum> prepareQuorums =
        parseVoteQuorums(
            record.get("prepare_quorums"), 31, activeHeight, activeView, activeContextId,
            heightContext, context + ".prepare_quorums");
    final List<VoteQuorum> commitQuorums =
        parseVoteQuorums(
            record.get("commit_quorums"), 32, activeHeight, activeView, activeContextId,
            heightContext, context + ".commit_quorums");

    final List<?> rawTimeoutQuorums =
        SumeragiJsonSupport.array(record.get("timeout_quorums"), context + ".timeout_quorums", 31);
    final ArrayList<TimeoutQuorum> timeoutQuorums = new ArrayList<>();
    for (int index = 0; index < rawTimeoutQuorums.size(); index++) {
      final String itemContext = context + ".timeout_quorums[" + index + "]";
      final Map<String, Object> item =
          SumeragiJsonSupport.exactObject(
              rawTimeoutQuorums.get(index),
              Set.of(
                  "round", "signer_count", "signed_power", "min_signers", "total_power",
                  "certificate_formed"),
              itemContext);
      final BigInteger[] fields = partialQuorumFields(item, heightContext, itemContext);
      final boolean certificateFormed =
          SumeragiJsonSupport.bool(
              item.get("certificate_formed"), itemContext + ".certificate_formed");
      if (certificateFormed) {
        require(
            fields[0].compareTo(fields[2]) >= 0
                && fields[1].multiply(THREE).compareTo(fields[3].multiply(TWO)) > 0,
            itemContext + " does not form its advertised dual quorum");
      }
      timeoutQuorums.add(
          new TimeoutQuorum(
              parseBoundRound(
                  item.get("round"), activeHeight, activeView, activeContextId,
                  itemContext + ".round", true),
              fields[0], fields[1], fields[2], fields[3], certificateFormed));
    }

    final Set<OutboundIntentKind> proposalKinds =
        Set.of(
            OutboundIntentKind.PROPOSAL,
            OutboundIntentKind.PREPARE_VOTE,
            OutboundIntentKind.COMMIT_VOTE,
            OutboundIntentKind.PREPARE_QC,
            OutboundIntentKind.COMMIT_QC);
    final List<?> rawOutboundIntents =
        SumeragiJsonSupport.array(record.get("outbound_intents"), context + ".outbound_intents", 7);
    final ArrayList<OutboundIntent> outboundIntents = new ArrayList<>();
    for (int index = 0; index < rawOutboundIntents.size(); index++) {
      final String itemContext = context + ".outbound_intents[" + index + "]";
      final Map<String, Object> item =
          SumeragiJsonSupport.object(rawOutboundIntents.get(index), itemContext);
      SumeragiJsonSupport.requireFields(
          item,
          Set.of("kind", "round", "stage"),
          Set.of("proposal_round", "subject", "execution_commitment"),
          itemContext);
      final OutboundIntentKind kind =
          parseTagged(
              item.get("kind"), "kind", itemContext + ".kind", OutboundIntentKind.values());
      final OutboundIntentStage stage =
          parseTagged(
              item.get("stage"), "stage", itemContext + ".stage",
              OutboundIntentStage.values());
      final Round intentRound =
          parseBoundRound(
              item.get("round"), activeHeight, activeView, activeContextId,
              itemContext + ".round", kind != OutboundIntentKind.COMMIT_QC);
      final Round proposalRound =
          item.get("proposal_round") == null
              ? null
              : parseBoundRound(
                  item.get("proposal_round"), activeHeight, activeView, activeContextId,
                  itemContext + ".proposal_round", false);
      require(
          proposalKinds.contains(kind) == (proposalRound != null),
          itemContext + " has inconsistent proposal_round for " + kind.wireName());
      if (proposalRound != null) {
        require(
            sameRound(intentRound, proposalRound),
            itemContext + ".proposal_round must equal round");
      }
      final BlockSubject subject =
          item.get("subject") == null
              ? null
              : parseBlockSubject(item.get("subject"), itemContext + ".subject");
      final ExecutionCommitment commitment =
          item.get("execution_commitment") == null
              ? null
              : parseExecutionCommitment(
                  item.get("execution_commitment"), itemContext + ".execution_commitment");
      final boolean validShape;
      switch (kind) {
        case PROPOSAL:
          validShape = subject != null && commitment == null;
          break;
        case TIMEOUT_VOTE:
        case TIMEOUT_CERTIFICATE:
          validShape = subject == null && commitment == null;
          break;
        default:
          validShape = subject != null && commitment != null;
          break;
      }
      require(validShape, itemContext + " has inconsistent proposal fields");
      outboundIntents.add(
          new OutboundIntent(kind, intentRound, proposalRound, subject, commitment, stage));
    }

    final Map<String, Object> workRecord =
        SumeragiJsonSupport.exactObject(
            record.get("work"),
            Set.of(
                "candidate", "body_recovery", "body_store", "validation", "application",
                "successor_height"),
            context + ".work");
    final Work work =
        new Work(
            parseWorkStage(workRecord, "candidate", context),
            parseWorkStage(workRecord, "body_recovery", context),
            parseWorkStage(workRecord, "body_store", context),
            parseWorkStage(workRecord, "validation", context),
            parseWorkStage(workRecord, "application", context),
            parseWorkStage(workRecord, "successor_height", context));

    final List<?> rawQueues =
        SumeragiJsonSupport.array(record.get("queues"), context + ".queues", 10);
    final ArrayList<Queue> queues = new ArrayList<>();
    final Set<QueueKind> seenQueues = new HashSet<>();
    for (int index = 0; index < rawQueues.size(); index++) {
      final String itemContext = context + ".queues[" + index + "]";
      final Map<String, Object> item = SumeragiJsonSupport.object(rawQueues.get(index), itemContext);
      SumeragiJsonSupport.requireFields(
          item,
          Set.of("queue", "depth", "capacity", "service_debt"),
          Set.of("oldest_age_ms"),
          itemContext);
      final QueueKind kind =
          parseTagged(item.get("queue"), "queue", itemContext + ".queue", QueueKind.values());
      require(seenQueues.add(kind), itemContext + ".queue is duplicated");
      final BigInteger depth =
          SumeragiJsonSupport.u32(item.get("depth"), itemContext + ".depth");
      final BigInteger capacity =
          SumeragiJsonSupport.positiveU32(item.get("capacity"), itemContext + ".capacity");
      final BigInteger oldestAge =
          item.get("oldest_age_ms") == null
              ? null
              : SumeragiJsonSupport.u64(
                  item.get("oldest_age_ms"), itemContext + ".oldest_age_ms");
      require(
          depth.compareTo(capacity) <= 0
              && (depth.signum() == 0) == (oldestAge == null),
          itemContext + " has inconsistent occupancy and age");
      queues.add(
          new Queue(
              kind,
              depth,
              capacity,
              oldestAge,
              SumeragiJsonSupport.u64(
                  item.get("service_debt"), itemContext + ".service_debt")));
    }

    final Progress lastProgress;
    if (record.get("last_progress") == null) {
      lastProgress = null;
    } else {
      final String itemContext = context + ".last_progress";
      final Map<String, Object> item =
          SumeragiJsonSupport.exactObject(
              record.get("last_progress"),
              Set.of("generation", "round", "transition", "age_ms"),
              itemContext);
      final BigInteger progressGeneration =
          SumeragiJsonSupport.u64(item.get("generation"), itemContext + ".generation");
      require(
          progressGeneration.compareTo(generation) <= 0,
          itemContext + ".generation is from the future");
      final Round progressRound =
          parseBoundRound(
              item.get("round"), activeHeight, activeView, activeContextId,
              itemContext + ".round", false);
      final ProgressTransition transition =
          parseTagged(
              item.get("transition"), "transition", itemContext + ".transition",
              ProgressTransition.values());
      final boolean permitsFutureView =
          transition == ProgressTransition.COMMIT_QUORUM
              || transition == ProgressTransition.DECISION_PERSISTED;
      require(
          progressRound.view().compareTo(activeView) <= 0 || permitsFutureView,
          itemContext + ".round.view must not exceed the active view");
      lastProgress =
          new Progress(
              progressGeneration,
              progressRound,
              transition,
              SumeragiJsonSupport.u64(item.get("age_ms"), itemContext + ".age_ms"));
    }

    final LivenessBlocker blocker =
        record.get("blocker") == null
            ? null
            : parseTagged(
                record.get("blocker"), "blocker", context + ".blocker",
                LivenessBlocker.values());
    final List<?> rawIgnoreCounts =
        SumeragiJsonSupport.array(record.get("ignore_counts"), context + ".ignore_counts", 12);
    final ArrayList<IgnoreCount> ignoreCounts = new ArrayList<>();
    final Set<IgnoreReason> seenReasons = new HashSet<>();
    for (int index = 0; index < rawIgnoreCounts.size(); index++) {
      final String itemContext = context + ".ignore_counts[" + index + "]";
      final Map<String, Object> item =
          SumeragiJsonSupport.exactObject(
              rawIgnoreCounts.get(index), Set.of("reason", "count"), itemContext);
      final IgnoreReason reason =
          parseTagged(
              item.get("reason"), "reason", itemContext + ".reason", IgnoreReason.values());
      require(seenReasons.add(reason), itemContext + ".reason is duplicated");
      ignoreCounts.add(
          new IgnoreCount(
              reason,
              SumeragiJsonSupport.u64(item.get("count"), itemContext + ".count")));
    }
    return new Liveness(
        generation,
        prepareQuorums,
        commitQuorums,
        timeoutQuorums,
        outboundIntents,
        work,
        queues,
        lastProgress,
        SumeragiJsonSupport.u64(
            record.get("no_progress_age_ms"), context + ".no_progress_age_ms"),
        blocker,
        ignoreCounts);
  }

  private static List<VoteQuorum> parseVoteQuorums(
      final Object value,
      final int maximum,
      final BigInteger activeHeight,
      final BigInteger activeView,
      final ContextId activeContextId,
      final HeightContext heightContext,
      final String context) {
    final List<?> raw = SumeragiJsonSupport.array(value, context, maximum);
    final ArrayList<VoteQuorum> result = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      final String itemContext = context + "[" + index + "]";
      final Map<String, Object> item =
          SumeragiJsonSupport.exactObject(
              raw.get(index),
              Set.of(
                  "round", "proposal_round", "subject", "execution_commitment",
                  "signer_count", "signed_power", "min_signers", "total_power"),
              itemContext);
      final Round round =
          parseBoundRound(
              item.get("round"), activeHeight, activeView, activeContextId,
              itemContext + ".round", true);
      final Round proposalRound =
          parseBoundRound(
              item.get("proposal_round"), activeHeight, activeView, activeContextId,
              itemContext + ".proposal_round", true);
      require(sameRound(round, proposalRound), itemContext + ".proposal_round must equal round");
      final BigInteger[] fields = partialQuorumFields(item, heightContext, itemContext);
      result.add(
          new VoteQuorum(
              round,
              proposalRound,
              parseBlockSubject(item.get("subject"), itemContext + ".subject"),
              parseExecutionCommitment(
                  item.get("execution_commitment"), itemContext + ".execution_commitment"),
              fields[0], fields[1], fields[2], fields[3]));
    }
    return result;
  }

  private static BigInteger[] partialQuorumFields(
      final Map<String, Object> item,
      final HeightContext heightContext,
      final String context) {
    final BigInteger signerCount =
        SumeragiJsonSupport.unsigned(
            item.get("signer_count"), heightContext.validatorCount(), context + ".signer_count");
    final BigInteger signedPower =
        SumeragiJsonSupport.u64(item.get("signed_power"), context + ".signed_power");
    final BigInteger minSigners =
        SumeragiJsonSupport.unsigned(
            item.get("min_signers"), heightContext.validatorCount(), context + ".min_signers");
    final BigInteger totalPower =
        SumeragiJsonSupport.positiveU64(item.get("total_power"), context + ".total_power");
    require(
        minSigners.equals(heightContext.quorum().minSigners())
            && totalPower.equals(heightContext.quorum().totalPower())
            && signedPower.equals(signerCount),
        context + " disagrees with the frozen dual quorum");
    return new BigInteger[] {signerCount, signedPower, minSigners, totalPower};
  }

  private static Round parseBoundRound(
      final Object value,
      final BigInteger activeHeight,
      final BigInteger activeView,
      final ContextId activeContextId,
      final String context,
      final boolean requireNonFuture) {
    final Round parsed = parseRound(value, context);
    require(
        sameContext(parsed.contextId(), activeContextId)
            && parsed.height().equals(activeHeight),
        context + " must match the active height context");
    require(
        !requireNonFuture || parsed.view().compareTo(activeView) <= 0,
        context + ".view must not exceed the active view");
    return parsed;
  }

  private static WorkStage parseWorkStage(
      final Map<String, Object> work, final String field, final String context) {
    return parseTagged(
        work.get(field), "stage", context + ".work." + field, WorkStage.values());
  }

  private static <E extends Enum<E> & WireEnum> E parseTagged(
      final Object value, final String tag, final String context, final E[] values) {
    final String wireName = SumeragiJsonSupport.taggedUnit(value, tag, context);
    for (final E item : values) {
      if (item.wireName().equals(wireName)) {
        return item;
      }
    }
    throw new IllegalArgumentException(context + "." + tag + " is not a supported v4 variant");
  }

  private static boolean sameContext(final ContextId left, final ContextId right) {
    return left.hash().equals(right.hash());
  }

  private static boolean sameRound(final Round left, final Round right) {
    return sameContext(left.contextId(), right.contextId())
        && left.height().equals(right.height())
        && left.view().equals(right.view());
  }

  private static boolean sameSubject(final BlockSubject left, final BlockSubject right) {
    return Objects.equals(left.parentBlockHash(), right.parentBlockHash())
        && left.blockHash().equals(right.blockHash())
        && left.payloadHash().equals(right.payloadHash());
  }

  private static boolean sameCommitment(
      final ExecutionCommitment left, final ExecutionCommitment right) {
    return left.parentStateRoot().equals(right.parentStateRoot())
        && left.postStateRoot().equals(right.postStateRoot())
        && left.ordinaryWritesRoot().equals(right.ordinaryWritesRoot())
        && Objects.equals(left.kagemushaTopUpRoot(), right.kagemushaTopUpRoot())
        && left.kagemushaTopUpCount().equals(right.kagemushaTopUpCount())
        && left.nativeAmxApplicationManifestVersion()
            == right.nativeAmxApplicationManifestVersion()
        && left.nativeAmxApplicationManifestRoot()
            .equals(right.nativeAmxApplicationManifestRoot())
        && left.nativeAmxApplicationManifestCount()
            .equals(right.nativeAmxApplicationManifestCount())
        && sameLaneFinalityManifest(
            left.laneFinalityManifest(), right.laneFinalityManifest())
        && sameMergeCarrier(left.mergeCarrier(), right.mergeCarrier())
        && left.executedBlockWireLen().equals(right.executedBlockWireLen())
        && left.executedBlockWireHash().equals(right.executedBlockWireHash());
  }

  private static boolean sameLaneFinalityManifest(
      final LaneFinalityManifestCommitment left,
      final LaneFinalityManifestCommitment right) {
    return left == null
        ? right == null
        : right != null
            && left.root().equals(right.root())
            && left.leafCount().equals(right.leafCount());
  }

  private static boolean sameMergeCarrier(
      final MergeCarrierCommitment left, final MergeCarrierCommitment right) {
    return left == null
        ? right == null
        : right != null && left.version() == right.version()
            && left.entryHash().equals(right.entryHash());
  }

  private static boolean sameQc(final QcReference left, final QcReference right) {
    return sameRound(left.round(), right.round())
        && sameRound(left.proposalRound(), right.proposalRound())
        && left.phase() == right.phase()
        && sameSubject(left.subject(), right.subject())
        && sameCommitment(left.executionCommitment(), right.executionCommitment());
  }

  private static <T> List<T> immutableCopy(final List<T> values) {
    return Collections.unmodifiableList(new ArrayList<>(values));
  }

  private static void require(final boolean condition, final String message) {
    SumeragiJsonSupport.require(condition, message);
  }
}
