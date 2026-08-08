package org.hyperledger.iroha.android.model.instructions;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.junit.Test;

/** Regression tests for canonical first-release governance selectors. */
public final class GovernanceSelectorV1Tests {

  @Test
  public void governanceBallotSelectorsAcceptExactV1Boundaries() {
    final String ownerAccountId = sampleI105(0x31);
    final List<String> selectors =
        Arrays.asList("a", "A9_selector~with.dots", repeat('a', 128));

    for (final String selector : selectors) {
      assertEquals(selector, plainBallot(selector, ownerAccountId).referendumId());
      assertEquals(selector, zkBallot(selector).electionId());
    }
  }

  @Test
  public void governanceBallotSelectorsRejectNoncanonicalV1Values() {
    final List<String> selectors =
        Arrays.asList(
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a%2Fb",
            "has space",
            "投票",
            repeat('a', 129));

    for (final String selector : selectors) {
      expectIllegalArgument(
          () -> CastPlainBallotInstruction.builder().setReferendumId(selector));
      expectIllegalArgument(() -> CastZkBallotInstruction.builder().setElectionId(selector));
    }
  }

  @Test
  public void governanceSelectorArgumentFactoriesShareTheStrictValidator() {
    final String invalid = ".hidden";
    final Map<String, String> plainArguments = new LinkedHashMap<>();
    plainArguments.put("action", "CastPlainBallot");
    plainArguments.put("referendum_id", invalid);
    plainArguments.put("owner", sampleI105(0x32));
    plainArguments.put("amount", "1");
    plainArguments.put("duration_blocks", "1");
    plainArguments.put("direction", "0");
    expectIllegalArgument(() -> CastPlainBallotInstruction.fromArguments(plainArguments));

    final Map<String, String> zkArguments = new LinkedHashMap<>();
    zkArguments.put("action", "CastZkBallot");
    zkArguments.put("election_id", invalid);
    zkArguments.put("proof_b64", "AA==");
    zkArguments.put("public_inputs_json", "{}");
    expectIllegalArgument(() -> CastZkBallotInstruction.fromArguments(zkArguments));
  }

  @Test
  public void referendumFinalizationRequiresOneExactLowercaseProposalDigest() {
    final String proposalId = "ab".repeat(32);
    final FinalizeReferendumInstruction finalization =
        FinalizeReferendumInstruction.builder()
            .setReferendumId(proposalId)
            .setProposalIdHex(proposalId)
            .build();

    assertEquals(proposalId, finalization.referendumId());
    assertEquals(proposalId, finalization.proposalIdHex());
    assertEquals(proposalId, finalization.toArguments().get("referendum_id"));
    assertEquals(proposalId, finalization.toArguments().get("proposal_id_hex"));

    expectIllegalArgument(
        () -> FinalizeReferendumInstruction.builder().setReferendumId("ref-1"));
    expectIllegalArgument(
        () -> FinalizeReferendumInstruction.builder().setReferendumId("AB".repeat(32)));
    expectIllegalArgument(
        () -> FinalizeReferendumInstruction.builder().setProposalIdHex("AB".repeat(32)));
    expectIllegalArgument(
        () -> FinalizeReferendumInstruction.builder().setProposalIdHex("0x" + proposalId));
    expectIllegalArgument(
        () ->
            FinalizeReferendumInstruction.builder()
                .setReferendumId(proposalId)
                .setProposalIdHex("cd".repeat(32))
                .build());

    final Map<String, String> mismatch = new LinkedHashMap<>();
    mismatch.put("action", "FinalizeReferendum");
    mismatch.put("referendum_id", proposalId);
    mismatch.put("proposal_id_hex", "cd".repeat(32));
    expectIllegalArgument(() -> FinalizeReferendumInstruction.fromArguments(mismatch));
  }

  private static CastPlainBallotInstruction plainBallot(
      final String selector, final String ownerAccountId) {
    return CastPlainBallotInstruction.builder()
        .setReferendumId(selector)
        .setOwnerAccountId(ownerAccountId)
        .setAmount("1")
        .setDurationBlocks(1)
        .setDirection(0)
        .build();
  }

  private static CastZkBallotInstruction zkBallot(final String selector) {
    return CastZkBallotInstruction.builder()
        .setElectionId(selector)
        .setProofBase64("AA==")
        .setPublicInputsJson("{}")
        .build();
  }

  private static String sampleI105(final int fill) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception error) {
      throw new IllegalStateException("failed to build canonical account fixture", error);
    }
  }

  private static String repeat(final char character, final int count) {
    final char[] characters = new char[count];
    Arrays.fill(characters, character);
    return new String(characters);
  }

  private static void expectIllegalArgument(final Runnable operation) {
    try {
      operation.run();
      fail("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }
}
