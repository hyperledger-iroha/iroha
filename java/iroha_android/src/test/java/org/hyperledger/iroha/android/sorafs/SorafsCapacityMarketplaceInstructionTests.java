package org.hyperledger.iroha.android.sorafs;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.ThreadLocalRandom;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDeclarationInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterCapacityDisputeInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CollateralPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CommitmentDiscountTier;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.CreditPolicy;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.DiscountSchedule;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.StorageClass;
import org.hyperledger.iroha.android.model.instructions.SetPricingScheduleInstruction.TierRate;
import org.hyperledger.iroha.android.model.instructions.UpsertProviderCreditInstruction;

/** Regression tests covering SoraFS capacity declaration/dispute instruction builders. */
public final class SorafsCapacityMarketplaceInstructionTests {

  private SorafsCapacityMarketplaceInstructionTests() {}

  private static final String PROVIDER_ID =
      "11".repeat(32); // 64 hex chars representing provider digest
  private static final String COMPLAINANT_ID =
      "22".repeat(32); // 64 hex chars representing complainant digest
  private static final String DISPUTE_ID = "33".repeat(32);

  public static void main(final String[] args) {
    testRegisterCapacityDeclarationBuilder();
    testDeclarationRejectsInvalidBase64();
    testRegisterCapacityDisputeBuilder();
    testDisputeRejectsInvalidBase64();
    testDisputeValidationFailure();
    testSetPricingScheduleBuilder();
    testUpsertProviderCreditBuilder();
    System.out.println(
        "[IrohaAndroid] SorafsCapacityMarketplaceInstructionTests passed (declaration/dispute/pricing/credit).");
  }

  private static void testRegisterCapacityDeclarationBuilder() {
    final byte[] declarationBytes = randomBytes(48);
    final RegisterCapacityDeclarationInstruction instruction =
        RegisterCapacityDeclarationInstruction.builder()
            .setProviderIdHex(PROVIDER_ID)
            .setDeclarationBytes(declarationBytes)
            .setCommittedCapacityGib(2_048L)
            .setRegisteredEpoch(1_701_234L)
            .setValidFromEpoch(1_701_300L)
            .setValidUntilEpoch(1_801_300L)
            .putMetadata("region", "ap-northeast-1")
            .putMetadata("notes", "First slate")
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "RegisterCapacityDeclaration".equals(args.get("action")) : "action mismatch";
    assert PROVIDER_ID.equals(args.get("provider_id_hex")) : "provider mismatch";
    assert args.get("declaration_b64") != null : "declaration missing";
    assert "2048".equals(args.get("committed_capacity_gib")) : "capacity mismatch";
    assert "region".equals("region") && "ap-northeast-1".equals(args.get("metadata.region"))
        : "metadata region mismatch";
    assert "First slate".equals(args.get("metadata.notes")) : "metadata notes mismatch";
    assert instruction.committedCapacityGib() == 2_048L : "round trip committed capacity mismatch";
    assert instruction.metadata().get("region").equals("ap-northeast-1") : "metadata mismatch";
  }

  private static void testDeclarationRejectsInvalidBase64() {
    boolean threw = false;
    try {
      RegisterCapacityDeclarationInstruction.builder()
          .setProviderIdHex(PROVIDER_ID)
          .setDeclarationBase64("not!base64")
          .setCommittedCapacityGib(512L)
          .setRegisteredEpoch(1_701_234L)
          .setValidFromEpoch(1_701_300L)
          .setValidUntilEpoch(1_801_300L)
          .build();
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid declaration base64 to throw";
  }

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
    final SetPricingScheduleInstruction schedule =
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

    final Map<String, String> args = schedule.toArguments();
    assert "SetPricingSchedule".equals(args.get("action")) : "action mismatch";
    assert "xor".equals(args.get("schedule.currency_code")) : "currency mismatch";
    assert "hot".equals(args.get("schedule.tiers.0.storage_class")) : "tier mismatch";
    assert schedule.tiers().size() == 2 : "tier count mismatch";
    assert "Test schedule".equals(schedule.notes()) : "notes mismatch";
  }

  private static void testUpsertProviderCreditBuilder() {
    final UpsertProviderCreditInstruction instruction =
        UpsertProviderCreditInstruction.builder()
            .setProviderIdHex(PROVIDER_ID)
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
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "UpsertProviderCredit".equals(args.get("action")) : "action mismatch";
    assert "123456789000".equals(args.get("record.available_credit_nano"))
        : "available credit mismatch";
    assert instruction.underDeliveryStrikes().equals(2) : "strikes mismatch";
    assert "jp".equals(instruction.metadata().get("region")) : "metadata mismatch";
  }

  private static byte[] randomBytes(final int length) {
    final byte[] bytes = new byte[length];
    ThreadLocalRandom.current().nextBytes(bytes);
    return bytes;
  }
}
