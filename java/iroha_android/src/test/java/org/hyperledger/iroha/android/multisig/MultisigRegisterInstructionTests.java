package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;

/** Sanity checks for the multisig registration builder and argument schema. */
public final class MultisigRegisterInstructionTests {

  private MultisigRegisterInstructionTests() {}

  public static void main(final String[] args) {
    testArgumentSchema();
    testControllerDomainMustMatchSpec();
    testSignatoryDomainDriftIsRejected();
    testDerivedControllerIsRejected();
  }

  private static void testArgumentSchema() {
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

    System.out.println("[IrohaAndroid] MultisigRegisterInstruction tests passed.");
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

  private static void testDerivedControllerIsRejected() {
    final String domain = "derived";
    final byte[] signerKey = new byte[32];
    Arrays.fill(signerKey, (byte) 0x11);
    final String signerId;
    try {
      signerId =
          AccountAddress.fromAccount(domain, signerKey, "ed25519")
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX)
              + "@"
              + domain;
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build signatory id", ex);
    }

    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(10_000)
            .addSignatory(signerId, 1)
            .build();

    final Optional<byte[]> derivedOpt =
        MultisigSeedHelper.deriveDeterministicPublicKey(domain, spec);
    final byte[] derivedKey =
        derivedOpt.orElseThrow(() -> new IllegalStateException("Expected derived controller key"));
    final String derivedId;
    try {
      derivedId =
          AccountAddress.fromAccount(domain, derivedKey, "ed25519")
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX)
              + "@"
              + domain;
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build derived controller id", ex);
    }

    boolean threw = false;
    try {
      MultisigRegisterInstruction.builder().setAccountId(derivedId).setSpec(spec).build();
    } catch (final IllegalArgumentException expected) {
      threw = expected.getMessage().contains("derived");
    }
    assert threw : "expected derived controller id to be rejected";
  }
}
