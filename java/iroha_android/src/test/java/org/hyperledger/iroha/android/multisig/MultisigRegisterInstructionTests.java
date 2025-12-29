package org.hyperledger.iroha.android.multisig;

import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;

/** Sanity checks for the multisig registration builder and argument schema. */
public final class MultisigRegisterInstructionTests {

  private MultisigRegisterInstructionTests() {}

  public static void main(final String[] args) {
    testRoundTripEncoding();
    testControllerDomainMustMatchSpec();
    testSignatoryDomainDriftIsRejected();
  }

  private static void testRoundTripEncoding() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(60_000)
            .addSignatory("alice@wonderland", 1)
            .addSignatory("bob@wonderland", 1)
            .build();

    final MultisigRegisterInstruction instruction =
        MultisigRegisterInstruction.builder()
            .setAccountId("controller@wonderland")
            .setSpec(spec)
            .build();

    final InstructionBox box = InstructionBox.of(instruction);
    final Map<String, String> args = box.arguments();

    assert box.kind() == InstructionKind.CUSTOM : "multisig register should be custom";
    assert MultisigRegisterInstruction.ACTION.equals(args.get("action")) : "action mismatch";
    assert instruction.accountId().equals(args.get("account")) : "account mismatch";
    assert "2".equals(args.get("spec.quorum")) : "quorum mismatch";
    assert "60000".equals(args.get("spec.transaction_ttl_ms")) : "ttl mismatch";
    assert "1".equals(args.get("spec.signatories.alice@wonderland")) : "alice weight mismatch";
    assert "1".equals(args.get("spec.signatories.bob@wonderland")) : "bob weight mismatch";

    final InstructionBox decoded = InstructionBox.fromNorito(InstructionKind.CUSTOM, args);
    assert decoded.payload() instanceof MultisigRegisterInstruction : "decoded payload mismatch";
    final MultisigRegisterInstruction roundTrip =
        (MultisigRegisterInstruction) decoded.payload();
    assert roundTrip.accountId().equals("controller@wonderland") : "roundtrip account mismatch";
    assert roundTrip.spec().equals(spec) : "roundtrip spec mismatch";

    System.out.println("[IrohaAndroid] MultisigRegisterInstruction roundtrip passed.");
  }

  private static void testControllerDomainMustMatchSpec() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(10_000)
            .addSignatory("alice@wonderland", 1)
            .build();

    boolean threw = false;
    try {
      MultisigRegisterInstruction.builder()
          .setAccountId("controller@narnia")
          .setSpec(spec)
          .build();
    } catch (final IllegalStateException expected) {
      threw = expected.getMessage().contains("domain");
    }
    assert threw : "expected controller domain mismatch to throw";
  }

  private static void testSignatoryDomainDriftIsRejected() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(5_000)
            .addSignatory("alice@wonderland", 1)
            .addSignatory("bob@narnia", 1)
            .build();

    boolean threw = false;
    try {
      MultisigRegisterInstruction.builder()
          .setAccountId("controller@wonderland")
          .setSpec(spec)
          .build();
    } catch (final IllegalStateException expected) {
      threw = expected.getMessage().contains("domain");
    }
    assert threw : "expected mixed signatory domains to be rejected";
  }
}
