package org.hyperledger.iroha.android.offline;

import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction.OfflineAllowance;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction.OfflineWalletCertificate;
import org.hyperledger.iroha.android.model.instructions.RegisterOfflineAllowanceInstruction.OfflineWalletPolicy;

/** Ensures `RegisterOfflineAllowance` builders stay aligned with Norito arguments. */
public final class OfflineAllowanceInstructionBuilderTests {

  private OfflineAllowanceInstructionBuilderTests() {}

  public static void main(final String[] args) {
    rejectsInvalidAttestationReportBase64();
    final OfflineAllowance allowance =
        OfflineAllowance.builder()
            .setAssetId("xor#sora#ed0120EXAMPLE@sora")
            .setAmount("250.00")
            .setCommitmentHex("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            .build();

    final OfflineWalletPolicy policy =
        OfflineWalletPolicy.builder()
            .setMaxBalance("500")
            .setMaxTxValue("125")
            .setExpiresAtMs(1745900000000L)
            .build();

    final OfflineWalletCertificate certificate =
        OfflineWalletCertificate.builder()
            .setController("ed0120ABCDEF@wonderland")
            .setAllowance(allowance)
            .setSpendPublicKey("ed0120ABCDEF")
            .setAttestationReportBase64("AAECAw==")
            .setIssuedAtMs(1730314876000L)
            .setExpiresAtMs(1745900000000L)
            .setPolicy(policy)
            .setOperatorSignatureHex("BEEFED")
            .putMetadata("ios.app_attest.team_id", "ABCD1234")
            .putMetadata("ios.app_attest.bundle_id", "tech.iroha.retail")
            .setVerdictIdHex("deadbeef")
            .setAttestationNonceHex("cafebabe")
            .setRefreshAtMs(1730914876000L)
            .build();

    final RegisterOfflineAllowanceInstruction instruction =
        RegisterOfflineAllowanceInstruction.builder().setCertificate(certificate).build();

    final InstructionBox box = InstructionBuilders.registerOfflineAllowance(instruction);
    final Map<String, String> argsMap = box.arguments();

    assert "RegisterOfflineAllowance".equals(argsMap.get("action"))
        : "action mismatch for offline allowance instruction";
    assert certificate.controller().equals(argsMap.get("certificate.controller"))
        : "controller mismatch";
    assert certificate.metadata().get("ios.app_attest.team_id")
        .equals(argsMap.get("certificate.metadata.ios.app_attest.team_id"))
        : "metadata mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof RegisterOfflineAllowanceInstruction
        : "Decoded payload type mismatch";
    final RegisterOfflineAllowanceInstruction decodedInstruction =
        (RegisterOfflineAllowanceInstruction) decoded.payload();
    assert certificate.equals(decodedInstruction.certificate())
        : "Decoded certificate mismatch";

    System.out.println("[IrohaAndroid] OfflineAllowanceInstructionBuilderTests passed.");
  }

  private static void rejectsInvalidAttestationReportBase64() {
    boolean threw = false;
    try {
      OfflineWalletCertificate.builder()
          .setController("ed0120ABCDEF@wonderland")
          .setAllowance(
              OfflineAllowance.builder()
                  .setAssetId("xor#sora#ed0120EXAMPLE@sora")
                  .setAmount("250.00")
                  .setCommitmentHex("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
                  .build())
          .setSpendPublicKey("ed0120ABCDEF")
          .setAttestationReportBase64("not!base64");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid attestation report base64 to throw";
  }
}
