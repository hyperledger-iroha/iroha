package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class KagemushaRecursiveSpendProverTest {
  private static final int TEST_NORITO_COMPACT_LEN_FLAG = 0x02;
  private static final int TEST_NORITO_PACKED_STRUCT_FLAG = 0x04;
  private static final int TEST_NORITO_FIELD_BITSET_FLAG = 0x20;
  private static final int SAMPLE_LINEAGE_OPENING_LEN = 2;
  private static final byte[] LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH =
      new byte[] {
        (byte) 0xC8, (byte) 0x84, (byte) 0x89, 0x61,
        (byte) 0x8A, 0x01, 0x2C, 0x28,
        0x3F, (byte) 0xF3, (byte) 0xBB, 0x2E,
        (byte) 0xBA, (byte) 0xBC, 0x77, 0x75
      };
  private static final byte[] OLD_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH =
      new byte[] {
        0x11, (byte) 0x9F, 0x4D, (byte) 0xF3,
        (byte) 0x8A, (byte) 0x98, (byte) 0xEF, 0x58,
        0x48, (byte) 0xAD, 0x0A, (byte) 0xAD,
        (byte) 0xB9, 0x71, 0x57, 0x79
      };
  private static final byte[] PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH =
      new byte[] {
        (byte) 0xfe, 0x38, 0x26, 0x32,
        (byte) 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, (byte) 0xfe,
        0x11, 0x02, 0x60, (byte) 0xca
      };
  private static final byte[] CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\","
              + "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);
  private static final byte[] CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\","
              + "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);

  private KagemushaRecursiveSpendProverTest() {}

  public static void main(final String[] args) {
    exposesStableModesAndCircuitIds();
    lineageKeyArtifactPackagesValidateReleaseProfiles();
    sharedRecursiveSpendAbi6FixtureMatchesSdkSurface();
    typedRequestCodecsRoundTripSharedFixtureArchives();
    typedRequestCodecsUseRustCompatibleCompactFieldLayouts();
    typedEvidenceHelpersAssembleCheckedProofArchives();
    typedEvidenceHelpersRejectAdversarialHopBindings();
    typedRequestCodecsRejectMalformedInputsBeforeNativeDispatch();
    typedEvidenceHelpersRejectUnsafeProofOnlyInputs();
    rejectsEmptyArchivesBeforeNativeDispatch();
    copiesNativeInputArchivesBeforeDispatch();
    rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch();
    nativeProbeRequiresAbiSixAndAllSymbols();
    rejectsNullAndEmptyNativeRedeemOutput();
    System.out.println("[IrohaAndroid] KagemushaRecursiveSpendProverTest passed.");
  }

  private static void exposesStableModesAndCircuitIds() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 6;
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
      {KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Integer.MIN_VALUE},
      {KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, Integer.MAX_VALUE},
      {KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 0},
      {KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, Integer.MIN_VALUE},
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
    assert !KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(Integer.MIN_VALUE);
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
    final String whitespaceLineageOutputCircuitId =
        " "
            + KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            + " ";
    assert whitespaceLineageOutputCircuitId.equals(
        KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(
            whitespaceLineageOutputCircuitId));
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
    assert !KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
        whitespaceLineageOutputCircuitId);
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
    assert !KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
        whitespaceLineageOutputCircuitId);
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
        Integer.MIN_VALUE);
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
    assert !KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
        whitespaceLineageOutputCircuitId, 1);
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
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        whitespaceLineageOutputCircuitId,
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        whitespaceLineageOutputCircuitId,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        1);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        0);
    assert !KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        Integer.MIN_VALUE);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1, 1);
    assert KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 64);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1, 0);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        Integer.MIN_VALUE);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
        KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1, 1);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(null, 1);
    assert !KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend("", 1);
    assert "checked_prefold_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName());
    assert "recursive_compact_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.RECURSIVE_COMPACT_V1.wireName());
    assert "recursive_spend_v1"
        .equals(KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName());
    assert KagemushaRecursiveSpendProver.preferredMode(true, true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(true, false)
        == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1;
    assert KagemushaRecursiveSpendProver.preferredMode(false)
        == KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1;
    assert KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 7;
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
    assertThrows(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    new byte[0], validRecursiveCompactInput, Long.MAX_VALUE));
    assertThrows(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    new byte[0], validRecursiveCompactInput, "9223372036854775808"));
    assertThrows(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    new byte[0],
                    validRecursiveCompactInput,
                    new BigInteger("18446744073709551615")));
    assertThrows(
        "blockHeight must be a canonical unsigned decimal integer",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, "01"));
    assertThrows(
        "blockHeight must be a canonical unsigned decimal integer",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, " 1"));
    assertThrows(
        "blockHeight must fit in u64",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, "18446744073709551616"));
    assertThrows(
        "blockHeight must be non-negative",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, new BigInteger("-1")));
    assertThrows(
        "blockHeight must not be null",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, (String) null));
    assertThrows(
        "blockHeight must not be null",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput, validRecursiveCompactInput, (BigInteger) null));
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
    final byte[] whitespaceCidVerifierKey =
        lineageVerifierKey(
            " "
                + KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                + " ",
            (byte) 0xA5);
    final byte[] whitespaceCidProvingKeyArchive =
        lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            whitespaceCidVerifierKey,
            (byte) 0xA6);
    assertThrows(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                whitespaceCidVerifierKey,
                whitespaceCidProvingKeyArchive));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                "not-norito".getBytes(StandardCharsets.UTF_8)));
    final byte[] missingCircuitArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xA5, 64));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                missingCircuitArchive));
    final byte[] smuggledCircuitArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            concat(
                KagemushaRecursiveSpendProver
                    .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                    .getBytes(StandardCharsets.UTF_8),
                repeat((byte) 0xA6, 64)));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                smuggledCircuitArchive));
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
    final byte[] smuggledCommitmentArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(appendVerifierKey),
            concat(verifierKeyCommitment(initVerifierKey), repeat((byte) 0xA7, 64)));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                smuggledCommitmentArchive));
    final byte[] wrongVersionArchive =
        lineageProvingKeyArchiveRaw(
            2,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xA8, 64));
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                wrongVersionArchive));
    final byte[] emptyProvingKeyArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            new byte[0]);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                emptyProvingKeyArchive));
    final byte[] trailingPayloadArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xA9, 64),
            TEST_NORITO_COMPACT_LEN_FLAG,
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            new byte[] {0x7F});
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                trailingPayloadArchive));
    final byte[] oldSchemaArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xAA, 64),
            TEST_NORITO_COMPACT_LEN_FLAG,
            OLD_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            new byte[0]);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                oldSchemaArchive));
    final byte[] packedStructArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xAB, 64),
            TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_PACKED_STRUCT_FLAG,
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            new byte[0]);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                packedStructArchive));
    final byte[] fieldBitsetArchive =
        lineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment(initVerifierKey),
            repeat((byte) 0xAC, 64),
            TEST_NORITO_COMPACT_LEN_FLAG | TEST_NORITO_FIELD_BITSET_FLAG,
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            new byte[0]);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                fieldBitsetArchive));
    final byte[] circuitIdBytes =
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
            .getBytes(StandardCharsets.UTF_8);
    final byte[] overlongVersionLengthArchive =
        kagemushaNoritoFrameFromSchemaHash(
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            concat(
                kagemushaOverlongCompactLength(2),
                new byte[] {1, 0},
                kagemushaNoritoField(
                    kagemushaNoritoString(
                        KagemushaRecursiveSpendProver
                            .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                        TEST_NORITO_COMPACT_LEN_FLAG),
                    TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    verifierKeyCommitment(initVerifierKey), TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    kagemushaNoritoByteVec(repeat((byte) 0xAD, 64)),
                    TEST_NORITO_COMPACT_LEN_FLAG)),
            TEST_NORITO_COMPACT_LEN_FLAG);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                overlongVersionLengthArchive));
    final byte[] oversizedTerminalCompactLengthArchive =
        kagemushaNoritoFrameFromSchemaHash(
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            concat(
                kagemushaOversizedTerminalCompactLength(),
                new byte[] {1, 0},
                kagemushaNoritoField(
                    kagemushaNoritoString(
                        KagemushaRecursiveSpendProver
                            .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                        TEST_NORITO_COMPACT_LEN_FLAG),
                    TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    verifierKeyCommitment(initVerifierKey), TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    kagemushaNoritoByteVec(repeat((byte) 0xB0, 64)),
                    TEST_NORITO_COMPACT_LEN_FLAG)),
            TEST_NORITO_COMPACT_LEN_FLAG);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                oversizedTerminalCompactLengthArchive));
    final byte[] hugeCanonicalCompactLengthArchive =
        kagemushaNoritoFrameFromSchemaHash(
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            concat(
                kagemushaHugeCanonicalCompactLength(),
                new byte[] {1, 0},
                kagemushaNoritoField(
                    kagemushaNoritoString(
                        KagemushaRecursiveSpendProver
                            .RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                        TEST_NORITO_COMPACT_LEN_FLAG),
                    TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    verifierKeyCommitment(initVerifierKey), TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    kagemushaNoritoByteVec(repeat((byte) 0xB1, 64)),
                    TEST_NORITO_COMPACT_LEN_FLAG)),
            TEST_NORITO_COMPACT_LEN_FLAG);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                hugeCanonicalCompactLengthArchive));
    final byte[] overlongCircuitStringArchive =
        kagemushaNoritoFrameFromSchemaHash(
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            concat(
                kagemushaNoritoField(new byte[] {1, 0}, TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    concat(kagemushaOverlongCompactLength(circuitIdBytes.length), circuitIdBytes),
                    TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    verifierKeyCommitment(initVerifierKey), TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    kagemushaNoritoByteVec(repeat((byte) 0xAE, 64)),
                    TEST_NORITO_COMPACT_LEN_FLAG)),
            TEST_NORITO_COMPACT_LEN_FLAG);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                overlongCircuitStringArchive));
    final byte[] invalidUtf8CircuitArchive =
        kagemushaNoritoFrameFromSchemaHash(
            LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
            concat(
                kagemushaNoritoField(new byte[] {1, 0}, TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    concat(kagemushaNoritoLength(1, TEST_NORITO_COMPACT_LEN_FLAG), new byte[] {(byte) 0xFF}),
                    TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    verifierKeyCommitment(initVerifierKey), TEST_NORITO_COMPACT_LEN_FLAG),
                kagemushaNoritoField(
                    kagemushaNoritoByteVec(concat(circuitIdBytes, repeat((byte) 0xAF, 64))),
                    TEST_NORITO_COMPACT_LEN_FLAG)),
            TEST_NORITO_COMPACT_LEN_FLAG);
    assertThrows(
        "lineage_proving_key_archive",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                2,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                initVerifierKey,
                invalidUtf8CircuitArchive));
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
    for (final String backend : new String[] {"halo2/kzg", " halo2/ipa", "halo2/ipa ", "HALO2/IPA"}) {
      assertThrows(
          "lineage_verifier_key",
          () ->
              KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                  2, backend, repeat((byte) 0xE7, 64), repeat((byte) 0xE8, 64)));
    }
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
        "\"native_bridge_abi_version\": " + KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION);
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

  private static void typedRequestCodecsRoundTripSharedFixtureArchives() {
    final KagemushaRecursiveSpendRequestCodecs.VerifySpendResult abi6Result =
        KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"));
    assert !abi6Result.valid;
    assert abi6Result.hopCount == 2;
    assert abi6Result.encodedBytes == 4011;
    assert "fixture recursive proof is not a production proof".equals(abi6Result.reason);
    assert !abi6Result.chainAdmissible;
    assert "offline verification failed".equals(abi6Result.chainAdmissionReason);
    assert !abi6Result.witnesslessRedeemSupported;
    assert abi6Result.lineageWitnessRequired;

    final KagemushaRecursiveSpendRequestCodecs.SpendBundleSummary init =
        KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"));
    assert init.hopCount == 1;
    assert KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
        .equals(init.proofCircuitId);
    assert "kagemusha-recursive-spend-abi-chain".equals(init.chainId);
    assert !init.asset.isBlank();
    assert !isAllZero(init.initialRoot());
    assert !isAllZero(init.finalRoot());
    assert !"0".equals(init.currentNote.amount);

    final KagemushaRecursiveSpendRequestCodecs.SpendBundleSummary append =
        KagemushaRecursiveSpendRequestCodecs.decodeBundle(
            sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"));
    assert append.hopCount >= init.hopCount;
    assert KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(append.proofCircuitId);
    assert !isAllZero(append.currentNote.noteCommitment());
    assert !isAllZero(append.currentNote.spendNullifier());
    assert !"0".equals(append.currentNote.amount);

    final SampleLineageArtifacts initLineageArtifacts = sampleInitLineageArtifacts();
    assertArchiveSchema(
        KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
            new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                sampleRecordBundle(),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote(),
                initLineageArtifacts.typed,
                7L)),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST);

    assertArchiveSchema(
        KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
            new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                sampleRecordBundle(),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote((byte) 0x31),
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                sampleVerifierRecord(),
                null,
                null,
                null,
                8L)),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST);

    assertArchiveSchema(
        KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
            new KagemushaRecursiveSpendRequestCodecs.VerifySpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                sampleVerifierRecord(),
                9L)),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_REQUEST);

    assertArchiveSchema(
        KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(
            new KagemushaRecursiveSpendRequestCodecs.RedeemSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                sampleRecipient(),
                "7",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "lineage_witness_append_result"),
                sampleVerifierRecord(),
                10L)),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_REDEEM_REQUEST);
  }

  private static void typedRequestCodecsUseRustCompatibleCompactFieldLayouts() {
    final byte[] recordBundle = sampleRecordBundle();
    final byte[] pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive();
    final SampleLineageArtifacts lineageArtifacts = sampleInitLineageArtifacts((byte) 0x5b);
    final byte[] lineageVerifierKey = lineageArtifacts.verifierKey;
    final byte[] lineageProvingKeyArchive = lineageArtifacts.provingKeyArchive;
    final KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor note = sampleNote();

    final List<byte[]> initFields =
        requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    recordBundle,
                    pallasOpenEnvelopes,
                    note,
                    lineageArtifacts.typed,
                    7L)),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST);

    assert initFields.size() == 6;
    assert Arrays.equals(
        compactPayload(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
        initFields.get(0));
    assert Arrays.equals(pallasOpenEnvelopes, readBytesVecPayload(initFields.get(1)));

    final List<byte[]> noteFields = fieldPayloads(initFields.get(2));
    assert noteFields.size() == 3;
    assert Arrays.equals(note.noteCommitment(), readFixedArrayPayload(noteFields.get(0), 32));
    assert Arrays.equals(note.spendNullifier(), readFixedArrayPayload(noteFields.get(1), 32));
    assert noteFields.get(0).length == 64;
    assert noteFields.get(1).length == 64;

    final List<byte[]> lineageKeyFields = fieldPayloads(optionSomePayload(initFields.get(3)));
    assert lineageKeyFields.size() == 2;
    assert KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND.equals(
        readStringPayload(lineageKeyFields.get(0)));
    assert Arrays.equals(lineageVerifierKey, readBytesVecPayload(lineageKeyFields.get(1)));
    assert Arrays.equals(lineageProvingKeyArchive, readBytesVecPayload(optionSomePayload(initFields.get(4))));
    assert readU64Payload(optionSomePayload(initFields.get(5))) == 7L;

    final byte[] verifyBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle");
    final List<byte[]> verifyFields =
        requestFields(
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                new KagemushaRecursiveSpendRequestCodecs.VerifySpendRequest(
                    verifyBundle, null, null)),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_REQUEST);
    assert verifyFields.size() == 3;
    assert Arrays.equals(
        compactPayload(verifyBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE),
        verifyFields.get(0));
    assertOptionNone(verifyFields.get(1));
    assertOptionNone(verifyFields.get(2));
  }

  private static void typedEvidenceHelpersAssembleCheckedProofArchives() {
    final ProofFixture unshieldFixture =
        proofFixture(
            KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            "unshield",
            "buildConfidentialUnshieldProofV3");

    final byte[] attachment =
        KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
            unshieldFixture.proofOutputArchive, unshieldFixture.verifierRecordRef);

    assertArchiveSchema(attachment, KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT);
    final List<byte[]> attachmentFields =
        requestFields(attachment, KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT);
    assert attachmentFields.size() == 6;
    assert "halo2/ipa".equals(readStringPayload(attachmentFields.get(0)));
    final List<byte[]> proofBoxFields = fieldPayloads(attachmentFields.get(1));
    assert "halo2/ipa".equals(readStringPayload(proofBoxFields.get(0)));
    assert Arrays.equals(unshieldFixture.envelopeArchive, readBytesVecPayload(proofBoxFields.get(1)));
    final List<byte[]> vkRefFields = fieldPayloads(attachmentFields.get(2));
    assert "halo2/ipa".equals(readStringPayload(vkRefFields.get(0)));
    assert unshieldFixture.verifierKeyName.equals(readStringPayload(vkRefFields.get(1)));
    assert Arrays.equals(
        unshieldFixture.commitment,
        readFixedArrayPayload(optionSomePayload(attachmentFields.get(3)), 32));
    assert Arrays.equals(
        Blake2b.digest256(unshieldFixture.envelopeArchive),
        readFixedArrayPayload(optionSomePayload(attachmentFields.get(4)), 32));
    assertOptionNone(attachmentFields.get(5));

    final byte[] rootBefore = repeat((byte) 0x31, 32);
    final byte[] rootAfter = repeat((byte) 0x32, 32);
    final ProofFixture transferFixture =
        transferProofFixture(rootBefore);
    final String asset = sampleAssetDefinition();
    final KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence evidence =
        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
            transferFixture.proofOutputArchive,
            transferFixture.verifierRecordRef,
            "kagemusha-test-chain",
            asset,
            rootAfter);

    final byte[] recordBundle =
        KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
            Arrays.asList(evidence));

    assertArchiveSchema(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE);
    final List<byte[]> fields =
        requestFields(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE);
    assert fields.size() == 2;
    final List<byte[]> bundleFields = fieldPayloads(fields.get(0));
    assert "kagemusha-test-chain".equals(readStringPayload(fieldPayloads(bundleFields.get(0)).get(0)));
    assert Arrays.equals(AssetDefinitionIdEncoder.parseAddressBytes(asset), bundleFields.get(1));

    final List<byte[]> steps = sequencePayloads(bundleFields.get(2));
    assert steps.size() == 1;
    final List<byte[]> stepFields = fieldPayloads(steps.get(0));
    assert Arrays.equals(rootBefore, readFixedArrayPayload(stepFields.get(0), 32));
    assert Arrays.equals(repeat((byte) 0x43, 32), readFixed32VecPayload(stepFields.get(1)).get(0));
    assert Arrays.equals(repeat((byte) 0x44, 32), readFixed32VecPayload(stepFields.get(2)).get(0));
    assert Arrays.equals(rootAfter, readFixedArrayPayload(stepFields.get(3), 32));
    assert "halo2/ipa".equals(readStringPayload(fieldPayloads(stepFields.get(4)).get(0)));
    assert "halo2/ipa".equals(readStringPayload(fieldPayloads(stepFields.get(5)).get(0)));

    final List<byte[]> records = sequencePayloads(fields.get(1));
    assert records.size() == 1;
    final List<byte[]> recordFields = fieldPayloads(records.get(0));
    final List<byte[]> idFields = fieldPayloads(recordFields.get(0));
    assert "halo2/ipa".equals(readStringPayload(idFields.get(0)));
    assert transferFixture.verifierKeyName.equals(readStringPayload(idFields.get(1)));
    assert Arrays.equals(
        compactPayload(
            transferFixture.verifierRecordRef.recordBytes(),
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD),
        recordFields.get(1));

    final byte[] pallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive();
    final SampleLineageArtifacts initLineageArtifacts = sampleInitLineageArtifacts((byte) 0x5c);
    final byte[] initRequest =
        KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
            evidence,
            pallasOpenEnvelopes,
            sampleNote(),
            initLineageArtifacts.verifierKey,
            initLineageArtifacts.provingKeyArchive,
            11L);
    assertArchiveSchema(initRequest, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST);
    final List<byte[]> initFields =
        requestFields(initRequest, KagemushaRecursiveSpendRequestCodecs.SCHEMA_INIT_REQUEST);
    assert Arrays.equals(
        compactPayload(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
        initFields.get(0));
    assert Arrays.equals(pallasOpenEnvelopes, readBytesVecPayload(initFields.get(1)));

    final byte[] previousBundle = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle");
    final byte[] appendRequest =
        KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
            previousBundle,
            evidence,
            pallasOpenEnvelopes,
            sampleNote((byte) 0x71),
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            sampleVerifierRecord(),
            null,
            null,
            null,
            12L);
    assertArchiveSchema(appendRequest, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST);
    final List<byte[]> appendFields =
        requestFields(appendRequest, KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST);
    assert Arrays.equals(
        compactPayload(previousBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_BUNDLE),
        appendFields.get(0));
    assert Arrays.equals(
        compactPayload(recordBundle, KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE),
        appendFields.get(1));
    assert Arrays.equals(pallasOpenEnvelopes, readBytesVecPayload(appendFields.get(2)));
  }

  private static void typedEvidenceHelpersRejectAdversarialHopBindings() {
    final String asset = sampleAssetDefinition();
    final String otherAsset = sampleAssetDefinition((byte) 0x41);
    final ProofFixture first = transferProofFixture(repeat((byte) 0x61, 32));
    final ProofFixture secondLinked = transferProofFixture(repeat((byte) 0x62, 32));
    final ProofFixture secondUnlinked = transferProofFixture(repeat((byte) 0x63, 32));

    final IllegalArgumentException extraColumnError =
        captureIllegalArgument(
            () ->
                KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                    Arrays.asList(
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            transferProofFixture(
                                    repeat((byte) 0x70, 32),
                                    repeat((byte) 0x71, 32))
                                .proofOutputArchive,
                            first.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x72, 32)))));
    assert extraColumnError.getMessage().contains("exactly 9 single-row");

    final IllegalArgumentException sameRootError =
        captureIllegalArgument(
            () ->
                KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                    Arrays.asList(
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            secondLinked.proofOutputArchive,
                            secondLinked.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x62, 32)))));
    assert sameRootError.getMessage().contains("rootAfter must differ");

    final IllegalArgumentException rootContinuityError =
        captureIllegalArgument(
            () ->
                KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                    Arrays.asList(
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            first.proofOutputArchive,
                            first.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x62, 32)),
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            secondUnlinked.proofOutputArchive,
                            secondUnlinked.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x64, 32)))));
    assert rootContinuityError.getMessage().contains("rootBefore must equal previous hop rootAfter");

    final IllegalArgumentException chainError =
        captureIllegalArgument(
            () ->
                KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                    Arrays.asList(
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            first.proofOutputArchive,
                            first.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x62, 32)),
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            secondLinked.proofOutputArchive,
                            secondLinked.verifierRecordRef,
                            "kagemusha-other-chain",
                            asset,
                            repeat((byte) 0x63, 32)))));
    assert chainError.getMessage().contains("chainId does not match");

    final IllegalArgumentException assetError =
        captureIllegalArgument(
            () ->
                KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                    Arrays.asList(
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            first.proofOutputArchive,
                            first.verifierRecordRef,
                            "kagemusha-test-chain",
                            asset,
                            repeat((byte) 0x62, 32)),
                        new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                            secondLinked.proofOutputArchive,
                            secondLinked.verifierRecordRef,
                            "kagemusha-test-chain",
                            otherAsset,
                            repeat((byte) 0x63, 32)))));
    assert assetError.getMessage().contains("asset does not match");
  }

  private static void typedEvidenceHelpersRejectUnsafeProofOnlyInputs() {
    final IllegalArgumentException pallasError =
        captureIllegalArgument(
            () -> KagemushaRecursiveSpendRequestCodecs.buildPallasOpenEnvelopesArchive(
                Arrays.asList(new byte[] {1})));
    assert pallasError.getMessage().contains("cannot be derived");

    final IllegalArgumentException bundleError =
        captureIllegalArgument(
            () -> KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                Arrays.asList(new byte[] {1}), Arrays.asList(sampleVerifierRecord())));
    assert bundleError.getMessage().contains("chainId, asset, and rootAfter");

    final IllegalArgumentException initError =
        captureIllegalArgument(
            () -> KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendInitRequest(
                new byte[] {1},
                sampleVerifierRecord(),
                sampleNote(),
                repeat((byte) 0x5a, 64),
                syntheticArchive("test.LineageProvingKeyArchive"),
                null));
    assert initError.getMessage().contains("VerifiedFoldHopEvidence");

    final IllegalArgumentException appendError =
        captureIllegalArgument(
            () -> KagemushaRecursiveSpendRequestCodecs.buildRecursiveSpendAppendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                new byte[] {1},
                sampleVerifierRecord(),
                sampleNote(),
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                sampleVerifierRecord(),
                null,
                null,
                null,
                null));
    assert appendError.getMessage().contains("Pallas open-envelopes archive");

    final ProofFixture fixture =
        proofFixture(
            KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            "unshield",
            "buildConfidentialUnshieldProofV3");
    assertThrows(
        () ->
            KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
                privacyBuildResultArchive(
                    "unshield",
                    "buildConfidentialUnshieldProofV3",
                    fixture.envelopeArchive,
                    1,
                    5,
                    "rejected"),
                fixture.verifierRecordRef));
    assertThrows(
        () ->
            KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(
                Arrays.asList(
                    new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
                        fixture.proofOutputArchive,
                        fixture.verifierRecordRef,
                        "chain",
                        sampleAssetDefinition(),
                        repeat((byte) 0x77, 32)))));
  }

  private static void typedRequestCodecsRejectMalformedInputsBeforeNativeDispatch() {
    final byte[] commitment = repeat((byte) 0x11, 32);
    final byte[] nullifier = repeat((byte) 0x22, 32);
    final KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor note =
        new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
            commitment, nullifier, "13");
    commitment[0] = 0x7f;
    nullifier[0] = 0x7e;
    assert note.noteCommitment()[0] == 0x11;
    assert note.spendNullifier()[0] == 0x22;
    final byte[] exposedCommitment = note.noteCommitment();
    final byte[] exposedNullifier = note.spendNullifier();
    exposedCommitment[1] = 0x55;
    exposedNullifier[1] = 0x66;
    assert note.noteCommitment()[1] == 0x11;
    assert note.spendNullifier()[1] == 0x22;

    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
                repeat((byte) 0x01, 31), repeat((byte) 0x02, 32), "1"));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
                new byte[32], repeat((byte) 0x02, 32), "1"));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
                repeat((byte) 0x03, 32), repeat((byte) 0x03, 32), "1"));
    for (final String amount : new String[] {
      "", "0", "01", "-1", "+1", "1.0", "1e3", "340282366920938463463374607431768211456"
    }) {
      assertThrows(
          () ->
              new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
                  repeat((byte) 0x04, 32), repeat((byte) 0x05, 32), amount));
    }

    final SampleLineageArtifacts initLineageArtifacts = sampleInitLineageArtifacts((byte) 0x6a);
    final SampleLineageArtifacts appendLineageArtifacts = sampleAppendLineageArtifacts((byte) 0x6b);
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                sampleRecordBundle(),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote(),
                null,
                null,
                null));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote(),
                initLineageArtifacts.verifierKey,
                initLineageArtifacts.provingKeyArchive,
                null));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef(
                "halo2/ipa:wrong-schema",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT)));
    final byte[] corruptedPallasOpenEnvelopes = pallasOpenEnvelopeVectorArchive();
    corruptedPallasOpenEnvelopes[corruptedPallasOpenEnvelopes.length - 1] =
        (byte) (corruptedPallasOpenEnvelopes[corruptedPallasOpenEnvelopes.length - 1] ^ 0x01);
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                sampleRecordBundle(),
                corruptedPallasOpenEnvelopes,
                sampleNote(),
                initLineageArtifacts.verifierKey,
                initLineageArtifacts.provingKeyArchive,
                null));
    final Object[][] malformedPallasOpenArchives = {
      {syntheticArchive("test.WrongPallasOpenEnvelopes"),
          "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>"},
      {pallasOpenEnvelopeVectorArchive(0), "requires exactly 1 envelope"},
      {pallasOpenEnvelopeVectorArchive(2), "requires exactly 1 envelope"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.publicCurveId = 2), "curve_id must be Pallas"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.includeDomainTag = false), "domain_tag is required"},
      {pallasOpenEnvelopeVectorArchiveWithPayload(new byte[] {0x00}), "Unexpected end of data"}
    };
    for (final Object[] malformed : malformedPallasOpenArchives) {
      final byte[] archive = (byte[]) malformed[0];
      final String expectedMessage = (String) malformed[1];
      final IllegalArgumentException archiveError =
          captureIllegalArgument(
              () ->
                  new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                      sampleRecordBundle(),
                      archive,
                      sampleNote(),
                      initLineageArtifacts.verifierKey,
                      initLineageArtifacts.provingKeyArchive,
                      null));
      assert messageContains(archiveError, expectedMessage)
          : "expected `" + expectedMessage + "` in " + archiveError.getMessage();
    }
    final IllegalArgumentException countMismatch =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    sampleRecordBundle(2),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote(),
                    initLineageArtifacts.verifierKey,
                    initLineageArtifacts.provingKeyArchive,
                    null));
    assert countMismatch.getMessage().contains("requires exactly 2 envelope");
    final SampleLineageArtifacts appendArtifactsOnInit = sampleAppendLineageArtifacts((byte) 0x6c);
    final IllegalArgumentException wrongInitLineage =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote(),
                    appendArtifactsOnInit.verifierKey,
                    appendArtifactsOnInit.provingKeyArchive,
                    null));
    assert messageContains(wrongInitLineage, "lineage key artifacts")
        || messageContains(wrongInitLineage, "lineage_verifier_key");
    final IllegalArgumentException wrongInitLineageProfile =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote(),
                    appendArtifactsOnInit.typed,
                    null));
    assert messageContains(wrongInitLineageProfile, "init artifacts");
    final byte[] forgedCommitmentArchive =
        lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            appendArtifactsOnInit.verifierKey,
            (byte) 0x6d);
    final IllegalArgumentException forgedCommitment =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote(),
                    initLineageArtifacts.verifierKey,
                    forgedCommitmentArchive,
                    null));
    assert messageContains(forgedCommitment, "lineage key artifacts")
        || messageContains(forgedCommitment, "lineage_proving_key_archive");
    final IllegalArgumentException malformedVerifierKey =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.InitSpendRequest(
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote(),
                    "not-zk1".getBytes(StandardCharsets.UTF_8),
                    initLineageArtifacts.provingKeyArchive,
                    null));
    assert messageContains(malformedVerifierKey, "lineage key artifacts")
        || messageContains(malformedVerifierKey, "lineage_verifier_key");
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.VerifySpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"), null, -1L));
    assertThrows(
        () ->
            KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(
                new KagemushaRecursiveSpendRequestCodecs.VerifySpendRequest(
                    sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"), null, null)));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.RedeemSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "verify_result"),
                sampleRecipient(),
                "7",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                null,
                null,
                null));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.RedeemSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                sampleRecipient(),
                "7",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                null,
                null,
                null));
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.RedeemSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI7, "append_bundle"),
                sampleRecipient(),
                "7",
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT),
                null,
                null));

    final byte[] tampered = sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle");
    tampered[tampered.length - 1] = (byte) (tampered[tampered.length - 1] ^ 0x01);
    assertThrows(() -> KagemushaRecursiveSpendRequestCodecs.decodeBundle(tampered));

    final IllegalArgumentException error =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                    sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote((byte) 0x41),
                    KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                    sampleVerifierRecord(),
                    null,
                    appendLineageArtifacts.verifierKey,
                    appendLineageArtifacts.provingKeyArchive,
                    null));
    assert error.getMessage().contains("previousProofOpenEnvelopes is required");

    final SampleLineageArtifacts initArtifactsOnAppend = sampleInitLineageArtifacts((byte) 0x6e);
    final IllegalArgumentException wrongAppendLineage =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                    sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote((byte) 0x43),
                    KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                    sampleVerifierRecord(),
                    pallasOpenEnvelopeVectorArchive(),
                    initArtifactsOnAppend.verifierKey,
                    initArtifactsOnAppend.provingKeyArchive,
                    null));
    assert messageContains(wrongAppendLineage, "lineage key artifacts")
        || messageContains(wrongAppendLineage, "lineage_verifier_key");
    final IllegalArgumentException wrongAppendLineageProfile =
        captureIllegalArgument(
            () ->
                new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                    sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                    sampleRecordBundle(),
                    pallasOpenEnvelopeVectorArchive(),
                    sampleNote((byte) 0x43),
                    KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                    sampleVerifierRecord(),
                    pallasOpenEnvelopeVectorArchive(),
                    initArtifactsOnAppend.typed,
                    null));
    assert messageContains(wrongAppendLineageProfile, "append artifacts");

    assertArchiveSchema(
        KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(
            new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                sampleRecordBundle(),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote((byte) 0x44),
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                sampleVerifierRecord(),
                pallasOpenEnvelopeVectorArchive(),
                appendLineageArtifacts.typed,
                null)),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_APPEND_REQUEST);

    final Object[][] malformedPreviousOpenArchives = {
      {syntheticArchive("test.WrongPreviousProofOpenEnvelopes"),
          "Vec<iroha_zkp_halo2::OpenVerifyEnvelope>"},
      {pallasOpenEnvelopeVectorArchive(0), "requires exactly 1 envelope"},
      {pallasOpenEnvelopeVectorArchive(2), "requires exactly 1 envelope"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.paramsCurveId = 2), "curve_id must be Pallas"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.transcriptLabel = ""), "transcript_label must be non-empty"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.includeVkCommitment = false), "vk_commitment is required"},
      {pallasOpenEnvelopeVectorArchive(spec -> spec.trailingEnvelopeBytes = new byte[] {0x7f}), "Trailing bytes"},
      {pallasOpenEnvelopeVectorArchiveWithPayload(new byte[] {0x00}), "Unexpected end of data"}
    };
    for (final Object[] malformed : malformedPreviousOpenArchives) {
      final byte[] archive = (byte[]) malformed[0];
      final String expectedMessage = (String) malformed[1];
      final IllegalArgumentException archiveError =
          captureIllegalArgument(
              () ->
                  new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                      sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                      sampleRecordBundle(),
                      pallasOpenEnvelopeVectorArchive(),
                      sampleNote((byte) 0x45),
                      KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                      sampleVerifierRecord(),
                      archive,
                      appendLineageArtifacts.verifierKey,
                      appendLineageArtifacts.provingKeyArchive,
                      null));
      final String actualMessage =
          archiveError.getMessage()
              + " / "
              + (archiveError.getCause() == null ? "" : archiveError.getCause().getMessage());
      assert actualMessage.contains(expectedMessage) : actualMessage;
    }
    assertThrows(
        () ->
            new KagemushaRecursiveSpendRequestCodecs.AppendSpendRequest(
                sharedRecursiveSpendArchive(FixtureAbi.ABI6, "init_bundle"),
                syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFY_RESULT),
                pallasOpenEnvelopeVectorArchive(),
                sampleNote((byte) 0x42),
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                sampleVerifierRecord(),
                null,
                null,
                null,
                null));
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

    final byte[] compressed = kagemushaNoritoFrameWithPayload(0x4b);
    compressed[22] = 1;
    assertRejectsMalformedNativeRedeemOutput(compressed);

    final byte[] unsupportedFlags = kagemushaNoritoFrameWithPayload(0x4b);
    unsupportedFlags[39] = 0x08;
    assertRejectsMalformedNativeRedeemOutput(unsupportedFlags);

    final byte[] invalidFieldBitset = kagemushaNoritoFrameWithPayload(0x4b);
    invalidFieldBitset[39] = 0x20;
    assertRejectsMalformedNativeRedeemOutput(invalidFieldBitset);

    assertRejectsMalformedNativeRedeemOutput(
        withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), new byte[] {0x7f}));
    assertRejectsMalformedNativeRedeemOutput(
        withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), new byte[65]));

    assertIllegalState(
        () ->
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                kagemushaNoritoFrame(0x4b), "redeem"),
        "native redeem returned empty Norito payload");

    final byte[] output = kagemushaNoritoFrameWithPayload(0x4b);
    assert KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem") == output;
  }

  private static void assertRejectsMalformedNativeRedeemOutput(final byte[] output) {
    assertIllegalState(
        () -> KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem"),
        "native redeem returned invalid Norito archive");
  }

  private static final class ProofFixture {
    final byte[] proofOutputArchive;
    final byte[] envelopeArchive;
    final KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef verifierRecordRef;
    final String verifierKeyName;
    final byte[] commitment;

    ProofFixture(
        final byte[] proofOutputArchive,
        final byte[] envelopeArchive,
        final KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef verifierRecordRef,
        final String verifierKeyName,
        final byte[] commitment) {
      this.proofOutputArchive = proofOutputArchive;
      this.envelopeArchive = envelopeArchive;
      this.verifierRecordRef = verifierRecordRef;
      this.verifierKeyName = verifierKeyName;
      this.commitment = commitment;
    }
  }

  private static ProofFixture proofFixture(
      final String circuitId,
      final byte[] schema,
      final String algorithmId,
      final String entrypoint) {
    return proofFixture(
        circuitId,
        schema,
        algorithmId,
        entrypoint,
        zk1Proof(
            Arrays.asList(
                repeat((byte) 0x11, 32),
                repeat((byte) 0x12, 32),
                repeat((byte) 0x13, 32),
                repeat((byte) 0x14, 32),
                repeat((byte) 0x15, 32),
                repeat((byte) 0x16, 32),
                repeat((byte) 0x17, 32),
                repeat((byte) 0x18, 32),
                repeat((byte) 0x19, 32))));
  }

  private static ProofFixture proofFixture(
      final String circuitId,
      final byte[] schema,
      final String algorithmId,
      final String entrypoint,
      final byte[] proofBytes) {
    final String verifierKeyName =
        "kagemusha-test-" + circuitId.substring(circuitId.lastIndexOf('/') + 1).replace('_', '-');
    final byte[] verifierKey = zk1VerifierKey(circuitId);
    final byte[] commitment = verifierKeyCommitment(verifierKey);
    final byte[] envelope =
        openVerifyEnvelopeArchive(circuitId, schema, commitment, proofBytes, new byte[0]);
    final byte[] proofOutput =
        privacyBuildResultArchive(algorithmId, entrypoint, envelope, 0, 0, "");
    final byte[] record = verifierRecordArchive(circuitId, schema, verifierKey, 1);
    return new ProofFixture(
        proofOutput,
        envelope,
        new KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef(
            "halo2/ipa:" + verifierKeyName, record),
        verifierKeyName,
        commitment);
  }

  private static ProofFixture transferProofFixture(
      final byte[] rootBefore, final byte[]... extraColumns) {
    final List<byte[]> columns =
        new ArrayList<>(
            Arrays.asList(
                repeat((byte) 0x41, 32),
                repeat((byte) 0x42, 32),
                repeat((byte) 0x43, 32),
                new byte[32],
                repeat((byte) 0x44, 32),
                new byte[32],
                rootBefore,
                repeat((byte) 0x45, 32),
                repeat((byte) 0x46, 32)));
    columns.addAll(Arrays.asList(extraColumns));
    return proofFixture(
        KagemushaRecursiveSpendRequestCodecs.CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA,
        "confidential-transfer-v2",
        "buildConfidentialTransferProofV2",
        zk1Proof(columns));
  }

  private static byte[] privacyBuildResultArchive(
      final String algorithmId,
      final String entrypoint,
      final byte[] proof,
      final int status,
      final int errorCode,
      final String message) {
    final byte[] archive =
        NoritoCodec.encode(
            new Object(),
            "privacy.BuildProofResultV1",
            new TypeAdapter<Object>() {
              @Override
              public void encode(final NoritoEncoder encoder, final Object value) {
                writeTestField(encoder, child -> child.writeUInt(1, 32));
                writeTestField(encoder, child -> child.writeUInt(status, 32));
                writeTestField(encoder, child -> child.writeUInt(errorCode, 32));
                writeTestField(encoder, child -> writeTestString(child, message));
                writeTestField(encoder, child -> writeTestString(child, algorithmId));
                writeTestField(encoder, child -> writeTestString(child, entrypoint));
                writeTestField(encoder, child -> writeTestString(child, "halo2/ipa:kagemusha-test"));
                writeTestField(encoder, child -> writeTestBytesVec(child, new byte[0]));
                writeTestField(encoder, child -> writeTestBytesVec(child, proof));
                writeTestField(encoder, child -> child.writeByte(0));
              }

              @Override
              public Object decode(final NoritoDecoder decoder) {
                throw new UnsupportedOperationException("test privacy results are encode-only");
              }
            },
            NoritoHeader.COMPACT_LEN);
    Arrays.fill(archive, 6, 22, (byte) 0x42);
    return archive;
  }

  private static byte[] openVerifyEnvelopeArchive(
      final String circuitId,
      final byte[] schema,
      final byte[] vkHash,
      final byte[] proofBytes,
      final byte[] aux) {
    return NoritoCodec.encode(
        new Object(),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_OPEN_VERIFY_ENVELOPE,
        new TypeAdapter<Object>() {
          @Override
          public void encode(final NoritoEncoder encoder, final Object value) {
            writeTestField(encoder, child -> child.writeUInt(0, 32));
            writeTestField(encoder, child -> writeTestString(child, circuitId));
            writeTestField(encoder, child -> child.writeBytes(vkHash));
            writeTestField(encoder, child -> writeTestBytesVec(child, schema));
            writeTestField(encoder, child -> writeTestBytesVec(child, proofBytes));
            writeTestField(encoder, child -> writeTestBytesVec(child, aux));
          }

          @Override
          public Object decode(final NoritoDecoder decoder) {
            throw new UnsupportedOperationException("test OpenVerifyEnvelope archives are encode-only");
          }
        },
        NoritoHeader.COMPACT_LEN);
  }

  private static byte[] verifierRecordArchive(
      final String circuitId, final byte[] schema, final byte[] verifierKey, final int status) {
    final byte[] commitment = verifierKeyCommitment(verifierKey);
    return NoritoCodec.encode(
        new Object(),
        KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
        new TypeAdapter<Object>() {
          @Override
          public void encode(final NoritoEncoder encoder, final Object value) {
            writeTestField(encoder, child -> child.writeUInt(1, 32));
            writeTestField(encoder, child -> writeTestString(child, circuitId));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(encoder, child -> writeTestString(child, "offline_kagemusha"));
            writeTestField(encoder, child -> child.writeUInt(0, 32));
            writeTestField(encoder, child -> writeTestString(child, "pallas"));
            writeTestField(encoder, child -> child.writeBytes(Blake2b.digest256(schema)));
            writeTestField(encoder, child -> child.writeBytes(commitment));
            writeTestField(encoder, child -> child.writeUInt(verifierKey.length, 32));
            writeTestField(encoder, child -> child.writeUInt(192 * 1024, 32));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(encoder, child -> writeTestOptionRaw(child, null));
            writeTestField(
                encoder,
                child ->
                    writeTestOptionRaw(
                        child,
                        testPayload(
                            box -> {
                              writeTestField(box, field -> writeTestString(field, "halo2/ipa"));
                              writeTestField(box, field -> writeTestBytesVec(field, verifierKey));
                            })));
            writeTestField(encoder, child -> child.writeUInt(status, 8));
          }

          @Override
          public Object decode(final NoritoDecoder decoder) {
            throw new UnsupportedOperationException("test verifier records are encode-only");
          }
        },
        NoritoHeader.COMPACT_LEN);
  }

  private interface PallasSpecMutator {
    void mutate(PallasOpenEnvelopeSpec spec);
  }

  private static final class PallasOpenEnvelopeSpec {
    int paramsCurveId = 1;
    int publicCurveId = 1;
    String transcriptLabel = "previous-proof-open";
    boolean includeVkCommitment = true;
    boolean includePublicInputsSchemaHash = true;
    boolean includeDomainTag = true;
    byte[] trailingEnvelopeBytes = new byte[0];
  }

  private static byte[] pallasOpenEnvelopeVectorArchive() {
    return pallasOpenEnvelopeVectorArchive(1, spec -> {});
  }

  private static byte[] pallasOpenEnvelopeVectorArchive(final int count) {
    return pallasOpenEnvelopeVectorArchive(count, spec -> {});
  }

  private static byte[] pallasOpenEnvelopeVectorArchive(final PallasSpecMutator mutator) {
    return pallasOpenEnvelopeVectorArchive(1, mutator);
  }

  private static byte[] pallasOpenEnvelopeVectorArchive(
      final int count, final PallasSpecMutator mutator) {
    final PallasOpenEnvelopeSpec spec = new PallasOpenEnvelopeSpec();
    mutator.mutate(spec);
    final byte[] archive =
        NoritoCodec.encode(
            new Object(),
            "test.PallasOpenEnvelopeVector",
            new TypeAdapter<Object>() {
              @Override
              public void encode(final NoritoEncoder encoder, final Object value) {
                encoder.writeUInt(count, 64);
                for (int index = 0; index < count; index++) {
                  writeTestField(
                      encoder,
                      envelope -> {
                        writeTestPallasOpenEnvelope(envelope, spec);
                        envelope.writeBytes(spec.trailingEnvelopeBytes);
                      });
                }
              }

              @Override
              public Object decode(final NoritoDecoder decoder) {
                throw new UnsupportedOperationException("test Pallas envelope vectors are encode-only");
              }
            },
            NoritoHeader.COMPACT_LEN);
    System.arraycopy(PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH, 0, archive, 6, 16);
    return archive;
  }

  private static byte[] pallasOpenEnvelopeVectorArchiveWithPayload(final byte[] payload) {
    final byte[] archive =
        NoritoCodec.encode(payload, "test.PallasOpenEnvelopeVector", RAW_PAYLOAD_ADAPTER, NoritoHeader.COMPACT_LEN);
    System.arraycopy(PALLAS_OPEN_ENVELOPE_VECTOR_SCHEMA_HASH, 0, archive, 6, 16);
    return archive;
  }

  private static void writeTestPallasOpenEnvelope(
      final NoritoEncoder encoder, final PallasOpenEnvelopeSpec spec) {
    final int n = 4;
    writeTestField(
        encoder,
        params -> {
          writeTestField(params, child -> child.writeUInt(1, 16));
          writeTestField(params, child -> child.writeUInt(spec.paramsCurveId, 16));
          writeTestField(params, child -> child.writeUInt(n, 32));
          writeTestField(params, child -> writeTestFixed32Sequence(child, n, (byte) 0x10));
          writeTestField(params, child -> writeTestFixed32Sequence(child, n, (byte) 0x20));
          writeTestField(params, child -> child.writeBytes(repeat((byte) 0x30, 32)));
        });
    writeTestField(
        encoder,
        publicInput -> {
          writeTestField(publicInput, child -> child.writeUInt(1, 16));
          writeTestField(publicInput, child -> child.writeUInt(spec.publicCurveId, 16));
          writeTestField(publicInput, child -> child.writeUInt(n, 32));
          writeTestField(publicInput, child -> child.writeBytes(repeat((byte) 0x31, 32)));
          writeTestField(publicInput, child -> child.writeBytes(repeat((byte) 0x32, 32)));
          writeTestField(publicInput, child -> child.writeBytes(repeat((byte) 0x33, 32)));
        });
    writeTestField(
        encoder,
        proof -> {
          writeTestField(proof, child -> child.writeUInt(1, 16));
          writeTestField(proof, child -> writeTestFixed32Sequence(child, 2, (byte) 0x40));
          writeTestField(proof, child -> writeTestFixed32Sequence(child, 2, (byte) 0x50));
          writeTestField(proof, child -> child.writeBytes(repeat((byte) 0x60, 32)));
          writeTestField(proof, child -> child.writeBytes(repeat((byte) 0x61, 32)));
        });
    writeTestField(encoder, child -> writeTestString(child, spec.transcriptLabel));
    writeTestField(
        encoder,
        child -> writeTestOptionRaw(child, spec.includeVkCommitment ? repeat((byte) 0x70, 32) : null));
    writeTestField(
        encoder,
        child ->
            writeTestOptionRaw(
                child, spec.includePublicInputsSchemaHash ? repeat((byte) 0x71, 32) : null));
    writeTestField(
        encoder,
        child -> writeTestOptionRaw(child, spec.includeDomainTag ? repeat((byte) 0x72, 32) : null));
  }

  private static void writeTestFixed32Sequence(
      final NoritoEncoder encoder, final int count, final byte seed) {
    encoder.writeUInt(count, 64);
    for (int index = 0; index < count; index++) {
      writeTestField(encoder, child -> child.writeBytes(repeat((byte) (seed + index), 32)));
    }
  }

  private static byte[] zk1VerifierKey(final String circuitId) {
    return concat(
        "ZK1\u0000".getBytes(StandardCharsets.US_ASCII),
        zk1Tlv("CID1", circuitId.getBytes(StandardCharsets.UTF_8)),
        zk1Tlv("IPAK", new byte[] {7, 0, 0, 0}),
        zk1Tlv("H2VK", incrementingBytes(32)));
  }

  private static byte[] zk1Proof(final List<byte[]> columns) {
    final byte[] publicInputs = new byte[8 + columns.size() * 32];
    writeIntLittleEndian(publicInputs, 0, columns.size());
    writeIntLittleEndian(publicInputs, 4, 1);
    int offset = 8;
    for (final byte[] column : columns) {
      if (column.length != 32) {
        throw new IllegalArgumentException("test ZK1 columns must be 32 bytes");
      }
      System.arraycopy(column, 0, publicInputs, offset, column.length);
      offset += column.length;
    }
    return concat(
        "ZK1\u0000".getBytes(StandardCharsets.US_ASCII),
        zk1Tlv("PROF", new byte[] {0x55}),
        zk1Tlv("I10P", publicInputs));
  }

  private interface EncoderWriter {
    void write(NoritoEncoder encoder);
  }

  private static byte[] testPayload(final EncoderWriter writer) {
    final NoritoEncoder encoder = new NoritoEncoder(NoritoHeader.COMPACT_LEN);
    writer.write(encoder);
    return encoder.toByteArray();
  }

  private static void writeTestField(
      final NoritoEncoder parent, final EncoderWriter writePayload) {
    final NoritoEncoder child = parent.childEncoder();
    writePayload.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, true);
    parent.writeBytes(payload);
  }

  private static void writeTestString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, true);
    encoder.writeBytes(bytes);
  }

  private static void writeTestBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static void writeTestOptionRaw(final NoritoEncoder encoder, final byte[] payload) {
    if (payload == null) {
      encoder.writeByte(0);
      return;
    }
    encoder.writeByte(1);
    encoder.writeLength(payload.length, true);
    encoder.writeBytes(payload);
  }

  private static String sampleAssetDefinition() {
    return sampleAssetDefinition((byte) 0x01);
  }

  private static String sampleAssetDefinition(final byte seed) {
    final byte[] bytes = new byte[16];
    for (int i = 0; i < bytes.length; i++) {
      bytes[i] = (byte) (seed + i);
    }
    bytes[6] = (byte) ((bytes[6] & 0x0F) | 0x40);
    bytes[8] = (byte) ((bytes[8] & 0x3F) | 0x80);
    return AssetDefinitionIdEncoder.encodeFromBytes(bytes);
  }

  private static KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor sampleNote() {
    return sampleNote((byte) 0x21);
  }

  private static KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor sampleNote(
      final byte seed) {
    return new KagemushaRecursiveSpendRequestCodecs.SpendableNoteDescriptor(
        repeat(seed, 32), repeat((byte) (seed + 1), 32), "17");
  }

  private static KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef sampleVerifierRecord() {
    return new KagemushaRecursiveSpendRequestCodecs.VerifierRecordRef(
        "halo2/ipa:kagemusha-recursive-spend-lineage-test",
        syntheticArchive(KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD));
  }

  private static SampleLineageArtifacts sampleInitLineageArtifacts() {
    return sampleInitLineageArtifacts((byte) 0x5a);
  }

  private static SampleLineageArtifacts sampleInitLineageArtifacts(final byte seed) {
    return sampleLineageArtifacts(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        seed);
  }

  private static SampleLineageArtifacts sampleAppendLineageArtifacts(final byte seed) {
    return sampleLineageArtifacts(
        KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        seed);
  }

  private static SampleLineageArtifacts sampleLineageArtifacts(
      final String circuitId, final byte seed) {
    final byte[] verifierKey = lineageVerifierKey(circuitId, seed);
    final byte[] provingKeyArchive =
        lineageProvingKeyArchive(circuitId, verifierKey, (byte) (seed + 1));
    return new SampleLineageArtifacts(
        verifierKey,
        provingKeyArchive,
        KagemushaRecursiveSpendProver.lineageKeyArtifacts(
            circuitId,
            SAMPLE_LINEAGE_OPENING_LEN,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            verifierKey,
            provingKeyArchive));
  }

  private static final class SampleLineageArtifacts {
    private final byte[] verifierKey;
    private final byte[] provingKeyArchive;
    private final KagemushaRecursiveSpendProver.LineageKeyArtifacts typed;

    private SampleLineageArtifacts(
        final byte[] verifierKey,
        final byte[] provingKeyArchive,
        final KagemushaRecursiveSpendProver.LineageKeyArtifacts typed) {
      this.verifierKey = verifierKey;
      this.provingKeyArchive = provingKeyArchive;
      this.typed = typed;
    }
  }

  private static byte[] sampleRecordBundle() {
    return sampleRecordBundle(1);
  }

  private static byte[] sampleRecordBundle(final int hopCount) {
    require(hopCount >= 1, "hopCount must be positive");
    final String asset = sampleAssetDefinition();
    final List<KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence> hops =
        new ArrayList<>();
    byte[] rootBefore = fixedBytes(0x31);
    for (int index = 0; index < hopCount; index++) {
      final byte[] rootAfter = fixedBytes(0x32 + index);
      final ProofFixture fixture = transferProofFixture(rootBefore);
      hops.add(
          new KagemushaRecursiveSpendRequestCodecs.VerifiedFoldHopEvidence(
              fixture.proofOutputArchive,
              fixture.verifierRecordRef,
              "kagemusha-test-chain",
              asset,
              rootAfter));
      rootBefore = rootAfter;
    }
    return KagemushaRecursiveSpendRequestCodecs.buildVerifiedFoldRecordBundle(hops);
  }

  private static String sampleRecipient() {
    try {
      return AccountAddress
          .fromAccount(repeat((byte) 0x2a, 32), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new AssertionError("failed to build sample recipient", ex);
    }
  }

  private static void assertArchiveSchema(final byte[] archive, final String schema) {
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(archive, SchemaHash.hash16(schema));
    decoded.header().validateChecksum(decoded.payload());
    assert decoded.header().flags() == NoritoHeader.COMPACT_LEN;
    assert decoded.payload().length > 0;
  }

  private static byte[] compactPayload(final byte[] archive, final String schema) {
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(archive, SchemaHash.hash16(schema));
    decoded.header().validateChecksum(decoded.payload());
    assert decoded.header().flags() == NoritoHeader.COMPACT_LEN;
    return decoded.payload();
  }

  private static List<byte[]> requestFields(final byte[] archive, final String schema) {
    return fieldPayloads(compactPayload(archive, schema));
  }

  private static List<byte[]> fieldPayloads(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final List<byte[]> fields = new ArrayList<>();
    while (decoder.remaining() > 0) {
      final long length = decoder.readLength(true);
      assert length <= Integer.MAX_VALUE;
      fields.add(decoder.readBytes((int) length));
    }
    return fields;
  }

  private static List<byte[]> sequencePayloads(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final long count = decoder.readUInt(64);
    assert count <= Integer.MAX_VALUE;
    final List<byte[]> values = new ArrayList<>();
    for (int index = 0; index < (int) count; index++) {
      final long length = decoder.readLength(true);
      assert length <= Integer.MAX_VALUE;
      values.add(decoder.readBytes((int) length));
    }
    assert decoder.remaining() == 0;
    return values;
  }

  private static List<byte[]> readFixed32VecPayload(final byte[] payload) {
    final List<byte[]> values = new ArrayList<>();
    for (final byte[] item : sequencePayloads(payload)) {
      values.add(readFixedArrayPayload(item, 32));
    }
    return values;
  }

  private static byte[] readBytesVecPayload(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final long length = decoder.readUInt(64);
    assert length <= Integer.MAX_VALUE;
    final byte[] bytes = decoder.readBytes((int) length);
    assert decoder.remaining() == 0;
    return bytes;
  }

  private static byte[] readFixedArrayPayload(final byte[] payload, final int expectedSize) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final byte[] bytes = new byte[expectedSize];
    for (int index = 0; index < expectedSize; index++) {
      assert decoder.readLength(true) == 1L;
      bytes[index] = (byte) decoder.readByte();
    }
    assert decoder.remaining() == 0;
    return bytes;
  }

  private static String readStringPayload(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final long length = decoder.readLength(true);
    assert length <= Integer.MAX_VALUE;
    final String value = new String(decoder.readBytes((int) length), StandardCharsets.UTF_8);
    assert decoder.remaining() == 0;
    return value;
  }

  private static long readU64Payload(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    final long value = decoder.readUInt(64);
    assert decoder.remaining() == 0;
    return value;
  }

  private static byte[] optionSomePayload(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    assert decoder.readByte() == 1;
    final long length = decoder.readLength(true);
    assert length <= Integer.MAX_VALUE;
    final byte[] value = decoder.readBytes((int) length);
    assert decoder.remaining() == 0;
    return value;
  }

  private static void assertOptionNone(final byte[] payload) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.COMPACT_LEN);
    assert decoder.readByte() == 0;
    assert decoder.remaining() == 0;
  }

  private static byte[] syntheticArchive(final String schema) {
    return NoritoCodec.encode(
        new byte[] {0x01, 0x02, 0x03},
        schema,
        RAW_PAYLOAD_ADAPTER,
        NoritoHeader.COMPACT_LEN);
  }

  private static final TypeAdapter<byte[]> RAW_PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final byte[] value) {
          encoder.writeBytes(value);
        }

        @Override
        public byte[] decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("synthetic archives are encode-only");
        }
      };

  private static byte[] incrementingBytes(final int length) {
    final byte[] bytes = new byte[length];
    for (int index = 0; index < length; index++) {
      bytes[index] = (byte) (index + 1);
    }
    return bytes;
  }

  private static byte[] sharedRecursiveSpendArchive(final FixtureAbi abi, final String name) {
    @SuppressWarnings("unchecked")
    final Map<String, Object> root =
        (Map<String, Object>) JsonParser.parse(sharedRecursiveSpendFixture(abi, "archives.json"));
    @SuppressWarnings("unchecked")
    final List<Map<String, Object>> archives =
        (List<Map<String, Object>>) root.get("archives");
    for (final Map<String, Object> archive : archives) {
      if (name.equals(archive.get("name"))) {
        return Base64.getDecoder().decode((String) archive.get("bytes_base64"));
      }
    }
    throw new AssertionError("missing shared recursive spend " + abi.name() + " archive " + name);
  }

  private static String sharedRecursiveSpendManifest() {
    return sharedRecursiveSpendFixture(FixtureAbi.ABI6, "manifest.json");
  }

  private static String sharedRecursiveSpendFixture(final String fileName) {
    return sharedRecursiveSpendFixture(FixtureAbi.ABI6, fileName);
  }

  private static String sharedRecursiveSpendFixture(final FixtureAbi abi, final String fileName) {
    Path directory = Path.of("").toAbsolutePath();
    while (directory != null) {
      final Path candidate = directory.resolve(abi.directory()).resolve(fileName);
      if (Files.isRegularFile(candidate)) {
        try {
          return Files.readString(candidate, StandardCharsets.UTF_8);
        } catch (final IOException error) {
          throw new AssertionError(
              "failed to read shared recursive spend " + abi.name() + " fixture", error);
        }
      }
      directory = directory.getParent();
    }
    throw new AssertionError(
        "missing shared recursive spend " + abi.name() + " fixture " + fileName);
  }

  private enum FixtureAbi {
    ABI6("fixtures/kagemusha_recursive_spend_abi6"),
    ABI7("fixtures/kagemusha_recursive_spend_abi7");

    private final String directory;

    FixtureAbi(final String directory) {
      this.directory = directory;
    }

    String directory() {
      return directory;
    }
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

  private static IllegalArgumentException captureIllegalArgument(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      return expected;
    }
  }

  private static boolean messageContains(final IllegalArgumentException error, final String expected) {
    return (error.getMessage() != null && error.getMessage().contains(expected))
        || (error.getCause() != null
            && error.getCause().getMessage() != null
            && error.getCause().getMessage().contains(expected));
  }

  private static boolean isAllZero(final byte[] bytes) {
    for (final byte value : bytes) {
      if (value != 0) {
        return false;
      }
    }
    return true;
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

  private static byte[] withHeaderPadding(final byte[] archive, final byte[] padding) {
    final byte[] padded = new byte[archive.length + padding.length];
    System.arraycopy(archive, 0, padded, 0, 40);
    System.arraycopy(padding, 0, padded, 40, padding.length);
    System.arraycopy(archive, 40, padded, 40 + padding.length, archive.length - 40);
    return padded;
  }

  private static final long[] TEST_CRC64_TABLE = buildTestCrc64Table();

  private static byte[] kagemushaNoritoFrameFromPayload(
      final int schemaByte, final byte[] payload) {
    final byte[] frame = concat(kagemushaNoritoFrame(schemaByte), payload);
    writeLongLittleEndian(frame, 23, payload.length);
    writeLongLittleEndian(frame, 31, testCrc64(payload));
    return frame;
  }

  private static byte[] kagemushaNoritoFrameFromSchemaHash(
      final byte[] schemaHash, final byte[] payload, final int flags) {
    final byte[] frame = new byte[40 + payload.length];
    System.arraycopy("NRT0".getBytes(StandardCharsets.US_ASCII), 0, frame, 0, 4);
    System.arraycopy(schemaHash, 0, frame, 6, schemaHash.length);
    frame[39] = (byte) flags;
    System.arraycopy(payload, 0, frame, 40, payload.length);
    writeLongLittleEndian(frame, 23, payload.length);
    writeLongLittleEndian(frame, 31, testCrc64(payload));
    return frame;
  }

  private static byte[] kagemushaNoritoLength(final int value, final int flags) {
    if ((flags & TEST_NORITO_COMPACT_LEN_FLAG) == 0) {
      final byte[] encoded = new byte[8];
      writeLongLittleEndian(encoded, 0, value);
      return encoded;
    }
    int remaining = value;
    final byte[] scratch = new byte[5];
    int count = 0;
    while (remaining >= 0x80) {
      scratch[count++] = (byte) ((remaining & 0x7F) | 0x80);
      remaining >>>= 7;
    }
    scratch[count++] = (byte) remaining;
    return Arrays.copyOf(scratch, count);
  }

  private static byte[] kagemushaOverlongCompactLength(final int value) {
    if (value < 0 || value >= 0x80) {
      throw new IllegalArgumentException("test helper only encodes small overlong lengths");
    }
    return new byte[] {(byte) (value | 0x80), 0};
  }

  private static byte[] kagemushaOversizedTerminalCompactLength() {
    return concat(repeat((byte) 0x80, 9), new byte[] {0x02});
  }

  private static byte[] kagemushaHugeCanonicalCompactLength() {
    return concat(repeat((byte) 0x80, 9), new byte[] {0x01});
  }

  private static byte[] kagemushaNoritoField(final byte[] payload, final int flags) {
    return concat(kagemushaNoritoLength(payload.length, flags), payload);
  }

  private static byte[] kagemushaNoritoString(final String value, final int flags) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    return concat(kagemushaNoritoLength(bytes.length, flags), bytes);
  }

  private static byte[] kagemushaNoritoByteVec(final byte[] bytes) {
    final byte[] encoded = new byte[8];
    writeLongLittleEndian(encoded, 0, bytes.length);
    return concat(encoded, bytes);
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
    return lineageProvingKeyArchiveRaw(
        1,
        circuitId,
        verifierKeyCommitment(verifierKey),
        repeat(seed, 64));
  }

  private static byte[] lineageProvingKeyArchiveRaw(
      final int version,
      final String circuitId,
      final byte[] verifierKeyCommitment,
      final byte[] provingKey) {
    return lineageProvingKeyArchiveRaw(
        version,
        circuitId,
        verifierKeyCommitment,
        provingKey,
        TEST_NORITO_COMPACT_LEN_FLAG,
        LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH,
        new byte[0]);
  }

  private static byte[] lineageProvingKeyArchiveRaw(
      final int version,
      final String circuitId,
      final byte[] verifierKeyCommitment,
      final byte[] provingKey,
      final int flags,
      final byte[] schemaHash,
      final byte[] trailingPayload) {
    final byte[] versionBytes = new byte[2];
    writeShortLittleEndian(versionBytes, 0, version);
    final byte[] payload =
        concat(
            kagemushaNoritoField(versionBytes, flags),
            kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags),
            kagemushaNoritoField(verifierKeyCommitment, flags),
            kagemushaNoritoField(kagemushaNoritoByteVec(provingKey), flags),
            trailingPayload);
    return kagemushaNoritoFrameFromSchemaHash(schemaHash, payload, flags);
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

  private static void writeShortLittleEndian(
      final byte[] bytes, final int offset, final int value) {
    for (int index = 0; index < 2; index++) {
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
