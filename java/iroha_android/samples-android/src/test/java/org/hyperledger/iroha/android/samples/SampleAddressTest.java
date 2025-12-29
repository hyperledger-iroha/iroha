package org.hyperledger.iroha.android.samples;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import org.hyperledger.iroha.android.address.AccountAddress;
import org.junit.Test;

public class SampleAddressTest {
  @Test
  public void buildsAddressFromAarSurface() {
    byte[] key = new byte[32];
    AccountAddress address = AccountAddress.fromAccount("wonderland", key, "ed25519");

    assertTrue(address.canonicalHex().contains("wonderland"));
    assertEquals(address.canonicalHex(), address.displayFormats().ih58);
  }
}
