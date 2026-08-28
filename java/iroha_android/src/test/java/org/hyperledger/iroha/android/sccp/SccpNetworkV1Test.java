package org.hyperledger.iroha.android.sccp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import java.util.Arrays;
import org.junit.Test;

/** Focused tests for the strict final-V1 SCCP network profile cut. */
public final class SccpNetworkV1Test {
  @Test
  public void admitsOnlyTheFiveFreshFinalV1Tags() {
    assertArrayEquals(
        new int[] {0x40, 0x41, 0x42, 0x43, 0x44},
        Arrays.stream(SccpNetworkV1.values()).mapToInt(SccpNetworkV1::tag).toArray());
    for (int tag = 0; tag <= 0xff; tag++) {
      if (tag < 0x40 || tag > 0x44) {
        assertNull("retired tag " + tag, SccpNetworkV1.fromTag(tag));
      }
    }
  }

  @Test
  public void acceptsCanonicalNamesAndRejectsRetiredProfiles() {
    assertEquals(
        SccpNetworkV1.SORA_TAIRA, SccpNetworkV1.fromProfileKey("sora-taira"));
    assertEquals(
        SccpNetworkV1.ETHEREUM_MAINNET,
        SccpNetworkV1.fromProfileKey("ethereum-mainnet"));
    assertEquals(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.fromProfileKey("bsc-mainnet"));
    assertEquals(SccpNetworkV1.TRON_MAINNET, SccpNetworkV1.fromProfileKey("tron-mainnet"));
    assertEquals(SccpNetworkV1.TON_MAINNET, SccpNetworkV1.fromProfileKey("ton-mainnet"));
    for (final String retired :
        Arrays.asList(
            "ethereum-sepolia",
            "bsc-testnet",
            "tron-nile",
            "tron-shasta",
            "ton-testnet",
            "solana-mainnet-beta",
            "solana-testnet")) {
      assertNull(retired, SccpNetworkV1.fromProfileKey(retired));
    }
  }
}
