// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.math.BigInteger;
import kotlinx.serialization.json.Json;
import kotlinx.serialization.json.JsonObject;
import kotlinx.serialization.json.JsonArray;
import kotlinx.serialization.json.JsonElement;
import kotlinx.serialization.json.JsonPrimitive;
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
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.hyperledger.iroha.sdk.client.JsonEncoder;
import org.hyperledger.iroha.sdk.core.util.HashLiteral;
import org.junit.jupiter.api.Test;

/** Sumeragi diagnostics model parity tests. */
public final class SumeragiDiagnosticsModelsTests {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);

  @Test
  public void autonomousExecutionStagesAndConflictAreExact() {
    final SumeragiAutonomousLaneExecution row =
        new SumeragiAutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2),
            hash(0x70), hash(0x71), hash(0x72), hash(0x73),
            hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
            BigInteger.valueOf(12), hash(0x7d), 2, 2,
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    assertEquals(1, new SumeragiAutonomousLaneExecutions(Arrays.asList(row)).rows.size());
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecutions(Arrays.asList(row, row)));
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecutions(Collections.nCopies(129, row)));
    assertEquals(
        SumeragiAutonomousLaneExecutionStage.CONFLICT,
        Json.Default.decodeFromString(SumeragiAutonomousLaneExecutionStage.Companion.serializer(), "\"conflict\""));
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2),
            hash(0x70), hash(0x71), hash(0x72), hash(0x73),
            hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
            BigInteger.valueOf(12), hash(0x7d), 1, 2,
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
    new SumeragiAutonomousLaneExecution(
        3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
        BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2),
        hash(0x70), hash(0x71), hash(0x72), hash(0x73),
        hash(0x75), null, null, null, null, null, 1, 2,
        SumeragiAutonomousLaneExecutionStage.CONFLICT,
        SumeragiAutonomousLaneExecutionStuckReason.EVIDENCE_CONFLICT);
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecution(
            3, BigInteger.valueOf(8), hash(0x54), BigInteger.valueOf(8),
            BigInteger.ONE, BigInteger.TEN, BigInteger.valueOf(2),
            hash(0x70), hash(0x71), hash(0x72), hash(0x73),
            hash(0x75), null, null, null, null, null, 2, 2,
            SumeragiAutonomousLaneExecutionStage.CONFLICT,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_MERGE_SELECTION));
  }

  @Test
  public void autonomousReservationsRequireProvisionalIdentityAndExactGeometry() {
    for (int field = 0; field < 3; field++) {
      final String[] missing = {hash(0x70), hash(0x71), hash(0x72)};
      missing[field] = null;
      // Kotlin rejects Java nulls at the non-null constructor boundary.
      assertThrows(
          NullPointerException.class,
          () -> autonomousExecution(
              BigInteger.valueOf(2), missing[0], missing[1], missing[2],
              hash(0x73), hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
              BigInteger.valueOf(12), hash(0x7d), 2,
              SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
              SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
      final String[] bare = {hash(0x70), hash(0x71), hash(0x72)};
      bare[field] = String.join("", Collections.nCopies(32, "ab"));
      assertThrows(
          IllegalArgumentException.class,
          () -> autonomousExecution(
              BigInteger.valueOf(2), bare[0], bare[1], bare[2],
              hash(0x73), hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
              BigInteger.valueOf(12), hash(0x7d), 2,
              SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
              SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
      final String[] zero = {hash(0x70), hash(0x71), hash(0x72)};
      zero[field] = "hash:" + String.join("", Collections.nCopies(32, "00")) + "#6A0A";
      assertThrows(
          IllegalArgumentException.class,
          () -> autonomousExecution(
              BigInteger.valueOf(2), zero[0], zero[1], zero[2],
              hash(0x73), hash(0x75), hash(0x77), hash(0x79), hash(0x7b),
              BigInteger.valueOf(12), hash(0x7d), 2,
              SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
              SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
    }

    final SumeragiAutonomousLaneExecution reservations = autonomousExecution(
        null, hash(0x70), hash(0x71), hash(0x72), null, null, null, null, null,
        null, null, 2, SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
        SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD);
    assertEquals(null, reservations.getProposalView());
    assertEquals(null, reservations.getProposalHash());
    assertEquals(
        "awaiting_executable_payload",
        ((JsonPrimitive) Json.Default.parseToJsonElement(Json.Default.encodeToString(SumeragiAutonomousLaneExecutionStuckReason.Companion.serializer(), SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD))).getContent());
    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            BigInteger.ZERO, hash(0x70), hash(0x71), hash(0x72), null, null,
            null, null, null, null, null, 2,
            SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD));

    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            null, hash(0x70), hash(0x71), hash(0x72), null, null, null, null, null,
            null, null, 2, SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_PAYLOAD_AVAILABILITY));
    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            null, hash(0x70), hash(0x71), hash(0x72), hash(0x73), hash(0x75),
            null, null, null, null, null, 2,
            SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD));
    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            null, hash(0x70), hash(0x71), hash(0x72), null, null, hash(0x77),
            null, null, null, null, 2,
            SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD));
    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            null, hash(0x70), hash(0x71), hash(0x72), null, null, null, null, null,
            null, null, 1, SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD));

    assertThrows(
        IllegalArgumentException.class,
        () -> autonomousExecution(
            BigInteger.valueOf(2), hash(0x70), hash(0x71), hash(0x72), hash(0x73),
            null, hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12),
            hash(0x7d), 2,
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE));
    assertEquals(
        null,
        autonomousExecution(
            null, hash(0x70), hash(0x71), hash(0x72), hash(0x73), hash(0x75),
            hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12), hash(0x7d), 2,
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE).getProposalView());

    final SumeragiAutonomousLaneExecution first = autonomousExecution(
        BigInteger.valueOf(2), hash(0x70), hash(0x71), hash(0x72), hash(0x73),
        hash(0x75), hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12),
        hash(0x7d), 2, SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    final SumeragiAutonomousLaneExecution sameProvisional = autonomousExecution(
        BigInteger.valueOf(2), hash(0x70), hash(0x71), hash(0x72), hash(0x7e),
        hash(0x7f), hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12),
        hash(0x7d), 2, SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecutions(Arrays.asList(first, sameProvisional)));
    final SumeragiAutonomousLaneExecution descendingFirst = autonomousExecution(
        BigInteger.valueOf(2), hash(0x70), hash(0x90), hash(0x72), hash(0x73),
        hash(0x75), hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12),
        hash(0x7d), 2, SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    final SumeragiAutonomousLaneExecution descendingSecond = autonomousExecution(
        BigInteger.valueOf(2), hash(0x70), hash(0x80), hash(0x72), hash(0x73),
        hash(0x75), hash(0x77), hash(0x79), hash(0x7b), BigInteger.valueOf(12),
        hash(0x7d), 2, SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE);
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiAutonomousLaneExecutions(Arrays.asList(descendingFirst, descendingSecond)));
  }

  @Test
  public void stateNamesMirrorTheToriiContract() {
    assertEquals(
        "certified_pending_carrier",
        ((JsonPrimitive) Json.Default.parseToJsonElement(Json.Default.encodeToString(SumeragiNativeAmxParticipantApplicationState.Companion.serializer(), SumeragiNativeAmxParticipantApplicationState.CERTIFIED_PENDING_CARRIER))).getContent());
    assertEquals(
        SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED,
        Json.Default.decodeFromString(SumeragiNativeAmxParticipantApplicationState.Companion.serializer(), "\"durably_applied\""));
    assertThrows(
        IllegalArgumentException.class,
        () -> Json.Default.decodeFromString(SumeragiNativeAmxParticipantApplicationState.Companion.serializer(), "\"applied\""));
  }

  @Test
  public void vectorEnforcesBoundAndCanonicalOrder() {
    final SumeragiNativeAmxParticipantApplications ordered =
        new SumeragiNativeAmxParticipantApplications(Arrays.asList(application(3), application(4)));
    assertEquals(2, ordered.rows.size());

    final List<SumeragiNativeAmxParticipantApplication> oversized = new ArrayList<>();
    for (int index = 0;
        index < SumeragiDiagnosticsModelsKt.SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX + 1;
        index++) {
      oversized.add(application(3));
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> new SumeragiNativeAmxParticipantApplications(oversized));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new SumeragiNativeAmxParticipantApplications(
                Arrays.asList(application(4), application(3))));
  }

  @Test
  public void rowEnforcesCarrierStateGeometryAndGroupBound() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            application(
                3,
                SumeragiDiagnosticsModelsKt.SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX + 1,
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

    final String geometryError =
        "Native AMX participant state and application block identity disagree";
    for (final SumeragiNativeAmxParticipantApplicationState state :
        Arrays.asList(
            SumeragiNativeAmxParticipantApplicationState.CERTIFIED_PENDING_CARRIER,
            SumeragiNativeAmxParticipantApplicationState.CONFLICT)) {
      assertEquals(state, application(3, 2, null, null, state).getState());
      final IllegalArgumentException error =
          assertThrows(
              IllegalArgumentException.class,
              () -> application(3, 2, hash(0x77), BigInteger.valueOf(15L), state));
      assertEquals(geometryError, error.getMessage());
    }
    for (final SumeragiNativeAmxParticipantApplicationState state :
        Arrays.asList(
            SumeragiNativeAmxParticipantApplicationState.COMMITTED_EVIDENCE_PENDING,
            SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED)) {
      assertEquals(
          state,
          application(3, 2, hash(0x77), BigInteger.valueOf(15L), state).getState());
      final IllegalArgumentException error =
          assertThrows(
              IllegalArgumentException.class,
              () -> application(3, 2, null, null, state));
      assertEquals(geometryError, error.getMessage());
    }
  }

  @Test
  public void rowAcceptsFullUnsigned64DomainAndOrdersDataspacesExactly() {
    final SumeragiNativeAmxParticipantApplication maximum =
        application(
            3,
            U64_MAX,
            U64_MAX,
            U64_MAX,
            U64_MAX.subtract(BigInteger.ONE),
            2,
            hash(0x77),
            U64_MAX);
    assertEquals(U64_MAX, maximum.getDataspaceId());
    assertEquals(U64_MAX, maximum.getParticipantHeight());
    assertEquals(U64_MAX, maximum.getParticipantView());
    assertEquals(U64_MAX.subtract(BigInteger.ONE), maximum.getPredecessorHeight());
    assertEquals(U64_MAX, maximum.getApplicationBlockHeight());

    final SumeragiNativeAmxParticipantApplication previousDataspace =
        application(
            3,
            U64_MAX.subtract(BigInteger.ONE),
            BigInteger.valueOf(8L),
            BigInteger.ONE,
            BigInteger.valueOf(7L),
            2,
            hash(0x77),
            BigInteger.valueOf(15L));
    final SumeragiNativeAmxParticipantApplications ordered =
        new SumeragiNativeAmxParticipantApplications(Arrays.asList(previousDataspace, maximum));
    assertEquals(Arrays.asList(previousDataspace, maximum), ordered.rows);
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
    assertEquals(BigInteger.ONE, status.getTxQueueCapacity());
    assertEquals(0, status.getNativeAmxParticipantApplications().size());
    assertEquals(0, status.getAutonomousLaneExecutions().size());

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
                Collections.nCopies(SumeragiDiagnosticsModelsKt.SUMERAGI_DIAGNOSTIC_LANES_MAX + 1, new JsonObject(Collections.emptyMap())),
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

    assertEquals(jsonObjects(Collections.singletonList(settlement)), status.getLaneSettlementCommitments());
    assertEquals(jsonObjects(Collections.singletonList(relay)), status.getLaneRelayEnvelopes());
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
      final List<JsonObject> laneCommitments,
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
        jsonObjects(laneSettlementCommitments),
        jsonObjects(laneRelayEnvelopes),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        0,
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList(),
        Collections.emptyList());
  }

  private static List<JsonObject> jsonObjects(final List<?> values) {
    final List<JsonObject> result = new ArrayList<>();
    for (Object value : values) {
      result.add((JsonObject) Json.Default.parseToJsonElement(JsonEncoder.encode(value)));
    }
    return result;
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

  private static SumeragiPipelineExecutionStatus pipeline() {
    final BigInteger zero = BigInteger.ZERO;
    return new SumeragiPipelineExecutionStatus(
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

  private static SumeragiNativeAmxParticipantApplication application(final long laneId) {
    return application(laneId, 2, hash(0x77), BigInteger.valueOf(15L));
  }

  private static SumeragiNativeAmxParticipantApplication application(
      final long laneId,
      final long sourceCount,
      final String applicationBlockHash,
      final BigInteger applicationBlockHeight) {
    return application(
        laneId,
        sourceCount,
        applicationBlockHash,
        applicationBlockHeight,
        SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED);
  }

  private static SumeragiNativeAmxParticipantApplication application(
      final long laneId,
      final long sourceCount,
      final String applicationBlockHash,
      final BigInteger applicationBlockHeight,
      final SumeragiNativeAmxParticipantApplicationState state) {
    return new SumeragiNativeAmxParticipantApplication(
        laneId,
        BigInteger.valueOf(8L),
        hash(0x51 + (int) laneId),
        BigInteger.valueOf(8L),
        BigInteger.ONE,
        BigInteger.valueOf(7L),
        hash(0x61),
        hash(0x71),
        hash(0x73),
        hash(0x75),
        sourceCount,
        applicationBlockHeight,
        applicationBlockHash,
        state);
  }

  private static SumeragiNativeAmxParticipantApplication application(
      final long laneId,
      final BigInteger dataspaceId,
      final BigInteger participantHeight,
      final BigInteger participantView,
      final BigInteger predecessorHeight,
      final long sourceCount,
      final String applicationBlockHash,
      final BigInteger applicationBlockHeight) {
    return new SumeragiNativeAmxParticipantApplication(
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
        SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED);
  }

  private static SumeragiAutonomousLaneExecution autonomousExecution(
      final BigInteger proposalView,
      final String reservationOwnerHash,
      final String proposalIdentityHash,
      final String reservationGroupHash,
      final String proposalHash,
      final String descriptorHash,
      final String executablePayloadHash,
      final String sourceBundleHash,
      final String mergeEntryHash,
      final BigInteger applicationBlockHeight,
      final String applicationBlockHash,
      final long reservationCount,
      final SumeragiAutonomousLaneExecutionStage stage,
      final SumeragiAutonomousLaneExecutionStuckReason reason) {
    return new SumeragiAutonomousLaneExecution(
        3,
        BigInteger.valueOf(8),
        hash(0x54),
        BigInteger.valueOf(8),
        BigInteger.ONE,
        BigInteger.TEN,
        proposalView,
        reservationOwnerHash,
        proposalIdentityHash,
        reservationGroupHash,
        proposalHash,
        descriptorHash,
        executablePayloadHash,
        sourceBundleHash,
        mergeEntryHash,
        applicationBlockHeight,
        applicationBlockHash,
        reservationCount,
        2,
        stage,
        reason);
  }

  private static String hash(final int seed) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) seed);
    return HashLiteral.canonicalize(bytes);
  }
}
