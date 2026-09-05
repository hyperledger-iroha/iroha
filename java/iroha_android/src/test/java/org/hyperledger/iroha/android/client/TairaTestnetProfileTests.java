package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;

import java.net.URI;
import org.hyperledger.iroha.android.model.NetworkId;
import org.junit.Test;

/** Tests for the public Taira config and KAGEMUSHA V1 asset profile. */
public final class TairaTestnetProfileTests {
  private static final String TEST_NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";

  @Test
  public void profileUsesCallerSuppliedDeploymentNetworkId() {
    final NetworkId deployedNetworkId = NetworkId.parse(TEST_NETWORK_ID);
    final ClientConfig config = TairaTestnetProfile.clientConfig(deployedNetworkId);

    assertEquals(URI.create("https://taira.sora.org"), config.baseUri());
    assertEquals(deployedNetworkId, config.localSigningContext().get().networkId());
    assertEquals("fc56984b-2be7-431d-840e-21514d1883f0", TairaTestnetProfile.CHAIN_ID);
    assertEquals(369, TairaTestnetProfile.I105_DISCRIMINANT);
    assertEquals(
        "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
        TairaTestnetProfile.KAGEMUSHA_ASSET_DEFINITION_ID);
    assertEquals("ds#boi.is", TairaTestnetProfile.KAGEMUSHA_ASSET_ALIAS);
    assertEquals(2, TairaTestnetProfile.KAGEMUSHA_ASSET_SCALE);
    assertEquals("6TEAJqbb8oEPmLncoNiMRbLEK6tw", TairaTestnetProfile.XOR_ASSET_DEFINITION_ID);
    assertEquals("xor#universal", TairaTestnetProfile.XOR_ASSET_ALIAS);
    assertEquals(9, TairaTestnetProfile.XOR_ASSET_SCALE);
  }
}
