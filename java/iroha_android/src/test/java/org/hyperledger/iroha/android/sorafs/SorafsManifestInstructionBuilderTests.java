package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.ApprovePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.BindManifestAliasInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.RetirePinManifestInstruction;

/**
 * Regression tests for the SoraFS manifest lifecycle instruction builders.
 */
public final class SorafsManifestInstructionBuilderTests {

  private SorafsManifestInstructionBuilderTests() {}

  public static void main(final String[] args) {
    approvePinManifestRoundTrip();
    approvePinManifestRejectsInvalidEnvelope();
    approvePinManifestRejectsNegativeEpoch();
    retirePinManifestRoundTrip();
    retirePinManifestRejectsNegativeEpoch();
    bindManifestAliasRoundTrip();
    bindManifestAliasRejectsNegativeEpoch();
    System.out.println("[IrohaAndroid] SorafsManifestInstructionBuilderTests passed.");
  }

  private static void approvePinManifestRoundTrip() {
    final String digest = "a0".repeat(32);
    final String envelopeDigest = "b1".repeat(32);
    final String envelopeBase64 =
        Base64.getEncoder().encodeToString("council-envelope".getBytes(StandardCharsets.UTF_8));

    final ApprovePinManifestInstruction instruction =
        ApprovePinManifestInstruction.builder()
            .setDigestHex(digest)
            .setApprovedEpoch(42)
            .setCouncilEnvelopeBase64(envelopeBase64)
            .setCouncilEnvelopeDigestHex(envelopeDigest)
            .build();

    final InstructionBox box = InstructionBuilders.approvePinManifest(instruction);
    assert "ApprovePinManifest".equals(box.arguments().get("action")) : "action mismatch";
    assert digest.equals(box.arguments().get("digest_hex")) : "digest mismatch";
    assert envelopeBase64.equals(box.arguments().get("council_envelope_base64"))
        : "envelope mismatch";
    assert envelopeDigest.equals(box.arguments().get("council_envelope_digest_hex"))
        : "envelope digest mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof ApprovePinManifestInstruction
        : "Expected ApprovePinManifestInstruction payload";
    final ApprovePinManifestInstruction payload =
        (ApprovePinManifestInstruction) decoded.payload();
    assert payload.approvedEpoch() == 42 : "approved epoch mismatch";
    assert envelopeBase64.equals(payload.councilEnvelopeBase64()) : "payload envelope mismatch";
  }

  private static void approvePinManifestRejectsInvalidEnvelope() {
    boolean threw = false;
    try {
      ApprovePinManifestInstruction.builder()
          .setDigestHex("a0".repeat(32))
          .setApprovedEpoch(42)
          .setCouncilEnvelopeBase64("not!base64");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid council envelope base64 to throw";
  }

  private static void approvePinManifestRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      ApprovePinManifestInstruction.builder()
          .setDigestHex("a0".repeat(32))
          .setApprovedEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative approved epoch to throw";
  }

  private static void retirePinManifestRoundTrip() {
    final RetirePinManifestInstruction instruction =
        RetirePinManifestInstruction.builder()
            .setDigestHex("c0".repeat(32))
            .setRetiredEpoch(99)
            .setReason("governance-retired")
            .build();
    final InstructionBox box = InstructionBuilders.retirePinManifest(instruction);
    assert "governance-retired".equals(box.arguments().get("reason")) : "reason mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof RetirePinManifestInstruction
        : "Expected RetirePinManifestInstruction payload";
    final RetirePinManifestInstruction payload =
        (RetirePinManifestInstruction) decoded.payload();
    assert payload.retiredEpoch() == 99 : "retired epoch mismatch";
  }

  private static void retirePinManifestRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      RetirePinManifestInstruction.builder()
          .setDigestHex("c0".repeat(32))
          .setRetiredEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative retired epoch to throw";
  }

  private static void bindManifestAliasRoundTrip() {
    final BindManifestAliasInstruction instruction =
        BindManifestAliasInstruction.builder()
            .setDigestHex("d0".repeat(32))
            .setAliasBinding("docs", "sora", "f0".repeat(32))
            .setBoundEpoch(12)
            .setExpiryEpoch(36)
            .build();

    final InstructionBox box = InstructionBuilders.bindManifestAlias(instruction);
    assert "docs".equals(box.arguments().get("alias.name")) : "alias name mismatch";
    assert "sora".equals(box.arguments().get("alias.namespace")) : "alias namespace mismatch";
    assert "36".equals(box.arguments().get("expiry_epoch")) : "expiry epoch mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof BindManifestAliasInstruction
        : "Expected BindManifestAliasInstruction payload";
    final BindManifestAliasInstruction payload =
        (BindManifestAliasInstruction) decoded.payload();
    assert payload.boundEpoch() == 12 : "bound epoch mismatch";
    assert "docs".equals(payload.aliasBinding().name()) : "alias binding mismatch";
  }

  private static void bindManifestAliasRejectsNegativeEpoch() {
    boolean threw = false;
    try {
      BindManifestAliasInstruction.builder()
          .setDigestHex("d0".repeat(32))
          .setAliasBinding("docs", "sora", "f0".repeat(32))
          .setBoundEpoch(-1)
          .setExpiryEpoch(1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative bound epoch to throw";

    threw = false;
    try {
      BindManifestAliasInstruction.builder()
          .setDigestHex("d0".repeat(32))
          .setAliasBinding("docs", "sora", "f0".repeat(32))
          .setBoundEpoch(1)
          .setExpiryEpoch(-1);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected negative expiry epoch to throw";
  }
}
