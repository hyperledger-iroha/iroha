package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public final class KagemushaRecursiveSpendProverTest {

  private KagemushaRecursiveSpendProverTest() {}

  public static void main(final String[] args) {
    exposesStableModesAndCircuitIds();
    sharedRecursiveSpendAbi6FixtureMatchesSdkSurface();
    rejectsEmptyArchivesBeforeNativeDispatch();
    nativeProbeRequiresAbiSixAndAllSymbols();
    rejectsNullAndEmptyNativeRedeemOutput();
    System.out.println("[IrohaAndroid] KagemushaRecursiveSpendProverTest passed.");
  }

  private static void exposesStableModesAndCircuitIds() {
    assert KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION == 6;
    assert "kagemusha-recursive-aggregation-v1"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert "kagemusha-recursive-spend-lineage-v1"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert "kagemusha-recursive-spend-lineage-onehop-v1"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert "kagemusha-recursive-spend-lineage-append-v1"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 == 64;
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1;
    assert KagemushaRecursiveSpendProver
            .RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1
        == 1;
    assert KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
        == 8 * 1024 * 1024;
    assert KagemushaRecursiveSpendProver.RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
        == 128;
    assert KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024;
    assert "iroha:kagemusha:v1:recursive-spend-transition-profile"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN);
    assert "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
        .equals(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN);
    assert "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
        .equals(
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN);
    assert "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
        .equals(
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1);
    assert "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
        .equals(
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1);
    assert "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
        .equals(
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1);
    assert "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"
        .equals(
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1);
    assert KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1, 2);
    assert !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1);
    assert !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1);
    assert !KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1);
    assert !KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0);
    assert !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 2);
    assert KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(null, 1);
    final Object[][] redeemCases = {
      {KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, -1},
      {KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Integer.MAX_VALUE},
      {KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0},
      {KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Integer.MAX_VALUE},
      {"", 1},
      {"unknown-kagemusha-recursive-spend-circuit", Integer.MAX_VALUE},
      {null, Integer.MAX_VALUE},
    };
    for (final Object[] redeemCase : redeemCases) {
      final String circuitId = (String) redeemCase[0];
      final int hopCount = (Integer) redeemCase[1];
      assert !KagemushaRecursiveSpendProver.canRedeemWitnessless(circuitId, hopCount);
      assert KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(circuitId, hopCount);
    }
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(0);
    assert KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(1);
    assert KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(63);
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(64);
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(-1);
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(Integer.MAX_VALUE);
    assert KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS == 64;
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(null));
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(""));
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1));
    assert "unknown-kagemusha-recursive-spend-circuit"
        .equals(
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"));
    assert KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(null);
    assert KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId("");
    assert KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        "unknown-kagemusha-recursive-spend-circuit");
    assert KagemushaRecursiveSpendProver.isLineageProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isLineageProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isLineageProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForInit();
    assert KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(null);
    assert !KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput("");
    assert !KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
        "unknown-kagemusha-recursive-spend-circuit");
    assert KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        "unknown-kagemusha-recursive-spend-circuit");
    assert !KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
        "unknown-kagemusha-recursive-spend-circuit");
    assert KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, "");
    assert KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        "unknown-kagemusha-recursive-spend-circuit",
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1);
    assert !KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit");
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(1));
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(63));
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(64))
        : "preferred append selector falls back at the witnessless hop cap";
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(0));
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(null, 1);
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS - 1);
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0);
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS);
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1, 1);
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 63);
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64);
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        "unknown-kagemusha-recursive-spend-circuit", 1);
    assert KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1);
    assert KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        "unknown-kagemusha-recursive-spend-circuit",
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1) : "semantic previous proofs cannot select Reserved-lineage output";
    assert KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        1);
    assert KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        "unknown-kagemusha-recursive-spend-circuit",
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        0);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(null, 1);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend("", 1);
    assert "recursive_spend_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName());
    assert "checked_prefold_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName());
    assert KagemushaRecursiveSpendProver.preferredMode(true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(false)
        == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1;
  }

  private static void sharedRecursiveSpendAbi6FixtureMatchesSdkSurface() {
    final String manifest = sharedRecursiveSpendManifest();
    assertContains(manifest, "\"schema\": \"iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1\"");
    assertContains(
        manifest,
        "\"bridge_abi_version\": " + KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION);
    assertContains(manifest, "\"operation_count\": 9");
    assertContains(
        manifest,
        "\"recursive_aggregation\": \""
            + KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            + "\"");
    assertContains(
        manifest,
        "\"reserved_lineage\": \""
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            + "\"");
    assertContains(
        manifest,
        "\"reserved_lineage_one_hop\": \""
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
            + "\"");
    assertContains(
        manifest,
        "\"reserved_lineage_append\": \""
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            + "\"");
    assertContains(manifest, "\"compact_token_max_hops\": " + KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS);
    assertContains(
        manifest,
        "\"reserved_lineage_witnessless_max_hops\": "
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1);
    assertContains(
        manifest,
        "\"previous_proof_open_envelopes_required_count\": "
            + KagemushaRecursiveSpendProver
                .RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1);
    assertContains(
        manifest,
        "\"previous_proof_open_envelopes_max_bytes\": "
            + KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES);
    assertContains(
        manifest,
        "\"pallas_open_envelope_max_transcript_label_bytes\": "
            + KagemushaRecursiveSpendProver.RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES);
    assertContains(
        manifest,
        "\"native_archive_max_bytes\": "
            + KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES);
    assertContains(
        manifest,
        "\"transition_profile\": \""
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN
            + "\"");
    assertContains(
        manifest,
        "\"lineage_append_boundary_final_note_binding\": \""
            + KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
            + "\"");
    for (final String symbol : new String[] {
      "connect_norito_kagemusha_recursive_spend_init",
      "connect_norito_kagemusha_recursive_spend_append",
      "connect_norito_kagemusha_recursive_spend_transition_profile_init",
      "connect_norito_kagemusha_recursive_spend_transition_profile_append",
      "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
      "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
      "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
      "connect_norito_kagemusha_recursive_spend_verify",
      "connect_norito_kagemusha_recursive_spend_redeem"
    }) {
      assertContains(manifest, "\"symbol\": \"" + symbol + "\"");
    }
    assertContains(manifest, "\"reserved_lineage_payload_bytes\": 3847");
    assertContains(manifest, "\"reserved_lineage_transition_profile_bytes\": 2817");
    final String archives = sharedRecursiveSpendFixture("archives.json");
    assertContains(
        archives,
        "\"schema\": \"iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1\"");
    for (final String archiveName : new String[] {
      "init_request",
      "init_bundle",
      "transition_profile_init",
      "append_request",
      "append_bundle",
      "transition_profile_append",
      "lineage_append_boundary",
      "lineage_witness_from_init_result",
      "lineage_witness_append_result",
      "verify_request",
      "verify_result",
      "redeem_request",
      "redeem_instruction"
    }) {
      assertContains(archives, "\"name\": \"" + archiveName + "\"");
    }
    assertContains(archives, "\"operation\": \"redeem\"");
    assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendRedeemRequestV1\"");
    assertContains(archives, "\"norito_type\": \"RedeemKagemushaRecursive\"");
    assertContains(archives, "\"request_archive_fields\"");
    assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendInitRequestV1\"");
    assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendAppendRequestV1\"");
    assertContains(archives, "\"name\": \"lineage_verifier_key\"");
    assertContains(archives, "\"name\": \"lineage_proving_key_archive\"");
    assertContains(archives, "\"name\": \"previous_recursive_proof_open_envelopes_archive\"");
    assertContains(archives, "\"name\": \"lineage_verifier_record\"");
    assertContains(archives, "\"name\": \"lineage_witness\"");
    assertContains(archives, "\"name\": \"block_height\"");
    assertContains(archives, "\"type\": \"Option<u64>\"");
    assertContains(archives, "\"norito_default\": true");
    assertContains(archives, "\"semantics\": \"verifier_record_activation_height\"");
    assertContains(archives, "\"sha256_hex\": \"b83b33541f50ab893ae356c1f42da60aaf81da95bc4daf871511509fc8eea5b2\"");
    assertContains(archives, "\"sha256_hex\": \"a598660cbfe91a207b64a69b7a9dbdc985fd901c60fe886aecb4dead4115169e\"");
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(1));
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(63));
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.equals(
        KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(64));
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(0);
    assert KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(63);
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(64);
    assert KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1, 2);
    assert !KagemushaRecursiveSpendProver.canRedeemWitnessless(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 65);
  }

  private static void rejectsEmptyArchivesBeforeNativeDispatch() {
    assertThrows(() -> KagemushaRecursiveSpendProver.initSpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.appendSpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.transitionProfileInit(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.transitionProfileAppend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.lineageAppendBoundary(new byte[0]));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                new byte[0], new byte[] {1}));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                new byte[] {1}, new byte[0]));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[0], new byte[] {1}, new byte[] {2}));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[] {1}, new byte[0], new byte[] {2}));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[] {1}, new byte[] {2}, new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.verifySpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.redeemSpend(new byte[0]));
  }

  private static void nativeProbeRequiresAbiSixAndAllSymbols() {
    assert KagemushaRecursiveSpendProver.expectIllegalArgumentProbe(
        () -> {
          throw new IllegalArgumentException("malformed probe");
        });
    assert !KagemushaRecursiveSpendProver.expectIllegalArgumentProbe(() -> {});
    assertIllegalState(
        () ->
            KagemushaRecursiveSpendProver.expectIllegalArgumentProbe(
                () -> {
                  throw new IllegalStateException("accepted malformed probe before backend work");
                }),
        "accepted malformed probe before backend work");

    assert KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {}, () -> 6, () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {}, () -> 5, () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {}, () -> 6, () -> false);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {
          throw new UnsatisfiedLinkError("missing bridge");
        },
        () -> 6,
        () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {},
        () -> {
          throw new UnsatisfiedLinkError("missing abi symbol");
        },
        () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new UnsatisfiedLinkError("missing recursive symbol");
        });
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {
          throw new IllegalArgumentException("bad bridge load");
        },
        () -> 6,
        () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {},
        () -> {
          throw new IllegalArgumentException("bad abi probe");
        },
        () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new IllegalArgumentException("bad malformed probe");
        });
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new IllegalStateException("bad malformed probe");
        });
  }

  private static void rejectsNullAndEmptyNativeRedeemOutput() {
    assertIllegalState(
        () -> KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(null, "redeem"),
        "native redeem returned no output");
    assertIllegalState(
        () -> KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(new byte[0], "redeem"),
        "native redeem returned empty output");
    assertIllegalState(
        () ->
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                new byte[KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1],
                "redeem"),
        "native redeem returned oversized output");
    final byte[] output = new byte[] {1, 2};
    assert KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem") == output;
  }

  private static String sharedRecursiveSpendManifest() {
    return sharedRecursiveSpendFixture("manifest.json");
  }

  private static String sharedRecursiveSpendFixture(final String fileName) {
    Path directory = Path.of("").toAbsolutePath();
    while (directory != null) {
      final Path candidate =
          directory.resolve("fixtures/kagemusha_recursive_spend_abi6").resolve(fileName);
      if (Files.isRegularFile(candidate)) {
        try {
          return Files.readString(candidate, StandardCharsets.UTF_8);
        } catch (final IOException error) {
          throw new AssertionError("failed to read shared recursive spend ABI-6 fixture", error);
        }
      }
      directory = directory.getParent();
    }
    throw new AssertionError("missing shared recursive spend ABI-6 fixture " + fileName);
  }

  private static void assertContains(final String text, final String needle) {
    if (!text.contains(needle)) {
      throw new AssertionError("missing shared fixture marker: " + needle);
    }
  }

  private static void assertThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static void assertIllegalState(final Runnable runnable, final String message) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      assert expected.getMessage().contains(message);
    }
  }
}
