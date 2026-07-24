// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Mirrored public models for Sumeragi operational diagnostics. */
public final class SumeragiDiagnosticsModels {
  /** Maximum rows in {@code native_amx_participant_applications}. */
  public static final int NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX = 1_024;

  /** Maximum grouped source count represented by one diagnostics row. */
  public static final long NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX = 4_096L;
  public static final int AUTONOMOUS_LANE_EXECUTIONS_MAX = 128;

  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);
  private static final Pattern CANONICAL_HASH =
      Pattern.compile("^hash:[0-9A-F]{64}#[0-9A-F]{4}$");

  private SumeragiDiagnosticsModels() {}

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
      require(
          state != NativeAmxParticipantApplicationState.DURABLY_APPLIED
              || applicationBlockHeight != null,
          "durably applied Native AMX evidence requires an application block");

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
        final String proposalHash, final String descriptorHash,
        final String executablePayloadHash, final String sourceBundleHash,
        final String mergeEntryHash, final BigInteger applicationBlockHeight,
        final String applicationBlockHash, final long reservationCount,
        final long transactionCount, final AutonomousLaneExecutionStage highestDurableStage,
        final AutonomousLaneExecutionStuckReason stuckReason) {
      require(laneId >= 0 && laneId <= 0xffff_ffffL, "laneId out of range");
      for (final BigInteger coordinate :
          new BigInteger[] {dataspaceId, laneBlockHeight, laneBlockView, proposalHeight, proposalView}) {
        requireUnsigned64(coordinate, "autonomous execution coordinate");
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
      for (final String hash : new String[] {laneIncarnation, proposalHash, descriptorHash}) {
        requireCanonicalNonzeroHash(hash, "autonomous execution hash");
      }
      for (final String hash :
          new String[] {executablePayloadHash, sourceBundleHash, mergeEntryHash, applicationBlockHash}) {
        if (hash != null) requireCanonicalNonzeroHash(hash, "autonomous optional hash");
      }
      require(highestDurableStage != null, "stage must not be null");
      require(stuckReason == expectedStuckReason(highestDurableStage),
          "stage and stuck reason disagree");
      if (highestDurableStage != AutonomousLaneExecutionStage.CONFLICT) {
        require(reservationCount == transactionCount,
            "reservation and transaction counts disagree");
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
      this.proposalView = proposalView; this.proposalHash = proposalHash;
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

  private static int compareAutonomous(
      final AutonomousLaneExecution left, final AutonomousLaneExecution right) {
    int result = Long.compare(left.laneId, right.laneId);
    if (result == 0) result = left.dataspaceId.compareTo(right.dataspaceId);
    if (result == 0) result = left.laneIncarnation.compareTo(right.laneIncarnation);
    if (result == 0) result = left.laneBlockHeight.compareTo(right.laneBlockHeight);
    if (result == 0) result = left.laneBlockView.compareTo(right.laneBlockView);
    if (result == 0) result = left.proposalHeight.compareTo(right.proposalHeight);
    if (result == 0) result = left.proposalView.compareTo(right.proposalView);
    if (result == 0) result = left.proposalHash.compareTo(right.proposalHash);
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
