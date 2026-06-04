package org.hyperledger.iroha.android.model.instructions;

import java.util.Base64;
import java.util.HashMap;
import java.util.Map;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;

public final class VerifyingKeyInstructionUtilsTests {

  private VerifyingKeyInstructionUtilsTests() {}

  public static void main(final String[] args) {
    deprecationHeightMapsToWithdrawHeight();
    deprecationHeightMismatchThrows();
    pendingProductionBackendTagsRoundtrip();
    catalogBackendAliasesClassifyAsPendingProduction();
    adversarialPendingBackendAliasesStayFailClosed();
    supportedBackendAliasesRemainNonPending();
    catalogBackendAliasesRejectNonAsciiConfusables();
    productionVerifierBackendClassifierMirrorsNativeAllowlist();
    inlineKeyCommitmentMustMatchSerializationBackend();
    registerAndUpdateRejectUnsupportedProductionBackends();
    registerAndUpdateRejectBlankNames();
    System.out.println("[IrohaAndroid] VerifyingKeyInstructionUtilsTests passed.");
  }

  private static void deprecationHeightMapsToWithdrawHeight() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.deprecation_height", "42");
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa");
    final Map<String, String> encoded = record.toArguments("halo2/ipa");
    assert "42".equals(encoded.get("record.withdraw_height"))
        : "deprecation height should map to withdraw_height";
    assert !encoded.containsKey("record.deprecation_height")
        : "deprecation height should not be emitted";
  }

  private static void deprecationHeightMismatchThrows() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.withdraw_height", "7");
    arguments.put("record.deprecation_height", "8");
    assertThrows(
        () -> VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa"),
        "expected mismatched deprecation/withdraw heights to fail");
  }

  private static void pendingProductionBackendTagsRoundtrip() {
    final Object[][] cases = {
      {"halo2-ipa-orchard", VerifyingKeyBackendTag.HALO2_IPA_ORCHARD},
      {"groth16-bls12-377", VerifyingKeyBackendTag.GROTH16_BLS12_377},
      {"fcmp-plus-plus-curve-tree", VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE},
      {"lattice-pcs-sis", VerifyingKeyBackendTag.LATTICE_PCS_SIS},
      {"miden-stark", VerifyingKeyBackendTag.MIDEN_STARK},
      {"aztec-plonkish-private-kernel", VerifyingKeyBackendTag.AZTEC_PLONKISH_PRIVATE_KERNEL},
      {"pq-masp-stark-fri", VerifyingKeyBackendTag.PQ_MASP_STARK_FRI},
      {"anonymous-pgc", VerifyingKeyBackendTag.ANONYMOUS_PGC},
      {"verange", VerifyingKeyBackendTag.VERANGE},
      {"zkat", VerifyingKeyBackendTag.ZKAT},
      {"recursive-anonymous-admission", VerifyingKeyBackendTag.RECURSIVE_ANONYMOUS_ADMISSION},
      {"vega-existing-credential-zk", VerifyingKeyBackendTag.VEGA_EXISTING_CREDENTIAL_ZK},
      {"silent-threshold-anoncred", VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED},
      {"zk-x509", VerifyingKeyBackendTag.ZK_X509},
      {"sis-with-hints", VerifyingKeyBackendTag.SIS_WITH_HINTS}
    };

    for (final Object[] entry : cases) {
      final String wireName = (String) entry[0];
      final VerifyingKeyBackendTag expected = (VerifyingKeyBackendTag) entry[1];
      final Map<String, String> arguments = baseArguments();
      arguments.put("record.backend_tag", wireName);
      final VerifyingKeyRecordDescription record =
          VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa");
      assert expected == record.backendTag() : "pending backend tag should parse";
      assert expected.isPendingProductionBackend() : "pending backend tag should classify pending";
      assert VerifyingKeyBackendTag.isPendingProductionBackendLabel(wireName)
          : "canonical pending backend label should classify pending";
      assert wireName.equals(record.toArguments("halo2/ipa").get("record.backend_tag"))
          : "pending backend tag should roundtrip";
    }
  }

  private static void catalogBackendAliasesClassifyAsPendingProduction() {
    final Object[][] cases = {
      {"halo2/ipa/orchard", VerifyingKeyBackendTag.HALO2_IPA_ORCHARD},
      {"orchard", VerifyingKeyBackendTag.HALO2_IPA_ORCHARD},
      {"zcash-orchard", VerifyingKeyBackendTag.HALO2_IPA_ORCHARD},
      {"groth16/bls12-377", VerifyingKeyBackendTag.GROTH16_BLS12_377},
      {"penumbra-masp", VerifyingKeyBackendTag.GROTH16_BLS12_377},
      {"halo2/ipa/penumbra", VerifyingKeyBackendTag.GROTH16_BLS12_377},
      {"halo2/ipa/masp", VerifyingKeyBackendTag.GROTH16_BLS12_377},
      {"monero-fcmp++", VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE},
      {"fcmp-plus-plus-curve-tree", VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE},
      {"halo2/ipa/monero", VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE},
      {"halo2/ipa/curve-tree", VerifyingKeyBackendTag.FCMP_PLUS_PLUS_CURVE_TREE},
      {"jindo-lattice-pcs-zk", VerifyingKeyBackendTag.LATTICE_PCS_SIS},
      {"verange-transparent-range", VerifyingKeyBackendTag.VERANGE},
      {"anonymous-pgc-k-out-of-n", VerifyingKeyBackendTag.ANONYMOUS_PGC},
      {"stark/fri/miden", VerifyingKeyBackendTag.MIDEN_STARK},
      {"aztec/private-kernel", VerifyingKeyBackendTag.AZTEC_PLONKISH_PRIVATE_KERNEL},
      {"stark/fri/pq-masp-stark-fri", VerifyingKeyBackendTag.PQ_MASP_STARK_FRI},
      {"post-quantum-masp", VerifyingKeyBackendTag.PQ_MASP_STARK_FRI},
      {"anonymous-pgc-k-out-of-n-v1", VerifyingKeyBackendTag.ANONYMOUS_PGC},
      {"ve-range-transparent-range-v1", VerifyingKeyBackendTag.VERANGE},
      {"zkAt policy-private authenticator", VerifyingKeyBackendTag.ZKAT},
      {"zk-at-policy-private-authenticator", VerifyingKeyBackendTag.ZKAT},
      {"zk-ams-recursive-admission-v0", VerifyingKeyBackendTag.RECURSIVE_ANONYMOUS_ADMISSION},
      {"vega-existing-credential-zk-v0", VerifyingKeyBackendTag.VEGA_EXISTING_CREDENTIAL_ZK},
      {"threshold-anonymous-credentials", VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED},
      {"silent-threshold-anonymous-credential", VerifyingKeyBackendTag.SILENT_THRESHOLD_ANONCRED},
      {"zkvm-x509-identity", VerifyingKeyBackendTag.ZK_X509},
      {"lattice-anonymous-credentials", VerifyingKeyBackendTag.SIS_WITH_HINTS}
    };

    for (final Object[] entry : cases) {
      final String label = (String) entry[0];
      final VerifyingKeyBackendTag expected = (VerifyingKeyBackendTag) entry[1];
      assert expected == VerifyingKeyBackendTag.fromCatalogLabel(label)
          : label + " should classify to the exact pending backend tag";
      assert VerifyingKeyBackendTag.isPendingProductionBackendLabel(label)
          : label + " should remain pending production";
    }
  }

  private static void adversarialPendingBackendAliasesStayFailClosed() {
    final String[] cases = {
      "halo2/ipa/orchard/dev-fixture",
      "stark/fri/miden/claimed-production",
      "anonymous-pgc-k-out-of-n-v1-production",
      "sis-hints-anoncred-pq-v0-devfixture",
      "groth16/bls12-377/../../prod",
      "post-quantum-masp/audit-claimed"
    };

    for (final String label : cases) {
      assert VerifyingKeyBackendTag.UNSUPPORTED == VerifyingKeyBackendTag.fromCatalogLabel(label)
          : label + " should stay unsupported";
      assert !VerifyingKeyBackendTag.isPendingProductionBackendLabel(label)
          : label + " must not classify as pending production";
      assertThrows(
          () -> VerifyingKeyBackendTag.parse(label),
          label + " must not parse as a canonical Norito backend tag");
    }
  }

  private static void supportedBackendAliasesRemainNonPending() {
    final Object[][] cases = {
      {"halo2/ipa", VerifyingKeyBackendTag.HALO2_IPA_PASTA},
      {"halo2/ipa/pasta", VerifyingKeyBackendTag.HALO2_IPA_PASTA},
      {"halo2/pasta/ipa/vote-bool", VerifyingKeyBackendTag.HALO2_IPA_PASTA},
      {"halo2/bn254", VerifyingKeyBackendTag.HALO2_BN254},
      {"groth16", VerifyingKeyBackendTag.GROTH16},
      {"groth16/bn254", VerifyingKeyBackendTag.GROTH16},
      {"stark", VerifyingKeyBackendTag.STARK},
      {"stark/fri/sha256-goldilocks", VerifyingKeyBackendTag.STARK},
      {"", VerifyingKeyBackendTag.UNSUPPORTED},
      {"unknown-backend", VerifyingKeyBackendTag.UNSUPPORTED},
      {"unknown/privacy/backend", VerifyingKeyBackendTag.UNSUPPORTED},
      {null, VerifyingKeyBackendTag.UNSUPPORTED}
    };

    for (final Object[] entry : cases) {
      final String label = (String) entry[0];
      final VerifyingKeyBackendTag expected = (VerifyingKeyBackendTag) entry[1];
      assert expected == VerifyingKeyBackendTag.fromCatalogLabel(label)
          : String.valueOf(label) + " should classify to the supported legacy tag";
      assert !VerifyingKeyBackendTag.isPendingProductionBackendLabel(label)
          : String.valueOf(label) + " should not classify pending";
    }
  }

  private static void catalogBackendAliasesRejectNonAsciiConfusables() {
    final String[] labels = {
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
      "stark\uFF0Ffri/sha256-goldilocks",
      "stark/fri/\u200Bsha256-goldilocks",
      "st\u0430rk/fri/sha256-goldilocks"
    };

    for (final String label : labels) {
      assert VerifyingKeyBackendTag.UNSUPPORTED == VerifyingKeyBackendTag.fromCatalogLabel(label)
          : label + " must stay unsupported before catalog alias compaction";
      assert !VerifyingKeyBackendTag.isPendingProductionBackendLabel(label)
          : label + " must not classify as pending production";
    }
  }

  private static void productionVerifierBackendClassifierMirrorsNativeAllowlist() {
    final String[] supported = {
      "halo2/ipa",
      "halo2/ipa:ivm-execution-v1",
      "halo2/pasta/ivm-execution-v1",
      "halo2/pasta/kagemusha-folded-v1",
      "halo2/pasta/kaigi-roster-v1",
      "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
      "stark/fri",
      "stark/fri/sha256-goldilocks"
    };
    for (final String backend : supported) {
      assert VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend)
          : backend + " should be production-admissible";
    }

    final String[] unsupported = {
      "",
      "unknown/privacy/backend",
      "halo2/unknown-native-v1",
      "halo2/ipa:unknown-native-v1",
      "stark/unknown-native-v1",
      "halo2/bn254",
      "groth16",
      "groth16/bls12-377",
      " halo2/ipa",
      "halo2/ipa ",
      "\thalo2/ipa",
      "halo2/ipa\n",
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
      " stark/fri/sha256-goldilocks",
      "stark/fri/sha256-goldilocks ",
      "halo2/ipa/orchard",
      "halo2-ipa-orchard",
      "halo2/ipa/penumbra",
      "halo2/ipa/masp",
      "halo2/ipa/monero",
      "halo2/ipa/curve-tree",
      "halo2/pasta/tiny-add",
      "halo2/ipa/tiny-add",
      "halo2/ipa:tiny-add",
      "halo2/pasta/tiny-commit-open",
      "halo2/pasta/anon-transfer-2x2",
      "halo2/ipa/anon-transfer-2x2",
      "halo2/ipa:anon-transfer-2x2",
      "halo2/pasta/anon-transfer-2x2-merkle2",
      "halo2/ipa/anon-transfer-2x2-merkle8",
      "halo2/ipa:anon-transfer-2x2-merkle16",
      "halo2/pasta/vote-bool-commit",
      "halo2/ipa/vote-bool-commit",
      "halo2/ipa:vote-bool-commit",
      "halo2/pasta/vote-bool-commit-merkle2",
      "halo2/ipa/vote-bool-commit-merkle8",
      "halo2/ipa:vote-bool-commit-merkle16",
      "halo2/pasta/asset-hidden-transfer-public-test",
      "halo2/ipa/asset-hidden-transfer-public-test",
      "halo2/ipa:asset-hidden-transfer-public-test",
      "stark/fri/miden",
      "stark/fri/miden/claimed-production",
      "stark/fri/latest",
      "stark/fri/attestation",
      "stark/fri/contest",
      "stark/fri/random-profile",
      "stark/fri/sha512-goldilocks",
      "stark/fri/audit-proof-v1",
      "stark/fri/sha256 goldilocks",
      "stark/fri/sha256+goldilocks",
      "halo2/ipa+mock",
      "halo2/ipa:production-ready",
      "halo2/ipa:claimed-production",
      "halo2/ipa:mainnet-ready",
      "stark/fri/audit-signoff",
      "stark/fri/externally-audited",
      "stark/fri/security-review-passed",
      "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
      "stark/fri/a-u-d-i-t-c-l-a-i-m",
      "stark/fri/dev-fixture",
      "stark/fri/d-e-v-f-i-x-t-u-r-e",
      "stark/fri/dev",
      "stark/fri/d-e-v",
      "stark/fri/test",
      "stark/fri/t-e-s-t",
      "stark/fri/placeholder",
      "halo2/ipa:dev-fixture",
      "halo2/ipa:dev",
      "halo2/ipa:d-e-v",
      "halo2/ipa:dummy",
      "halo2/ipa:f-a-k-e",
      "halo2/ipa:stub",
      "halo2/ipa:s-a-m-p-l-e",
      "halo2/kzg",
      "halo2/pasta/mock",
      "halo2/pasta/debug-vote",
      "mock/dev",
      "kzg/powersoftau",
      "../halo2/ipa",
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
      "halo2/ipa" + '\0'
    };
    for (final String backend : unsupported) {
      assert !VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend)
          : backend + " should remain fail-closed";
      assertThrows(
          () -> VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend, "backend"),
          backend + " should not pass production backend validation");
    }
  }

  private static void registerAndUpdateRejectUnsupportedProductionBackends() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    for (final String backend : unsafeProductionBackends()) {
      assertThrows(
          () -> RegisterVerifyingKeyInstruction.builder().setBackend(backend),
          backend + " should be rejected by register builder");
      assertThrows(
          () -> UpdateVerifyingKeyInstruction.builder().setBackend(backend),
          backend + " should be rejected by update builder");

      final Map<String, String> registerArguments = baseArguments();
      registerArguments.put("backend", backend);
      registerArguments.put("name", "vk_test");
      assertThrows(
          () -> RegisterVerifyingKeyInstruction.fromArguments(registerArguments),
          backend + " should be rejected by register fromArguments");

      final Map<String, String> updateArguments = baseArguments();
      updateArguments.put("backend", backend);
      updateArguments.put("name", "vk_test");
      assertThrows(
          () -> UpdateVerifyingKeyInstruction.fromArguments(updateArguments),
          backend + " should be rejected by update fromArguments");
    }

    final RegisterVerifyingKeyInstruction validRegister =
        RegisterVerifyingKeyInstruction.builder()
            .setBackend("halo2/ipa")
            .setName("vk_test")
            .setRecord(record)
            .build();
    assert "halo2/ipa".equals(validRegister.backend()) : "valid register backend should survive";

    final UpdateVerifyingKeyInstruction validUpdate =
        UpdateVerifyingKeyInstruction.builder()
            .setBackend("stark/fri/sha256-goldilocks")
            .setName("vk_test")
            .setRecord(
                VerifyingKeyInstructionUtils.parseRecord(
                    baseArguments(), "stark/fri/sha256-goldilocks"))
            .build();
    assert "stark/fri/sha256-goldilocks".equals(validUpdate.backend())
        : "valid update backend should survive";
  }

  private static void inlineKeyCommitmentMustMatchSerializationBackend() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    assertThrows(
        () -> record.toArguments("stark/fri/sha256-goldilocks"),
        "inline verifier records must not serialize under a different backend");

    final VerifyingKeyRecordDescription mismatchedRecord =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "stark/fri/sha256-goldilocks");
    assertThrows(
        () ->
            RegisterVerifyingKeyInstruction.builder()
                .setBackend("halo2/ipa")
                .setName("vk_test")
                .setRecord(mismatchedRecord)
                .build(),
        "register builder must reject inline verifier records from another backend");
    assertThrows(
        () ->
            UpdateVerifyingKeyInstruction.builder()
                .setBackend("halo2/ipa")
                .setName("vk_test")
                .setRecord(mismatchedRecord)
                .build(),
        "update builder must reject inline verifier records from another backend");
  }

  private static void registerAndUpdateRejectBlankNames() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    for (final String name : new String[] {"", "   ", "\t", "\n"}) {
      assertThrows(
          () ->
              RegisterVerifyingKeyInstruction.builder()
                  .setBackend("halo2/ipa")
                  .setName(name)
                  .setRecord(record)
                  .build(),
          "blank register builder name should be rejected");
      assertThrows(
          () ->
              UpdateVerifyingKeyInstruction.builder()
                  .setBackend("halo2/ipa")
                  .setName(name)
                  .setRecord(record)
                  .build(),
          "blank update builder name should be rejected");

      final Map<String, String> registerArguments = baseArguments();
      registerArguments.put("backend", "halo2/ipa");
      registerArguments.put("name", name);
      assertThrows(
          () -> RegisterVerifyingKeyInstruction.fromArguments(registerArguments),
          "blank register fromArguments name should be rejected");

      final Map<String, String> updateArguments = baseArguments();
      updateArguments.put("backend", "halo2/ipa");
      updateArguments.put("name", name);
      assertThrows(
          () -> UpdateVerifyingKeyInstruction.fromArguments(updateArguments),
          "blank update fromArguments name should be rejected");
    }
  }

  private static Map<String, String> baseArguments() {
    final Map<String, String> arguments = new HashMap<>();
    arguments.put("record.version", "1");
    arguments.put("record.circuit_id", "vk-test");
    arguments.put("record.backend_tag", "halo2-ipa-pasta");
    arguments.put("record.public_inputs_schema_hash_hex", repeatChar('a', 64));
    arguments.put("record.gas_schedule_id", "default");
    arguments.put(
        "record.vk_bytes_b64", Base64.getEncoder().encodeToString(new byte[] {1, 2, 3}));
    return arguments;
  }

  private static String[] unsafeProductionBackends() {
    return new String[] {
      "",
      "unknown/privacy/backend",
      "halo2/unknown-native-v1",
      "halo2/ipa:unknown-native-v1",
      "stark/unknown-native-v1",
      "halo2/bn254",
      "groth16",
      "groth16/bls12-377",
      "halo2/ipa/orchard",
      "halo2-ipa-orchard",
      "halo2/ipa/penumbra",
      "halo2/ipa/masp",
      "halo2/ipa/monero",
      "halo2/ipa/curve-tree",
      "halo2/pasta/tiny-add",
      "halo2/ipa/tiny-add",
      "halo2/ipa:tiny-add",
      "halo2/pasta/tiny-commit-open",
      "halo2/pasta/anon-transfer-2x2",
      "halo2/ipa/anon-transfer-2x2",
      "halo2/ipa:anon-transfer-2x2",
      "halo2/pasta/anon-transfer-2x2-merkle2",
      "halo2/ipa/anon-transfer-2x2-merkle8",
      "halo2/ipa:anon-transfer-2x2-merkle16",
      "halo2/pasta/vote-bool-commit",
      "halo2/ipa/vote-bool-commit",
      "halo2/ipa:vote-bool-commit",
      "halo2/pasta/vote-bool-commit-merkle2",
      "halo2/ipa/vote-bool-commit-merkle8",
      "halo2/ipa:vote-bool-commit-merkle16",
      "halo2/pasta/asset-hidden-transfer-public-test",
      "halo2/ipa/asset-hidden-transfer-public-test",
      "halo2/ipa:asset-hidden-transfer-public-test",
      "stark/fri/miden",
      "stark/fri/miden/claimed-production",
      "stark/fri/latest",
      "stark/fri/attestation",
      "stark/fri/contest",
      "stark/fri/sha256 goldilocks",
      "stark/fri/sha256+goldilocks",
      "halo2/ipa+mock",
      "stark/fri/dev-fixture",
      "stark/fri/d-e-v-f-i-x-t-u-r-e",
      "stark/fri/dev",
      "stark/fri/d-e-v",
      "stark/fri/test",
      "stark/fri/t-e-s-t",
      "stark/fri/placeholder",
      "halo2/ipa:dev-fixture",
      "halo2/ipa:dev",
      "halo2/ipa:d-e-v",
      "halo2/ipa:dummy",
      "halo2/ipa:f-a-k-e",
      "halo2/ipa:stub",
      "halo2/ipa:s-a-m-p-l-e",
      "halo2/kzg",
      "halo2/pasta/mock",
      "halo2/pasta/debug-vote",
      "mock/dev",
      "kzg/powersoftau",
      "../halo2/ipa",
      "halo2/ipa" + '\0'
    };
  }

  private static String repeatChar(final char value, final int count) {
    final StringBuilder builder = new StringBuilder(count);
    for (int i = 0; i < count; i++) {
      builder.append(value);
    }
    return builder.toString();
  }

  private static void assertThrows(final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }
}
