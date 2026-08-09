// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.BodyState;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.ConsensusMode;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.Phase;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.ProgressTransition;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.QueueKind;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.SumeragiV2Status;
import org.hyperledger.iroha.android.consensus.SumeragiStatusModels.WorkStage;
import org.hyperledger.iroha.android.util.HashLiteral;
import org.junit.Test;

/** Fail-closed authoritative status model and parser tests. */
public final class SumeragiStatusModelsTests {
  private static final String EMPTY_MANIFEST_ROOT =
      "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F";

  @Test
  public void parserPreservesCompleteTypedV4SnapshotAndFullU64Range() {
    final String maximum = "18446744073709551615";
    final SumeragiV2Status status =
        SumeragiStatusModels.parseStatus(statusJson(maximum, maximum, maximum));

    assertEquals(4, status.protocolVersion());
    assertEquals(new BigInteger(maximum), status.view());
    assertEquals(new BigInteger(maximum), status.liveness().noProgressAgeMs());
    assertEquals(
        new BigInteger(maximum),
        status.lastCommitQc().certificate().executionCommitment().executedBlockWireLen());
    assertEquals(Phase.PREPARE, status.phase());
    assertEquals(BodyState.VALIDATED, status.bodyState());
    assertEquals(ConsensusMode.PERMISSIONED, status.heightContext().mode());
    assertEquals(WorkStage.COMPLETE, status.liveness().work().validation());
    assertEquals(QueueKind.NETWORK_INGRESS, status.liveness().queues().get(0).queue());
    assertEquals(
        ProgressTransition.PREPARE_VOTE_ADMITTED,
        status.liveness().lastProgress().transition());
    assertNull(status.lockedPrepareQc());
    assertFalse(status.restartRequired());
  }

  @Test
  public void parserRejectsUnknownMissingDuplicateTrailingAndNoncanonicalScalars() {
    final String payload = statusJson("2", "123", "19");
    assertRejected(payload.replaceFirst("\\{", "{\"mode_tag\":\"legacy\","));
    assertRejected(payload.replace("\"restart_required\": false,", ""));
    assertRejected(payload.replaceFirst("\\{", "{\"protocol_version\":4,"));
    assertRejected(payload + " false");
    assertRejected(payload.replaceFirst("\"height\": 10", "\"height\": \"10\""));
    assertRejected(payload.replaceFirst("\"view\": 2", "\"view\": -0"));
    assertRejected(
        payload.replaceFirst("\"view\": 2", "\"view\": 18446744073709551616"));
  }

  @Test
  public void byteParserRejectsMalformedUtf8UnicodeAndExcessiveDepth() {
    assertThrows(
        IllegalArgumentException.class,
        () -> SumeragiStatusModels.parseStatus(new byte[0]));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            SumeragiStatusModels.parseStatus(
                new byte[(int) SumeragiStatusModels.STATUS_JSON_MAX_BYTES + 1]));
    assertThrows(
        IllegalArgumentException.class,
        () -> SumeragiStatusModels.parseStatus(new byte[] {0x7b, 0x22, (byte) 0xc3, 0x28}));
    assertRejected("{\"x\":\"" + '\ud800' + "\"}");

    final StringBuilder nested = new StringBuilder("{\"x\":");
    for (int index = 0; index < 130; index++) nested.append('[');
    nested.append('0');
    for (int index = 0; index < 130; index++) nested.append(']');
    nested.append('}');
    assertRejected(nested.toString());
  }

  @Test
  public void parserEnforcesExactTagsPhaseBodyAndCommitFrontierGeometry() {
    final String payload = statusJson("2", "123", "19");
    assertRejected(
        payload.replaceFirst(
            "\"phase\": \"prepare\", \"details\": null",
            "\"phase\": \"prepare\""));
    assertRejected(payload.replaceFirst("\"state\": \"validated\"", "\"state\": \"missing\""));
    assertRejected(
        payload.replaceFirst("\"pending_persistence_id\": null", "\"pending_persistence_id\": 0"));
    assertRejected(payload.replaceFirst("\"phase\": \"prepare\"", "\"phase\": \"commit\""));
    assertRejected(
        payload.replaceFirst("\"last_committed_height\": 9", "\"last_committed_height\": 10"));
  }

  @Test
  public void parserEnforcesManifestCarrierQcAndLivenessGeometry() {
    final String payload = statusJson("2", "123", "19");
    assertRejected(
        payload.replace(
            "\"native_amx_application_manifest_version\": 1",
            "\"native_amx_application_manifest_version\": 2"));
    assertRejected(
        payload.replace(
            "\"native_amx_application_manifest_count\": 0",
            "\"native_amx_application_manifest_count\": 1"));
    assertRejected(payload.replace("\"merge_carrier\": null,", ""));
    assertRejected(payload.replace("\"executed_block_wire_len\": 123", "\"executed_block_wire_len\": 0"));
    assertRejected(payload.replaceFirst("\"signed_power\":3", "\"signed_power\":2"));
    assertRejected(payload.replaceFirst("\"depth\": 1, \"capacity\": 4", "\"depth\": 5, \"capacity\": 4"));
    assertRejected(
        payload.replace(
            "\"ignore_counts\": [" + ignoreCount() + "]",
            "\"ignore_counts\": [" + ignoreCount() + "," + ignoreCount() + "]"));
  }

  @Test
  public void collectionsAreImmutableAndOperationalFieldsAreRejected() {
    final String payload = statusJson("2", "123", "19");
    final SumeragiV2Status status =
        SumeragiStatusModels.parseStatus(payload.getBytes(StandardCharsets.UTF_8));
    assertThrows(UnsupportedOperationException.class, () -> status.liveness().queues().clear());
    assertTrue(status.lastCommitQc() != null);
    assertRejected(payload.replaceFirst("\\{", "{\"lane_settlement_commitments\":[],"));
  }

  private static void assertRejected(final String payload) {
    assertThrows(RuntimeException.class, () -> SumeragiStatusModels.parseStatus(payload));
  }

  private static String statusJson(
      final String rootView, final String executedWireLen, final String noProgressAge) {
    final String subject = subject();
    final String commitment = executionCommitment(executedWireLen);
    final String activeRound = round(hash(0x14), "10", "1");
    final String committedRound = round(hash(0x41), "9", "1");
    return "{"
        + "\"protocol_version\":4,"
        + "\"node_fingerprint\":\"" + hash(0x11) + "\","
        + "\"build_fingerprint\":\"" + hash(0x12) + "\","
        + "\"config_fingerprint\":\"" + hash(0x13) + "\","
        + "\"restart_required\": false,"
        + "\"height_context_id\":[\"" + hash(0x14) + "\"],"
        + "\"height\": 10,"
        + "\"view\": " + rootView + ","
        + "\"phase\":{\"phase\": \"prepare\", \"details\": null},"
        + "\"leader\":1,"
        + "\"locked_prepare_qc\":null,"
        + "\"highest_prepare_qc\":null,"
        + "\"last_timeout_certificate\":null,"
        + "\"body_state\":{\"state\": \"validated\",\"details\":null},"
        + "\"pending_persistence_id\": null,"
        + "\"last_committed_height\": 9,"
        + "\"last_committed_subject\":" + subject + ","
        + "\"height_context\":{"
        + "\"epoch\":1,\"epoch_end_height\":20,"
        + "\"mode\":{\"mode\":\"permissioned\",\"details\":null},"
        + "\"epoch_seed\":\"" + byte32Sequence() + "\","
        + "\"validator_count\":4,"
        + "\"quorum\":{\"min_signers\":3,\"total_power\":4}},"
        + "\"last_commit_qc\":{"
        + "\"certificate\":{\"round\":" + committedRound
        + ",\"proposal_round\":" + committedRound
        + ",\"phase\":{\"phase\":\"commit\",\"details\":null}"
        + ",\"subject\":" + subject
        + ",\"execution_commitment\":" + commitment + "},"
        + "\"validator_count\":4,\"signer_count\":3,\"min_signers\":3,"
        + "\"signed_power\":3,\"total_power\":4},"
        + "\"liveness\":{"
        + "\"generation\":2,"
        + "\"prepare_quorums\":[{\"round\":" + activeRound
        + ",\"proposal_round\":" + activeRound
        + ",\"subject\":" + subject
        + ",\"execution_commitment\":" + commitment
        + ",\"signer_count\":2,\"signed_power\":2,\"min_signers\":3,\"total_power\":4}],"
        + "\"commit_quorums\":[],\"timeout_quorums\":[],"
        + "\"outbound_intents\":[{"
        + "\"kind\":{\"kind\":\"proposal\",\"details\":null},"
        + "\"round\":" + activeRound + ",\"proposal_round\":" + activeRound + ","
        + "\"subject\":" + subject + ","
        + "\"stage\":{\"stage\":\"sent\",\"details\":null}}],"
        + "\"work\":{"
        + "\"candidate\":{\"stage\":\"idle\",\"details\":null},"
        + "\"body_recovery\":{\"stage\":\"idle\",\"details\":null},"
        + "\"body_store\":{\"stage\":\"idle\",\"details\":null},"
        + "\"validation\":{\"stage\":\"complete\",\"details\":null},"
        + "\"application\":{\"stage\":\"idle\",\"details\":null},"
        + "\"successor_height\":{\"stage\":\"idle\",\"details\":null}},"
        + "\"queues\":[" + queue() + "],"
        + "\"last_progress\":{\"generation\":2,\"round\":" + activeRound + ","
        + "\"transition\":{\"transition\":\"prepare_vote_admitted\",\"details\":null},"
        + "\"age_ms\":19},"
        + "\"no_progress_age_ms\":" + noProgressAge + ","
        + "\"blocker\":{\"blocker\":\"prepare_quorum_missing\",\"details\":null},"
        + "\"ignore_counts\":[" + ignoreCount() + "]}}";
  }

  private static String executionCommitment(final String executedWireLen) {
    return "{"
        + "\"parent_state_root\":\"" + hash(0x34) + "\","
        + "\"post_state_root\":\"" + hash(0x35) + "\","
        + "\"ordinary_writes_root\":\"" + hash(0x36) + "\","
        + "\"topup_anchor_count\":0,"
        + "\"native_amx_application_manifest_version\": 1,"
        + "\"native_amx_application_manifest_root\":\"" + EMPTY_MANIFEST_ROOT + "\","
        + "\"native_amx_application_manifest_count\": 0,"
        + "\"merge_carrier\": null,"
        + "\"executed_block_wire_len\": " + executedWireLen + ","
        + "\"executed_block_wire_hash\":\"" + hash(0x37) + "\"}";
  }

  private static String subject() {
    return "{\"parent_block_hash\":\"" + hash(0x31)
        + "\",\"block_hash\":\"" + hash(0x32)
        + "\",\"payload_hash\":\"" + hash(0x33) + "\"}";
  }

  private static String round(final String contextId, final String height, final String view) {
    return "{\"context_id\":[\"" + contextId + "\"],\"height\":" + height
        + ",\"view\":" + view + "}";
  }

  private static String queue() {
    return "{\"queue\":{\"queue\":\"network_ingress\",\"details\":null},"
        + "\"depth\": 1, \"capacity\": 4,\"oldest_age_ms\":17,\"service_debt\":2}";
  }

  private static String ignoreCount() {
    return "{\"reason\":{\"reason\":\"duplicate\",\"details\":null},\"count\":2}";
  }

  private static String byte32Sequence() {
    final StringBuilder value = new StringBuilder();
    for (int index = 0; index < 32; index++) value.append(String.format("%02X", index));
    return value.toString();
  }

  static String hash(final int seed) {
    final byte[] value = new byte[32];
    java.util.Arrays.fill(value, (byte) seed);
    return HashLiteral.canonicalize(value);
  }
}
