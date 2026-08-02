package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Base64;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyStatus;

public final class VerifyingKeyInstructionUtilsTests {

  private static final String[] EXACT_REGISTRY = {
    "halo2/ipa",
    "halo2/pasta/kaigi-roster-v1",
    "halo2/pasta/kaigi-usage-v1",
    "halo2/pasta/ivm-execution-v1",
    "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
    "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    "stark/fri",
    "stark/fri/sha256-goldilocks",
    "stark/fri/poseidon2-goldilocks",
    "stark/fri/sha256_goldilocks.v1"
  };

  private VerifyingKeyInstructionUtilsTests() {}

  public static void main(final String[] args) {
    canonicalEngineTagsAreExact();
    verifierRegistryIsClosedExactTypedAndImmutable();
    verifierRegistryRejectsAliasesRetiredFamiliesAndConfusables();
    everyRegistryLabelRejectsStructuralMutations();
    recordBackendTagIsRequiredAndMustMatchRegistryEngine();
    deprecationHeightMapsToWithdrawHeight();
    deprecationHeightMismatchThrows();
    noritoStatusParserRejectsNonExactLabels();
    inlineKeyCommitmentMustMatchSerializationBackend();
    protocolAndRetiredCatalogAliasesAreUnsupported();
    catalogClassifierAcceptsOnlyExactProductionLabels();
    adversarialAliasSplicesStayUnsupported();
    registerAndUpdateRejectUnsupportedProductionBackends();
    registerAndUpdateRejectNoncanonicalRecordFields();
    registerAndUpdateRejectBlankNames();
    System.out.println("[IrohaAndroid] VerifyingKeyInstructionUtilsTests passed.");
  }

  private static void canonicalEngineTagsAreExact() {
    assert Arrays.equals(
            new VerifyingKeyBackendTag[] {
              VerifyingKeyBackendTag.HALO2_IPA_PASTA, VerifyingKeyBackendTag.STARK
            },
            VerifyingKeyBackendTag.values())
        : "only the two native proof engines may be encoded";
    assert VerifyingKeyBackendTag.HALO2_IPA_PASTA
        == VerifyingKeyBackendTag.parse("halo2-ipa-pasta");
    assert VerifyingKeyBackendTag.STARK == VerifyingKeyBackendTag.parse("stark");

    for (final String label :
        new String[] {
          "",
          " halo2-ipa-pasta",
          "halo2-ipa-pasta ",
          "HALO2-IPA-PASTA",
          "Stark",
          "stark ",
          "halo2/ipa",
          "stark/fri",
          "halo2-bn254",
          "groth16",
          "groth16-bls12-377",
          "aztec-plonkish-private-kernel",
          "zkat",
          "silent-threshold-anoncred",
          "unsupported",
          "stark\u0000",
          "st\u0430rk"
        }) {
      assertThrows(
          IllegalArgumentException.class,
          () -> VerifyingKeyBackendTag.parse(label),
          label + " must not parse as a canonical engine tag");
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> VerifyingKeyBackendTag.parse(null),
        "null must not parse as a canonical engine tag");
  }

  private static void verifierRegistryIsClosedExactTypedAndImmutable() {
    final Set<String> expected = new LinkedHashSet<>(Arrays.asList(EXACT_REGISTRY));
    assert expected.size() == 12 : "test registry must not contain duplicates";
    assert expected.equals(VerifyingKeyBackendTag.VERIFIER_BACKEND_REGISTRY_LABELS_V1)
        : "Java registry must exactly mirror the native registry";

    for (final String label : EXACT_REGISTRY) {
      final VerifyingKeyBackendTag expectedTag =
          label.startsWith("halo2/")
              ? VerifyingKeyBackendTag.HALO2_IPA_PASTA
              : VerifyingKeyBackendTag.STARK;
      assert expectedTag == VerifyingKeyBackendTag.verifierBackendRegistryTagV1(label)
          : label + " must resolve to the expected native engine";
      assert VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(label)
          : label + " must be admitted exactly";
      assert label.equals(
          VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(label, "backend"));
    }

    assertThrows(
        UnsupportedOperationException.class,
        () ->
            VerifyingKeyBackendTag.VERIFIER_BACKEND_REGISTRY_LABELS_V1.add(
                "stark/fri/latest"),
        "public registry must be immutable");
  }

  private static void verifierRegistryRejectsAliasesRetiredFamiliesAndConfusables() {
    final String[] rejected = {
      "",
      "halo2-ipa-pasta",
      "stark",
      " halo2/ipa",
      "halo2/ipa ",
      "\thalo2/ipa",
      "halo2/ipa\n",
      "HALO2/IPA",
      "halo2//ipa",
      "halo2/ipa/",
      "halo2/ipa:",
      "halo2/ipa:ivm-execution-v1",
      "halo2/ipa::ivm-execution-v1",
      "halo2/ipa/ivm-execution-v1",
      "halo2/pasta/ipa/ivm-execution-v1",
      "halo2/pasta/ivm_execution_v1",
      "halo2/pasta/ivm-execution-v1/",
      "halo2/pasta/ivm-execution-v1\u0000",
      "halo2/pasta/ipa-pasta-cycle-v1",
      "halo2/ipa-pasta-cycle-v1",
      "halo2/pasta/ivm-overlay-bind",
      "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
      "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
      "halo2/pasta/tiny-add",
      "stark/fri/",
      "STARK/FRI",
      "stark/FRI",
      "stark/fri/latest",
      "stark/fri/sha256-goldilocks/extra",
      "stark/fri/sha256 goldilocks",
      "stark/fri/sha256+goldilocks",
      "stark/fri/sha256-goldilocks\u200B",
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
      "../halo2/ipa",
      "groth16",
      "groth16/bn254",
      "groth16/bls12-377",
      "halo2/bn254",
      "halo2/kzg",
      "kzg/powersoftau",
      "aztec-plonkish-private-kernel",
      "zkat",
      "silent-threshold-anoncred",
      "penumbra-masp",
      "orchard",
      "fcmp++",
      "jindo-lattice-pcs-zk",
      "sis-hints-anoncred-pq-v0",
      "sis-with-hints",
      "vega-existing-credential-zk-v0",
      "anonymous-pgc-k-out-of-n-v1",
      "stark/fri/dev-fixture",
      "stark/fri/externally-audited",
      "halo2/ipa:production-ready",
      "halo2/ipa:kzg"
    };

    for (final String label : rejected) {
      assert VerifyingKeyBackendTag.verifierBackendRegistryTagV1(label) == null
          : label + " must not resolve";
      assert !VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(label)
          : label + " must remain rejected";
      assertThrows(
          IllegalArgumentException.class,
          () -> VerifyingKeyBackendTag.requireVerifierBackendRegistryLabelV1(label, "backend"),
          label + " must fail closed");
    }
    assert VerifyingKeyBackendTag.verifierBackendRegistryTagV1(null) == null;
    assert !VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(null);
  }

  private static void everyRegistryLabelRejectsStructuralMutations() {
    for (final String label : EXACT_REGISTRY) {
      final char replacement = label.charAt(label.length() - 1) == 'x' ? 'y' : 'x';
      final String[] mutations = {
        " " + label,
        label + " ",
        label.toUpperCase(java.util.Locale.ROOT),
        label + "/",
        label + '\0',
        label + '\u200B',
        label.replaceFirst("/", "//"),
        label.substring(0, label.length() - 1) + replacement
      };
      for (final String mutation : mutations) {
        assert !VerifyingKeyBackendTag.isVerifierBackendRegistryLabelV1(mutation)
            : mutation + " mutated from " + label + " must be rejected";
      }
    }
  }

  private static void recordBackendTagIsRequiredAndMustMatchRegistryEngine() {
    final VerifyingKeyRecordDescription.Builder missing =
        VerifyingKeyRecordDescription.builder()
            .setVersion(1)
            .setCircuitId("vk-test")
            .setSchemaHashHex(repeatChar('a', 64))
            .setGasScheduleId("default")
            .setInlineKeyBytes(new byte[] {1, 2, 3});
    assertThrows(
        IllegalStateException.class,
        () -> missing.build("halo2/ipa"),
        "backendTag must be required");

    assertThrows(
        NullPointerException.class,
        () -> VerifyingKeyRecordDescription.builder().setBackendTag(null),
        "backendTag must reject null");

    final VerifyingKeyRecordDescription.Builder mismatch =
        VerifyingKeyRecordDescription.builder()
            .setVersion(1)
            .setCircuitId("vk-test")
            .setBackendTag(VerifyingKeyBackendTag.HALO2_IPA_PASTA)
            .setSchemaHashHex(repeatChar('a', 64))
            .setGasScheduleId("default")
            .setInlineKeyBytes(new byte[] {1, 2, 3});
    assertThrows(
        IllegalArgumentException.class,
        () -> mismatch.build("stark/fri"),
        "record engine must match the exact registry label");
    assertThrows(
        IllegalArgumentException.class,
        () -> mismatch.build("groth16"),
        "record backend must be in the exact registry");

    final Map<String, String> mismatchedArguments = baseArguments();
    assertThrows(
        IllegalArgumentException.class,
        () -> VerifyingKeyInstructionUtils.parseRecord(mismatchedArguments, "stark/fri"),
        "decoded record engine must match its registry label");
  }

  private static void deprecationHeightMapsToWithdrawHeight() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.deprecation_height", "42");
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa");
    final Map<String, String> encoded = record.toArguments("halo2/ipa");
    assert "42".equals(encoded.get("record.withdraw_height"));
    assert !encoded.containsKey("record.deprecation_height");
  }

  private static void deprecationHeightMismatchThrows() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.withdraw_height", "7");
    arguments.put("record.deprecation_height", "8");
    assertThrows(
        IllegalArgumentException.class,
        () -> VerifyingKeyInstructionUtils.parseRecord(arguments, "halo2/ipa"),
        "mismatched deprecation/withdraw heights must fail");
  }

  private static void noritoStatusParserRejectsNonExactLabels() {
    assert VerifyingKeyStatus.ACTIVE == VerifyingKeyStatus.parse("Active");
    for (final String label : new String[] {" Active", "Active ", "active", "ACTIVE", ""}) {
      assertThrows(
          IllegalArgumentException.class,
          () -> VerifyingKeyStatus.parse(label),
          label + " must not parse as an exact verifying-key status");
    }
  }

  private static void inlineKeyCommitmentMustMatchSerializationBackend() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    assertThrows(
        IllegalArgumentException.class,
        () -> record.toArguments("stark/fri/sha256-goldilocks"),
        "record engine must not change during serialization");
    assertThrows(
        IllegalArgumentException.class,
        () -> record.toArguments("halo2/pasta/kaigi-roster-v1"),
        "inline verifier commitment must remain bound to the exact registry label");

    final VerifyingKeyRecordDescription mismatchedRecord =
        VerifyingKeyInstructionUtils.parseRecord(starkArguments(), "stark/fri/sha256-goldilocks");
    assertThrows(
        IllegalArgumentException.class,
        () ->
            RegisterVerifyingKeyInstruction.builder()
                .setBackend("halo2/ipa")
                .setName("vk_test")
                .setRecord(mismatchedRecord)
                .build(),
        "register builder must reject a record from another engine");
    assertThrows(
        IllegalArgumentException.class,
        () ->
            UpdateVerifyingKeyInstruction.builder()
                .setBackend("halo2/ipa")
                .setName("vk_test")
                .setRecord(mismatchedRecord)
                .build(),
        "update builder must reject a record from another engine");
  }

  private static void protocolAndRetiredCatalogAliasesAreUnsupported() {
    for (final String label :
        new String[] {
          "halo2-ipa-orchard",
          "groth16-bls12-377",
          "fcmp-plus-plus-curve-tree",
          "lattice-pcs-sis",
          "miden-stark",
          "aztec-plonkish-private-kernel",
          "pq-masp-stark-fri",
          "anonymous-pgc",
          "verange",
          "zkat",
          "recursive-anonymous-admission",
          "vega-existing-credential-zk",
          "silent-threshold-anoncred",
          "zk-x509",
          "sis-with-hints"
        }) {
      assert VerifyingKeyBackendTag.CatalogBackendTag.UNSUPPORTED
          == VerifyingKeyBackendTag.fromCatalogLabel(label);
      assert !VerifyingKeyBackendTag.isProductionVerifyBackendLabel(label);
    }
  }

  private static void catalogClassifierAcceptsOnlyExactProductionLabels() {
    for (final String label :
        new String[] {"halo2-ipa-pasta", "stark", "halo2/ipa", "stark/fri"}) {
      assert VerifyingKeyBackendTag.CatalogBackendTag.PRODUCTION
          == VerifyingKeyBackendTag.fromCatalogLabel(label);
    }
    for (final String label :
        new String[] {"HALO2/IPA", " halo2/ipa", "halo2/ipa ", "Stark"}) {
      assert VerifyingKeyBackendTag.CatalogBackendTag.UNSUPPORTED
          == VerifyingKeyBackendTag.fromCatalogLabel(label);
    }
  }

  private static void adversarialAliasSplicesStayUnsupported() {
    for (final String label :
        new String[] {
          "halo2/ipa/orchard/dev-fixture",
          "stark/fri/miden/claimed-production",
          "anonymous-pgc-k-out-of-n-v1-production",
          "sis-hints-anoncred-pq-v0-devfixture",
          "groth16/bls12-377/../../prod",
          "post-quantum-masp/audit-claimed"
        }) {
      assert VerifyingKeyBackendTag.CatalogBackendTag.UNSUPPORTED
          == VerifyingKeyBackendTag.fromCatalogLabel(label);
      assertThrows(
          IllegalArgumentException.class,
          () -> VerifyingKeyBackendTag.parse(label),
          label + " must not parse as a canonical Norito tag");
    }
  }

  private static void registerAndUpdateRejectUnsupportedProductionBackends() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    final IllegalArgumentException paddedError =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(
                    " halo2/ipa", "backend"),
            "surrounding whitespace must fail before unsupported-backend classification");
    assert paddedError.getMessage().contains("surrounding whitespace");
    for (final String backend : rejectedRegistryBackends()) {
      assertThrows(
          IllegalArgumentException.class,
          () -> RegisterVerifyingKeyInstruction.builder().setBackend(backend),
          backend + " must be rejected by register builder");
      assertThrows(
          IllegalArgumentException.class,
          () -> UpdateVerifyingKeyInstruction.builder().setBackend(backend),
          backend + " must be rejected by update builder");

      final Map<String, String> registerArguments = baseArguments();
      registerArguments.put("backend", backend);
      registerArguments.put("name", "vk_test");
      assertThrows(
          IllegalArgumentException.class,
          () -> RegisterVerifyingKeyInstruction.fromArguments(registerArguments),
          backend + " must be rejected by register decoding");

      final Map<String, String> updateArguments = baseArguments();
      updateArguments.put("backend", backend);
      updateArguments.put("name", "vk_test");
      assertThrows(
          IllegalArgumentException.class,
          () -> UpdateVerifyingKeyInstruction.fromArguments(updateArguments),
          backend + " must be rejected by update decoding");
    }

    final RegisterVerifyingKeyInstruction validRegister =
        RegisterVerifyingKeyInstruction.builder()
            .setBackend("halo2/ipa")
            .setName("vk_test")
            .setRecord(record)
            .build();
    assert "halo2/ipa".equals(validRegister.backend());

    final UpdateVerifyingKeyInstruction validUpdate =
        UpdateVerifyingKeyInstruction.builder()
            .setBackend("stark/fri/sha256-goldilocks")
            .setName("vk_test")
            .setRecord(
                VerifyingKeyInstructionUtils.parseRecord(
                    starkArguments(), "stark/fri/sha256-goldilocks"))
            .build();
    assert "stark/fri/sha256-goldilocks".equals(validUpdate.backend());
  }

  private static void registerAndUpdateRejectBlankNames() {
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa");
    for (final String name : new String[] {"", "   ", "\t", "\n", " vk_test", "vk_test "}) {
      assertThrows(
          IllegalArgumentException.class,
          () ->
              RegisterVerifyingKeyInstruction.builder()
                  .setBackend("halo2/ipa")
                  .setName(name)
                  .setRecord(record)
                  .build(),
          "invalid register name must fail");
      assertThrows(
          IllegalArgumentException.class,
          () ->
              UpdateVerifyingKeyInstruction.builder()
                  .setBackend("halo2/ipa")
                  .setName(name)
                  .setRecord(record)
                  .build(),
          "invalid update name must fail");
    }
  }

  private static void registerAndUpdateRejectNoncanonicalRecordFields() {
    final Map<String, String> canonical =
        VerifyingKeyInstructionUtils.parseRecord(baseArguments(), "halo2/ipa")
            .toArguments("halo2/ipa");
    final String[][] cases = {
      {"record.circuit_id", " vk-test"},
      {"record.circuit_id", "vk-test "},
      {"record.backend_tag", " halo2-ipa-pasta"},
      {"record.backend_tag", "HALO2-IPA-PASTA"},
      {"record.backend_tag", "stark"},
      {"record.curve", " pallas"},
      {"record.curve", "pallas "},
      {"record.public_inputs_schema_hash_hex", " " + canonical.get("record.public_inputs_schema_hash_hex")},
      {"record.public_inputs_schema_hash_hex", canonical.get("record.public_inputs_schema_hash_hex") + " "},
      {"record.commitment_hex", " " + canonical.get("record.commitment_hex")},
      {"record.commitment_hex", canonical.get("record.commitment_hex") + " "},
      {"record.vk_bytes_b64", " " + canonical.get("record.vk_bytes_b64")},
      {"record.vk_bytes_b64", canonical.get("record.vk_bytes_b64") + " "},
      {"record.vk_len", " " + canonical.get("record.vk_len")},
      {"record.max_proof_bytes", " 1024"},
      {"record.gas_schedule_id", " default"},
      {"record.gas_schedule_id", "default "},
      {"record.metadata_uri_cid", " bafy-metadata"},
      {"record.metadata_uri_cid", "bafy-metadata "},
      {"record.vk_bytes_cid", " bafy-vk"},
      {"record.vk_bytes_cid", "bafy-vk "},
      {"record.activation_height", " 10"},
      {"record.withdraw_height", "10 "},
      {"record.deprecation_height", " 10"},
      {"record.status", " Active"},
      {"record.status", "active"}
    };
    for (final String[] entry : cases) {
      final Map<String, String> registerArguments = baseArguments();
      registerArguments.put("backend", "halo2/ipa");
      registerArguments.put("name", "vk_test");
      registerArguments.put(entry[0], entry[1]);
      assertThrows(
          RuntimeException.class,
          () -> RegisterVerifyingKeyInstruction.fromArguments(registerArguments),
          entry[0] + " mutation must fail register decoding");

      final Map<String, String> updateArguments = baseArguments();
      updateArguments.put("backend", "halo2/ipa");
      updateArguments.put("name", "vk_test");
      updateArguments.put(entry[0], entry[1]);
      assertThrows(
          RuntimeException.class,
          () -> UpdateVerifyingKeyInstruction.fromArguments(updateArguments),
          entry[0] + " mutation must fail update decoding");
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

  private static Map<String, String> starkArguments() {
    final Map<String, String> arguments = baseArguments();
    arguments.put("record.backend_tag", "stark");
    return arguments;
  }

  private static String[] rejectedRegistryBackends() {
    return new String[] {
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
      "HALO2/IPA",
      "stark/FRI",
      "halo2/ipa::ivm-execution-v1",
      "stark/fri/sha256..goldilocks",
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
      "halo2/pasta/vote-bool-commit",
      "halo2/ipa/vote-bool-commit",
      "halo2/ipa:vote-bool-commit",
      "halo2/pasta/vote-bool-commit-merkle2",
      "halo2/ipa/vote-bool-commit-merkle8",
      "halo2/ipa:vote-bool-commit-merkle16",
      "halo2/pasta/anon-transfer-2x2",
      "halo2/ipa/anon-transfer-2x2",
      "halo2/ipa:anon-transfer-2x2",
      "halo2/pasta/anon-transfer-2x2-merkle2",
      "halo2/ipa/anon-transfer-2x2-merkle8",
      "halo2/ipa:anon-transfer-2x2-merkle16",
      "halo2/pasta/ipa-pasta-cycle-v1",
      "stark/fri/miden",
      "stark/fri/latest",
      "stark/fri/attestation",
      "stark/fri/contest",
      "stark/fri/random-profile",
      "stark/fri/sha512-goldilocks",
      "stark/fri/audit-proof-v1",
      "stark/fri/sha256 goldilocks",
      "stark/fri/sha256+goldilocks",
      "stark/fri/dev-fixture",
      "stark/fri/d-e-v-f-i-x-t-u-r-e",
      "stark/fri/dev",
      "stark/fri/d-e-v",
      "stark/fri/test",
      "stark/fri/t-e-s-t",
      "stark/fri/todo",
      "stark/fri/t-o-d-o",
      "stark/fri/draft-only",
      "stark/fri/d-r-a-f-t",
      "stark/fri/pending-audit",
      "stark/fri/replace-before-mainnet",
      "stark/fri/not-production-ready",
      "stark/fri/placeholder",
      "stark/fri/externally-audited",
      "halo2/ipa:production-ready",
      "halo2/ipa:mainnet-ready",
      "halo2/ipa:release-ready",
      "halo2/ipa:certified-mainnet",
      "halo2/ipa:third-party-audited",
      "stark/fri/audit-signoff",
      "stark/fri/boi-audited",
      "stark/fri/external-security-review",
      "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
      "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
      "stark/fri/a-u-d-i-t-c-l-a-i-m",
      "halo2/ipa:dev-fixture",
      "halo2/ipa:dev",
      "halo2/ipa:d-e-v",
      "halo2/ipa:todo-proof",
      "halo2/ipa:t-o-d-o-proof",
      "halo2/ipa:draft-proof",
      "halo2/ipa:d-r-a-f-t-proof",
      "halo2/ipa:pending-audit",
      "halo2/ipa:replace-before-production",
      "halo2/ipa:not-for-production",
      "halo2/ipa:dummy",
      "halo2/ipa:f-a-k-e",
      "halo2/ipa:stub",
      "halo2/ipa:s-a-m-p-l-e",
      "halo2/ipa:kzg",
      "halo2/kzg",
      "kzg/powersoftau",
      "../halo2/ipa",
      "halo2\uFF0Fipa",
      "halo2/\u200Bipa",
      "h\u0430lo2/ipa",
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

  private static <T extends Throwable> T assertThrows(
      final Class<T> type, final Runnable runnable, final String message) {
    try {
      runnable.run();
    } catch (final Throwable thrown) {
      if (type.isInstance(thrown)) {
        return type.cast(thrown);
      }
      throw new AssertionError(
          message + ": expected " + type.getName() + " but got " + thrown, thrown);
    }
    throw new AssertionError(message + ": expected " + type.getName());
  }
}
