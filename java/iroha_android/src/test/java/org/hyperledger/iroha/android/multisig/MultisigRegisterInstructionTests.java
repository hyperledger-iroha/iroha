package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import java.util.Map;
import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;

/** Sanity checks for the multisig registration builder and argument schema. */
public final class MultisigRegisterInstructionTests {

  private MultisigRegisterInstructionTests() {}

  public static void main(final String[] args) {
    testArgumentSchema();
    testLegacyDomainLiteralIsRejected();
    testDerivedControllerIsRejected();
  }

  private static void testArgumentSchema() {
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
    final String alice = accountIdForSeed(domain, (byte) 0x11);
    final String bob = accountIdForSeed(domain, (byte) 0x22);
    final String controller = accountIdForSeed(domain, (byte) 0x33);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(60_000)
            .addSignatory(alice, 1)
            .addSignatory(bob, 1)
            .build();

    final MultisigRegisterInstruction instruction =
        MultisigRegisterInstruction.builder()
            .setAccountId(controller)
            .setSpec(spec)
            .build();

    final InstructionBox box = InstructionBox.of(instruction);
    final Map<String, String> args = box.arguments();

    assert box.kind() == InstructionKind.CUSTOM : "multisig register should be custom";
    assert MultisigRegisterInstruction.ACTION.equals(args.get("action")) : "action mismatch";
    assert instruction.accountId().equals(args.get("account")) : "account mismatch";
    assert "2".equals(args.get("spec.quorum")) : "quorum mismatch";
    assert "60000".equals(args.get("spec.transaction_ttl_ms")) : "ttl mismatch";
    assert "1".equals(args.get("spec.signatories." + alice)) : "alice weight mismatch";
    assert "1".equals(args.get("spec.signatories." + bob)) : "bob weight mismatch";

    System.out.println("[IrohaAndroid] MultisigRegisterInstruction tests passed.");
  }

  private static void testLegacyDomainLiteralIsRejected() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(10_000)
            .addSignatory(accountIdForSeed(AccountAddress.DEFAULT_DOMAIN_NAME, (byte) 0x44), 1)
            .build();

    boolean threw = false;
    try {
      MultisigRegisterInstruction.builder()
          .setAccountId("controller@narnia")
          .setSpec(spec)
          .build();
    } catch (final IllegalArgumentException expected) {
      threw = expected.getMessage().contains("encoded");
    }
    assert threw : "expected legacy @domain literal to throw";
  }

  private static void testDerivedControllerIsRejected() {
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
    final byte[] signerKey = new byte[32];
    Arrays.fill(signerKey, (byte) 0x11);
    final String signerId;
    try {
      signerId =
          AccountAddress.fromAccount(signerKey, "ed25519")
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
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
          AccountAddress.fromAccount(derivedKey, "ed25519")
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
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

  private static String accountIdForSeed(final String domain, final byte seed) {
    final byte[] seedBytes = new byte[32];
    Arrays.fill(seedBytes, seed);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seedBytes, 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    try {
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to derive account id", ex);
    }
  }
}
