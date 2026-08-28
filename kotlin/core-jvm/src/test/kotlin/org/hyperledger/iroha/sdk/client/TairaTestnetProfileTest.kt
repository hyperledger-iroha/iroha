package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId

class TairaTestnetProfileTest {
    @Test
    fun profileUsesCallerSuppliedDeploymentNetworkId() {
        val deployedNetworkId = NetworkId.parse(TEST_NETWORK_ID)
        val config = TairaTestnetProfile.clientConfig(deployedNetworkId)

        assertEquals(URI.create("https://taira.sora.org"), config.baseUri())
        assertEquals(deployedNetworkId, config.localSigningContext().get().networkId())
        assertEquals("fc56984b-2be7-431d-840e-21514d1883f0", TairaTestnetProfile.CHAIN_ID)
        assertEquals(369, TairaTestnetProfile.I105_DISCRIMINANT)
        assertEquals(
            "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
            TairaTestnetProfile.KAGEMUSHA_ASSET_DEFINITION_ID,
        )
        assertEquals("ds#boi.is", TairaTestnetProfile.KAGEMUSHA_ASSET_ALIAS)
        assertEquals(2, TairaTestnetProfile.KAGEMUSHA_ASSET_SCALE)
        assertEquals("6TEAJqbb8oEPmLncoNiMRbLEK6tw", TairaTestnetProfile.XOR_ASSET_DEFINITION_ID)
        assertEquals("xor#universal", TairaTestnetProfile.XOR_ASSET_ALIAS)
        assertEquals(9, TairaTestnetProfile.XOR_ASSET_SCALE)
    }

    @Test
    fun configAdapterTargetsThePublicKagemushaCapabilityRoute() {
        val captured = AtomicReference<TransportRequest>()
        val executor = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                captured.set(request)
                return CompletableFuture.completedFuture(
                    TransportResponse.builder()
                        .setStatusCode(200)
                        .addHeader("Content-Type", "application/json")
                        .setBody(CAPABILITY_JSON.toByteArray(StandardCharsets.UTF_8))
                        .build(),
                )
            }
        }
        val requestTimeout = Duration.ofSeconds(37)
        val config = TairaTestnetProfile.clientConfig(NetworkId.parse(TEST_NETWORK_ID))
            .toBuilder()
            .setRequestTimeout(requestTimeout)
            .putDefaultHeader("Authorization", "Bearer must-not-leak")
            .build()

        val capability = config.toKagemushaToriiClient(executor).getOfflineCapability().join()

        assertEquals(
            URI.create("https://taira.sora.org/v1/offline/readiness"),
            captured.get().uri,
        )
        assertEquals("GET", captured.get().method)
        assertEquals(listOf("application/json"), captured.get().headers["Accept"])
        assertEquals(requestTimeout, captured.get().timeout)
        assertFalse(captured.get().headers.containsKey("Authorization"))
        assertEquals("cash_handoff_v1", capability.cashHandoffCapability)
        assertTrue(capability.ready)
    }

    @Test
    fun configAdapterRequiresADeploymentNetworkIdAndSupportsTheDefaultExecutor() {
        val withoutNetworkId = ClientConfig.builder()
            .setBaseUri(TairaTestnetProfile.TORII_BASE_URI)
            .build()
        assertFailsWith<IllegalStateException> {
            withoutNetworkId.toKagemushaToriiClient(PlatformHttpTransportExecutor.createDefault())
        }

        val configured = TairaTestnetProfile.clientConfig(NetworkId.parse(TEST_NETWORK_ID))
        assertNotNull(configured.toKagemushaToriiClient())
    }

    private companion object {
        private const val TEST_NETWORK_ID =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        private const val CAPABILITY_JSON =
            "{\"cash_handoff_capability\":\"cash_handoff_v1\"," +
                "\"required_bridge_abi_version\":23,\"max_hops\":8,\"ready\":true}"
    }
}
