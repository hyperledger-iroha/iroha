package org.hyperledger.iroha.sdk.offline

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class KagemushaOmapiDiscoveryDeviceTest {
    @Test
    fun discoveryAlwaysReachesATerminalBridgeWithinItsBound() {
        val executor = Executors.newSingleThreadExecutor()
        try {
            val bridge = KagemushaOmapiDeviceLifecycleV1.openAsync(
                InstrumentationRegistry.getInstrumentation().targetContext,
                executor,
                discoveryTimeoutMillis = 1_500,
            ).get(5, TimeUnit.SECONDS)

            assertNotNull(bridge.availability)
        } finally {
            executor.shutdownNow()
        }
    }
}
