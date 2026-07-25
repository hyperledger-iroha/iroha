// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecution;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutionStage;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutionStuckReason;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.AutonomousLaneExecutions;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplication;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.NativeAmxParticipantApplications;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.PipelineExecutionStatus;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.SumeragiDiagnosticsStatus;
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

  @Test
  public void completeDiagnosticsModelMirrorsRequiredVectorsAndBounds() {
    final SumeragiDiagnosticsStatus status =
        diagnostics(BigInteger.ZERO, BigInteger.ONE, Collections.emptyList(), 0, Collections.emptyList());
    assertEquals(BigInteger.ONE, status.txQueueCapacity());
    assertEquals(0, status.nativeAmxParticipantApplications().size());
    assertEquals(0, status.autonomousLaneExecutions().size());

    assertThrows(
        IllegalArgumentException.class,
        () ->
            diagnostics(
                BigInteger.valueOf(2),
                BigInteger.ONE,
                Collections.emptyList(),
                0,
                Collections.emptyList()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            diagnostics(
                BigInteger.ZERO,
                BigInteger.ONE,
                Collections.nCopies(SumeragiDiagnosticsModels.DIAGNOSTIC_LANES_MAX + 1, new Object()),
                0,
                Collections.emptyList()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            diagnostics(
                BigInteger.ZERO,
                BigInteger.ONE,
                Collections.emptyList(),
                1,
                Collections.emptyList()));
  }

  @Test
  public void completeDiagnosticsModelValidatesNativeAmxSettlementAndRelayEvidence()
      throws Exception {
    final Map<String, Object> settlement = nativeAmxReceiptGroupFixture();
    final Map<String, Object> relay = new LinkedHashMap<>();
    relay.put("settlement_commitment", settlement);

    final SumeragiDiagnosticsStatus status =
        diagnosticsWithNativeEvidence(
            Collections.singletonList(settlement), Collections.singletonList(relay));

    assertEquals(Collections.singletonList(settlement), status.laneSettlementCommitments());
    assertEquals(Collections.singletonList(relay), status.laneRelayEnvelopes());
  }

  @Test
  public void completeDiagnosticsModelRejectsMalformedNativeAmxSettlementAndRelayEvidence()
      throws Exception {
    final Map<String, Object> malformed =
        malformedNativeAmxReceiptGroup(nativeAmxReceiptGroupFixture());

    final IllegalArgumentException directError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                diagnosticsWithNativeEvidence(
                    Collections.singletonList(malformed), Collections.emptyList()));
    assertStrictNativeAmxFailure(directError);

    final Map<String, Object> relay = new LinkedHashMap<>();
    relay.put("settlement_commitment", malformed);
    final IllegalArgumentException relayError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                diagnosticsWithNativeEvidence(
                    Collections.emptyList(), Collections.singletonList(relay)));
    assertStrictNativeAmxFailure(relayError);
  }

  private static SumeragiDiagnosticsStatus diagnostics(
      final BigInteger depth,
      final BigInteger capacity,
      final List<?> laneCommitments,
      final long sealedTotal,
      final List<String> sealedAliases) {
    return new SumeragiDiagnosticsStatus(
        pipeline(),
        depth,
        capacity,
        BigInteger.ZERO,
        BigInteger.ONE,
        false,
        false,
        false,
        false,
        BigInteger.ZERO,
        null,
        laneCommitments,
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        sealedTotal,
        sealedAliases,
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList());
  }

  private static SumeragiDiagnosticsStatus diagnosticsWithNativeEvidence(
      final List<?> laneSettlementCommitments, final List<?> laneRelayEnvelopes) {
    return new SumeragiDiagnosticsStatus(
        pipeline(),
        BigInteger.ZERO,
        BigInteger.ONE,
        BigInteger.ZERO,
        BigInteger.ONE,
        false,
        false,
        false,
        false,
        BigInteger.ZERO,
        null,
        Collections.emptyList(),
        Collections.emptyList(),
        laneSettlementCommitments,
        laneRelayEnvelopes,
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        0,
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList());
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> malformedNativeAmxReceiptGroup(
      final Map<String, Object> group) {
    final Map<String, Object> malformed = new LinkedHashMap<>(group);
    final List<Object> receipts =
        new ArrayList<>((List<Object>) group.get("native_amx_receipts"));
    final Map<String, Object> first =
        new LinkedHashMap<>((Map<String, Object>) receipts.get(0));
    first.put("version", 1L);
    receipts.set(0, first);
    malformed.put("native_amx_receipts", receipts);
    return malformed;
  }

  private static void assertStrictNativeAmxFailure(final IllegalArgumentException error) {
    assertTrue(error.getCause() instanceof IllegalArgumentException);
    assertTrue(error.getCause().getMessage().contains("version must equal 2"));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> nativeAmxReceiptGroupFixture() throws Exception {
    final Map<String, Object> fixture =
        (Map<String, Object>)
            JsonParser.parse(
                new String(
                    Files.readAllBytes(nativeAmxFixturePath()), StandardCharsets.UTF_8));
    return (Map<String, Object>)
        ((Map<String, Object>) fixture.get("golden")).get("receipt_group");
  }

  private static Path nativeAmxFixturePath() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate =
          current.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json");
      if (Files.isRegularFile(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new AssertionError(
        "fixtures/sumeragi_v2/native_amx_v2_grouped.json was not found");
  }

  private static PipelineExecutionStatus pipeline() {
    final BigInteger zero = BigInteger.ZERO;
    return new PipelineExecutionStatus(
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero,
        zero);
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
