package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.NetworkId;
import org.junit.Test;

/** Tests for the public Taira config profile and Kagemusha client adapter. */
public final class TairaTestnetProfileTests {
  private static final String TEST_NETWORK_ID =
      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
  private static final byte[] CAPABILITY_JSON =
      ("{\"cash_handoff_capability\":\"cash_handoff_v1\","
              + "\"required_bridge_abi_version\":23,\"max_hops\":8,\"ready\":true}")
          .getBytes(StandardCharsets.UTF_8);

  @Test
  public void profileUsesCallerSuppliedDeploymentNetworkId() {
    final NetworkId deployedNetworkId = NetworkId.parse(TEST_NETWORK_ID);
    final ClientConfig config = TairaTestnetProfile.clientConfig(deployedNetworkId);

    assertEquals(URI.create("https://taira.sora.org"), config.baseUri());
    assertEquals(deployedNetworkId, config.localSigningContext().get().networkId());
    assertEquals("fc56984b-2be7-431d-840e-21514d1883f0", TairaTestnetProfile.CHAIN_ID);
    assertEquals(369, TairaTestnetProfile.I105_DISCRIMINANT);
    assertEquals("6TEAJqbb8oEPmLncoNiMRbLEK6tw", TairaTestnetProfile.XOR_ASSET_DEFINITION_ID);
    assertEquals(9, TairaTestnetProfile.XOR_ASSET_SCALE);
  }

  @Test
  public void configAdapterTargetsThePublicKagemushaCapabilityRoute() {
    final AtomicReference<TransportRequest> captured = new AtomicReference<>();
    final HttpTransportExecutor executor =
        request -> {
          captured.set(request);
          return CompletableFuture.completedFuture(
              new TransportResponse(
                  200,
                  CAPABILITY_JSON,
                  "OK",
                  Collections.singletonMap(
                      "Content-Type", Collections.singletonList("application/json"))));
        };
    final Duration requestTimeout = Duration.ofSeconds(37);
    final ClientConfig config =
        TairaTestnetProfile.clientConfig(NetworkId.parse(TEST_NETWORK_ID))
            .toBuilder()
            .setRequestTimeout(requestTimeout)
            .putDefaultHeader("Authorization", "Bearer must-not-leak")
            .build();

    final boolean ready =
        config.toKagemushaToriiClient(executor).getOfflineCapability().join().ready();

    assertEquals(
        URI.create("https://taira.sora.org/v1/offline/readiness"), captured.get().uri());
    assertEquals("GET", captured.get().method());
    assertEquals(
        Collections.singletonList("application/json"),
        captured.get().headers().get("Accept"));
    assertEquals(requestTimeout, captured.get().timeout());
    assertFalse(captured.get().headers().containsKey("Authorization"));
    assertTrue(ready);
  }

  @Test
  public void configAdapterRequiresADeploymentNetworkIdAndSupportsTheDefaultExecutor() {
    final ClientConfig withoutNetworkId =
        ClientConfig.builder().setBaseUri(TairaTestnetProfile.TORII_BASE_URI).build();
    assertThrows(
        IllegalStateException.class,
        () ->
            withoutNetworkId.toKagemushaToriiClient(
                PlatformHttpTransportExecutor.createDefault()));

    final ClientConfig configured =
        TairaTestnetProfile.clientConfig(NetworkId.parse(TEST_NETWORK_ID));
    assertNotNull(configured.toKagemushaToriiClient());
  }
}
