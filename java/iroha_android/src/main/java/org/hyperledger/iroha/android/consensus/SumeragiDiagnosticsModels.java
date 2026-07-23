// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Mirrored public models for Native AMX participant application diagnostics. */
public final class SumeragiDiagnosticsModels {
  /** Maximum rows in {@code native_amx_participant_applications}. */
  public static final int NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX = 1_024;

  /** Maximum grouped source count represented by one diagnostics row. */
  public static final long NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX = 4_096L;

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
    private final long dataspaceId;
    private final String laneIncarnation;
    private final long participantHeight;
    private final long participantView;
    private final long predecessorHeight;
    private final String predecessorDescriptorHash;
    private final String descriptorHash;
    private final String proposalHash;
    private final String settlementHash;
    private final long sourceCount;
    private final Long applicationBlockHeight;
    private final String applicationBlockHash;
    private final NativeAmxParticipantApplicationState state;

    public NativeAmxParticipantApplication(
        final long laneId,
        final long dataspaceId,
        final String laneIncarnation,
        final long participantHeight,
        final long participantView,
        final long predecessorHeight,
        final String predecessorDescriptorHash,
        final String descriptorHash,
        final String proposalHash,
        final String settlementHash,
        final long sourceCount,
        final Long applicationBlockHeight,
        final String applicationBlockHash,
        final NativeAmxParticipantApplicationState state) {
      require(laneId >= 0 && laneId <= 0xffff_ffffL, "laneId must be an unsigned 32-bit value");
      require(dataspaceId >= 0, "dataspaceId must be non-negative");
      require(
          participantHeight > 0 && participantView >= 0,
          "participant height must be positive and view must be non-negative");
      require(
          predecessorHeight >= 0
              && predecessorHeight < Long.MAX_VALUE
              && predecessorHeight + 1 == participantHeight
              && (predecessorHeight == 0) == (predecessorDescriptorHash == null),
          "Native AMX participant predecessor geometry is inconsistent");
      require(
          sourceCount >= 1 && sourceCount <= NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX,
          "Native AMX participant source count is out of bounds");
      require(
          (applicationBlockHeight == null) == (applicationBlockHash == null),
          "application block height and hash must appear together");
      require(
          applicationBlockHeight == null || applicationBlockHeight > 0,
          "application block height must be positive");
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
    public long dataspaceId() { return dataspaceId; }
    public String laneIncarnation() { return laneIncarnation; }
    public long participantHeight() { return participantHeight; }
    public long participantView() { return participantView; }
    public long predecessorHeight() { return predecessorHeight; }
    public String predecessorDescriptorHash() { return predecessorDescriptorHash; }
    public String descriptorHash() { return descriptorHash; }
    public String proposalHash() { return proposalHash; }
    public String settlementHash() { return settlementHash; }
    public long sourceCount() { return sourceCount; }
    public Long applicationBlockHeight() { return applicationBlockHeight; }
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

  private static int compareRoute(
      final NativeAmxParticipantApplication left,
      final NativeAmxParticipantApplication right) {
    int result = Long.compare(left.laneId, right.laneId);
    if (result != 0) {
      return result;
    }
    result = Long.compare(left.dataspaceId, right.dataspaceId);
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

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }
}
