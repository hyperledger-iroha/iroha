package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
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
  public void lifecycleWirePayloadParity() {
    main(new String[0]);
  }

  public static void main(final String[] args) {
    allLifecycleInstructionsPreserveExpectedRevisionAndOwnerVariant();
    lifecycleRevisionGuardsRejectZeroOverflowAndSchemaSubstitution();
    System.out.println("[IrohaAndroid] Contract lifecycle wire payload tests passed.");
  }

  private static void allLifecycleInstructionsPreserveExpectedRevisionAndOwnerVariant() {
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
    assert ContractLifecycleWirePayloadEncoder.WIRE_NAMES.size() == 4
        : "lifecycle wire catalog must remain closed";

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
  }

  private static byte[] wirePayload(final InstructionBox box) {
    if (!(box.payload() instanceof InstructionBox.WirePayload wirePayload)) {
      throw new AssertionError("expected canonical wire payload");
    }
    return wirePayload.payloadBytes();
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
