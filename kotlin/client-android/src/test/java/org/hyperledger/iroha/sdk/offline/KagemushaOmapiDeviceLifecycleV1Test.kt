package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CompletableFuture
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class KagemushaOmapiDeviceLifecycleV1Test {
    @Test
    fun `default discovery admits only embedded readers and explicit pins stay exact`() {
        for (candidate in listOf("eSE", "eSE1", "eSE2", "eSE10")) {
            assertTrue(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName(candidate, null))
        }
        for (candidate in listOf("SIM1", "SD1", "vendor-secure-element", "eSE0", "eSE01", "eSE1junk")) {
            assertFalse(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName(candidate, null))
        }
        assertTrue(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("eSE2", "eSE2"))
        assertFalse(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("eSE1", "eSE2"))
        assertFalse(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("ese2", "eSE2"))
    }

    @Test
    fun `configuration keeps an exact reader pin and defensive applet AID`() {
        val aid = KagemushaOmapiDeviceLifecycleV1.defaultAppletAid()
        val configuration = KagemushaOmapiDeviceLifecycleV1.Configuration("eSE1", aid)
        aid.fill(0)

        assertEquals("eSE1", configuration.readerName)
        assertContentEquals(
            byteArrayOf(
                0xf0.toByte(), 0x4f, 0x44, 0x4a, 0x52, 0x4e, 0x00, 0x01,
            ),
            configuration.appletAid,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaOmapiDeviceLifecycleV1.Configuration(" eSE1 ")
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOmapiDeviceLifecycleV1.Configuration("SIM1")
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOmapiDeviceLifecycleV1.Configuration(appletAid = ByteArray(8))
        }
    }

    @Test
    fun `timeout wins once and cannot replace an earlier terminal result`() {
        val pending = CompletableFuture<KagemushaDeviceLifecycleBridgeV1>()
        var timeoutCallbacks = 0
        assertTrue(
            KagemushaOmapiDeviceLifecycleV1.completeUnavailableUnlessResolved(pending) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(
            KagemushaDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY,
            pending.join().availability,
        )
        assertFalse(
            KagemushaOmapiDeviceLifecycleV1.completeUnavailableUnlessResolved(pending) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(1, timeoutCallbacks)

        val failed = CompletableFuture<KagemushaDeviceLifecycleBridgeV1>()
        failed.completeExceptionally(IllegalStateException("terminal discovery failure"))
        assertFalse(
            KagemushaOmapiDeviceLifecycleV1.completeUnavailableUnlessResolved(failed) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(1, timeoutCallbacks)
    }
}
