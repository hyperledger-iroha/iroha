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
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
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
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
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
  public void deterministicControllerCheckRejectsPublicKeyMultihashLiteral() throws Exception {
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
    final String signer = accountIdForSeed(domain, (byte) 0x21);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final Optional<byte[]> derivedKey = MultisigSeedHelper.deriveDeterministicPublicKey(domain, spec);
    assert derivedKey.isPresent() : "derived key must be computed";

    final String derivedLiteral = publicKeyLiteral(derivedKey.get());
    assert !MultisigSeedHelper.isDeterministicDerivedControllerId(derivedLiteral, spec)
        : "public key multihash literals must not pass encoded account-id checks";
  }

  @Test
  public void deterministicSeedDerivationRejectsCanonicalHexSignatory() throws Exception {
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
    final byte[] signerKey = new byte[32];
    java.util.Arrays.fill(signerKey, (byte) 0x31);
    final String canonical = AccountAddress.fromAccount(signerKey, "ed25519").canonicalHex();
    try {
      MultisigSpec.builder()
          .setQuorum(1)
          .setTransactionTtlMs(1)
          .addSignatory(canonical, 1)
          .build();
      throw new AssertionError("canonical-hex signatories must be rejected");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void deterministicControllerCheckRejectsNestedDomainIdentifier() throws Exception {
    final String domain = AccountAddress.DEFAULT_DOMAIN_NAME;
    final String signer = accountIdForSeed(domain, (byte) 7);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(1)
            .addSignatory(signer, 1)
            .build();

    final String nested = accountIdForSeed(domain, (byte) 8) + "@fallback";
    assert !MultisigSeedHelper.isDeterministicDerivedControllerId(nested, spec)
        : "nested-domain identifiers must be rejected";
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
    final AccountAddress address = AccountAddress.fromAccount(publicKey, "ed25519");
    return address.toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
  }

  private static String publicKeyLiteral(final byte[] publicKey) {
    final String algorithm = PublicKeyCodec.algorithmForCurveId(0x01);
    final String multihash = PublicKeyCodec.encodePublicKeyMultihash(0x01, publicKey);
    return algorithm + ":" + multihash;
  }
}
