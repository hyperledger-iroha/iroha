package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.time.Duration
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.api.Assumptions.assumeTrue

class TairaKagemushaReadOnlyPublicTest {
    @Test
    fun publicCapabilityMatchesExactUniversalContract() {
        assumeTrue(
            System.getenv(OPT_IN_ENV) == "1",
            "Set $OPT_IN_ENV=1 to run the read-only public Taira probe.",
        )

        val transport = UrlConnectionTransportExecutor(DEADLINE)
        val client = KagemushaRecursiveSpendProver.newToriiClient(
            publicRoot(),
            transport,
            LocalSigningContext(NetworkId.parse(NON_SIGNING_TEST_NETWORK_ID)),
        )
        val capability = client.getOfflineCapability().get(DEADLINE.seconds, TimeUnit.SECONDS)

        assertEquals("cash_handoff_v1", capability.cashHandoffCapability)
        assertEquals(23, capability.requiredBridgeAbiVersion)
        assertEquals(8, capability.maximumHops)
        assertTrue(capability.ready)
    }

    private fun publicRoot(): URI {
        val raw = System.getenv(PUBLIC_ROOT_ENV) ?: DEFAULT_PUBLIC_ROOT
        require(raw == raw.trim()) {
            "$PUBLIC_ROOT_ENV must not contain surrounding whitespace"
        }
        val root = URI.create(raw)
        require(
            root.isAbsolute &&
                !root.isOpaque &&
                root.scheme.equals("https", ignoreCase = true) &&
                !root.host.isNullOrEmpty() &&
                root.rawUserInfo == null &&
                root.rawQuery == null &&
                root.rawFragment == null &&
                (root.rawPath.isNullOrEmpty() || root.rawPath == "/"),
        ) {
            "$PUBLIC_ROOT_ENV must be a credential-free HTTPS origin without a path, query, or fragment"
        }
        return URI.create(raw.removeSuffix("/"))
    }

    private companion object {
        private const val OPT_IN_ENV = "IROHA_TAIRA_KAGEMUSHA_READ_ONLY"
        private const val PUBLIC_ROOT_ENV = "IROHA_TAIRA_PUBLIC_ROOT"
        private const val DEFAULT_PUBLIC_ROOT = "https://taira.sora.org"
        private val DEADLINE: Duration = Duration.ofSeconds(20)

        // The read-only endpoint never consumes this required construction context.
        private const val NON_SIGNING_TEST_NETWORK_ID =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    }
}
