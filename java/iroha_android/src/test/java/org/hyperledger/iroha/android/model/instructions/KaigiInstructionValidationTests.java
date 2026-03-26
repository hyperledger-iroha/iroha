package org.hyperledger.iroha.android.model.instructions;

import java.util.HashMap;
import java.util.Map;

public final class KaigiInstructionValidationTests {

  private KaigiInstructionValidationTests() {}

  public static void main(final String[] args) {
    rejectsInvalidRelayHpkeKey();
    rejectsInvalidRelayManifestHpkeKey();
    rejectsInvalidRelayManifestParse();
    rejectsInvalidCreateRelayManifestHpkeKey();
    rejectsInvalidJoinProofBase64();
    rejectsInvalidLeaveProofBase64();
    rejectsInvalidUsageProofBase64();
    System.out.println("[IrohaAndroid] KaigiInstructionValidationTests passed.");
  }

  private static void rejectsInvalidRelayHpkeKey() {
    assertThrows(
        () ->
            RegisterKaigiRelayInstruction.builder()
                .setRelayId("relay-alpha@wonderland")
                .setHpkePublicKeyBase64("not!base64")
                .setBandwidthClass(1)
                .build(),
        "expected invalid hpke public key base64 to throw");
  }

  private static void rejectsInvalidRelayManifestHpkeKey() {
    assertThrows(
        () ->
            SetKaigiRelayManifestInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .addRelayManifestHop("relay-alpha@wonderland", "not!base64", 7)
                .build(),
        "expected invalid relay manifest hpke key to throw");
  }

  private static void rejectsInvalidRelayManifestParse() {
    final Map<String, String> args = new HashMap<>();
    args.put("action", "SetKaigiRelayManifest");
    args.put("call.domain_id", "wonderland");
    args.put("call.call_name", "weekly-sync");
    args.put("relay_manifest.hop.0.relay_id", "relay-alpha@wonderland");
    args.put("relay_manifest.hop.0.hpke_public_key", "not!base64");
    args.put("relay_manifest.hop.0.weight", "1");

    assertThrows(
        () -> SetKaigiRelayManifestInstruction.fromArguments(args),
        "expected invalid relay manifest parse to throw");
  }

  private static void rejectsInvalidCreateRelayManifestHpkeKey() {
    assertThrows(
        () ->
            CreateKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setHost("host@wonderland")
                .addRelayManifestHop("relay-alpha@wonderland", "not!base64", 7)
                .build(),
        "expected invalid create relay manifest hpke key to throw");
  }

  private static void rejectsInvalidJoinProofBase64() {
    assertThrows(
        () ->
            JoinKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
                .setProofBase64("not!base64")
                .build(),
        "expected invalid join proof base64 to throw");
  }

  private static void rejectsInvalidLeaveProofBase64() {
    assertThrows(
        () ->
            LeaveKaigiInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setParticipant("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
                .setProofBase64("not!base64")
                .build(),
        "expected invalid leave proof base64 to throw");
  }

  private static void rejectsInvalidUsageProofBase64() {
    assertThrows(
        () ->
            RecordKaigiUsageInstruction.builder()
                .setCallId("wonderland", "weekly-sync")
                .setDurationMs(1)
                .setBilledGas(0)
                .setProofBase64("not!base64")
                .build(),
        "expected invalid usage proof base64 to throw");
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
