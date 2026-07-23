// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplication;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplications;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.junit.Test;

/** Native AMX participant diagnostics model parity tests. */
public final class SumeragiDiagnosticsModelsTests {
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
                15L));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                2,
                null,
                15L));
  }

  private static NativeAmxParticipantApplication application(final long laneId) {
    return application(laneId, 2, hash(0x77), 15L);
  }

  private static NativeAmxParticipantApplication application(
      final long laneId,
      final long sourceCount,
      final String applicationBlockHash,
      final Long applicationBlockHeight) {
    return new NativeAmxParticipantApplication(
        laneId,
        8,
        hash(0x51 + (int) laneId),
        8,
        1,
        7,
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
