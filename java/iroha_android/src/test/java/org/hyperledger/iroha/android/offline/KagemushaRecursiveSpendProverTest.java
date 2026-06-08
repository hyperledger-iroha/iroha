package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

public final class KagemushaRecursiveSpendProverTest {

  private KagemushaRecursiveSpendProverTest() {}

  public static void main(final String[] args) {
    exposesStableModesAndCircuitIds();
    lineageKeyArtifactPackagesValidateReleaseProfiles();
    sharedRecursiveSpendAbi6FixtureMatchesSdkSurface();
    rejectsEmptyArchivesBeforeNativeDispatch();
    copiesNativeInputArchivesBeforeDispatch();
    rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch();
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
    assert "recursive_compact_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.RECURSIVE_COMPACT_V1.wireName());
    assert "recursive_spend_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName());
    assert "checked_prefold_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName());
    assert KagemushaRecursiveSpendProver.preferredMode(true, true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(true, false)
        == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(false)
        == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1;
    assert KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION == 7;
    assert "kagemusha-recursive-compact-v1"
        .equals(KagemushaRecursiveCompactPaymentTokenProver.RECURSIVE_COMPACT_CIRCUIT_ID_V1);
    final boolean verifierNativeAvailable =
        KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable();
    assert verifierNativeAvailable
        == KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable();
    final boolean projectionVerifierNativeAvailable =
        KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable();
    assert projectionVerifierNativeAvailable
        == KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable();
    assert KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException(
            "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch"));
    assert KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException(
            "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch"));
    assert !KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(null);
    assert !KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException());
    assert !KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException("recordBundleArchive must be a valid Norito archive"));
    assert !KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException(
            "Kagemusha recursive compact token public instance column 0 must contain exactly one row; found 2"));
    assert !KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
        new IllegalArgumentException(
            "Kagemusha recursive compact token envelope verifier-key hash mismatch"));
    final byte[] validRecursiveCompactInput = kagemushaNoritoFrameWithPayload(0x4b);
    final byte[] validRecursiveCompactKeyArtifacts = kagemushaNoritoFrameWithPayload(0xe1);
    final byte[] validRecursiveCompactVerifierKeys = kagemushaNoritoFrameWithPayload(0xe2);
    final byte[] recursiveCompactCopyInput = kagemushaNoritoFrameWithPayload(0x4c);
    final byte[] expectedRecursiveCompactInput =
        Arrays.copyOf(recursiveCompactCopyInput, recursiveCompactCopyInput.length);
    final byte[] ownedRecursiveCompactInput =
        KagemushaRecursiveCompactPaymentTokenProver.ownedNativeInput(
            recursiveCompactCopyInput, "compactTokenArchive");
    recursiveCompactCopyInput[6] = (byte) 0x7F;
    assert ownedRecursiveCompactInput != recursiveCompactCopyInput;
    assert Arrays.equals(expectedRecursiveCompactInput, ownedRecursiveCompactInput);
    final byte[] oversizedRecursiveCompactInput =
        new byte[KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1];
    assertThrows(
        "recordBundleArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    new byte[0], validRecursiveCompactInput, validRecursiveCompactKeyArtifacts));
    assertThrows(
        "pallasOpenEnvelopesArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput, new byte[0], validRecursiveCompactKeyArtifacts));
    assertThrows(
        "recursiveCompactKeyArtifactsArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput, validRecursiveCompactInput, new byte[0]));
    assertThrows(
        "recordBundleArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    oversizedRecursiveCompactInput,
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "pallasOpenEnvelopesArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    oversizedRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "recursiveCompactKeyArtifactsArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    oversizedRecursiveCompactInput));
    assertThrows(
        "recordBundleArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    new byte[] {1, 2},
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "pallasOpenEnvelopesArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    new byte[] {1, 2},
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "recursiveCompactKeyArtifactsArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput, validRecursiveCompactInput, new byte[] {1, 2}));
    assertThrows(
        "recordBundleArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    kagemushaNoritoFrame(0x4b),
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "pallasOpenEnvelopesArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    kagemushaNoritoFrame(0x4b),
                    validRecursiveCompactKeyArtifacts));
    assertThrows(
        "recursiveCompactKeyArtifactsArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    kagemushaNoritoFrame(0xe1)));
    assertThrows(
        "bundleArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(new byte[0]));
    assertThrows(
        "bundleArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(oversizedRecursiveCompactInput));
    assertThrows(
        "bundleArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(new byte[] {1, 2}));
    assertThrows(
        "bundleArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(kagemushaNoritoFrame(0x4b)));
    assertThrows(
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                new byte[0], validRecursiveCompactVerifierKeys));
    assertThrows(
        "compactTokenArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                oversizedRecursiveCompactInput, validRecursiveCompactVerifierKeys));
    assertThrows(
        "compactTokenArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                new byte[] {1, 2}, validRecursiveCompactVerifierKeys));
    assertThrows(
        "compactTokenArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                kagemushaNoritoFrame(0x4b), validRecursiveCompactVerifierKeys));
    assertThrows(
        "recursiveCompactVerifierKeysArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput, new byte[0]));
    assertThrows(
        "recursiveCompactVerifierKeysArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput, oversizedRecursiveCompactInput));
    assertThrows(
        "recursiveCompactVerifierKeysArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput, new byte[] {1, 2}));
    assertThrows(
        "recursiveCompactVerifierKeysArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput, kagemushaNoritoFrame(0xe2)));
    assertThrows(
        "verifierRecordArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput, new byte[0]));
    assertThrows(
        "verifierRecordArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput, oversizedRecursiveCompactInput));
    assertThrows(
        "verifierRecordArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput, new byte[] {1, 2}));
    assertThrows(
        "verifierRecordArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput, kagemushaNoritoFrame(0x4b)));
    assertThrows(
        "blockHeight must be non-negative",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, -1L));
  }

  private static void lineageKeyArtifactPackagesValidateReleaseProfiles() {
    assert KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(2);
    assert KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(128);
    assert !KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(3);
    assert !KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(0);

    final byte[] initVerifierKey =
        lineageVerifierKey(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            (byte) 0xA1);
    final byte[] initProvingKeyArchive =
        lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            initVerifierKey,
            (byte) 0xA2);
    final byte[] appendVerifierKey =
        lineageVerifierKey(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            (byte) 0xA3);
    final byte[] appendProvingKeyArchive =
        lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            appendVerifierKey,
            (byte) 0xA4);
    final byte[] verifierKey = initVerifierKey.clone();
    final byte[] provingKeyArchive = initProvingKeyArchive.clone();
    final KagemushaRecursiveSpendProver.LineageKeyArtifacts initArtifacts =
        KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            verifierKey,
            provingKeyArchive);
    assert initArtifacts.isInitArtifact();
    assert !initArtifacts.isAppendArtifact();
    assert Arrays.equals(initVerifierKey, initArtifacts.lineageVerifierKey());
    assert Arrays.equals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive());
    assert KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(initArtifacts)
        == initArtifacts;

    verifierKey[0] = 0;
    provingKeyArchive[0] = 0;
    assert initArtifacts.lineageVerifierKey()[0] == (byte) 0x5A;
    assert Arrays.equals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive());
    final byte[] exposedVerifierKey = initArtifacts.lineageVerifierKey();
    exposedVerifierKey[0] = 0;
    assert initArtifacts.lineageVerifierKey()[0] == (byte) 0x5A;
    final byte[] exposedProvingKeyArchive = initArtifacts.lineageProvingKeyArchive();
    exposedProvingKeyArchive[0] = 0;
    assert Arrays.equals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive());

    final KagemushaRecursiveSpendProver.LineageKeyArtifacts appendArtifacts =
        KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            appendVerifierKey,
            appendProvingKeyArchive);
    assert !appendArtifacts.isInitArtifact();
    assert appendArtifacts.isAppendArtifact();

    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                appendVerifierKey,
                appendProvingKeyArchive));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                appendProvingKeyArchive));
    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                "not-zk1".getBytes(StandardCharsets.UTF_8),
                initProvingKeyArchive));
    final byte[] duplicateCidVerifierKey =
        concat(
            initVerifierKey,
            zk1Tlv(
                "CID1",
                KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                    .getBytes(StandardCharsets.UTF_8)));
    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                duplicateCidVerifierKey,
                initProvingKeyArchive));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                "not-norito".getBytes(StandardCharsets.UTF_8)));
    final byte[] missingCircuitArchive =
        kagemushaNoritoFrameFromPayload(
            0x9a,
            concat(
                "package".getBytes(StandardCharsets.UTF_8),
                verifierKeyCommitment(initVerifierKey),
                repeat((byte) 0xA5, 64)));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                missingCircuitArchive));
    final byte[] wrongCommitmentArchive =
        lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            appendVerifierKey,
            (byte) 0xA6);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                wrongCommitmentArchive));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                kagemushaNoritoFrame(0x9a)));

    assertThrows(
        "proof_circuit_id",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifacts(
                "kagemusha-recursive-spend-lineage-forged-circuit",
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                repeat((byte) 0xE7, 64),
                repeat((byte) 0xE8, 64)));
    assertThrows(
        "verifier_opening_len",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                3,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                repeat((byte) 0xE7, 64),
                repeat((byte) 0xE8, 64)));
    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2, "halo2/kzg", repeat((byte) 0xE7, 64), repeat((byte) 0xE8, 64)));
    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                new byte[0],
                repeat((byte) 0xE8, 64)));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                repeat((byte) 0xE7, 64),
                new byte[0]));
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
    assertContains(archives, "\"sha256_hex\": \"f5a4a6a25fd9bfd8a121893ddb0c977753c16d8b9dfd835477d2965957c7c03e\"");
    assertContains(archives, "\"sha256_hex\": \"88f293dccb455b6fbcd85d7c06426ce45f02a42fc330e68afda490d504903c03\"");
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
    final byte[] validArchive = kagemushaNoritoFrameWithPayload(0x4b);

    assertThrows(() -> KagemushaRecursiveSpendProver.initSpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.appendSpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.transitionProfileInit(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.transitionProfileAppend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.lineageAppendBoundary(new byte[0]));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                new byte[0], validArchive));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive, new byte[0]));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                new byte[0], validArchive, validArchive));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, new byte[0], validArchive));
    assertThrows(
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, validArchive, new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.verifySpend(new byte[0]));
    assertThrows(() -> KagemushaRecursiveSpendProver.redeemSpend(new byte[0]));
    assertThrows(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(new byte[0], validArchive));
    assertThrows(
        "verifierRecordArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(validArchive, new byte[0]));
    assertThrows(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    new byte[0], validArchive, 0L));
    assertThrows(
        "verifierRecordArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validArchive, new byte[0], 0L));
  }

  private static void copiesNativeInputArchivesBeforeDispatch() {
    final byte[] archive = kagemushaNoritoFrameWithPayload(0x4C);
    final byte[] expected = Arrays.copyOf(archive, archive.length);
    final byte[] ownedArchive =
        KagemushaRecursiveSpendProver.ownedNativeInput(archive, "requestArchive");

    archive[6] = (byte) 0x7F;

    assert ownedArchive != archive;
    assert Arrays.equals(expected, ownedArchive);
    assertThrows(
        "requestArchive must not be empty",
        () -> KagemushaRecursiveSpendProver.ownedNativeInput(new byte[0], "requestArchive"));
    assertThrows(
        "requestArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.ownedNativeInput(
                new byte[KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1],
                "requestArchive"));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.ownedNativeInput(new byte[] {0x01}, "requestArchive"));
  }

  private static void rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch() {
    final byte[] validArchive = kagemushaNoritoFrameWithPayload(0x4b);
    final byte[] malformedArchive = new byte[] {1, 2};
    final byte[] emptyPayloadArchive = kagemushaNoritoFrame(0x4b);
    final byte[] oversizedArchive =
        new byte[KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1];

    assertThrows(
        "requestArchive must not exceed 67108864 bytes",
        () -> KagemushaRecursiveSpendProver.initSpend(oversizedArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.initSpend(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.initSpend(emptyPayloadArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.appendSpend(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.appendSpend(emptyPayloadArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.transitionProfileInit(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.transitionProfileInit(emptyPayloadArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.transitionProfileAppend(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.transitionProfileAppend(emptyPayloadArchive));
    assertThrows(
        "profileArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.lineageAppendBoundary(malformedArchive));
    assertThrows(
        "profileArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.lineageAppendBoundary(emptyPayloadArchive));
    assertThrows(
        "profileArchive must not exceed 67108864 bytes",
        () -> KagemushaRecursiveSpendProver.lineageAppendBoundary(oversizedArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.verifySpend(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.verifySpend(emptyPayloadArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () -> KagemushaRecursiveSpendProver.redeemSpend(malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () -> KagemushaRecursiveSpendProver.redeemSpend(emptyPayloadArchive));

    assertThrows(
        "requestArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                malformedArchive, validArchive));
    assertThrows(
        "bundleArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive, malformedArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                emptyPayloadArchive, validArchive));
    assertThrows(
        "bundleArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive, emptyPayloadArchive));
    assertThrows(
        "requestArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                oversizedArchive, validArchive));
    assertThrows(
        "bundleArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive, oversizedArchive));

    assertThrows(
        "previousWitnessArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                malformedArchive, validArchive, validArchive));
    assertThrows(
        "requestArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, malformedArchive, validArchive));
    assertThrows(
        "bundleArchive must be a valid Norito archive",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, validArchive, malformedArchive));
    assertThrows(
        "previousWitnessArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                emptyPayloadArchive, validArchive, validArchive));
    assertThrows(
        "requestArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, emptyPayloadArchive, validArchive));
    assertThrows(
        "bundleArchive must contain a non-empty Norito payload",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, validArchive, emptyPayloadArchive));
    assertThrows(
        "previousWitnessArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                oversizedArchive, validArchive, validArchive));
    assertThrows(
        "requestArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, oversizedArchive, validArchive));
    assertThrows(
        "bundleArchive must not exceed 67108864 bytes",
        () ->
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive, validArchive, oversizedArchive));
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
    assert KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {}, () -> 7, () -> true);
    assert !KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> {}, () -> 6, () -> true, 7);
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

    assertIllegalState(
        () ->
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                new byte[] {1, 2}, "redeem"),
        "native redeem returned invalid Norito archive");
    assertIllegalState(
        () ->
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                kagemushaNoritoFrame(0x4b), "redeem"),
        "native redeem returned empty Norito payload");

    final byte[] output = kagemushaNoritoFrameWithPayload(0x4b);
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

  private static void assertThrows(final String message, final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert message.equals(expected.getMessage());
    }
  }

  private static byte[] repeat(final byte value, final int count) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, value);
    return bytes;
  }

  private static byte[] kagemushaNoritoFrame(final int schemaByte) {
    final byte[] frame = new byte[40];
    frame[0] = (byte) 'N';
    frame[1] = (byte) 'R';
    frame[2] = (byte) 'T';
    frame[3] = (byte) '0';
    Arrays.fill(frame, 6, 22, (byte) schemaByte);
    return frame;
  }

  private static byte[] kagemushaNoritoFrameWithPayload(final int schemaByte) {
    final byte[] frame = new byte[45];
    System.arraycopy(kagemushaNoritoFrame(schemaByte), 0, frame, 0, 40);
    frame[23] = 3;
    final byte[] crc = new byte[] {
      (byte) 0xb9,
      (byte) 0xd3,
      (byte) 0xa8,
      0x0c,
      (byte) 0xcd,
      0x5d,
      0x13,
      0x24
    };
    System.arraycopy(crc, 0, frame, 31, crc.length);
    frame[42] = (byte) 0xa5;
    frame[43] = 0x5a;
    frame[44] = 0x11;
    return frame;
  }

  private static final long[] TEST_CRC64_TABLE = buildTestCrc64Table();

  private static byte[] kagemushaNoritoFrameFromPayload(
      final int schemaByte, final byte[] payload) {
    final byte[] frame = concat(kagemushaNoritoFrame(schemaByte), payload);
    writeLongLittleEndian(frame, 23, payload.length);
    writeLongLittleEndian(frame, 31, testCrc64(payload));
    return frame;
  }

  private static byte[] zk1Tlv(final String tag, final byte[] payload) {
    final byte[] encoded = new byte[8 + payload.length];
    final byte[] tagBytes = tag.getBytes(StandardCharsets.US_ASCII);
    System.arraycopy(tagBytes, 0, encoded, 0, tagBytes.length);
    writeIntLittleEndian(encoded, 4, payload.length);
    System.arraycopy(payload, 0, encoded, 8, payload.length);
    return encoded;
  }

  private static byte[] lineageVerifierKey(final String circuitId, final byte seed) {
    return concat(
        new byte[] {0x5a, 0x4b, 0x31, 0x00},
        zk1Tlv("IPAK", new byte[] {8, 0, 0, 0}),
        zk1Tlv("CID1", circuitId.getBytes(StandardCharsets.UTF_8)),
        zk1Tlv("H2VK", repeat(seed, 32)));
  }

  private static byte[] lineageProvingKeyArchive(
      final String circuitId, final byte[] verifierKey, final byte seed) {
    return kagemushaNoritoFrameFromPayload(
        0x9a,
        concat(
            new byte[] {1, 0},
            circuitId.getBytes(StandardCharsets.UTF_8),
            verifierKeyCommitment(verifierKey),
            repeat(seed, 64)));
  }

  private static byte[] verifierKeyCommitment(final byte[] verifierKey) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      final byte[] backend =
          KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND.getBytes(
              StandardCharsets.UTF_8);
      digest.update("iroha:zk:v1:vk".getBytes(StandardCharsets.US_ASCII));
      digest.update(longBigEndian(backend.length));
      digest.update(backend);
      digest.update(longBigEndian(verifierKey.length));
      digest.update(verifierKey);
      return digest.digest();
    } catch (final NoSuchAlgorithmException ex) {
      throw new AssertionError("SHA-256 is unavailable", ex);
    }
  }

  private static byte[] concat(final byte[]... parts) {
    int length = 0;
    for (final byte[] part : parts) {
      length += part.length;
    }
    final byte[] output = new byte[length];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, output, offset, part.length);
      offset += part.length;
    }
    return output;
  }

  private static long[] buildTestCrc64Table() {
    final long reflectedPoly = 0xC96C5795D7870F42L;
    final long[] table = new long[256];
    for (int index = 0; index < table.length; index++) {
      long crc = index;
      for (int bit = 0; bit < 8; bit++) {
        crc = (crc & 1L) != 0L ? (crc >>> 1) ^ reflectedPoly : crc >>> 1;
      }
      table[index] = crc;
    }
    return table;
  }

  private static long testCrc64(final byte[] payload) {
    long crc = -1L;
    for (final byte value : payload) {
      crc = TEST_CRC64_TABLE[((int) crc ^ value) & 0xFF] ^ (crc >>> 8);
    }
    return crc ^ -1L;
  }

  private static void writeIntLittleEndian(
      final byte[] bytes, final int offset, final int value) {
    for (int index = 0; index < 4; index++) {
      bytes[offset + index] = (byte) ((value >>> (index * 8)) & 0xFF);
    }
  }

  private static void writeLongLittleEndian(
      final byte[] bytes, final int offset, final long value) {
    for (int index = 0; index < 8; index++) {
      bytes[offset + index] = (byte) ((value >>> (index * 8)) & 0xFF);
    }
  }

  private static byte[] longBigEndian(final long value) {
    final byte[] output = new byte[8];
    for (int index = 0; index < output.length; index++) {
      output[index] = (byte) ((value >>> ((7 - index) * 8)) & 0xFF);
    }
    return output;
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
