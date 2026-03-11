package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import org.junit.Test;

public final class AccountIdLiteralTests {

  @Test
  public void keepsEncodedAuthority() throws Exception {
    final String address = sampleI105(0x11);
    final String normalized = AccountIdLiteral.extractI105Address(address);
    assert address.equals(normalized) : "encoded account id must remain unchanged";
  }

  @Test
  public void trimsWhitespaceBeforeNormalization() throws Exception {
    final String address = sampleI105(0x22);
    final String normalized = AccountIdLiteral.extractI105Address("  " + address + "  ");
    assert address.equals(normalized) : "encoded account id normalization must trim whitespace";
  }

  @Test
  public void rejectsLegacyDomainSuffix() throws Exception {
    try {
      AccountIdLiteral.extractI105Address(sampleI105(0x33) + "@wonderland");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void rejectsCanonicalHex() throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x44);
    final String canonical =
        AccountAddress.fromAccount(publicKey, "ed25519").canonicalHex();
    try {
      AccountIdLiteral.extractI105Address(canonical);
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void rejectsBlankAccountId() {
    try {
      AccountIdLiteral.extractI105Address("   ");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static String sampleI105(final int fill) throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fill);
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }
}
