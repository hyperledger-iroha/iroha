package org.hyperledger.iroha.android.governance;

import java.math.BigInteger;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.instructions.CastPlainBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CastZkBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.EnactReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.FinalizeReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.GovernanceInstructionUtils;
import org.hyperledger.iroha.android.model.instructions.PersistCouncilForEpochInstruction;
import org.hyperledger.iroha.android.model.instructions.ProposeDeployContractInstruction;

/** Regression tests covering the governance instruction builders. */
public final class GovernanceInstructionBuilderTests {

  private GovernanceInstructionBuilderTests() {}

  public static void main(final String[] args) {
    proposeDeployContractRoundTrip();
    castZkBallotRoundTrip();
    castZkBallotRejectsDeprecatedPublicInputs();
    castZkBallotNormalizesPublicInputs();
    castZkBallotFromArgumentsNormalizesPublicInputs();
    castZkBallotRejectsIncompleteLockHints();
    castZkBallotRejectsNonObjectPublicInputs();
    castZkBallotRejectsInvalidHexHints();
    castPlainBallotRoundTrip();
    enactReferendumRoundTrip();
    finalizeReferendumRoundTrip();
    persistCouncilRoundTrip();
    System.out.println("[IrohaAndroid] GovernanceInstructionBuilderTests passed.");
  }

  private static void proposeDeployContractRoundTrip() {
    final ProposeDeployContractInstruction instruction =
        ProposeDeployContractInstruction.builder()
            .setNamespace("apps")
            .setContractId("demo.contract")
            .setCodeHashHex("a0".repeat(32))
            .setAbiHashHex("b1".repeat(32))
            .setAbiVersion("1")
            .setWindow(new GovernanceInstructionUtils.AtWindow(10, 20))
            .setVotingMode(GovernanceInstructionUtils.VotingMode.PLAIN)
            .build();
    final Map<String, String> args = instruction.toArguments();
    assert "apps".equals(args.get("namespace")) : "namespace mismatch";
    assert "Plain".equals(args.get("mode")) : "mode mismatch";
    assert "demo.contract".equals(instruction.contractId()) : "contract id mismatch";
    assert instruction.window() != null : "window missing";
    assert instruction.window().lower() == 10 : "window lower mismatch";
    assert instruction.votingMode() == GovernanceInstructionUtils.VotingMode.PLAIN : "mode mismatch";
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

  private static void castZkBallotRejectsDeprecatedPublicInputs() {
    final String rootHint = "0x" + "Aa".repeat(32);
    final String nullifier = "blake2b32:" + "BB".repeat(32);
    boolean failed = false;
    try {
      CastZkBallotInstruction.builder()
          .setElectionId("election-2")
          .setProofBase64("AQID")
          .setPublicInputsJson(
              "{\"durationBlocks\":64,\"owner\":\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\",\"amount\":\"100\","
                  + "\"rootHintHex\":\""
                  + rootHint
                  + "\",\"nullifierHex\":\""
                  + nullifier
                  + "\"}")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("durationBlocks");
    }
    assert failed : "expected deprecated alias rejection";
  }

  private static void castZkBallotNormalizesPublicInputs() {
    final String rootHint = "0x" + "Cc".repeat(32);
    final String nullifier = "blake2b32:" + "DD".repeat(32);
    final CastZkBallotInstruction instruction =
        CastZkBallotInstruction.builder()
            .setElectionId("election-2b")
            .setProofBase64("AQID")
            .setPublicInputsJson(
                "{\"owner\":\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\",\"amount\":\"100\",\"duration_blocks\":64,"
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
        "{\"duration_blocks\":12,\"owner\":\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\",\"amount\":\"100\","
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
          .setPublicInputsJson("{\"owner\":\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\"}")
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
              "{\"owner\":\"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB\",\"amount\":\"100\",\"duration_blocks\":5,"
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
            .setAmount(new BigInteger("125000"))
            .setDurationBlocks(512)
            .setDirection(1)
            .build();
    assert "ref-42".equals(instruction.referendumId()) : "referendum id mismatch";
    assert ownerAccountId.equals(instruction.ownerAccountId()) : "owner mismatch";
    assert "125000".equals(instruction.amount()) : "amount mismatch";
    assert instruction.direction() == 1 : "direction mismatch";
  }

  private static void enactReferendumRoundTrip() {
    final EnactReferendumInstruction instruction =
        EnactReferendumInstruction.builder()
            .setReferendumIdHex("c0".repeat(32))
            .setPreimageHashHex("d1".repeat(32))
            .setWindow(new GovernanceInstructionUtils.AtWindow(5, 15))
            .build();
    assert "c0".repeat(32).equals(instruction.referendumIdHex()) : "referendum id hex mismatch";
    assert instruction.window().upper() == 15 : "enact window mismatch";
  }

  private static void finalizeReferendumRoundTrip() {
    final FinalizeReferendumInstruction instruction =
        FinalizeReferendumInstruction.builder()
            .setReferendumId("ref-final")
            .setProposalIdHex("e1".repeat(32))
            .build();
    assert "ref-final".equals(instruction.referendumId()) : "referendum id mismatch";
    assert "e1".repeat(32).equals(instruction.proposalIdHex()) : "proposal id mismatch";
  }

  private static void persistCouncilRoundTrip() {
    final String memberA = sampleI105(0x21);
    final String memberB = sampleI105(0x22);
    final String alternate = sampleI105(0x23);
    final PersistCouncilForEpochInstruction instruction =
        PersistCouncilForEpochInstruction.builder()
            .setEpoch(99)
            .addMember(memberA)
            .addMember(memberB)
            .addAlternate(alternate)
            .setCandidatesCount(5)
            .setVerified(2)
            .setDerivedBy(GovernanceInstructionUtils.CouncilDerivationKind.VRF)
            .build();
    assert instruction.members().equals(List.of(memberA, memberB)) : "members mismatch";
    assert instruction.alternates().equals(List.of(alternate)) : "alternates mismatch";
    assert instruction.candidatesCount() == 5 : "candidates mismatch";
    assert instruction.verified() == 2 : "verified mismatch";
    assert instruction.derivedBy() == GovernanceInstructionUtils.CouncilDerivationKind.VRF
        : "derived_by mismatch";
  }

  private static String sampleI105(final int fill) {
    try {
      final byte[] publicKey = new byte[32];
      Arrays.fill(publicKey, (byte) fill);
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }
}
