package org.hyperledger.iroha.android.sorafs;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Map;
import org.hyperledger.iroha.android.model.instructions.ApprovePinManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.BindManifestAliasInstruction;
import org.hyperledger.iroha.android.model.instructions.RetirePinManifestInstruction;

/**
 * Regression tests for the SoraFS manifest lifecycle instruction builders.
 */
public final class SorafsManifestInstructionBuilderTests {

  private SorafsManifestInstructionBuilderTests() {}

  public static void main(final String[] args) {
    approvePinManifestRoundTrip();
    approvePinManifestRejectsInvalidEnvelope();
    approvePinManifestRejectsRetiredEpochField();
    retirePinManifestRoundTrip();
    retirePinManifestRejectsRetiredEpochField();
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
            .setCouncilEnvelopeBase64(envelopeBase64)
            .setCouncilEnvelopeDigestHex(envelopeDigest)
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "ApprovePinManifest".equals(args.get("action")) : "action mismatch";
    assert digest.equals(args.get("digest_hex")) : "digest mismatch";
    assert envelopeBase64.equals(args.get("council_envelope_base64"))
        : "envelope mismatch";
    assert envelopeDigest.equals(args.get("council_envelope_digest_hex"))
        : "envelope digest mismatch";
    assert !args.containsKey("approved_epoch") : "caller-supplied approval epoch resurfaced";
    assert envelopeBase64.equals(instruction.councilEnvelopeBase64()) : "payload envelope mismatch";
    assert instruction.equals(ApprovePinManifestInstruction.fromArguments(args))
        : "approval arguments must round-trip";
  }

  private static void approvePinManifestRejectsInvalidEnvelope() {
    boolean threw = false;
    try {
      ApprovePinManifestInstruction.builder()
          .setDigestHex("a0".repeat(32))
          .setCouncilEnvelopeBase64("not!base64");
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected invalid council envelope base64 to throw";
  }

  private static void approvePinManifestRejectsRetiredEpochField() {
    final Map<String, String> arguments =
        new java.util.LinkedHashMap<>(
            ApprovePinManifestInstruction.builder()
                .setDigestHex("a0".repeat(32))
                .build()
                .toArguments());
    arguments.put("approved_epoch", "42");
    boolean threw = false;
    try {
      ApprovePinManifestInstruction.fromArguments(arguments);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected caller-supplied approved epoch to throw";
  }

  private static void retirePinManifestRoundTrip() {
    final RetirePinManifestInstruction instruction =
        RetirePinManifestInstruction.builder()
            .setDigestHex("c0".repeat(32))
            .setReason("governance-retired")
            .build();
    final Map<String, String> args = instruction.toArguments();
    assert "governance-retired".equals(args.get("reason")) : "reason mismatch";
    assert !args.containsKey("retired_epoch") : "caller-supplied retirement epoch resurfaced";
    assert instruction.equals(RetirePinManifestInstruction.fromArguments(args))
        : "retirement arguments must round-trip";
  }

  private static void retirePinManifestRejectsRetiredEpochField() {
    final Map<String, String> arguments =
        new java.util.LinkedHashMap<>(
            RetirePinManifestInstruction.builder()
                .setDigestHex("c0".repeat(32))
                .build()
                .toArguments());
    arguments.put("retired_epoch", "99");
    boolean threw = false;
    try {
      RetirePinManifestInstruction.fromArguments(arguments);
    } catch (final IllegalArgumentException ex) {
      threw = true;
    }
    assert threw : "Expected caller-supplied retired epoch to throw";
  }

  private static void bindManifestAliasRoundTrip() {
    final BindManifestAliasInstruction instruction =
        BindManifestAliasInstruction.builder()
            .setDigestHex("d0".repeat(32))
            .setAliasBinding("docs", "sora", "f0".repeat(32))
            .setBoundEpoch(12)
            .setExpiryEpoch(36)
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "docs".equals(args.get("alias.name")) : "alias name mismatch";
    assert "sora".equals(args.get("alias.namespace")) : "alias namespace mismatch";
    assert "36".equals(args.get("expiry_epoch")) : "expiry epoch mismatch";
    assert instruction.boundEpoch() == 12 : "bound epoch mismatch";
    assert "docs".equals(instruction.aliasBinding().name()) : "alias binding mismatch";
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
