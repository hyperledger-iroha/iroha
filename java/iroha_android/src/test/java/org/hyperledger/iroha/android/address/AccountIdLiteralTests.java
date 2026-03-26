package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import org.junit.Test;

public final class AccountIdLiteralTests {

  @Test
  public void acceptsCanonicalI105Literal() throws Exception {
    final String address = sampleI105(0x11);
    final String normalized = AccountIdLiteral.requireCanonicalI105Address(address, "accountId");
    assert address.equals(normalized) : "canonical I105 literal must pass through unchanged";
  }

  @Test
  public void trimsWhitespaceBeforeValidation() throws Exception {
    final String address = sampleI105(0x22);
    final String normalized =
        AccountIdLiteral.requireCanonicalI105Address("  " + address + "  ", "accountId");
    assert address.equals(normalized) : "validation must trim surrounding whitespace";
  }

  @Test
  public void rejectsDomainSuffixedLiterals() throws Exception {
    final String address = sampleI105(0x33);
    try {
      AccountIdLiteral.requireCanonicalI105Address(address + "@hbl.dataspace", "accountId");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("without @domain")
          : "expected domain suffix rejection";
    }
  }

  @Test
  public void rejectsLegacyAndHexLiterals() throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x44);
    final AccountAddress address = AccountAddress.fromAccount(publicKey, "ed25519");
    try {
      AccountIdLiteral.requireCanonicalI105Address(
          "soraチキVMXfkAweDFテqSkhXウrdUイヒニ4eYqサアYsミヰヲt4オサウキHヰkNチヲメミwjQgmワdnク9h5BSkヱワルvセ6サyWtSヨロAカaヱロレサxトメシMAyス8ソjDZナイスMwチBモヰ9ホヰロRユQコk3キニリシmDラyiRGユfGコHaマVY5phTKQ316", "accountId");
      throw new AssertionError("expected legacy non-i105 literal to be rejected");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical I105")
          : "legacy rejection must mention canonical I105";
    }
    try {
      AccountIdLiteral.requireCanonicalI105Address(address.canonicalHex(), "accountId");
      throw new AssertionError("expected canonical hex literal to be rejected");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical I105")
          : "hex rejection must mention canonical I105";
    }
  }

  @Test
  public void rejectsBlankAccountId() {
    try {
      AccountIdLiteral.requireCanonicalI105Address("   ", "accountId");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("must not be blank") : "blank rejection mismatch";
    }
  }

  private static String sampleI105(final int fill) throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fill);
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }
}
