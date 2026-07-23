package org.hyperledger.iroha.android.testing;

import org.hyperledger.iroha.android.address.AccountAddress;

/** Deterministic canonical I105 account identifiers for Android tests. */
public final class TestAccountIds {

  private TestAccountIds() {}

  public static String ed25519Authority(final int fill) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build test authority", ex);
    }
  }
}
