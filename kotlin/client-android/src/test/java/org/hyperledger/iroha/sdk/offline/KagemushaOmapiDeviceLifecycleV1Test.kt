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
    fun `default discovery admits every reader class but an explicit pin stays exact`() {
        for (candidate in listOf("eSE1", "SIM1", "SD1", "vendor-secure-element")) {
            assertTrue(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName(candidate, null))
        }
        assertTrue(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("SIM2", "SIM2"))
        assertFalse(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("SIM1", "SIM2"))
        assertFalse(KagemushaOmapiDeviceLifecycleV1.acceptsReaderName("sim2", "SIM2"))
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
