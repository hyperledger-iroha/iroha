package org.hyperledger.iroha.android.multisig;

import java.util.Optional;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.instructions.MultisigRegisterInstruction;
import org.junit.Test;

public final class MultisigSeedHelperTests {

  @Test
  public void derivedControllerIdIsRejected() throws Exception {
    final String domain = "derived";
    final String signer = accountIdForSeed(domain, (byte) 7);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final Optional<byte[]> derivedKey = MultisigSeedHelper.deriveDeterministicPublicKey(domain, spec);
    assert derivedKey.isPresent() : "derived key must be computed";

    final String multisigAccount = accountIdForPublicKey(domain, derivedKey.get());
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
    final String domain = "non-derived";
    final String signer = accountIdForSeed(domain, (byte) 8);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final String multisigAccount = accountIdForSeed(domain, (byte) 9);
    assert !MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "non-derived controller must not be rejected";

    MultisigRegisterInstruction.builder()
        .setAccountId(multisigAccount)
        .setSpec(spec)
        .build();
  }

  @Test
  public void derivedControllerIdIsRejectedForMultihashLiteral() {
    final String domain = "derived-literal";
    final byte[] signerKey = new byte[32];
    java.util.Arrays.fill(signerKey, (byte) 0x21);
    final String signerLiteral = publicKeyLiteral(signerKey);
    final String signerId = signerLiteral + "@" + domain;
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signerId, 1)
            .build();

    final Optional<byte[]> derivedKey = MultisigSeedHelper.deriveDeterministicPublicKey(domain, spec);
    assert derivedKey.isPresent() : "derived key must be computed";

    final String derivedLiteral = publicKeyLiteral(derivedKey.get());
    final String multisigAccount = derivedLiteral + "@" + domain;
    assert MultisigSeedHelper.isDeterministicDerivedControllerId(multisigAccount, spec)
        : "derived controller literal must be rejected";

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

  private static String accountIdForSeed(final String domain, final byte seed) throws Exception {
    final byte[] seedBytes = new byte[32];
    java.util.Arrays.fill(seedBytes, seed);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seedBytes, 0);
    final byte[] publicKey = privateKey.generatePublicKey().getEncoded();
    return accountIdForPublicKey(domain, publicKey);
  }

  private static String accountIdForPublicKey(final String domain, final byte[] publicKey)
      throws Exception {
    final AccountAddress address = AccountAddress.fromAccount(domain, publicKey, "ed25519");
    final String ih58 = address.toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    return ih58 + "@" + domain;
  }

  private static String publicKeyLiteral(final byte[] publicKey) {
    final String algorithm = PublicKeyCodec.algorithmForCurveId(0x01);
    final String multihash = PublicKeyCodec.encodePublicKeyMultihash(0x01, publicKey);
    return algorithm + ":" + multihash;
  }
}
