package org.hyperledger.iroha.android.sorafs;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.ThreadLocalRandom;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDisputeInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CollateralPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CommitmentDiscountTier;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CreditPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.DiscountSchedule;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.StorageClass;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.TierRate;

/** Regression tests covering SoraFS capacity declaration/dispute instruction builders. */
public final class SorafsCapacityMarketplaceInstructionTests {

  private SorafsCapacityMarketplaceInstructionTests() {}

  private static final String PROVIDER_ID =
      "11".repeat(32); // 64 hex chars representing provider digest
  private static final String COMPLAINANT_ID =
      "22".repeat(32); // 64 hex chars representing complainant digest
  private static final String DISPUTE_ID = "33".repeat(32);

  public static void main(final String[] args) {
    testRegisterCapacityDisputeBuilder();
    testDisputeRejectsInvalidBase64();
    testDisputeValidationFailure();
    testSetPricingScheduleBuilder();
    testPricingScheduleIntegerParsing();
    System.out.println(
        "[IrohaAndroid] SorafsCapacityMarketplaceInstructionTests passed (dispute/pricing).");
  }

  // Capacity declaration coverage lives in the Kotlin-owned JDK-8 Java consumer:
  // SorafsCapacityDeclarationJavaConsumerTest. Consensus-derived projections are rejected.

  private static void testRegisterCapacityDisputeBuilder() {
    final byte[] payload = "capacity-dispute".getBytes(StandardCharsets.UTF_8);
    final RegisterCapacityDisputeInstruction instruction =
        RegisterCapacityDisputeInstruction.builder()
            .setDisputeIdHex(DISPUTE_ID)
            .setDisputePayload(payload)
            .setProviderIdHex(PROVIDER_ID)
            .setComplainantIdHex(COMPLAINANT_ID)
            .setKind(RegisterCapacityDisputeInstruction.Kind.PROOF_FAILURE)
            .setSubmittedEpoch(1_801_222L)
            .setDescription("Proof failure during nightly probe")
            .setRequestedRemedy("Slash collateral")
            .setEvidence(
                RegisterCapacityDisputeInstruction.Evidence.builder()
                    .setDigestHex("aa".repeat(32))
                    .setMediaType("application/json")
                    .setUri("sorafs://evidence/alpha")
                    .setSizeBytes(1_024L)
                    .build())
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "RegisterCapacityDispute".equals(args.get("action")) : "action mismatch";
    assert args.get("dispute_b64") != null : "payload missing";
    assert "proof_failure".equals(args.get("kind")) : "kind mismatch";
    assert "Slash collateral".equals(args.get("requested_remedy")) : "remedy mismatch";
    assert "application/json".equals(args.get("evidence.media_type")) : "media type mismatch";
    assert "1024".equals(args.get("evidence.size_bytes")) : "size mismatch";
    assert instruction.disputeKind() == RegisterCapacityDisputeInstruction.Kind.PROOF_FAILURE
        : "kind mismatch after decode";
    assert instruction.evidence().sizeBytes().equals(1_024L) : "evidence mismatch";
  }

  private static void testDisputeRejectsInvalidBase64() {
    boolean threw = false;
    try {
      RegisterCapacityDisputeInstruction.builder()
          .setDisputeIdHex(DISPUTE_ID)
          .setDisputePayloadBase64("not!base64");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid dispute payload base64 to throw";
  }

  private static void testDisputeValidationFailure() {
    boolean threw = false;
    try {
      RegisterCapacityDisputeInstruction.builder()
          .setDisputeIdHex(DISPUTE_ID)
          .setDisputePayloadBase64("Cg==")
          .setProviderIdHex(PROVIDER_ID)
          .setComplainantIdHex(COMPLAINANT_ID)
          .setKind(RegisterCapacityDisputeInstruction.Kind.OTHER)
          .setSubmittedEpoch(1)
          .setDescription("Missing evidence should fail")
          .build();
    } catch (final IllegalStateException ex) {
      threw = true;
    }
    assert threw : "Expected missing evidence validation to fire";
  }

  private static void testSetPricingScheduleBuilder() {
    final SetPricingScheduleInstruction schedule = newPricingSchedule();

    final Map<String, String> args = schedule.toArguments();
    assert "SetPricingSchedule".equals(args.get("action")) : "action mismatch";
    assert "xor".equals(args.get("schedule.currency_code")) : "currency mismatch";
    assert "hot".equals(args.get("schedule.tiers.0.storage_class")) : "tier mismatch";
    assert schedule.tiers().size() == 2 : "tier count mismatch";
    assert "Test schedule".equals(schedule.notes()) : "notes mismatch";
  }

  private static SetPricingScheduleInstruction newPricingSchedule() {
    return SetPricingScheduleInstruction.builder()
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
  }

  private static void testPricingScheduleIntegerParsing() {
    final Map<String, String> base = newPricingSchedule().toArguments();
    final String commitmentPrefix = "schedule.discounts.commitment_tiers.0.";
    final String[] integerKeys = {
      "schedule.credit.low_balance_alert_bps",
      "schedule.discounts.loyalty_months_required",
      "schedule.discounts.loyalty_discount_bps",
      commitmentPrefix + "discount_bps"
    };

    for (final String key : integerKeys) {
      assertPricingIntegerRejected(base, key, "4294967296");
      assertPricingIntegerRejected(base, key, "-2147483649");
    }

    final Map<String, String> maximums = new LinkedHashMap<>(base);
    for (final String key : integerKeys) {
      maximums.put(key, Integer.toString(Integer.MAX_VALUE));
    }
    final String minimumCommitmentKey = commitmentPrefix + "minimum_commitment_gib_month";
    maximums.put(minimumCommitmentKey, "4294967296");

    final Map<String, String> parsed =
        SetPricingScheduleInstruction.fromArguments(maximums).toArguments();
    for (final String key : integerKeys) {
      assert Integer.toString(Integer.MAX_VALUE).equals(parsed.get(key))
          : key + " should retain Integer.MAX_VALUE";
    }
    assert "4294967296".equals(parsed.get(minimumCommitmentKey))
        : "long minimum commitment should not be narrowed";
  }

  private static void assertPricingIntegerRejected(
      final Map<String, String> base, final String key, final String value) {
    final Map<String, String> arguments = new LinkedHashMap<>(base);
    arguments.put(key, value);
    try {
      SetPricingScheduleInstruction.fromArguments(arguments);
      throw new AssertionError(key + " should reject " + value);
    } catch (final IllegalArgumentException ex) {
      final String expected =
          "Instruction argument '" + key + "' is outside the signed 32-bit integer range";
      assert expected.equals(ex.getMessage()) : "unexpected range error for " + key;
    }
  }

  // Provider-credit coverage uses the canonical Kotlin API in
  // SorafsProviderCreditJavaConsumerTest, including the required current-row guard.

  private static byte[] randomBytes(final int length) {
    final byte[] bytes = new byte[length];
    ThreadLocalRandom.current().nextBytes(bytes);
    return bytes;
  }
}
