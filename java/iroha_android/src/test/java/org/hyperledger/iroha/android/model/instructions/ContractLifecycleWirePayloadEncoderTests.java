package org.hyperledger.iroha.android.model.instructions;

import java.io.IOException;
import java.math.BigInteger;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.test.FixtureGeneratorRunner;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.junit.Test;

/** Mirrored Java parity tests for revision-guarded contract lifecycle instructions. */
public final class ContractLifecycleWirePayloadEncoderTests {
  private static final String CONTRACT_ADDRESS =
      "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
  private static final String ACCOUNT_ID = TestAccountIds.ed25519Authority(0x11);

  public ContractLifecycleWirePayloadEncoderTests() {}

  /** Runs the mirrored lifecycle codec checks under the Gradle JUnit harness. */
  @Test
  public void lifecycleWirePayloadParity() throws Exception {
    main(new String[0]);
  }

  public static void main(final String[] args) throws Exception {
    allLifecycleInstructionsMatchRust();
    allLifecycleInstructionsPreserveExpectedRevisionAndOwnerVariant();
    lifecycleRevisionGuardsRejectZeroOverflowAndSchemaSubstitution();
    System.out.println("[IrohaAndroid] Contract lifecycle wire payload tests passed.");
  }

  private static void allLifecycleInstructionsMatchRust()
      throws IOException, InterruptedException {
    final List<String> fixture = FixtureGeneratorRunner.run("contract-lifecycle");
    assert fixture.size() == 10 : "unexpected Rust lifecycle fixture row count";
    final String contractAddress = fixture.get(7);
    final String accountId = fixture.get(8);
    final String codeHashHex = fixture.get(9);
    final InstructionBox[] instructions = {
      ContractLifecycleWirePayloadEncoder.encodeDeactivateContractInstance(
          contractAddress, BigInteger.valueOf(5), "planned rotation"),
      ContractLifecycleWirePayloadEncoder.encodeActivateContractInstance(
          contractAddress, BigInteger.valueOf(6), codeHashHex),
      ContractLifecycleWirePayloadEncoder.encodeSetContractParliamentDelegation(
          contractAddress, BigInteger.valueOf(7), true),
      ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToAccount(
          contractAddress, BigInteger.valueOf(8), accountId),
      ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToParliament(
          contractAddress, BigInteger.valueOf(9)),
      ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
          contractAddress, BigInteger.TEN),
      ContractLifecycleWirePayloadEncoder.encodeCancelContractOwnershipOffer(
          contractAddress, BigInteger.valueOf(11)),
    };
    for (int index = 0; index < instructions.length; index++) {
      assert Arrays.equals(hexToBytes(fixture.get(index)), wirePayload(instructions[index]))
          : "contract lifecycle payload " + index + " must match Rust";
    }
  }

  private static void allLifecycleInstructionsPreserveExpectedRevisionAndOwnerVariant() {
    final String codeHashHex =
        "abababababababababababababababababababababababababababababababab";
    final InstructionBox deactivate =
        ContractLifecycleWirePayloadEncoder.encodeDeactivateContractInstance(
            CONTRACT_ADDRESS, BigInteger.valueOf(5), "planned rotation");
    final InstructionBox activate =
        ContractLifecycleWirePayloadEncoder.encodeActivateContractInstance(
            CONTRACT_ADDRESS, BigInteger.valueOf(6), codeHashHex);
    final InstructionBox set =
        ContractLifecycleWirePayloadEncoder.encodeSetContractParliamentDelegation(
            CONTRACT_ADDRESS, BigInteger.valueOf(7), true);
    final InstructionBox offerAccount =
        ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToAccount(
            CONTRACT_ADDRESS, BigInteger.valueOf(8), ACCOUNT_ID);
    final InstructionBox offerParliament =
        ContractLifecycleWirePayloadEncoder.encodeOfferContractOwnershipToParliament(
            CONTRACT_ADDRESS, BigInteger.valueOf(9));
    final InstructionBox accept =
        ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
            CONTRACT_ADDRESS, BigInteger.TEN);
    final InstructionBox cancel =
        ContractLifecycleWirePayloadEncoder.encodeCancelContractOwnershipOffer(
            CONTRACT_ADDRESS, BigInteger.valueOf(11));

    assert ContractLifecycleWirePayloadEncoder.DEACTIVATE_INSTANCE_WIRE_NAME.equals(
        deactivate.name()) : "deactivation wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.ACTIVATE_INSTANCE_WIRE_NAME.equals(
        activate.name()) : "activation wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.SET_PARLIAMENT_DELEGATION_WIRE_NAME.equals(
        set.name()) : "delegation wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.OFFER_OWNERSHIP_WIRE_NAME.equals(
        offerAccount.name()) : "account ownership-offer wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.OFFER_OWNERSHIP_WIRE_NAME.equals(
        offerParliament.name()) : "Parliament ownership-offer wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.ACCEPT_OWNERSHIP_WIRE_NAME.equals(
        accept.name()) : "ownership-acceptance wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.CANCEL_OWNERSHIP_OFFER_WIRE_NAME.equals(
        cancel.name()) : "ownership-offer cancellation wire id mismatch";
    assert ContractLifecycleWirePayloadEncoder.WIRE_NAMES.size() == 6
        : "lifecycle wire catalog must remain closed";

    final ContractLifecycleWirePayloadEncoder.DecodedDeactivation decodedDeactivation =
        ContractLifecycleWirePayloadEncoder.decodeDeactivateContractInstance(
            wirePayload(deactivate));
    assert BigInteger.valueOf(5).equals(decodedDeactivation.expectedRevision())
        : "deactivation expected revision mismatch";
    assert "planned rotation".equals(decodedDeactivation.reason())
        : "deactivation reason mismatch";

    final ContractLifecycleWirePayloadEncoder.DecodedActivation decodedActivation =
        ContractLifecycleWirePayloadEncoder.decodeActivateContractInstance(
            wirePayload(activate));
    assert BigInteger.valueOf(6).equals(decodedActivation.expectedRevision())
        : "activation expected revision mismatch";
    assert codeHashHex.equals(decodedActivation.codeHashHex())
        : "activation code hash mismatch";

    final ContractLifecycleWirePayloadEncoder.DecodedDelegation decodedSet =
        ContractLifecycleWirePayloadEncoder.decodeSetContractParliamentDelegation(
            wirePayload(set));
    assert CONTRACT_ADDRESS.equals(decodedSet.contractAddress())
        : "delegation contract address mismatch";
    assert BigInteger.valueOf(7).equals(decodedSet.expectedRevision())
        : "delegation expected revision mismatch";
    assert decodedSet.delegated() : "delegation flag mismatch";

    final ContractLifecycleWirePayloadEncoder.DecodedOwnershipOffer decodedAccount =
        ContractLifecycleWirePayloadEncoder.decodeOfferContractOwnership(
            wirePayload(offerAccount), AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert BigInteger.valueOf(8).equals(decodedAccount.expectedRevision())
        : "account offer expected revision mismatch";
    assert ACCOUNT_ID.equals(decodedAccount.newOwnerAccountId())
        : "account owner variant mismatch";

    final ContractLifecycleWirePayloadEncoder.DecodedOwnershipOffer decodedParliament =
        ContractLifecycleWirePayloadEncoder.decodeOfferContractOwnership(
            wirePayload(offerParliament), AccountAddress.DEFAULT_I105_DISCRIMINANT);
    assert BigInteger.valueOf(9).equals(decodedParliament.expectedRevision())
        : "Parliament offer expected revision mismatch";
    assert decodedParliament.newOwnerAccountId() == null
        : "Parliament owner must remain a distinct unit variant";

    assert BigInteger.TEN.equals(
        ContractLifecycleWirePayloadEncoder.decodeAcceptContractOwnership(
                wirePayload(accept))
            .expectedRevision()) : "accept expected revision mismatch";
    assert BigInteger.valueOf(11).equals(
        ContractLifecycleWirePayloadEncoder.decodeCancelContractOwnershipOffer(
                wirePayload(cancel))
            .expectedRevision()) : "cancel expected revision mismatch";
  }

  private static void lifecycleRevisionGuardsRejectZeroOverflowAndSchemaSubstitution() {
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.encodeDeactivateContractInstance(
                CONTRACT_ADDRESS, BigInteger.ZERO, null));
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.encodeActivateContractInstance(
                CONTRACT_ADDRESS, BigInteger.ONE, "00"));
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.encodeActivateContractInstance(
                CONTRACT_ADDRESS,
                BigInteger.ONE,
                "abababababababababababababababababababababababababababababababaa"));
    final byte[] invalidDecodedHash =
        wirePayload(
            ContractLifecycleWirePayloadEncoder.encodeActivateContractInstance(
                CONTRACT_ADDRESS,
                BigInteger.ONE,
                "abababababababababababababababababababababababababababababababab"));
    invalidDecodedHash[invalidDecodedHash.length - 1] =
        (byte) (invalidDecodedHash[invalidDecodedHash.length - 1] & 0xfe);
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.decodeActivateContractInstance(
                invalidDecodedHash));
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
                CONTRACT_ADDRESS, BigInteger.ZERO));
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.encodeCancelContractOwnershipOffer(
                CONTRACT_ADDRESS, BigInteger.ONE.shiftLeft(64)));

    final BigInteger maxRevision =
        BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
    final InstructionBox accept =
        ContractLifecycleWirePayloadEncoder.encodeAcceptContractOwnership(
            CONTRACT_ADDRESS, maxRevision);
    assert maxRevision.equals(
        ContractLifecycleWirePayloadEncoder.decodeAcceptContractOwnership(
                wirePayload(accept))
            .expectedRevision()) : "full-u64 expected revision must round-trip losslessly";
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.decodeCancelContractOwnershipOffer(
                wirePayload(accept)));
    expectIllegalArgument(
        () ->
            ContractLifecycleWirePayloadEncoder.decodeDeactivateContractInstance(
                wirePayload(accept)));
  }

  private static byte[] wirePayload(final InstructionBox box) {
    if (!(box.payload() instanceof InstructionBox.WirePayload wirePayload)) {
      throw new AssertionError("expected canonical wire payload");
    }
    return wirePayload.payloadBytes();
  }

  private static byte[] hexToBytes(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex fixture must have an even length");
    }
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      final int offset = index * 2;
      final int high = Character.digit(value.charAt(offset), 16);
      final int low = Character.digit(value.charAt(offset + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException("hex fixture contains a non-hex character");
      }
      bytes[index] = (byte) ((high << 4) | low);
    }
    return bytes;
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected fail-closed admission.
    }
  }
}
