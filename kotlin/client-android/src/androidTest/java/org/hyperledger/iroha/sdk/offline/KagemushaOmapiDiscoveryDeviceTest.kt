package org.hyperledger.iroha.sdk.offline

import android.os.Build
import android.util.Log
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
                discoveryTimeoutMillis = KagemushaOmapiDeviceLifecycleV1.DEFAULT_DISCOVERY_TIMEOUT_MILLIS,
            ).get(15, TimeUnit.SECONDS)

            assertNotNull(bridge.availability)
            if (Build.MANUFACTURER.equals("Google", ignoreCase = true) && Build.DEVICE == "oriole") {
                Log.i("IrohaKagemushaOmapiProbe", "Pixel 6 provisioned-app discovery: ${bridge.availability}")
            }
        } finally {
            executor.shutdownNow()
        }
    }
}
