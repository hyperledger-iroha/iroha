package org.hyperledger.iroha.android.governance;

import java.math.BigInteger;
import java.util.List;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.CastPlainBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.CastZkBallotInstruction;
import org.hyperledger.iroha.android.model.instructions.EnactReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.FinalizeReferendumInstruction;
import org.hyperledger.iroha.android.model.instructions.GovernanceInstructionUtils;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
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
    final InstructionBox box = InstructionBuilders.proposeDeployContract(instruction);
    assert "apps".equals(box.arguments().get("namespace")) : "namespace mismatch";
    assert "Plain".equals(box.arguments().get("mode")) : "mode mismatch";

    final InstructionBox decoded =
        InstructionBox.fromNorito(InstructionKind.CUSTOM, box.arguments());
    assert decoded.payload() instanceof ProposeDeployContractInstruction : "payload type mismatch";
    final ProposeDeployContractInstruction payload =
        (ProposeDeployContractInstruction) decoded.payload();
    assert "demo.contract".equals(payload.contractId()) : "contract id mismatch";
    assert payload.window() != null : "window missing";
    assert payload.window().lower() == 10 : "window lower mismatch";
    assert payload.votingMode() == GovernanceInstructionUtils.VotingMode.PLAIN : "mode mismatch";
  }

  private static void castZkBallotRoundTrip() {
    final CastZkBallotInstruction instruction =
        CastZkBallotInstruction.builder()
            .setElectionId("election-1")
            .setProofBase64("AQID")
            .setPublicInputsJson("{\"foo\":1}")
            .build();
    final InstructionBox decoded = decode(instruction);
    final CastZkBallotInstruction payload = (CastZkBallotInstruction) decoded.payload();
    assert "election-1".equals(payload.electionId()) : "election id mismatch";
    assert "AQID".equals(payload.proofBase64()) : "proof mismatch";
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
              "{\"durationBlocks\":64,\"owner\":\"alice@wonderland\",\"amount\":\"100\","
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
                "{\"owner\":\"alice@wonderland\",\"amount\":\"100\",\"duration_blocks\":64,"
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
        "{\"duration_blocks\":12,\"owner\":\"alice@wonderland\",\"amount\":\"100\","
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
          .setPublicInputsJson("{\"owner\":\"alice@wonderland\"}")
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
              "{\"owner\":\"alice@wonderland\",\"amount\":\"100\",\"duration_blocks\":5,"
                  + "\"root_hint\":\"not-hex\"}")
          .build();
    } catch (final IllegalArgumentException ex) {
      failed = ex.getMessage().contains("root_hint");
    }
    assert failed : "expected invalid hex hints to be rejected";
  }

  private static void castPlainBallotRoundTrip() {
    final CastPlainBallotInstruction instruction =
        CastPlainBallotInstruction.builder()
            .setReferendumId("ref-42")
            .setOwnerAccountId("alice@sora")
            .setAmount(new BigInteger("125000"))
            .setDurationBlocks(512)
            .setDirection(1)
            .build();
    final InstructionBox decoded = decode(instruction);
    final CastPlainBallotInstruction payload = (CastPlainBallotInstruction) decoded.payload();
    assert "ref-42".equals(payload.referendumId()) : "referendum id mismatch";
    assert "125000".equals(payload.amount()) : "amount mismatch";
    assert payload.direction() == 1 : "direction mismatch";
  }

  private static void enactReferendumRoundTrip() {
    final EnactReferendumInstruction instruction =
        EnactReferendumInstruction.builder()
            .setReferendumIdHex("c0".repeat(32))
            .setPreimageHashHex("d1".repeat(32))
            .setWindow(new GovernanceInstructionUtils.AtWindow(5, 15))
            .build();
    final InstructionBox decoded = decode(instruction);
    final EnactReferendumInstruction payload = (EnactReferendumInstruction) decoded.payload();
    assert "c0".repeat(32).equals(payload.referendumIdHex()) : "referendum id hex mismatch";
    assert payload.window().upper() == 15 : "enact window mismatch";
  }

  private static void finalizeReferendumRoundTrip() {
    final FinalizeReferendumInstruction instruction =
        FinalizeReferendumInstruction.builder()
            .setReferendumId("ref-final")
            .setProposalIdHex("e1".repeat(32))
            .build();
    final InstructionBox decoded = decode(instruction);
    final FinalizeReferendumInstruction payload =
        (FinalizeReferendumInstruction) decoded.payload();
    assert "ref-final".equals(payload.referendumId()) : "referendum id mismatch";
    assert "e1".repeat(32).equals(payload.proposalIdHex()) : "proposal id mismatch";
  }

  private static void persistCouncilRoundTrip() {
    final PersistCouncilForEpochInstruction instruction =
        PersistCouncilForEpochInstruction.builder()
            .setEpoch(99)
            .addMember("alice@sora")
            .addMember("bob@sora")
            .addAlternate("carol@sora")
            .setCandidatesCount(5)
            .setVerified(2)
            .setDerivedBy(GovernanceInstructionUtils.CouncilDerivationKind.VRF)
            .build();
    final InstructionBox decoded = decode(instruction);
    final PersistCouncilForEpochInstruction payload =
        (PersistCouncilForEpochInstruction) decoded.payload();
    assert payload.members().equals(List.of("alice@sora", "bob@sora")) : "members mismatch";
    assert payload.alternates().equals(List.of("carol@sora")) : "alternates mismatch";
    assert payload.candidatesCount() == 5 : "candidates mismatch";
    assert payload.verified() == 2 : "verified mismatch";
    assert payload.derivedBy() == GovernanceInstructionUtils.CouncilDerivationKind.VRF
        : "derived_by mismatch";
  }

  private static InstructionBox decode(final InstructionBox.InstructionPayload payload) {
    return InstructionBox.fromNorito(InstructionKind.CUSTOM, payload.toArguments());
  }
}
