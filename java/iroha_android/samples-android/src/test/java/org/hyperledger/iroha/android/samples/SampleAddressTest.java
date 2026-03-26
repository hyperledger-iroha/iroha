package org.hyperledger.iroha.android.samples;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import org.hyperledger.iroha.android.address.AccountAddress;
import org.junit.Test;

public class SampleAddressTest {
  @Test
  public void buildsAddressFromAarSurface() throws AccountAddress.AccountAddressException {
    byte[] key = new byte[32];
    AccountAddress address = AccountAddress.fromAccount(key, "ed25519");

    assertTrue(address.canonicalHex().startsWith("0x"));
    AccountAddress.DisplayFormats formats = address.displayFormats();
    assertEquals(address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT), formats.i105);
    assertTrue(formats.i105Warning.contains("canonical account literal encoding"));
  }
}
