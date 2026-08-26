// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Mirrored public models for Sumeragi operational diagnostics. */
public final class SumeragiDiagnosticsModels {
  /** Maximum rows in {@code native_amx_participant_applications}. */
  public static final int NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX = 1_024;

  /** Maximum grouped source count represented by one diagnostics row. */
  public static final long NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX = 4_096L;
  public static final int AUTONOMOUS_LANE_EXECUTIONS_MAX = 128;
  public static final int DIAGNOSTIC_LANES_MAX = 128;

  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);
  private static final Pattern CANONICAL_HASH =
      Pattern.compile("^hash:[0-9A-F]{64}#[0-9A-F]{4}$");

  private SumeragiDiagnosticsModels() {}

  /** Aggregate execution diagnostics for the latest block-pipeline run. */
  public static final class PipelineExecutionStatus {
    private final BigInteger txVerticesTotal;
    private final BigInteger txEdgesTotal;
    private final BigInteger overlayCountTotal;
    private final BigInteger overlayInstrTotal;
    private final BigInteger overlayBytesTotal;
    private final BigInteger rbcChunksTotal;
    private final BigInteger rbcBytesTotal;
    private final BigInteger detachedPreparedTotal;
    private final BigInteger detachedMergedTotal;
    private final BigInteger detachedFallbackTotal;
    private final BigInteger detachedFallbackFeePostprocessingTotal;
    private final BigInteger detachedFallbackUserExecutorTotal;
    private final BigInteger detachedFallbackDurableStateTotal;
    private final BigInteger detachedFallbackUnsupportedInstructionTotal;
    private final BigInteger detachedFallbackRejectedEvalTotal;
    private final BigInteger detachedFallbackOverlayErrorTotal;
    private final BigInteger quarantineExecutedTotal;

    public PipelineExecutionStatus(
        final BigInteger txVerticesTotal,
        final BigInteger txEdgesTotal,
        final BigInteger overlayCountTotal,
        final BigInteger overlayInstrTotal,
        final BigInteger overlayBytesTotal,
        final BigInteger rbcChunksTotal,
        final BigInteger rbcBytesTotal,
        final BigInteger detachedPreparedTotal,
        final BigInteger detachedMergedTotal,
        final BigInteger detachedFallbackTotal,
        final BigInteger detachedFallbackFeePostprocessingTotal,
        final BigInteger detachedFallbackUserExecutorTotal,
        final BigInteger detachedFallbackDurableStateTotal,
        final BigInteger detachedFallbackUnsupportedInstructionTotal,
        final BigInteger detachedFallbackRejectedEvalTotal,
        final BigInteger detachedFallbackOverlayErrorTotal,
        final BigInteger quarantineExecutedTotal) {
      final BigInteger[] counters = {
        txVerticesTotal,
        txEdgesTotal,
        overlayCountTotal,
        overlayInstrTotal,
        overlayBytesTotal,
        rbcChunksTotal,
        rbcBytesTotal,
        detachedPreparedTotal,
        detachedMergedTotal,
        detachedFallbackTotal,
        detachedFallbackFeePostprocessingTotal,
        detachedFallbackUserExecutorTotal,
        detachedFallbackDurableStateTotal,
        detachedFallbackUnsupportedInstructionTotal,
        detachedFallbackRejectedEvalTotal,
        detachedFallbackOverlayErrorTotal,
        quarantineExecutedTotal
      };
      for (final BigInteger counter : counters) {
        requireUnsigned64(counter, "pipeline execution counter");
      }
      this.txVerticesTotal = txVerticesTotal;
      this.txEdgesTotal = txEdgesTotal;
      this.overlayCountTotal = overlayCountTotal;
      this.overlayInstrTotal = overlayInstrTotal;
      this.overlayBytesTotal = overlayBytesTotal;
      this.rbcChunksTotal = rbcChunksTotal;
      this.rbcBytesTotal = rbcBytesTotal;
      this.detachedPreparedTotal = detachedPreparedTotal;
      this.detachedMergedTotal = detachedMergedTotal;
      this.detachedFallbackTotal = detachedFallbackTotal;
      this.detachedFallbackFeePostprocessingTotal = detachedFallbackFeePostprocessingTotal;
      this.detachedFallbackUserExecutorTotal = detachedFallbackUserExecutorTotal;
      this.detachedFallbackDurableStateTotal = detachedFallbackDurableStateTotal;
      this.detachedFallbackUnsupportedInstructionTotal =
          detachedFallbackUnsupportedInstructionTotal;
      this.detachedFallbackRejectedEvalTotal = detachedFallbackRejectedEvalTotal;
      this.detachedFallbackOverlayErrorTotal = detachedFallbackOverlayErrorTotal;
      this.quarantineExecutedTotal = quarantineExecutedTotal;
    }

    public BigInteger txVerticesTotal() { return txVerticesTotal; }
    public BigInteger txEdgesTotal() { return txEdgesTotal; }
    public BigInteger overlayCountTotal() { return overlayCountTotal; }
    public BigInteger overlayInstrTotal() { return overlayInstrTotal; }
    public BigInteger overlayBytesTotal() { return overlayBytesTotal; }
    public BigInteger rbcChunksTotal() { return rbcChunksTotal; }
    public BigInteger rbcBytesTotal() { return rbcBytesTotal; }
    public BigInteger detachedPreparedTotal() { return detachedPreparedTotal; }
    public BigInteger detachedMergedTotal() { return detachedMergedTotal; }
    public BigInteger detachedFallbackTotal() { return detachedFallbackTotal; }
    public BigInteger detachedFallbackFeePostprocessingTotal() {
      return detachedFallbackFeePostprocessingTotal;
    }
    public BigInteger detachedFallbackUserExecutorTotal() {
      return detachedFallbackUserExecutorTotal;
    }
    public BigInteger detachedFallbackDurableStateTotal() {
      return detachedFallbackDurableStateTotal;
    }
    public BigInteger detachedFallbackUnsupportedInstructionTotal() {
      return detachedFallbackUnsupportedInstructionTotal;
    }
    public BigInteger detachedFallbackRejectedEvalTotal() {
      return detachedFallbackRejectedEvalTotal;
    }
    public BigInteger detachedFallbackOverlayErrorTotal() {
      return detachedFallbackOverlayErrorTotal;
    }
    public BigInteger quarantineExecutedTotal() { return quarantineExecutedTotal; }
  }

  /** Current permissionless-election schedule and PRF context while NPoS mode is active. */
  public static final class NposDiagnostics {
    private final BigInteger epochLengthBlocks;
    private final BigInteger vrfCommitDeadlineOffset;
    private final BigInteger vrfRevealDeadlineOffset;
    private final List<Integer> epochSeed;
    private final BigInteger prfHeight;
    private final BigInteger prfView;

    public NposDiagnostics(
        final BigInteger epochLengthBlocks,
        final BigInteger vrfCommitDeadlineOffset,
        final BigInteger vrfRevealDeadlineOffset,
        final List<Integer> epochSeed,
        final BigInteger prfHeight,
        final BigInteger prfView) {
      requireUnsigned64(epochLengthBlocks, "epochLengthBlocks");
      requireUnsigned64(vrfCommitDeadlineOffset, "vrfCommitDeadlineOffset");
      requireUnsigned64(vrfRevealDeadlineOffset, "vrfRevealDeadlineOffset");
      require(
          epochLengthBlocks.signum() > 0
              && vrfCommitDeadlineOffset.signum() > 0
              && vrfRevealDeadlineOffset.signum() > 0
              && vrfCommitDeadlineOffset.compareTo(vrfRevealDeadlineOffset) < 0
              && vrfRevealDeadlineOffset.compareTo(epochLengthBlocks) <= 0,
          "NPoS diagnostics windows must be strictly ordered within the epoch");
      require(epochSeed != null && epochSeed.size() == 32, "epochSeed must contain 32 bytes");
      boolean seedNonzero = false;
      for (final Integer item : epochSeed) {
        require(item != null && item >= 0 && item <= 255, "epochSeed contains an invalid byte");
        seedNonzero |= item != 0;
      }
      require(seedNonzero, "epochSeed must not be all zero");
      for (final BigInteger value : new BigInteger[] {prfHeight, prfView}) {
        requireUnsigned64(value, "NPoS diagnostics PRF context");
      }
      this.epochLengthBlocks = epochLengthBlocks;
      this.vrfCommitDeadlineOffset = vrfCommitDeadlineOffset;
      this.vrfRevealDeadlineOffset = vrfRevealDeadlineOffset;
      this.epochSeed = Collections.unmodifiableList(new ArrayList<>(epochSeed));
      this.prfHeight = prfHeight;
      this.prfView = prfView;
    }

    public BigInteger epochLengthBlocks() { return epochLengthBlocks; }
    public BigInteger vrfCommitDeadlineOffset() { return vrfCommitDeadlineOffset; }
    public BigInteger vrfRevealDeadlineOffset() { return vrfRevealDeadlineOffset; }
    public List<Integer> epochSeed() { return epochSeed; }
    public BigInteger prfHeight() { return prfHeight; }
    public BigInteger prfView() { return prfView; }
  }

  /** Evidence-derived Native AMX participant application state. */
  public enum NativeAmxParticipantApplicationState {
    CERTIFIED_PENDING_CARRIER("certified_pending_carrier"),
    COMMITTED_EVIDENCE_PENDING("committed_evidence_pending"),
    DURABLY_APPLIED("durably_applied"),
    CONFLICT("conflict");

    private final String wireName;

    NativeAmxParticipantApplicationState(final String wireName) {
      this.wireName = wireName;
    }

    /** Exact JSON string used by Torii. */
    public String wireName() {
      return wireName;
    }

    /** Resolve an exact Torii JSON state string. */
    public static NativeAmxParticipantApplicationState fromWireName(final String value) {
      for (final NativeAmxParticipantApplicationState state : values()) {
        if (state.wireName.equals(value)) {
          return state;
        }
      }
      throw new IllegalArgumentException("unknown Native AMX participant application state");
    }
  }

  /** One row from {@code native_amx_participant_applications}. */
  public static final class NativeAmxParticipantApplication {
    private final long laneId;
    private final BigInteger dataspaceId;
    private final String laneIncarnation;
    private final BigInteger participantHeight;
    private final BigInteger participantView;
    private final BigInteger predecessorHeight;
    private final String predecessorDescriptorHash;
    private final String descriptorHash;
    private final String proposalHash;
    private final String settlementHash;
    private final long sourceCount;
    private final BigInteger applicationBlockHeight;
    private final String applicationBlockHash;
    private final NativeAmxParticipantApplicationState state;

    public NativeAmxParticipantApplication(
        final long laneId,
        final BigInteger dataspaceId,
        final String laneIncarnation,
        final BigInteger participantHeight,
        final BigInteger participantView,
        final BigInteger predecessorHeight,
        final String predecessorDescriptorHash,
        final String descriptorHash,
        final String proposalHash,
        final String settlementHash,
        final long sourceCount,
        final BigInteger applicationBlockHeight,
        final String applicationBlockHash,
        final NativeAmxParticipantApplicationState state) {
      require(laneId >= 0 && laneId <= 0xffff_ffffL, "laneId must be an unsigned 32-bit value");
      requireUnsigned64(dataspaceId, "dataspaceId");
      requireUnsigned64(participantHeight, "participantHeight");
      requireUnsigned64(participantView, "participantView");
      requireUnsigned64(predecessorHeight, "predecessorHeight");
      require(
          participantHeight.signum() > 0, "participant height must be positive");
      require(
          predecessorHeight.add(BigInteger.ONE).equals(participantHeight)
              && (predecessorHeight.signum() == 0) == (predecessorDescriptorHash == null),
          "Native AMX participant predecessor geometry is inconsistent");
      require(
          sourceCount >= 1 && sourceCount <= NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX,
          "Native AMX participant source count is out of bounds");
      require(
          (applicationBlockHeight == null) == (applicationBlockHash == null),
          "application block height and hash must appear together");
      if (applicationBlockHeight != null) {
        requireUnsigned64(applicationBlockHeight, "applicationBlockHeight");
        require(
            applicationBlockHeight.signum() > 0, "application block height must be positive");
      }
      require(state != null, "state must not be null");
      final boolean requiresApplicationBlock =
          state == NativeAmxParticipantApplicationState.COMMITTED_EVIDENCE_PENDING
              || state == NativeAmxParticipantApplicationState.DURABLY_APPLIED;
      require(
          (applicationBlockHeight != null) == requiresApplicationBlock,
          "Native AMX participant state and application block identity disagree");

      requireCanonicalNonzeroHash(laneIncarnation, "laneIncarnation");
      if (predecessorDescriptorHash != null) {
        requireCanonicalNonzeroHash(predecessorDescriptorHash, "predecessorDescriptorHash");
      }
      requireCanonicalNonzeroHash(descriptorHash, "descriptorHash");
      requireCanonicalNonzeroHash(proposalHash, "proposalHash");
      requireCanonicalNonzeroHash(settlementHash, "settlementHash");
      if (applicationBlockHash != null) {
        requireCanonicalNonzeroHash(applicationBlockHash, "applicationBlockHash");
      }

      this.laneId = laneId;
      this.dataspaceId = dataspaceId;
      this.laneIncarnation = laneIncarnation;
      this.participantHeight = participantHeight;
      this.participantView = participantView;
      this.predecessorHeight = predecessorHeight;
      this.predecessorDescriptorHash = predecessorDescriptorHash;
      this.descriptorHash = descriptorHash;
      this.proposalHash = proposalHash;
      this.settlementHash = settlementHash;
      this.sourceCount = sourceCount;
      this.applicationBlockHeight = applicationBlockHeight;
      this.applicationBlockHash = applicationBlockHash;
      this.state = state;
    }

    public long laneId() { return laneId; }
    public BigInteger dataspaceId() { return dataspaceId; }
    public String laneIncarnation() { return laneIncarnation; }
    public BigInteger participantHeight() { return participantHeight; }
    public BigInteger participantView() { return participantView; }
    public BigInteger predecessorHeight() { return predecessorHeight; }
    public String predecessorDescriptorHash() { return predecessorDescriptorHash; }
    public String descriptorHash() { return descriptorHash; }
    public String proposalHash() { return proposalHash; }
    public String settlementHash() { return settlementHash; }
    public long sourceCount() { return sourceCount; }
    public BigInteger applicationBlockHeight() { return applicationBlockHeight; }
    public String applicationBlockHash() { return applicationBlockHash; }
    public NativeAmxParticipantApplicationState state() { return state; }
  }

  /** Immutable bounded vector ordered by route and incarnation. */
  public static final class NativeAmxParticipantApplications {
    private final List<NativeAmxParticipantApplication> rows;

    public NativeAmxParticipantApplications(final List<NativeAmxParticipantApplication> rows) {
      require(rows != null, "rows must not be null");
      require(
          rows.size() <= NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX,
          "Native AMX participant diagnostics exceed the 1024-row limit");
      final ArrayList<NativeAmxParticipantApplication> copy = new ArrayList<>(rows);
      for (int index = 1; index < copy.size(); index++) {
        require(
            compareRoute(copy.get(index - 1), copy.get(index)) < 0,
            "Native AMX participant diagnostics must be strictly ordered by route and incarnation");
      }
      this.rows = Collections.unmodifiableList(copy);
    }

    public List<NativeAmxParticipantApplication> rows() {
      return rows;
    }
  }

  public enum AutonomousLaneExecutionStage {
    RESERVATIONS_DURABLE("reservations_durable"),
    EXECUTABLE_PAYLOAD_DURABLE("executable_payload_durable"),
    PAYLOAD_AVAILABILITY_CERTIFIED("payload_availability_certified"),
    LANE_CERTIFIED("lane_certified"),
    CERTIFIED_BUNDLE_DURABLE("certified_bundle_durable"),
    MERGE_CANDIDATE_DURABLE("merge_candidate_durable"),
    GLOBAL_CARRIER_COMMITTED("global_carrier_committed"),
    KURA_WSV_APPLICATION_RECEIPT_DURABLE("kura_wsv_application_receipt_durable"),
    QUEUE_FINALIZED("queue_finalized"),
    CONFLICT("conflict");
    private final String wireName;
    AutonomousLaneExecutionStage(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
    public static AutonomousLaneExecutionStage fromWireName(final String value) {
      for (final AutonomousLaneExecutionStage item : values()) {
        if (item.wireName.equals(value)) return item;
      }
      throw new IllegalArgumentException("unknown autonomous lane execution stage");
    }
  }

  public enum AutonomousLaneExecutionStuckReason {
    AWAITING_EXECUTABLE_PAYLOAD("awaiting_executable_payload"),
    AWAITING_PAYLOAD_AVAILABILITY("awaiting_payload_availability"),
    AWAITING_LANE_CERTIFICATION("awaiting_lane_certification"),
    CERTIFIED_BUNDLE_UNAVAILABLE("certified_bundle_unavailable"),
    AWAITING_MERGE_SELECTION("awaiting_merge_selection"),
    AWAITING_GLOBAL_CARRIER("awaiting_global_carrier"),
    AWAITING_APPLICATION_RECEIPT("awaiting_application_receipt"),
    QUEUE_FINALIZATION_UNVERIFIABLE("queue_finalization_unverifiable"),
    EVIDENCE_CONFLICT("evidence_conflict");
    private final String wireName;
    AutonomousLaneExecutionStuckReason(final String wireName) { this.wireName = wireName; }
    public String wireName() { return wireName; }
    public static AutonomousLaneExecutionStuckReason fromWireName(final String value) {
      for (final AutonomousLaneExecutionStuckReason item : values()) {
        if (item.wireName.equals(value)) return item;
      }
      throw new IllegalArgumentException("unknown autonomous lane execution stuck reason");
    }
  }

  public static final class AutonomousLaneExecution {
    public final long laneId;
    public final BigInteger dataspaceId;
    public final String laneIncarnation;
    public final BigInteger laneBlockHeight;
    public final BigInteger laneBlockView;
    public final BigInteger proposalHeight;
    public final BigInteger proposalView;
    public final String reservationOwnerHash;
    public final String proposalIdentityHash;
    public final String reservationGroupHash;
    public final String proposalHash;
    public final String descriptorHash;
    public final String executablePayloadHash;
    public final String sourceBundleHash;
    public final String mergeEntryHash;
    public final BigInteger applicationBlockHeight;
    public final String applicationBlockHash;
    public final long reservationCount;
    public final long transactionCount;
    public final AutonomousLaneExecutionStage highestDurableStage;
    public final AutonomousLaneExecutionStuckReason stuckReason;

    public AutonomousLaneExecution(
        final long laneId, final BigInteger dataspaceId, final String laneIncarnation,
        final BigInteger laneBlockHeight, final BigInteger laneBlockView,
        final BigInteger proposalHeight, final BigInteger proposalView,
        final String reservationOwnerHash, final String proposalIdentityHash,
        final String reservationGroupHash,
        final String proposalHash, final String descriptorHash,
        final String executablePayloadHash, final String sourceBundleHash,
        final String mergeEntryHash, final BigInteger applicationBlockHeight,
        final String applicationBlockHash, final long reservationCount,
        final long transactionCount, final AutonomousLaneExecutionStage highestDurableStage,
        final AutonomousLaneExecutionStuckReason stuckReason) {
      require(laneId >= 0 && laneId <= 0xffff_ffffL, "laneId out of range");
      for (final BigInteger coordinate :
          new BigInteger[] {dataspaceId, laneBlockHeight, laneBlockView, proposalHeight}) {
        requireUnsigned64(coordinate, "autonomous execution coordinate");
      }
      if (proposalView != null) {
        requireUnsigned64(proposalView, "autonomous proposal view");
      }
      require(laneBlockHeight.signum() > 0 && proposalHeight.signum() > 0, "height must be positive");
      require(transactionCount >= 1 && transactionCount <= 4096
          && reservationCount >= 0 && reservationCount <= 4096, "counts are invalid");
      require((applicationBlockHeight == null) == (applicationBlockHash == null),
          "carrier height and hash must appear together");
      if (applicationBlockHeight != null) {
        requireUnsigned64(applicationBlockHeight, "applicationBlockHeight");
        require(applicationBlockHeight.signum() > 0, "carrier height must be positive");
      }
      for (final String hash :
          new String[] {
            laneIncarnation, reservationOwnerHash, proposalIdentityHash, reservationGroupHash
          }) {
        requireCanonicalNonzeroHash(hash, "autonomous execution hash");
      }
      for (final String hash :
          new String[] {
            proposalHash, descriptorHash, executablePayloadHash, sourceBundleHash,
            mergeEntryHash, applicationBlockHash
          }) {
        if (hash != null) requireCanonicalNonzeroHash(hash, "autonomous optional hash");
      }
      require((proposalHash == null) == (descriptorHash == null),
          "proposal and descriptor hashes must appear together");
      require(highestDurableStage != null, "stage must not be null");
      require(stuckReason == expectedStuckReason(highestDurableStage),
          "stage and stuck reason disagree");
      if (highestDurableStage != AutonomousLaneExecutionStage.CONFLICT) {
        require(reservationCount == transactionCount,
            "reservation and transaction counts disagree");
        require(
            (highestDurableStage == AutonomousLaneExecutionStage.RESERVATIONS_DURABLE)
                == (proposalHash == null),
            "finalized identity disagrees with durable stage");
        require(
            highestDurableStage != AutonomousLaneExecutionStage.RESERVATIONS_DURABLE
                || proposalView == null,
            "proposal view disagrees with durable stage");
        final boolean hasPayload = executablePayloadHash != null;
        final boolean hasBundle = sourceBundleHash != null;
        final boolean hasMerge = mergeEntryHash != null;
        final boolean hasCarrier = applicationBlockHeight != null;
        final boolean geometryMatches;
        switch (highestDurableStage) {
          case RESERVATIONS_DURABLE:
            geometryMatches = !hasPayload && !hasBundle && !hasMerge && !hasCarrier;
            break;
          case EXECUTABLE_PAYLOAD_DURABLE:
          case PAYLOAD_AVAILABILITY_CERTIFIED:
          case LANE_CERTIFIED:
            geometryMatches = hasPayload && !hasBundle && !hasMerge && !hasCarrier;
            break;
          case CERTIFIED_BUNDLE_DURABLE:
            geometryMatches = hasPayload && hasBundle && !hasMerge && !hasCarrier;
            break;
          case MERGE_CANDIDATE_DURABLE:
          case GLOBAL_CARRIER_COMMITTED:
            geometryMatches = hasPayload && hasBundle && hasMerge && !hasCarrier;
            break;
          case KURA_WSV_APPLICATION_RECEIPT_DURABLE:
          case QUEUE_FINALIZED:
            geometryMatches = hasPayload && hasBundle && hasMerge && hasCarrier;
            break;
          case CONFLICT:
          default:
            geometryMatches = true;
            break;
        }
        require(geometryMatches, "evidence does not match durable stage");
      }
      this.laneId = laneId; this.dataspaceId = dataspaceId;
      this.laneIncarnation = laneIncarnation; this.laneBlockHeight = laneBlockHeight;
      this.laneBlockView = laneBlockView; this.proposalHeight = proposalHeight;
      this.proposalView = proposalView; this.reservationOwnerHash = reservationOwnerHash;
      this.proposalIdentityHash = proposalIdentityHash;
      this.reservationGroupHash = reservationGroupHash; this.proposalHash = proposalHash;
      this.descriptorHash = descriptorHash; this.executablePayloadHash = executablePayloadHash;
      this.sourceBundleHash = sourceBundleHash; this.mergeEntryHash = mergeEntryHash;
      this.applicationBlockHeight = applicationBlockHeight;
      this.applicationBlockHash = applicationBlockHash; this.reservationCount = reservationCount;
      this.transactionCount = transactionCount; this.highestDurableStage = highestDurableStage;
      this.stuckReason = stuckReason;
    }
    public long laneId() { return laneId; }
    public BigInteger dataspaceId() { return dataspaceId; }
    public String laneIncarnation() { return laneIncarnation; }
    public BigInteger laneBlockHeight() { return laneBlockHeight; }
    public BigInteger laneBlockView() { return laneBlockView; }
    public BigInteger proposalHeight() { return proposalHeight; }
    public BigInteger proposalView() { return proposalView; }
    public String reservationOwnerHash() { return reservationOwnerHash; }
    public String proposalIdentityHash() { return proposalIdentityHash; }
    public String reservationGroupHash() { return reservationGroupHash; }
    public String proposalHash() { return proposalHash; }
    public String descriptorHash() { return descriptorHash; }
    public String executablePayloadHash() { return executablePayloadHash; }
    public String sourceBundleHash() { return sourceBundleHash; }
    public String mergeEntryHash() { return mergeEntryHash; }
    public BigInteger applicationBlockHeight() { return applicationBlockHeight; }
    public String applicationBlockHash() { return applicationBlockHash; }
    public long reservationCount() { return reservationCount; }
    public long transactionCount() { return transactionCount; }
    public AutonomousLaneExecutionStage highestDurableStage() { return highestDurableStage; }
    public AutonomousLaneExecutionStuckReason stuckReason() { return stuckReason; }
  }

  private static AutonomousLaneExecutionStuckReason expectedStuckReason(
      final AutonomousLaneExecutionStage stage) {
    switch (stage) {
      case RESERVATIONS_DURABLE:
        return AutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD;
      case EXECUTABLE_PAYLOAD_DURABLE:
        return AutonomousLaneExecutionStuckReason.AWAITING_PAYLOAD_AVAILABILITY;
      case PAYLOAD_AVAILABILITY_CERTIFIED:
        return AutonomousLaneExecutionStuckReason.AWAITING_LANE_CERTIFICATION;
      case LANE_CERTIFIED:
        return AutonomousLaneExecutionStuckReason.CERTIFIED_BUNDLE_UNAVAILABLE;
      case CERTIFIED_BUNDLE_DURABLE:
        return AutonomousLaneExecutionStuckReason.AWAITING_MERGE_SELECTION;
      case MERGE_CANDIDATE_DURABLE:
        return AutonomousLaneExecutionStuckReason.AWAITING_GLOBAL_CARRIER;
      case GLOBAL_CARRIER_COMMITTED:
        return AutonomousLaneExecutionStuckReason.AWAITING_APPLICATION_RECEIPT;
      case KURA_WSV_APPLICATION_RECEIPT_DURABLE:
        return AutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE;
      case QUEUE_FINALIZED:
        return null;
      case CONFLICT:
        return AutonomousLaneExecutionStuckReason.EVIDENCE_CONFLICT;
      default:
        throw new IllegalArgumentException("unknown autonomous lane execution stage");
    }
  }

  public static final class AutonomousLaneExecutions {
    private final List<AutonomousLaneExecution> rows;
    public AutonomousLaneExecutions(final List<AutonomousLaneExecution> rows) {
      require(rows != null && rows.size() <= AUTONOMOUS_LANE_EXECUTIONS_MAX,
          "autonomous lane diagnostics exceed the 128-row limit");
      final ArrayList<AutonomousLaneExecution> copy = new ArrayList<>(rows);
      for (int index = 1; index < copy.size(); index++) {
        require(compareAutonomous(copy.get(index - 1), copy.get(index)) < 0,
            "autonomous lane diagnostics must be strictly ordered");
      }
      this.rows = Collections.unmodifiableList(copy);
    }
    public List<AutonomousLaneExecution> rows() { return rows; }
  }

  /**
   * Complete public model for {@code /v1/sumeragi/diagnostics}.
   *
   * <p>Native-bearing settlement commitments, including relay-contained commitments, are
   * validated by the strict Native AMX V2 parser before this model is constructed.
   */
  public static final class SumeragiDiagnosticsStatus {
    private final PipelineExecutionStatus pipelineExecution;
    private final BigInteger txQueueDepth;
    private final BigInteger txQueueCapacity;
    private final BigInteger txQueueRetainedBytes;
    private final BigInteger txQueueMaxRetainedBytes;
    private final boolean txQueueSaturated;
    private final boolean txQueueSaturatedByCount;
    private final boolean txQueueSaturatedByBytes;
    private final boolean txQueueSaturatedByAge;
    private final BigInteger txQueueOldestQueuedAgeMs;
    private final NposDiagnostics npos;
    private final List<?> laneCommitments;
    private final List<?> dataspaceCommitments;
    private final List<?> laneSettlementCommitments;
    private final List<?> laneRelayEnvelopes;
    private final List<?> lanePayloadOwnerships;
    private final List<?> committedLaneBlocks;
    private final List<?> laneBlockSessions;
    private final long laneGovernanceSealedTotal;
    private final List<String> laneGovernanceSealedAliases;
    private final List<?> laneGovernance;
    private final List<NativeAmxParticipantApplication> nativeAmxParticipantApplications;
    private final List<AutonomousLaneExecution> autonomousLaneExecutions;

    public SumeragiDiagnosticsStatus(
        final PipelineExecutionStatus pipelineExecution,
        final BigInteger txQueueDepth,
        final BigInteger txQueueCapacity,
        final BigInteger txQueueRetainedBytes,
        final BigInteger txQueueMaxRetainedBytes,
        final boolean txQueueSaturated,
        final boolean txQueueSaturatedByCount,
        final boolean txQueueSaturatedByBytes,
        final boolean txQueueSaturatedByAge,
        final BigInteger txQueueOldestQueuedAgeMs,
        final NposDiagnostics npos,
        final List<?> laneCommitments,
        final List<?> dataspaceCommitments,
        final List<?> laneSettlementCommitments,
        final List<?> laneRelayEnvelopes,
        final List<?> lanePayloadOwnerships,
        final List<?> committedLaneBlocks,
        final List<?> laneBlockSessions,
        final long laneGovernanceSealedTotal,
        final List<String> laneGovernanceSealedAliases,
        final List<?> laneGovernance,
        final List<NativeAmxParticipantApplication> nativeAmxParticipantApplications,
        final List<AutonomousLaneExecution> autonomousLaneExecutions) {
      require(pipelineExecution != null, "pipelineExecution must not be null");
      requireUnsigned64(txQueueDepth, "txQueueDepth");
      requireUnsigned64(txQueueCapacity, "txQueueCapacity");
      requireUnsigned64(txQueueRetainedBytes, "txQueueRetainedBytes");
      requireUnsigned64(txQueueMaxRetainedBytes, "txQueueMaxRetainedBytes");
      requireUnsigned64(txQueueOldestQueuedAgeMs, "txQueueOldestQueuedAgeMs");
      require(
          txQueueDepth.compareTo(txQueueCapacity) <= 0,
          "transaction queue depth exceeds capacity");
      require(
          txQueueRetainedBytes.compareTo(txQueueMaxRetainedBytes) <= 0,
          "retained transaction bytes exceed their budget");
      require(
          txQueueSaturated
              == (txQueueSaturatedByCount || txQueueSaturatedByBytes || txQueueSaturatedByAge),
          "queue saturation disagrees with its causes");
      require(
          laneGovernanceSealedTotal >= 0 && laneGovernanceSealedTotal <= 0xffff_ffffL,
          "laneGovernanceSealedTotal must be an unsigned 32-bit value");

      final List<?> copiedLaneCommitments =
          boundedOpaqueCopy(laneCommitments, "laneCommitments", DIAGNOSTIC_LANES_MAX);
      final List<?> copiedDataspaceCommitments =
          boundedOpaqueCopy(dataspaceCommitments, "dataspaceCommitments", DIAGNOSTIC_LANES_MAX);
      final List<?> copiedLaneSettlementCommitments =
          boundedOpaqueCopy(
              laneSettlementCommitments,
              "laneSettlementCommitments",
              DIAGNOSTIC_LANES_MAX);
      final List<?> copiedLaneRelayEnvelopes =
          boundedOpaqueCopy(
              laneRelayEnvelopes, "laneRelayEnvelopes", DIAGNOSTIC_LANES_MAX);
      final List<?> copiedLanePayloadOwnerships =
          boundedOpaqueCopy(
              lanePayloadOwnerships,
              "lanePayloadOwnerships",
              DIAGNOSTIC_LANES_MAX);
      final List<?> copiedCommittedLaneBlocks =
          boundedOpaqueCopy(
              committedLaneBlocks,
              "committedLaneBlocks",
              DIAGNOSTIC_LANES_MAX);
      final List<?> copiedLaneBlockSessions =
          boundedOpaqueCopy(laneBlockSessions, "laneBlockSessions", DIAGNOSTIC_LANES_MAX);
      final List<?> copiedLaneGovernance =
          boundedOpaqueCopy(laneGovernance, "laneGovernance", DIAGNOSTIC_LANES_MAX);
      validateNativeAmxDiagnosticsEvidence(
          copiedLaneSettlementCommitments, copiedLaneRelayEnvelopes);
      final List<String> copiedAliases =
          boundedCopy(
              laneGovernanceSealedAliases,
              "laneGovernanceSealedAliases",
              DIAGNOSTIC_LANES_MAX);
      require(
          copiedAliases.size() == laneGovernanceSealedTotal,
          "sealed lane aliases must match laneGovernanceSealedTotal");
      for (int index = 0; index < copiedAliases.size(); index++) {
        final String alias = copiedAliases.get(index);
        require(
            alias != null && !alias.isEmpty() && alias.trim().equals(alias),
            "sealed lane aliases must be exact non-empty strings");
        require(
            copiedAliases.indexOf(alias) == index,
            "sealed lane aliases must be unique");
      }

      final NativeAmxParticipantApplications checkedNative =
          new NativeAmxParticipantApplications(nativeAmxParticipantApplications);
      final AutonomousLaneExecutions checkedAutonomous =
          new AutonomousLaneExecutions(autonomousLaneExecutions);
      this.pipelineExecution = pipelineExecution;
      this.txQueueDepth = txQueueDepth;
      this.txQueueCapacity = txQueueCapacity;
      this.txQueueRetainedBytes = txQueueRetainedBytes;
      this.txQueueMaxRetainedBytes = txQueueMaxRetainedBytes;
      this.txQueueSaturated = txQueueSaturated;
      this.txQueueSaturatedByCount = txQueueSaturatedByCount;
      this.txQueueSaturatedByBytes = txQueueSaturatedByBytes;
      this.txQueueSaturatedByAge = txQueueSaturatedByAge;
      this.txQueueOldestQueuedAgeMs = txQueueOldestQueuedAgeMs;
      this.npos = npos;
      this.laneCommitments = copiedLaneCommitments;
      this.dataspaceCommitments = copiedDataspaceCommitments;
      this.laneSettlementCommitments = copiedLaneSettlementCommitments;
      this.laneRelayEnvelopes = copiedLaneRelayEnvelopes;
      this.lanePayloadOwnerships = copiedLanePayloadOwnerships;
      this.committedLaneBlocks = copiedCommittedLaneBlocks;
      this.laneBlockSessions = copiedLaneBlockSessions;
      this.laneGovernanceSealedTotal = laneGovernanceSealedTotal;
      this.laneGovernanceSealedAliases = copiedAliases;
      this.laneGovernance = copiedLaneGovernance;
      this.nativeAmxParticipantApplications = checkedNative.rows();
      this.autonomousLaneExecutions = checkedAutonomous.rows();
    }

    public PipelineExecutionStatus pipelineExecution() { return pipelineExecution; }
    public BigInteger txQueueDepth() { return txQueueDepth; }
    public BigInteger txQueueCapacity() { return txQueueCapacity; }
    public BigInteger txQueueRetainedBytes() { return txQueueRetainedBytes; }
    public BigInteger txQueueMaxRetainedBytes() { return txQueueMaxRetainedBytes; }
    public boolean txQueueSaturated() { return txQueueSaturated; }
    public boolean txQueueSaturatedByCount() { return txQueueSaturatedByCount; }
    public boolean txQueueSaturatedByBytes() { return txQueueSaturatedByBytes; }
    public boolean txQueueSaturatedByAge() { return txQueueSaturatedByAge; }
    public BigInteger txQueueOldestQueuedAgeMs() { return txQueueOldestQueuedAgeMs; }
    public NposDiagnostics npos() { return npos; }
    public List<?> laneCommitments() { return laneCommitments; }
    public List<?> dataspaceCommitments() { return dataspaceCommitments; }
    public List<?> laneSettlementCommitments() { return laneSettlementCommitments; }
    public List<?> laneRelayEnvelopes() { return laneRelayEnvelopes; }
    public List<?> lanePayloadOwnerships() { return lanePayloadOwnerships; }
    public List<?> committedLaneBlocks() { return committedLaneBlocks; }
    public List<?> laneBlockSessions() { return laneBlockSessions; }
    public long laneGovernanceSealedTotal() { return laneGovernanceSealedTotal; }
    public List<String> laneGovernanceSealedAliases() { return laneGovernanceSealedAliases; }
    public List<?> laneGovernance() { return laneGovernance; }
    public List<NativeAmxParticipantApplication> nativeAmxParticipantApplications() {
      return nativeAmxParticipantApplications;
    }
    public List<AutonomousLaneExecution> autonomousLaneExecutions() {
      return autonomousLaneExecutions;
    }

    /** Parse one fatal-UTF-8, duplicate-key rejecting diagnostics response. */
    public static SumeragiDiagnosticsStatus parseJson(final byte[] payload) {
      return SumeragiDiagnosticsModels.parseDiagnostics(payload);
    }

    /** Parse one duplicate-key rejecting diagnostics response. */
    public static SumeragiDiagnosticsStatus parseJson(final String payload) {
      return SumeragiDiagnosticsModels.parseDiagnostics(payload);
    }
  }

  /** Parse one fatal-UTF-8, duplicate-key rejecting diagnostics response. */
  public static SumeragiDiagnosticsStatus parseDiagnostics(final byte[] payload) {
    require(
        payload != null && payload.length > 0,
        "Sumeragi diagnostics response must not be empty");
    require(
        (long) payload.length <= SumeragiStatusModels.DIAGNOSTICS_JSON_MAX_BYTES,
        "Sumeragi diagnostics response exceeds "
            + SumeragiStatusModels.DIAGNOSTICS_JSON_MAX_BYTES
            + " bytes");
    return parseDiagnostics(
        SumeragiJsonSupport.decodeUtf8(payload, "Sumeragi diagnostics"));
  }

  /** Parse one duplicate-key rejecting diagnostics response. */
  public static SumeragiDiagnosticsStatus parseDiagnostics(final String payload) {
    final String context = "Sumeragi diagnostics";
    final Map<String, Object> root = SumeragiJsonSupport.parseObject(payload, context);
    SumeragiJsonSupport.requireFields(
        root,
        Set.of(
            "pipeline_execution",
            "tx_queue_depth",
            "tx_queue_capacity",
            "tx_queue_retained_bytes",
            "tx_queue_max_retained_bytes",
            "tx_queue_saturated",
            "tx_queue_saturated_by_count",
            "tx_queue_saturated_by_bytes",
            "tx_queue_saturated_by_age",
            "tx_queue_oldest_queued_age_ms",
            "lane_commitments",
            "dataspace_commitments",
            "lane_settlement_commitments",
            "lane_relay_envelopes",
            "lane_payload_ownerships",
            "committed_lane_blocks",
            "lane_block_sessions",
            "lane_governance_sealed_total",
            "lane_governance_sealed_aliases",
            "lane_governance",
            "native_amx_participant_applications",
            "autonomous_lane_executions"),
        Set.of("npos"),
        context);
    return new SumeragiDiagnosticsStatus(
        parsePipelineExecution(root.get("pipeline_execution"), context + ".pipeline_execution"),
        SumeragiJsonSupport.u64(root.get("tx_queue_depth"), context + ".tx_queue_depth"),
        SumeragiJsonSupport.u64(
            root.get("tx_queue_capacity"), context + ".tx_queue_capacity"),
        SumeragiJsonSupport.u64(
            root.get("tx_queue_retained_bytes"), context + ".tx_queue_retained_bytes"),
        SumeragiJsonSupport.u64(
            root.get("tx_queue_max_retained_bytes"),
            context + ".tx_queue_max_retained_bytes"),
        SumeragiJsonSupport.bool(
            root.get("tx_queue_saturated"), context + ".tx_queue_saturated"),
        SumeragiJsonSupport.bool(
            root.get("tx_queue_saturated_by_count"),
            context + ".tx_queue_saturated_by_count"),
        SumeragiJsonSupport.bool(
            root.get("tx_queue_saturated_by_bytes"),
            context + ".tx_queue_saturated_by_bytes"),
        SumeragiJsonSupport.bool(
            root.get("tx_queue_saturated_by_age"),
            context + ".tx_queue_saturated_by_age"),
        SumeragiJsonSupport.u64(
            root.get("tx_queue_oldest_queued_age_ms"),
            context + ".tx_queue_oldest_queued_age_ms"),
        root.get("npos") == null ? null : parseNpos(root.get("npos"), context + ".npos"),
        opaqueRows(root.get("lane_commitments"), context + ".lane_commitments"),
        opaqueRows(root.get("dataspace_commitments"), context + ".dataspace_commitments"),
        opaqueRows(
            root.get("lane_settlement_commitments"),
            context + ".lane_settlement_commitments"),
        opaqueRows(root.get("lane_relay_envelopes"), context + ".lane_relay_envelopes"),
        opaqueRows(
            root.get("lane_payload_ownerships"), context + ".lane_payload_ownerships"),
        opaqueRows(root.get("committed_lane_blocks"), context + ".committed_lane_blocks"),
        opaqueRows(root.get("lane_block_sessions"), context + ".lane_block_sessions"),
        SumeragiJsonSupport.u32(
                root.get("lane_governance_sealed_total"),
                context + ".lane_governance_sealed_total")
            .longValueExact(),
        parseAliases(
            root.get("lane_governance_sealed_aliases"),
            context + ".lane_governance_sealed_aliases"),
        opaqueRows(root.get("lane_governance"), context + ".lane_governance"),
        parseNativeApplications(
            root.get("native_amx_participant_applications"),
            context + ".native_amx_participant_applications"),
        parseAutonomousExecutions(
            root.get("autonomous_lane_executions"),
            context + ".autonomous_lane_executions"));
  }

  private static PipelineExecutionStatus parsePipelineExecution(
      final Object value, final String context) {
    final Set<String> fields =
        Set.of(
            "tx_vertices_total",
            "tx_edges_total",
            "overlay_count_total",
            "overlay_instr_total",
            "overlay_bytes_total",
            "rbc_chunks_total",
            "rbc_bytes_total",
            "detached_prepared_total",
            "detached_merged_total",
            "detached_fallback_total",
            "detached_fallback_fee_postprocessing_total",
            "detached_fallback_user_executor_total",
            "detached_fallback_durable_state_total",
            "detached_fallback_unsupported_instruction_total",
            "detached_fallback_rejected_eval_total",
            "detached_fallback_overlay_error_total",
            "quarantine_executed_total");
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(value, fields, context);
    return new PipelineExecutionStatus(
        diagnosticU64(record, "tx_vertices_total", context),
        diagnosticU64(record, "tx_edges_total", context),
        diagnosticU64(record, "overlay_count_total", context),
        diagnosticU64(record, "overlay_instr_total", context),
        diagnosticU64(record, "overlay_bytes_total", context),
        diagnosticU64(record, "rbc_chunks_total", context),
        diagnosticU64(record, "rbc_bytes_total", context),
        diagnosticU64(record, "detached_prepared_total", context),
        diagnosticU64(record, "detached_merged_total", context),
        diagnosticU64(record, "detached_fallback_total", context),
        diagnosticU64(record, "detached_fallback_fee_postprocessing_total", context),
        diagnosticU64(record, "detached_fallback_user_executor_total", context),
        diagnosticU64(record, "detached_fallback_durable_state_total", context),
        diagnosticU64(record, "detached_fallback_unsupported_instruction_total", context),
        diagnosticU64(record, "detached_fallback_rejected_eval_total", context),
        diagnosticU64(record, "detached_fallback_overlay_error_total", context),
        diagnosticU64(record, "quarantine_executed_total", context));
  }

  private static BigInteger diagnosticU64(
      final Map<String, Object> record, final String field, final String context) {
    return SumeragiJsonSupport.u64(record.get(field), context + "." + field);
  }

  private static NposDiagnostics parseNpos(final Object value, final String context) {
    final Map<String, Object> record =
        SumeragiJsonSupport.exactObject(
            value,
            Set.of(
                "epoch_length_blocks",
                "vrf_commit_deadline_offset",
                "vrf_reveal_deadline_offset",
                "epoch_seed",
                "prf_height",
                "prf_view"),
            context);
    final List<?> rawSeed = SumeragiJsonSupport.array(record.get("epoch_seed"), context + ".epoch_seed", 32);
    require(rawSeed.size() == 32, context + ".epoch_seed must contain exactly 32 bytes");
    final ArrayList<Integer> seed = new ArrayList<>();
    for (int index = 0; index < rawSeed.size(); index++) {
      seed.add(
          SumeragiJsonSupport.unsigned(
                  rawSeed.get(index), BigInteger.valueOf(255L),
                  context + ".epoch_seed[" + index + "]")
              .intValueExact());
    }
    return new NposDiagnostics(
        diagnosticU64(record, "epoch_length_blocks", context),
        diagnosticU64(record, "vrf_commit_deadline_offset", context),
        diagnosticU64(record, "vrf_reveal_deadline_offset", context),
        seed,
        diagnosticU64(record, "prf_height", context),
        diagnosticU64(record, "prf_view", context));
  }

  private static List<?> opaqueRows(final Object value, final String context) {
    final List<?> raw = SumeragiJsonSupport.array(value, context, DIAGNOSTIC_LANES_MAX);
    final ArrayList<Object> rows = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      final String rowContext = context + "[" + index + "]";
      final Map<String, Object> row = SumeragiJsonSupport.object(raw.get(index), rowContext);
      rows.add(SumeragiJsonSupport.deepFreeze(row, rowContext));
    }
    return Collections.unmodifiableList(rows);
  }

  private static List<String> parseAliases(final Object value, final String context) {
    final List<?> raw = SumeragiJsonSupport.array(value, context, DIAGNOSTIC_LANES_MAX);
    final ArrayList<String> aliases = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      aliases.add(
          SumeragiJsonSupport.exactNonemptyString(
              raw.get(index), context + "[" + index + "]"));
    }
    return Collections.unmodifiableList(aliases);
  }

  private static List<NativeAmxParticipantApplication> parseNativeApplications(
      final Object value, final String context) {
    final List<?> raw =
        SumeragiJsonSupport.array(value, context, NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX);
    final ArrayList<NativeAmxParticipantApplication> rows = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      final String rowContext = context + "[" + index + "]";
      final Map<String, Object> row = SumeragiJsonSupport.object(raw.get(index), rowContext);
      SumeragiJsonSupport.requireFields(
          row,
          Set.of(
              "lane_id",
              "dataspace_id",
              "lane_incarnation",
              "participant_height",
              "participant_view",
              "predecessor_height",
              "descriptor_hash",
              "proposal_hash",
              "settlement_hash",
              "source_count",
              "state"),
          Set.of(
              "predecessor_descriptor_hash",
              "application_block_height",
              "application_block_hash"),
          rowContext);
      rows.add(
          new NativeAmxParticipantApplication(
              SumeragiJsonSupport.u32(row.get("lane_id"), rowContext + ".lane_id")
                  .longValueExact(),
              SumeragiJsonSupport.u64(row.get("dataspace_id"), rowContext + ".dataspace_id"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("lane_incarnation"), rowContext + ".lane_incarnation"),
              SumeragiJsonSupport.positiveU64(
                  row.get("participant_height"), rowContext + ".participant_height"),
              SumeragiJsonSupport.u64(
                  row.get("participant_view"), rowContext + ".participant_view"),
              SumeragiJsonSupport.u64(
                  row.get("predecessor_height"), rowContext + ".predecessor_height"),
              optionalNonzeroHash(
                  row.get("predecessor_descriptor_hash"),
                  rowContext + ".predecessor_descriptor_hash"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("descriptor_hash"), rowContext + ".descriptor_hash"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("proposal_hash"), rowContext + ".proposal_hash"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("settlement_hash"), rowContext + ".settlement_hash"),
              SumeragiJsonSupport.unsigned(
                      row.get("source_count"),
                      BigInteger.valueOf(NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX),
                      rowContext + ".source_count",
                      true)
                  .longValueExact(),
              optionalPositiveU64(
                  row.get("application_block_height"),
                  rowContext + ".application_block_height"),
              optionalNonzeroHash(
                  row.get("application_block_hash"), rowContext + ".application_block_hash"),
              NativeAmxParticipantApplicationState.fromWireName(
                  SumeragiJsonSupport.string(row.get("state"), rowContext + ".state"))));
    }
    return Collections.unmodifiableList(rows);
  }

  private static List<AutonomousLaneExecution> parseAutonomousExecutions(
      final Object value, final String context) {
    final List<?> raw =
        SumeragiJsonSupport.array(value, context, AUTONOMOUS_LANE_EXECUTIONS_MAX);
    final ArrayList<AutonomousLaneExecution> rows = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      final String rowContext = context + "[" + index + "]";
      final Map<String, Object> row = SumeragiJsonSupport.object(raw.get(index), rowContext);
      SumeragiJsonSupport.requireFields(
          row,
          Set.of(
              "lane_id",
              "dataspace_id",
              "lane_incarnation",
              "lane_block_height",
              "lane_block_view",
              "proposal_height",
              "reservation_owner_hash",
              "proposal_identity_hash",
              "reservation_group_hash",
              "reservation_count",
              "transaction_count",
              "highest_durable_stage"),
          Set.of(
              "proposal_view",
              "proposal_hash",
              "descriptor_hash",
              "executable_payload_hash",
              "source_bundle_hash",
              "merge_entry_hash",
              "application_block_height",
              "application_block_hash",
              "stuck_reason"),
          rowContext);
      rows.add(
          new AutonomousLaneExecution(
              SumeragiJsonSupport.u32(row.get("lane_id"), rowContext + ".lane_id")
                  .longValueExact(),
              SumeragiJsonSupport.u64(row.get("dataspace_id"), rowContext + ".dataspace_id"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("lane_incarnation"), rowContext + ".lane_incarnation"),
              SumeragiJsonSupport.positiveU64(
                  row.get("lane_block_height"), rowContext + ".lane_block_height"),
              SumeragiJsonSupport.u64(
                  row.get("lane_block_view"), rowContext + ".lane_block_view"),
              SumeragiJsonSupport.positiveU64(
                  row.get("proposal_height"), rowContext + ".proposal_height"),
              optionalU64(row.get("proposal_view"), rowContext + ".proposal_view"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("reservation_owner_hash"), rowContext + ".reservation_owner_hash"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("proposal_identity_hash"), rowContext + ".proposal_identity_hash"),
              SumeragiJsonSupport.nonzeroHash(
                  row.get("reservation_group_hash"), rowContext + ".reservation_group_hash"),
              optionalNonzeroHash(row.get("proposal_hash"), rowContext + ".proposal_hash"),
              optionalNonzeroHash(row.get("descriptor_hash"), rowContext + ".descriptor_hash"),
              optionalNonzeroHash(
                  row.get("executable_payload_hash"), rowContext + ".executable_payload_hash"),
              optionalNonzeroHash(
                  row.get("source_bundle_hash"), rowContext + ".source_bundle_hash"),
              optionalNonzeroHash(
                  row.get("merge_entry_hash"), rowContext + ".merge_entry_hash"),
              optionalPositiveU64(
                  row.get("application_block_height"),
                  rowContext + ".application_block_height"),
              optionalNonzeroHash(
                  row.get("application_block_hash"), rowContext + ".application_block_hash"),
              SumeragiJsonSupport.unsigned(
                      row.get("reservation_count"), BigInteger.valueOf(4_096L),
                      rowContext + ".reservation_count")
                  .longValueExact(),
              SumeragiJsonSupport.unsigned(
                      row.get("transaction_count"), BigInteger.valueOf(4_096L),
                      rowContext + ".transaction_count", true)
                  .longValueExact(),
              AutonomousLaneExecutionStage.fromWireName(
                  SumeragiJsonSupport.string(
                      row.get("highest_durable_stage"),
                      rowContext + ".highest_durable_stage")),
              row.get("stuck_reason") == null
                  ? null
                  : AutonomousLaneExecutionStuckReason.fromWireName(
                      SumeragiJsonSupport.string(
                          row.get("stuck_reason"), rowContext + ".stuck_reason"))));
    }
    return Collections.unmodifiableList(rows);
  }

  private static BigInteger optionalU64(final Object value, final String context) {
    return value == null ? null : SumeragiJsonSupport.u64(value, context);
  }

  private static BigInteger optionalPositiveU64(final Object value, final String context) {
    return value == null ? null : SumeragiJsonSupport.positiveU64(value, context);
  }

  private static String optionalNonzeroHash(final Object value, final String context) {
    return value == null ? null : SumeragiJsonSupport.nonzeroHash(value, context);
  }

  private static void validateNativeAmxDiagnosticsEvidence(
      final List<?> settlements, final List<?> relays) {
    for (int index = 0; index < settlements.size(); index++) {
      validateNativeAmxSettlementEvidence(
          settlements.get(index), "lane_settlement_commitments[" + index + "]");
    }
    for (int index = 0; index < relays.size(); index++) {
      final String field = "lane_relay_envelopes[" + index + "]";
      final Map<String, Object> relay = jsonObject(relays.get(index), field);
      validateNativeAmxSettlementEvidence(
          relay.get("settlement_commitment"), field + ".settlement_commitment");
    }
  }

  private static void validateNativeAmxSettlementEvidence(
      final Object value, final String field) {
    final Map<String, Object> settlement = jsonObject(value, field);
    final Object nativeReceipts = settlement.get("native_amx_receipts");
    require(
        nativeReceipts instanceof List,
        field + ".native_amx_receipts must be a JSON array");
    if (((List<?>) nativeReceipts).isEmpty()) {
      return;
    }
    try {
      NativeAmxV2Models.parseReceiptGroup(settlement);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          field + " contains invalid Native AMX V2 evidence", error);
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> jsonObject(final Object value, final String field) {
    require(value instanceof Map, field + " must be a JSON object");
    return (Map<String, Object>) value;
  }

  private static <T> List<T> boundedCopy(
      final List<T> values, final String field, final int maximum) {
    require(values != null, field + " must not be null");
    require(values.size() <= maximum, field + " exceeds its bounded row limit");
    final ArrayList<T> copy = new ArrayList<>(values);
    for (final T value : copy) {
      require(value != null, field + " must not contain null rows");
    }
    return Collections.unmodifiableList(copy);
  }

  private static List<?> boundedOpaqueCopy(
      final List<?> values, final String field, final int maximum) {
    require(values != null, field + " must not be null");
    require(values.size() <= maximum, field + " exceeds its bounded row limit");
    final ArrayList<Object> copy = new ArrayList<>();
    for (int index = 0; index < values.size(); index++) {
      final Object value = values.get(index);
      require(value instanceof Map<?, ?>, field + " must contain only JSON objects");
      copy.add(SumeragiJsonSupport.deepFreeze(value, field + "[" + index + "]"));
    }
    return Collections.unmodifiableList(copy);
  }

  private static int compareAutonomous(
      final AutonomousLaneExecution left, final AutonomousLaneExecution right) {
    int result = Long.compare(left.laneId, right.laneId);
    if (result == 0) result = left.dataspaceId.compareTo(right.dataspaceId);
    if (result == 0) result = left.laneIncarnation.compareTo(right.laneIncarnation);
    if (result == 0) result = left.laneBlockHeight.compareTo(right.laneBlockHeight);
    if (result == 0) result = left.laneBlockView.compareTo(right.laneBlockView);
    if (result == 0) result = left.proposalHeight.compareTo(right.proposalHeight);
    if (result == 0) result = left.proposalIdentityHash.compareTo(right.proposalIdentityHash);
    return result;
  }

  private static int compareRoute(
      final NativeAmxParticipantApplication left,
      final NativeAmxParticipantApplication right) {
    int result = Long.compare(left.laneId, right.laneId);
    if (result != 0) {
      return result;
    }
    result = left.dataspaceId.compareTo(right.dataspaceId);
    if (result != 0) {
      return result;
    }
    return left.laneIncarnation.compareTo(right.laneIncarnation);
  }

  private static void requireCanonicalNonzeroHash(final String value, final String field) {
    require(
        value != null && CANONICAL_HASH.matcher(value).matches(),
        field + " must be a canonical Iroha hash literal");
    final byte[] bytes = HashLiteral.decode(value);
    boolean nonzero = false;
    for (final byte item : bytes) {
      nonzero |= item != 0;
    }
    require(nonzero, field + " must not be the zero hash");
    require((bytes[bytes.length - 1] & 1) == 1, field + " has an invalid Iroha hash marker bit");
  }

  private static void requireUnsigned64(final BigInteger value, final String field) {
    require(
        value != null && value.signum() >= 0 && value.compareTo(U64_MAX) <= 0,
        field + " must fit in unsigned 64-bit range");
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }
}
