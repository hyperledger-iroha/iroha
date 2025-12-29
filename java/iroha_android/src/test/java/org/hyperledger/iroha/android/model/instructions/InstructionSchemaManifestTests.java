package org.hyperledger.iroha.android.model.instructions;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.GovernanceInstructionUtils.AtWindow;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CollateralPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CommitmentDiscountTier;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CreditPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.DiscountSchedule;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.StorageClass;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.TierRate;
import org.hyperledger.iroha.android.testing.SimpleJson;

/**
 * Snapshot-based regression test that gates the governance/manifest/trigger instruction builders.
 *
 * <p>The generated manifest lives in {@code instruction_schema_manifest.json} under the test
 * resources. When instruction layouts change intentionally, run this test with {@code
 * PRINT_MANIFEST=1} to refresh the manifest and commit the updated snapshot so other SDKs can reuse
 * the same Norito arguments.
 */
public final class InstructionSchemaManifestTests {

  private InstructionSchemaManifestTests() {}

  public static void main(final String[] args) throws Exception {
    final LinkedHashMap<String, ManifestEntry> manifest = buildManifest();
    final String generated = renderManifest(manifest);
    final String expected =
        loadResource("/instruction_schema_manifest.json", "instruction_schema_manifest.json");

    final boolean shouldPrint =
        Boolean.parseBoolean(System.getenv().getOrDefault("PRINT_MANIFEST", "false"));
    if (shouldPrint) {
      System.out.println(generated);
    }

    assert expected != null
        : "instruction_schema_manifest.json is missing; run with PRINT_MANIFEST=1 and add the snapshot.";
    if (!expected.equals(generated)) {
      throw new AssertionError(
          "Instruction schema manifest drift detected. "
              + "Run with PRINT_MANIFEST=1 and update instruction_schema_manifest.json.");
    }

    validateJson(expected);
    System.out.println("[IrohaAndroid] InstructionSchemaManifestTests passed.");
  }

  private static LinkedHashMap<String, ManifestEntry> buildManifest() {
    final LinkedHashMap<String, ManifestEntry> manifest = new LinkedHashMap<>();

    manifest.put(
        "governance_propose_deploy_contract",
        from(
            InstructionBuilders.proposeDeployContract(
                ProposeDeployContractInstruction.builder()
                    .setNamespace("apps")
                    .setContractId("demo.contract")
                    .setCodeHashHex("a0".repeat(32))
                    .setAbiHashHex("b1".repeat(32))
                    .setAbiVersion("1")
                    .setWindow(new AtWindow(10, 20))
                    .setVotingMode(GovernanceInstructionUtils.VotingMode.PLAIN)
                    .build())));

    manifest.put(
        "governance_cast_zk_ballot",
        from(
            InstructionBuilders.castZkBallot(
                CastZkBallotInstruction.builder()
                    .setElectionId("election-1")
                    .setProofBase64("AQID")
                    .setPublicInputsJson("{\"foo\":1}")
                    .build())));

    manifest.put(
        "governance_persist_council",
        from(
            InstructionBuilders.persistCouncilForEpoch(
                PersistCouncilForEpochInstruction.builder()
                    .setEpoch(99)
                    .addMember("alice@sora")
                    .addMember("bob@sora")
                    .addAlternate("carol@sora")
                    .setCandidatesCount(5)
                    .setVerified(2)
                    .setDerivedBy(GovernanceInstructionUtils.CouncilDerivationKind.VRF)
                    .build())));

    final String spaceManifestJson = "{\"uaid\":\"uaid-demo\",\"dataspace\":11,\"version\":1}";
    manifest.put(
        "sorafs_publish_space_directory_manifest",
        from(
            InstructionBuilders.publishSpaceDirectoryManifest(
                PublishSpaceDirectoryManifestInstruction.builder()
                    .setManifestJson(spaceManifestJson)
                    .build())));

    manifest.put(
        "sorafs_revoke_space_directory_manifest",
        from(
            InstructionBuilders.revokeSpaceDirectoryManifest(
                RevokeSpaceDirectoryManifestInstruction.builder()
                    .setUaid("uaid-demo")
                    .setDataspace(11L)
                    .setRevokedEpoch(4610L)
                    .setReason("Emergency deny")
                    .build())));

    manifest.put(
        "sorafs_expire_space_directory_manifest",
        from(
            InstructionBuilders.expireSpaceDirectoryManifest(
                ExpireSpaceDirectoryManifestInstruction.builder()
                    .setUaid("uaid-demo")
                    .setDataspace(11L)
                    .setExpiredEpoch(4600L)
                    .build())));

    final String councilEnvelope =
        Base64.getEncoder().encodeToString("council-envelope".getBytes(StandardCharsets.UTF_8));
    manifest.put(
        "sorafs_approve_pin_manifest",
        from(
            InstructionBuilders.approvePinManifest(
                ApprovePinManifestInstruction.builder()
                    .setDigestHex("aa".repeat(32))
                    .setApprovedEpoch(42)
                    .setCouncilEnvelopeBase64(councilEnvelope)
                    .setCouncilEnvelopeDigestHex("bb".repeat(32))
                    .build())));

    manifest.put(
        "sorafs_retire_pin_manifest",
        from(
            InstructionBuilders.retirePinManifest(
                RetirePinManifestInstruction.builder()
                    .setDigestHex("cc".repeat(32))
                    .setRetiredEpoch(99)
                    .setReason("rotated")
                    .build())));

    manifest.put(
        "sorafs_bind_manifest_alias",
        from(
            InstructionBuilders.bindManifestAlias(
                BindManifestAliasInstruction.builder()
                    .setDigestHex("dd".repeat(32))
                    .setAliasBinding("docs", "sora", "ee".repeat(32))
                    .setBoundEpoch(12)
                    .setExpiryEpoch(36)
                    .build())));

    final IssueReplicationOrderInstruction replicationOrder =
        IssueReplicationOrderInstruction.builder()
            .setOrderIdHex(
                "44b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c044b3b7c174c8e9c0")
            .setOrderPayloadBase64(
                Base64.getEncoder()
                    .encodeToString("replication-order".getBytes(StandardCharsets.UTF_8)))
            .setIssuedEpoch(20)
            .setDeadlineEpoch(28)
            .build();
    manifest.put("sorafs_issue_replication_order", from(InstructionBuilders.issueReplicationOrder(replicationOrder)));

    manifest.put(
        "sorafs_complete_replication_order",
        from(
            InstructionBuilders.completeReplicationOrder(
                CompleteReplicationOrderInstruction.builder()
                    .setOrderIdHex(replicationOrder.orderIdHex())
                    .setCompletionEpoch(31)
                    .build())));

    manifest.put(
        "sorafs_record_replication_receipt",
        from(
            InstructionBuilders.recordReplicationReceipt(
                RecordReplicationReceiptInstruction.builder()
                    .setOrderIdHex(replicationOrder.orderIdHex())
                    .setProviderIdHex(
                        "51fdb0bf4c6a79ce51fdb0bf4c6a79ce51fdb0bf4c6a79ce51fdb0bf4c6a79ce")
                    .setStatus(RecordReplicationReceiptInstruction.Status.ACCEPTED)
                    .setTimestamp(1_717_171_111L)
                    .setPorSampleDigestHex("aabbccdd")
                    .build())));

    final SetPricingScheduleInstruction pricingSchedule =
        SetPricingScheduleInstruction.builder()
            .setVersion(1)
            .setCurrencyCode("xor")
            .setDefaultStorageClass(StorageClass.HOT)
            .addTier(
                TierRate.builder()
                    .setStorageClass(StorageClass.HOT)
                    .setStoragePriceNanoPerGibMonth(BigInteger.valueOf(500_000_000L))
                    .setEgressPriceNanoPerGib(BigInteger.valueOf(50_000_000L))
                    .build())
            .addTier(
                TierRate.builder()
                    .setStorageClass(StorageClass.WARM)
                    .setStoragePriceNanoPerGibMonth(BigInteger.valueOf(200_000_000L))
                    .setEgressPriceNanoPerGib(BigInteger.valueOf(20_000_000L))
                    .build())
            .setCollateralPolicy(
                CollateralPolicy.builder()
                    .setMultiplierBps(30_000)
                    .setOnboardingDiscountBps(5_000)
                    .setOnboardingPeriodSecs(86_400L)
                    .build())
            .setCreditPolicy(
                CreditPolicy.builder()
                    .setSettlementWindowSecs(86_400L)
                    .setSettlementGraceSecs(3_600L)
                    .setLowBalanceAlertBps(1_000)
                    .build())
            .setDiscountSchedule(
                DiscountSchedule.builder()
                    .setLoyaltyMonthsRequired(12)
                    .setLoyaltyDiscountBps(1_000)
                    .addCommitmentTier(
                        CommitmentDiscountTier.builder()
                            .setMinimumCommitmentGibMonth(500L)
                            .setDiscountBps(500)
                            .build())
                    .build())
            .setNotes("Test schedule")
            .build();
    manifest.put("sorafs_set_pricing_schedule", from(InstructionBuilders.setPricingSchedule(pricingSchedule)));

    manifest.put(
        "sorafs_upsert_provider_credit",
        from(
            InstructionBuilders.upsertProviderCredit(
                UpsertProviderCreditInstruction.builder()
                    .setProviderIdHex("11".repeat(32))
                    .setAvailableCreditNano(new BigInteger("123456789000"))
                    .setBondedNano(new BigInteger("555000000000"))
                    .setRequiredBondNano(new BigInteger("777000000000"))
                    .setExpectedSettlementNano(new BigInteger("333000000000"))
                    .setOnboardingEpoch(1_700_000L)
                    .setLastSettlementEpoch(1_700_800L)
                    .setLowBalanceSinceEpoch(1_700_500L)
                    .setSlashedNano(new BigInteger("1000"))
                    .setUnderDeliveryStrikes(2)
                    .setLastPenaltyEpoch(1_700_600L)
                    .putMetadata("region", "jp")
                    .build())));

    final InstructionBox nestedPermission =
        InstructionBuilders.grantPermission("bob@wonderland", "CanSubmitTransactions");
    final String sampleHash =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    manifest.put(
        "trigger_register_pipeline",
        from(
            InstructionBuilders.registerPipelineTrigger(
                "tx_monitor",
                "monitor@wonderland",
                List.of(nestedPermission),
                RegisterPipelineTriggerInstruction.PipelineFilter.transaction()
                    .setTransactionHash(sampleHash)
                    .setTransactionBlockHeight(42L)
                    .setTransactionStatus("Approved"),
                5,
                Map.of("label", "pipeline"))));

    manifest.put(
        "trigger_register_time",
        from(
            InstructionBuilders.registerTimeTrigger(
                "daily_report",
                "alice@wonderland",
                List.of(
                    InstructionBuilders.mintAsset(
                        "rose#wonderland#treasury@wonderland", "5")),
                1_735_000_000_000L,
                60_000L,
                3,
                Map.of("label", "periodic"))));

    manifest.put(
        "trigger_register_precommit",
        from(
            InstructionBuilders.registerPrecommitTrigger(
                "pc_guard",
                "carol@wonderland",
                List.of(nestedPermission),
                null,
                Map.of("purpose", "guard"))));

    manifest.put(
        "trigger_execute",
        from(InstructionBuilders.executeTrigger("pipeline_trigger", "executor@wonderland")));

    return manifest;
  }

  private static ManifestEntry from(final InstructionBox box) {
    final LinkedHashMap<String, String> args = canonicalArguments(box.arguments());
    return new ManifestEntry(box.kind(), args, fingerprint(box.kind(), args));
  }

  private static LinkedHashMap<String, String> canonicalArguments(final Map<String, String> args) {
    final LinkedHashMap<String, String> ordered = new LinkedHashMap<>();
    args.keySet().stream().sorted().forEach(key -> ordered.put(key, args.get(key)));
    return ordered;
  }

  private static String fingerprint(final InstructionKind kind, final Map<String, String> args) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      digest.update(kind.displayName().getBytes(StandardCharsets.UTF_8));
      args.forEach(
          (key, value) -> {
            digest.update((byte) 0x00);
            digest.update(key.getBytes(StandardCharsets.UTF_8));
            digest.update((byte) '=');
            digest.update(value.getBytes(StandardCharsets.UTF_8));
          });
      return bytesToHex(digest.digest());
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest unavailable", ex);
    }
  }

  private static String renderManifest(final LinkedHashMap<String, ManifestEntry> manifest) {
    final StringBuilder builder = new StringBuilder();
    builder.append("{\n");
    int entryIndex = 0;
    for (final Map.Entry<String, ManifestEntry> entry : manifest.entrySet()) {
      builder.append("  \"").append(entry.getKey()).append("\": {\n");
      builder.append("    \"kind\": \"").append(entry.getValue().kind.displayName()).append("\",\n");
      builder.append("    \"fingerprint\": \"").append(entry.getValue().fingerprint).append("\",\n");
      builder.append("    \"arguments\": {\n");
      final int argCount = entry.getValue().arguments.size();
      int argIndex = 0;
      for (final Map.Entry<String, String> arg : entry.getValue().arguments.entrySet()) {
        builder.append("      \"")
            .append(escape(arg.getKey()))
            .append("\": \"")
            .append(escape(arg.getValue()))
            .append("\"");
        builder.append(argIndex + 1 == argCount ? "\n" : ",\n");
        argIndex++;
      }
      builder.append("    }\n");
      builder.append(entryIndex + 1 == manifest.size() ? "  }\n" : "  },\n");
      entryIndex++;
    }
    builder.append("}\n");
    return builder.toString();
  }

  private static String escape(final String value) {
    final StringBuilder result = new StringBuilder();
    for (final char c : value.toCharArray()) {
      switch (c) {
        case '\\':
          result.append("\\\\");
          break;
        case '"':
          result.append("\\\"");
          break;
        case '\n':
          result.append("\\n");
          break;
        case '\r':
          result.append("\\r");
          break;
        case '\t':
          result.append("\\t");
          break;
        default:
          result.append(c);
      }
    }
    return result.toString();
  }

  private static String loadResource(final String resourcePath, final String nameForErrors)
      throws IOException {
    final InputStream classpathStream =
        InstructionSchemaManifestTests.class.getResourceAsStream(resourcePath);
    if (classpathStream != null) {
      return readStream(classpathStream, nameForErrors);
    }
    final String[] fallbacks = {
      "java/iroha_android/src/test/resources/" + nameForErrors,
      "src/test/resources/" + nameForErrors
    };
    for (final String fallback : fallbacks) {
      try (InputStream stream =
          InstructionSchemaManifestTests.class.getClassLoader().getResourceAsStream(fallback)) {
        if (stream != null) {
          return readStream(stream, nameForErrors);
        }
      }
      final java.nio.file.Path path = java.nio.file.Path.of(fallback);
      if (java.nio.file.Files.exists(path)) {
        return java.nio.file.Files.readString(path, StandardCharsets.UTF_8);
      }
    }
    return null;
  }

  private static String readStream(final InputStream stream, final String nameForErrors)
      throws IOException {
    try (stream;
        BufferedReader reader =
            new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
      final StringBuilder builder = new StringBuilder();
      String line;
      while ((line = reader.readLine()) != null) {
        builder.append(line).append('\n');
      }
      return builder.toString();
    } catch (final IOException ex) {
      throw new IOException("Failed to load " + nameForErrors, ex);
    }
  }

  private static void validateJson(final String json) {
    try {
      final Object parsed = SimpleJson.parse(json);
      assert parsed instanceof Map : "Manifest JSON should decode to an object";
    } catch (final Exception ex) {
      throw new AssertionError("Manifest JSON is not valid: " + ex.getMessage(), ex);
    }
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private record ManifestEntry(InstructionKind kind, Map<String, String> arguments, String fingerprint) {}
}
