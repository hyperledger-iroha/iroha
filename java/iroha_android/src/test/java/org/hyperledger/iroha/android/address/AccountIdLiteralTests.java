package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import org.junit.Test;

public final class AccountIdLiteralTests {

  @Test
  public void keepsEncodedAuthority() throws Exception {
    final String address = sampleIh58(0x11);
    final String normalized = AccountIdLiteral.extractIh58Address(address);
    assert address.equals(normalized) : "encoded account id must remain unchanged";
  }

  @Test
  public void trimsWhitespaceBeforeNormalization() throws Exception {
    final String address = sampleIh58(0x22);
    final String normalized = AccountIdLiteral.extractIh58Address("  " + address + "  ");
    assert address.equals(normalized) : "encoded account id normalization must trim whitespace";
  }

  @Test
  public void rejectsLegacyDomainSuffix() throws Exception {
    try {
      AccountIdLiteral.extractIh58Address(sampleIh58(0x33) + "@wonderland");
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
      AccountIdLiteral.extractIh58Address(canonical);
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  @Test
  public void rejectsBlankAccountId() {
    try {
      AccountIdLiteral.extractIh58Address("   ");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static String sampleIh58(final int fill) throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fill);
    return AccountAddress.fromAccount(publicKey, "ed25519")
        .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
  }
}
