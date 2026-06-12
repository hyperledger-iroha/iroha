package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import org.junit.Test;

public final class AccountIdLiteralTests {
  public static void main(final String[] args) throws Exception {
    final AccountIdLiteralTests tests = new AccountIdLiteralTests();
    tests.acceptsCanonicalI105Literal();
    tests.rejectsSurroundingWhitespaceBeforeValidation();
    tests.rejectsDomainSuffixedLiterals();
    tests.rejectsMalformedAndHexLiterals();
    tests.rejectsBlankAccountId();
    System.out.println("[IrohaAndroid] Account ID literal tests passed.");
  }

  @Test
  public void acceptsCanonicalI105Literal() throws Exception {
    final String address = sampleI105(0x11);
    final String normalized = AccountIdLiteral.requireCanonicalI105Address(address, "accountId");
    assert address.equals(normalized) : "canonical I105 literal must pass through unchanged";
  }

  @Test
  public void rejectsSurroundingWhitespaceBeforeValidation() throws Exception {
    final String address = sampleI105(0x22);
    final String[] paddedInputs = {
        " " + address,
        address + " ",
        "\t" + address,
        address + "\n",
        " \t" + address + "\n "
    };
    for (final String input : paddedInputs) {
      try {
        AccountIdLiteral.requireCanonicalI105Address(input, "accountId");
        throw new AssertionError("expected surrounding whitespace to be rejected");
      } catch (final IllegalArgumentException expected) {
        assert expected.getMessage().contains("surrounding whitespace")
            : "expected surrounding whitespace rejection";
      }
    }
  }

  @Test
  public void rejectsDomainSuffixedLiterals() throws Exception {
    final String address = sampleI105(0x33);
    try {
      AccountIdLiteral.requireCanonicalI105Address(address + "@banka.dataspace", "accountId");
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("without @domain")
          : "expected domain suffix rejection";
    }
  }

  @Test
  public void rejectsMalformedAndHexLiterals() throws Exception {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x44);
    final AccountAddress address = AccountAddress.fromAccount(publicKey, "ed25519");
    try {
      AccountIdLiteral.requireCanonicalI105Address("malformed-i105", "accountId");
      throw new AssertionError("expected malformed non-i105 literal to be rejected");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("canonical I105")
          : "malformed rejection must mention canonical I105";
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
