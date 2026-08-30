package org.hyperledger.iroha.android.governance;

import java.math.BigInteger;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.instructions.CastPlainBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CastZkBallotInstruction;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;

/** Regression tests covering the governance instruction builders. */
public final class GovernanceInstructionBuilderTests {

  private GovernanceInstructionBuilderTests() {}

  public static void main(final String[] args) {
    castZkBallotRoundTrip();
    castZkBallotRejectsUnsupportedPublicInputs();
    castZkBallotNormalizesPublicInputs();
    castZkBallotFromArgumentsNormalizesPublicInputs();
    castZkBallotRejectsIncompleteLockHints();
    castZkBallotRejectsNonObjectPublicInputs();
    castZkBallotRejectsInvalidHexHints();
    castPlainBallotRoundTrip();
    castPlainBallotRejectsNoncanonicalQuantities();
    System.out.println("[IrohaAndroid] GovernanceInstructionBuilderTests passed.");
  }

  private static void castZkBallotRoundTrip() {
    final CastZkBallotInstruction instruction =
        CastZkBallotInstruction.builder()
            .setElectionId("election-1")
            .setProofBase64("AQID")
            .setPublicInputsJson("{\"foo\":1}")
            .build();
    assert "election-1".equals(instruction.electionId()) : "election id mismatch";
    assert "AQID".equals(instruction.proofBase64()) : "proof mismatch";
  }

  private static void castZkBallotRejectsUnsupportedPublicInputs() {
    final String rootHint = "0x" + "Aa".repeat(32);
    final String nullifier = "blake2b32:" + "BB".repeat(32);
    boolean failed = false;
    try {
      CastZkBallotInstruction.builder()
          .setElectionId("election-2")
          .setProofBase64("AQID")
          .setPublicInputsJson(
              "{\"durationBlocks\":64,\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\",\"amount\":\"100\","
                  + "\"rootHintHex\":\""
                  + rootHint
                  + "\",\"nullifierHex\":\""
                  + nullifier
                  + "\"}")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("durationBlocks");
    }
    assert failed : "expected unsupported alias rejection";
  }

  private static void castZkBallotNormalizesPublicInputs() {
    final String rootHint = "0x" + "Cc".repeat(32);
    final String nullifier = "blake2b32:" + "DD".repeat(32);
    final CastZkBallotInstruction instruction =
        CastZkBallotInstruction.builder()
            .setElectionId("election-2b")
            .setProofBase64("AQID")
            .setPublicInputsJson(
                "{\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\",\"amount\":\"100\",\"duration_blocks\":64,"
                    + "\"root_hint\":\""
                    + rootHint
                    + "\",\"nullifier\":\""
                    + nullifier
                    + "\"}")
            .build();
    final String normalized = instruction.publicInputsJson();
    assert normalized.contains("\"root_hint\"") : "root_hint should be preserved";
    assert normalized.contains("\"root_hint\":\"" + "cc".repeat(32) + "\"")
        : "root_hint should be canonicalized";
    assert normalized.contains("\"nullifier\"") : "nullifier should be preserved";
    assert normalized.contains("\"nullifier\":\"" + "dd".repeat(32) + "\"")
        : "nullifier should be canonicalized";
  }

  private static void castZkBallotFromArgumentsNormalizesPublicInputs() {
    final Map<String, String> args = new java.util.LinkedHashMap<>();
    args.put("action", "CastZkBallot");
    args.put("election_id", "election-args");
    args.put("proof_b64", "AQID");
    args.put(
        "public_inputs_json",
        "{\"duration_blocks\":12,\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\",\"amount\":\"100\","
            + "\"root_hint\":\"0x"
            + "Aa".repeat(32)
            + "\"}");
    final CastZkBallotInstruction instruction = CastZkBallotInstruction.fromArguments(args);
    final String normalized = instruction.toArguments().get("public_inputs_json");
    assert normalized != null && normalized.contains("\"duration_blocks\"")
        : "fromArguments should retain public inputs";
    assert normalized.contains("\"root_hint\":\"" + "aa".repeat(32) + "\"")
        : "fromArguments should canonicalize hex hints";
  }

  private static void castZkBallotRejectsIncompleteLockHints() {
    boolean failed = false;
    try {
      CastZkBallotInstruction.builder()
          .setElectionId("election-3")
          .setProofBase64("AQID")
          .setPublicInputsJson("{\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"}")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("lock hints");
    }
    assert failed : "expected lock hint validation failure";
  }

  private static void castZkBallotRejectsNonObjectPublicInputs() {
    boolean failed = false;
    try {
      CastZkBallotInstruction.builder()
          .setElectionId("election-4")
          .setProofBase64("AQID")
          .setPublicInputsJson("[1,2,3]")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("JSON object");
    }
    assert failed : "expected non-object public inputs to be rejected";
  }

  private static void castZkBallotRejectsInvalidHexHints() {
    boolean failed = false;
    try {
      CastZkBallotInstruction.builder()
          .setElectionId("election-5")
          .setProofBase64("AQID")
          .setPublicInputsJson(
              "{\"owner\":\"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\",\"amount\":\"100\",\"duration_blocks\":5,"
                  + "\"root_hint\":\"not-hex\"}")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("root_hint");
    }
    assert failed : "expected invalid hex hints to be rejected";
  }

  private static void castPlainBallotRoundTrip() {
    final String ownerAccountId = sampleI105(0x11);
    final CastPlainBallotInstruction instruction =
        CastPlainBallotInstruction.builder()
            .setReferendumId("ref-42")
            .setOwnerAccountId(ownerAccountId)
            .setAmount("18446744073709551616.25")
            .setDurationBlocks(512)
            .setDirection(1)
            .build();
    assert "ref-42".equals(instruction.referendumId()) : "referendum id mismatch";
    assert ownerAccountId.equals(instruction.ownerAccountId()) : "owner mismatch";
    assert "18446744073709551616.25".equals(instruction.amount()) : "amount mismatch";
    assert instruction.direction() == 1 : "direction mismatch";

    final CastPlainBallotInstruction typedQuantity =
        CastPlainBallotInstruction.builder()
            .setReferendumId("ref-typed")
            .setOwnerAccountId(ownerAccountId)
            .setAmount(
                org.hyperledger.iroha.android.numeric.NumericV1.QuantityValue.parseCanonical(
                    "1.25"))
            .setDurationBlocks(1)
            .setDirection(0)
            .build();
    assert "1.25".equals(typedQuantity.amount()) : "typed Quantity mismatch";

    final CastPlainBallotInstruction integerConvenience =
        CastPlainBallotInstruction.builder()
            .setReferendumId("ref-integer")
            .setOwnerAccountId(ownerAccountId)
            .setAmount(new BigInteger("125000"))
            .setDurationBlocks(1)
            .setDirection(0)
            .build();
    assert "125000".equals(integerConvenience.amount()) : "integer convenience mismatch";
  }

  private static void castPlainBallotRejectsNoncanonicalQuantities() {
    final String ownerAccountId = sampleI105(0x12);
    for (final String amount :
        List.of("", " ", "\t1", "1 ", "+1", "01", "1.", ".5", "1e0", "-1", "-0", "1.0",
            "1.2300", "0.0")) {
      boolean builderFailed = false;
      try {
        CastPlainBallotInstruction.builder()
            .setReferendumId("ref-invalid")
            .setOwnerAccountId(ownerAccountId)
            .setAmount(amount);
      } catch (final IllegalArgumentException expected) {
        builderFailed = true;
      }
      assert builderFailed : "builder accepted noncanonical Quantity `" + amount + "`";

      final Map<String, String> arguments = new java.util.LinkedHashMap<>();
      arguments.put("action", "CastPlainBallot");
      arguments.put("referendum_id", "ref-invalid");
      arguments.put("owner", ownerAccountId);
      arguments.put("amount", amount);
      arguments.put("duration_blocks", "10");
      arguments.put("direction", "1");
      boolean readbackFailed = false;
      try {
        CastPlainBallotInstruction.fromArguments(arguments);
      } catch (final IllegalArgumentException expected) {
        readbackFailed = true;
      }
      assert readbackFailed : "readback accepted noncanonical Quantity `" + amount + "`";
    }
  }

  private static String sampleI105(final int fill) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }
}
