// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecution;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutionStage;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutionStuckReason;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutions;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplication;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplications;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.junit.Test;

/** Sumeragi diagnostics model parity tests. */
public final class SumeragiDiagnosticsModelsTests {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);

  @Test
  public void autonomousExecutionStagesAndConflictAreExact() {
    final AutonomousLaneExecution row =
        new AutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2), hash(0x73),
            hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
            BigInteger.valueOf(12), hash(0x7d), 2, 2,
            AutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            AutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    assertEquals(1, new AutonomousLaneExecutions(Arrays.asList(row)).rows().size());
    assertThrows(
        IllegalArgumentException.class,
        () -> new AutonomousLaneExecutions(Arrays.asList(row, row)));
    assertThrows(
        IllegalArgumentException.class,
        () -> new AutonomousLaneExecutions(Collections.nCopies(129, row)));
    assertEquals(
        AutonomousLaneExecutionStage.CONFLICT,
        AutonomousLaneExecutionStage.fromWireName("conflict"));
    assertThrows(
        IllegalArgumentException.class,
        () -> new AutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2), hash(0x73),
            hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
            BigInteger.valueOf(12), hash(0x7d), 1, 2,
            AutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            AutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
    new AutonomousLaneExecution(
        3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
        BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2), hash(0x73),
        hash(0x75), null, null, null, null, null, 1, 2,
        AutonomousLaneExecutionStage.CONFLICT,
        AutonomousLaneExecutionStuckReason.EVIDENCE_CONFLICT);
    assertThrows(
        IllegalArgumentException.class,
        () -> new AutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2), hash(0x73),
            hash(0x75), null, null, null, null, null, 2, 2,
            AutonomousLaneExecutionStage.CONFLICT,
            AutonomousLaneExecutionStuckReason.AWAITING_MERGE_SELECTION));
  }

  @Test
  public void stateNamesMirrorTheToriiContract() {
    assertEquals(
        "certified_pending_carrier",
        NativeAmxParticipantApplicationState.CERTIFIED_PENDING_CARRIER.wireName());
    assertEquals(
        NativeAmxParticipantApplicationState.DURABLY_APPLIED,
        NativeAmxParticipantApplicationState.fromWireName("durably_applied"));
    assertThrows(
        IllegalArgumentException.class,
        () -> NativeAmxParticipantApplicationState.fromWireName("applied"));
  }

  @Test
  public void vectorEnforcesBoundAndCanonicalOrder() {
    final NativeAmxParticipantApplications ordered =
        new NativeAmxParticipantApplications(Arrays.asList(application(3), application(4)));
    assertEquals(2, ordered.rows().size());

    final List<NativeAmxParticipantApplication> oversized = new ArrayList<>();
    for (int index = 0;
        index < SumeragiDiagnosticsModels.NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX + 1;
        index++) {
      oversized.add(application(3));
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> new NativeAmxParticipantApplications(oversized));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new NativeAmxParticipantApplications(
                Arrays.asList(application(4), application(3))));
  }

  @Test
  public void rowRejectsIncompleteCarrierAndOversizedGroup() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                SumeragiDiagnosticsModels.NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX + 1,
                hash(0x77),
                BigInteger.valueOf(15L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                2,
                null,
                BigInteger.valueOf(15L)));
  }

  @Test
  public void rowAcceptsFullUnsigned64DomainAndOrdersDataspacesExactly() {
    final NativeAmxParticipantApplication maximum =
        application(
            3,
            U64_MAX,
            U64_MAX,
            U64_MAX,
            U64_MAX.subtract(BigInteger.ONE),
            2,
            hash(0x77),
            U64_MAX);
    assertEquals(U64_MAX, maximum.dataspaceId());
    assertEquals(U64_MAX, maximum.participantHeight());
    assertEquals(U64_MAX, maximum.participantView());
    assertEquals(U64_MAX.subtract(BigInteger.ONE), maximum.predecessorHeight());
    assertEquals(U64_MAX, maximum.applicationBlockHeight());

    final NativeAmxParticipantApplication previousDataspace =
        application(
            3,
            U64_MAX.subtract(BigInteger.ONE),
            BigInteger.valueOf(8L),
            BigInteger.ONE,
            BigInteger.valueOf(7L),
            2,
            hash(0x77),
            BigInteger.valueOf(15L));
    final NativeAmxParticipantApplications ordered =
        new NativeAmxParticipantApplications(Arrays.asList(previousDataspace, maximum));
    assertEquals(Arrays.asList(previousDataspace, maximum), ordered.rows());
  }

  @Test
  public void rowRejectsValuesOutsideUnsigned64Domain() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                U64_MAX.add(BigInteger.ONE),
                BigInteger.valueOf(8L),
                BigInteger.ONE,
                BigInteger.valueOf(7L),
                2,
                hash(0x77),
                BigInteger.valueOf(15L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                BigInteger.valueOf(8L),
                BigInteger.valueOf(8L),
                BigInteger.ONE.negate(),
                BigInteger.valueOf(7L),
                2,
                hash(0x77),
                BigInteger.valueOf(15L)));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                BigInteger.valueOf(8L),
                BigInteger.valueOf(8L),
                BigInteger.ONE,
                BigInteger.valueOf(7L),
                2,
                hash(0x77),
                U64_MAX.add(BigInteger.ONE)));
  }

  private static NativeAmxParticipantApplication application(final long laneId) {
    return application(laneId, 2, hash(0x77), BigInteger.valueOf(15L));
  }

  private static NativeAmxParticipantApplication application(
      final long laneId,
      final long sourceCount,
      final String applicationBlockHash,
      final BigInteger applicationBlockHeight) {
    return application(
        laneId,
        BigInteger.valueOf(8L),
        BigInteger.valueOf(8L),
        BigInteger.ONE,
        BigInteger.valueOf(7L),
        sourceCount,
        applicationBlockHash,
        applicationBlockHeight);
  }

  private static NativeAmxParticipantApplication application(
      final long laneId,
      final BigInteger dataspaceId,
      final BigInteger participantHeight,
      final BigInteger participantView,
      final BigInteger predecessorHeight,
      final long sourceCount,
      final String applicationBlockHash,
      final BigInteger applicationBlockHeight) {
    return new NativeAmxParticipantApplication(
        laneId,
        dataspaceId,
        hash(0x51 + (int) laneId),
        participantHeight,
        participantView,
        predecessorHeight,
        hash(0x61),
        hash(0x71),
        hash(0x73),
        hash(0x75),
        sourceCount,
        applicationBlockHeight,
        applicationBlockHash,
        NativeAmxParticipantApplicationState.DURABLY_APPLIED);
  }

  private static String hash(final int seed) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) seed);
    return HashLiteral.canonicalize(bytes);
  }
}
