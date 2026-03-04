package org.hyperledger.iroha.android.address;

import org.junit.Test;

public final class AccountIdLiteralTests {

  @Test
  public void extractsIh58Address() {
    final String address = AccountIdLiteral.extractIh58Address("ih58example@wonderland");
    assert "ih58example".equals(address) : "IH58 extraction mismatch";
  }

  @Test
  public void trimsWhitespaceBeforeExtraction() {
    final String address = AccountIdLiteral.extractIh58Address("  ih58example@wonderland  ");
    assert "ih58example".equals(address) : "IH58 extraction must trim";
  }

  @Test
  public void rejectsMissingDomainSeparator() {
    try {
      AccountIdLiteral.extractIh58Address("ih58example");
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
}
