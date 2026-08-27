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
import java.util.Collections;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.consensus.SumeragiDiagnosticsModels.SumeragiDiagnosticsStatus;
import org.junit.Test;

/** Strict operational diagnostics parsing tests. */
public final class SumeragiDiagnosticsParsingTests {
  @Test
  public void parserPreservesTypedNativeAndAutonomousRowsAndFullU64Values() {
    final String maximum = "18446744073709551615";
    final SumeragiDiagnosticsStatus parsed =
        SumeragiDiagnosticsModels.parseDiagnostics(
            diagnosticsJson("[]", "[]", "[]", nativeRow(maximum), autonomousRow(maximum)));

    assertEquals(new BigInteger(maximum), parsed.txQueueMaxRetainedBytes());
    assertEquals(
        new BigInteger(maximum),
        parsed.nativeAmxParticipantApplications().get(0).dataspaceId());
    assertEquals(
        new BigInteger(maximum), parsed.autonomousLaneExecutions().get(0).dataspaceId());
    assertEquals(
        SumeragiDiagnosticsModels.NativeAmxParticipantApplicationState.DURABLY_APPLIED,
        parsed.nativeAmxParticipantApplications().get(0).state());
    assertEquals(
        SumeragiDiagnosticsModels.AutonomousLaneExecutionStage
            .KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        parsed.autonomousLaneExecutions().get(0).highestDurableStage());
  }

  @Test
  public void parserRejectsMalformedUtf8UnknownMissingAndNoncanonicalIntegers() {
    assertThrows(
        IllegalArgumentException.class,
        () -> SumeragiDiagnosticsModels.parseDiagnostics(new byte[0]));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SumeragiDiagnosticsModels.parseDiagnostics(
                new byte[(int) SumeragiStatusModels.DIAGNOSTICS_JSON_MAX_BYTES + 1]));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SumeragiDiagnosticsModels.parseDiagnostics(
                new byte[] {0x7b, 0x22, (byte) 0xc3, 0x28}));
    final String payload = diagnosticsJson("[]", "[]", "[]", "", "");
    assertRejected(payload.replaceFirst("\\{", "{\"legacy_round\":1,"));
    assertRejected(payload.replace(",\"autonomous_lane_executions\":[]", ""));
    assertRejected(payload.replaceFirst("\"tx_queue_depth\":0", "\"tx_queue_depth\":\"0\""));
    assertRejected(payload.replaceFirst("\"tx_queue_depth\":0", "\"tx_queue_depth\":-0"));
    assertRejected(
        payload.replaceFirst(
            "\"tx_queue_depth\":0", "\"tx_queue_depth\":18446744073709551616"));
  }

  @Test
  public void nposDiagnosticsAcceptOnlyTheFirstReleaseBeaconContext() {
    final String npos =
        "{\"epoch_length_blocks\":3600,\"epoch_seed\":["
            + String.join(",", Collections.nCopies(32, "1"))
            + "],\"prf_height\":7,\"prf_view\":2}";
    final String current =
        diagnosticsJson("[]", "[]", "[]", "", "")
            .replaceFirst("\\{", "{\"npos\":" + npos + ",");

    assertEquals(
        BigInteger.valueOf(3600),
        SumeragiDiagnosticsModels.parseDiagnostics(current).npos().epochLengthBlocks());
    for (final String retired :
        new String[] {"vrf_commit_deadline_offset", "vrf_reveal_deadline_offset"}) {
      final String hostile =
          current.replace("\"prf_height\":7", "\"" + retired + "\":1,\"prf_height\":7");
      assertRejected(hostile);
    }
  }

  @SuppressWarnings("unchecked")
  @Test
  public void parserDeepFreezesEveryAllowedOpaqueDiagnosticsValue() {
    final String opaque = "[{\"nested\":[{\"value\":1}]}]";
    final SumeragiDiagnosticsStatus parsed =
        SumeragiDiagnosticsModels.parseDiagnostics(
            diagnosticsJson(opaque, "[]", "[]", "", ""));
    final List<Object> lanes = (List<Object>) parsed.laneCommitments();
    final Map<String, Object> row = (Map<String, Object>) lanes.get(0);
    final List<Object> nested = (List<Object>) row.get("nested");
    final Map<String, Object> leaf = (Map<String, Object>) nested.get(0);

    assertThrows(UnsupportedOperationException.class, () -> lanes.clear());
    assertThrows(UnsupportedOperationException.class, () -> row.put("other", 2L));
    assertThrows(UnsupportedOperationException.class, () -> nested.clear());
    assertThrows(UnsupportedOperationException.class, () -> leaf.put("value", 2L));
  }

  @Test
  public void parserAcceptsOnlyStrictCurrentNativeAmxV2ReceiptGroups() throws Exception {
    final Map<String, Object> group = nativeReceiptGroupFixture();
    final String groupJson = JsonEncoder.encode(group);
    final String relay = "[{\"settlement_commitment\":" + groupJson + "}]";
    final String payload = diagnosticsJson("[]", "[" + groupJson + "]", relay, "", "");
    final SumeragiDiagnosticsStatus parsed =
        SumeragiDiagnosticsModels.parseDiagnostics(payload);
    assertEquals(1, parsed.laneSettlementCommitments().size());
    assertEquals(1, parsed.laneRelayEnvelopes().size());

    final String legacy = groupJson.replaceFirst("\"version\":2", "\"version\":1");
    final RuntimeException error =
        assertThrows(
            RuntimeException.class,
            () ->
                SumeragiDiagnosticsModels.parseDiagnostics(
                    diagnosticsJson("[]", "[" + legacy + "]", "[]", "", "")));
    assertTrue(
        generateCauseMessages(error).contains("version must equal 2"));
  }

  private static void assertRejected(final String payload) {
    assertThrows(RuntimeException.class, () -> SumeragiDiagnosticsModels.parseDiagnostics(payload));
  }

  private static String diagnosticsJson(
      final String laneCommitments,
      final String settlements,
      final String relays,
      final String nativeRow,
      final String autonomousRow) {
    final String maximum = "18446744073709551615";
    return "{"
        + "\"pipeline_execution\":" + pipeline() + ","
        + "\"tx_queue_depth\":0,\"tx_queue_capacity\":1,"
        + "\"tx_queue_retained_bytes\":0,"
        + "\"tx_queue_max_retained_bytes\":" + maximum + ","
        + "\"tx_queue_saturated\":false,"
        + "\"tx_queue_saturated_by_count\":false,"
        + "\"tx_queue_saturated_by_bytes\":false,"
        + "\"tx_queue_saturated_by_age\":false,"
        + "\"tx_queue_oldest_queued_age_ms\":0,"
        + "\"lane_commitments\":" + laneCommitments + ","
        + "\"dataspace_commitments\":[],"
        + "\"lane_settlement_commitments\":" + settlements + ","
        + "\"lane_relay_envelopes\":" + relays + ","
        + "\"lane_payload_ownerships\":[],"
        + "\"committed_lane_blocks\":[],"
        + "\"lane_block_sessions\":[],"
        + "\"lane_governance_sealed_total\":0,"
        + "\"lane_governance_sealed_aliases\":[],"
        + "\"lane_governance\":[],"
        + "\"native_amx_participant_applications\":[" + nativeRow + "],"
        + "\"autonomous_lane_executions\":[" + autonomousRow + "]}";
  }

  private static String pipeline() {
    return "{"
        + "\"tx_vertices_total\":0,\"tx_edges_total\":0,"
        + "\"overlay_count_total\":0,\"overlay_instr_total\":0,"
        + "\"overlay_bytes_total\":0,\"rbc_chunks_total\":0,\"rbc_bytes_total\":0,"
        + "\"detached_prepared_total\":0,\"detached_merged_total\":0,"
        + "\"detached_fallback_total\":0,"
        + "\"detached_fallback_fee_postprocessing_total\":0,"
        + "\"detached_fallback_user_executor_total\":0,"
        + "\"detached_fallback_durable_state_total\":0,"
        + "\"detached_fallback_unsupported_instruction_total\":0,"
        + "\"detached_fallback_rejected_eval_total\":0,"
        + "\"detached_fallback_overlay_error_total\":0,"
        + "\"quarantine_executed_total\":0}";
  }

  private static String nativeRow(final String dataspaceId) {
    return "{"
        + "\"lane_id\":3,\"dataspace_id\":" + dataspaceId + ","
        + "\"lane_incarnation\":\"" + SumeragiStatusModelsTests.hash(0x54) + "\","
        + "\"participant_height\":8,\"participant_view\":1,\"predecessor_height\":7,"
        + "\"predecessor_descriptor_hash\":\"" + SumeragiStatusModelsTests.hash(0x61) + "\","
        + "\"descriptor_hash\":\"" + SumeragiStatusModelsTests.hash(0x71) + "\","
        + "\"proposal_hash\":\"" + SumeragiStatusModelsTests.hash(0x73) + "\","
        + "\"settlement_hash\":\"" + SumeragiStatusModelsTests.hash(0x75) + "\","
        + "\"source_count\":2,\"application_block_height\":15,"
        + "\"application_block_hash\":\"" + SumeragiStatusModelsTests.hash(0x77) + "\","
        + "\"state\":\"durably_applied\"}";
  }

  private static String autonomousRow(final String dataspaceId) {
    return "{"
        + "\"lane_id\":3,\"dataspace_id\":" + dataspaceId + ","
        + "\"lane_incarnation\":\"" + SumeragiStatusModelsTests.hash(0x54) + "\","
        + "\"lane_block_height\":8,\"lane_block_view\":1,"
        + "\"proposal_height\":10,\"proposal_view\":2,"
        + "\"reservation_owner_hash\":\"" + SumeragiStatusModelsTests.hash(0x6f) + "\","
        + "\"proposal_identity_hash\":\"" + SumeragiStatusModelsTests.hash(0x70) + "\","
        + "\"reservation_group_hash\":\"" + SumeragiStatusModelsTests.hash(0x71) + "\","
        + "\"proposal_hash\":\"" + SumeragiStatusModelsTests.hash(0x73) + "\","
        + "\"descriptor_hash\":\"" + SumeragiStatusModelsTests.hash(0x75) + "\","
        + "\"executable_payload_hash\":\"" + SumeragiStatusModelsTests.hash(0x77) + "\","
        + "\"source_bundle_hash\":\"" + SumeragiStatusModelsTests.hash(0x79) + "\","
        + "\"merge_entry_hash\":\"" + SumeragiStatusModelsTests.hash(0x7b) + "\","
        + "\"application_block_height\":12,"
        + "\"application_block_hash\":\"" + SumeragiStatusModelsTests.hash(0x7d) + "\","
        + "\"reservation_count\":2,\"transaction_count\":2,"
        + "\"highest_durable_stage\":\"kura_wsv_application_receipt_durable\","
        + "\"stuck_reason\":\"queue_finalization_unverifiable\"}";
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> nativeReceiptGroupFixture() throws Exception {
    final Object fixture =
        JsonParser.parse(
            Files.readString(nativeFixturePath(), StandardCharsets.UTF_8));
    final Map<String, Object> root = (Map<String, Object>) fixture;
    final Map<String, Object> golden = (Map<String, Object>) root.get("golden");
    return (Map<String, Object>) golden.get("receipt_group");
  }

  private static Path nativeFixturePath() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate =
          current.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new IllegalStateException("Native AMX grouped fixture was not found");
  }

  private static String generateCauseMessages(final Throwable error) {
    final StringBuilder messages = new StringBuilder();
    Throwable current = error;
    while (current != null) {
      messages.append(current.getMessage()).append('\n');
      current = current.getCause();
    }
    return messages.toString();
  }
}
