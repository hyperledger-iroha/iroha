package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;
import org.junit.Test;

public final class MultisigSeedHelperTests {

  @Test
  public void derivedControllerIdIsRejected() throws Exception {
    final String signer = accountIdForSeed((byte) 7);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final Optional<byte[]> derivedKey = MultisigSeedHelper.deriveDeterministicPublicKey(spec);
    assert derivedKey.isPresent() : "derived key must be computed";

    final String multisigAccount = accountIdForPublicKey(derivedKey.get());
    assert MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "derived controller must be rejected";

    try {
      MultisigRegisterInstruction.builder()
          .setAccountId(multisigAccount)
          .setSpec(spec)
          .build();
      throw new AssertionError("expected derived controller rejection");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void nonDerivedControllerIdIsAllowed() throws Exception {
    final String signer = accountIdForSeed((byte) 8);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final String multisigAccount = accountIdForSeed((byte) 9);
    assert !MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "non-derived controller must not be rejected";

    MultisigRegisterInstruction.builder()
        .setAccountId(multisigAccount)
        .setSpec(spec)
        .build();
  }

  @Test
  public void deterministicSeedIgnoresInsertionOrder() throws Exception {
    final String signerA = accountIdForSeed((byte) 0x21);
    final String signerB = accountIdForSeed((byte) 0x22);
    final MultisigSpec left =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(1)
            .addSignatory(signerA, 1)
            .addSignatory(signerB, 1)
            .build();
    final MultisigSpec right =
        MultisigSpec.builder()
            .setQuorum(2)
            .setTransactionTtlMs(1)
            .addSignatory(signerB, 1)
            .addSignatory(signerA, 1)
            .build();
    final Optional<byte[]> leftKey = MultisigSeedHelper.deriveDeterministicPublicKey(left);
    final Optional<byte[]> rightKey = MultisigSeedHelper.deriveDeterministicPublicKey(right);
    assert leftKey.isPresent() : "left derived key must be computed";
    assert rightKey.isPresent() : "right derived key must be computed";
    assert Arrays.equals(leftKey.get(), rightKey.get())
        : "deterministic controller seed must depend only on the canonical signatory set";
  }

  private static String accountIdForSeed(final byte seed) throws Exception {
    final byte[] seedBytes = new byte[32];
    Arrays.fill(seedBytes, seed);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seedBytes, 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    return accountIdForPublicKey(publicKey);
  }

  private static String accountIdForPublicKey(final byte[] publicKey) throws Exception {
    final AccountAddress address = AccountAddress.fromAccount(publicKey, "ed25519");
    return address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }
}
