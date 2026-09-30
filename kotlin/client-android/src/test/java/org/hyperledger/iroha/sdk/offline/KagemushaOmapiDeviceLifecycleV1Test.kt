package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
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
        val pending = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        var timeoutCallbacks = 0
        assertTrue(
            KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(pending) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(
            KagemushaDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY,
            pending.join().bridge.availability,
        )
        assertFalse(
            KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(pending) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(1, timeoutCallbacks)

        val failed = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        failed.completeExceptionally(IllegalStateException("terminal discovery failure"))
        assertFalse(
            KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(failed) {
                timeoutCallbacks += 1
            },
        )
        assertEquals(1, timeoutCallbacks)
    }

    @Test
    fun `platform access denial and IO retain original evidence without body rendering`() {
        val denied = SecurityException("Access Control Enforcer denied the selected AID")
        val io = java.io.IOException("OEM channel failure")
        val first = KagemushaOmapiDeviceLifecycleV1.classifyFailure("eSE1", denied)
        val second = KagemushaOmapiDeviceLifecycleV1.classifyFailure("eSE2", io)
        assertEquals(KagemushaOmapiDeviceLifecycleV1.FailureReason.ACCESS_DENIED, first.reason)
        assertTrue(first.cause === denied)
        assertEquals(KagemushaOmapiDeviceLifecycleV1.FailureReason.PLATFORM_IO, second.reason)
        assertTrue(second.cause === io)
        assertFalse(first.toString().contains(denied.message!!))
        assertEquals(KagemushaOmapiDeviceLifecycleV1.FailureReason.APPLET_NOT_FOUND,
            KagemushaOmapiDeviceLifecycleV1.classifyFailure("eSE1", java.util.NoSuchElementException()).reason)
        assertEquals(KagemushaOmapiDeviceLifecycleV1.FailureReason.NO_LOGICAL_CHANNEL,
            KagemushaOmapiDeviceLifecycleV1.classifyFailure("eSE1", UnsupportedOperationException()).reason)
    }

    @Test
    fun `diagnostic timeout remains unavailable and preserves earlier original failures`() {
        val pending = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        var shutdowns = 0
        assertTrue(KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(pending) { shutdowns++ })
        assertEquals(KagemushaOmapiDeviceLifecycleV1.DiscoveryStatus.TIMED_OUT, pending.join().status)
        assertEquals(KagemushaDeviceLifecycleBridgeV1.Availability.ONLINE_ONLY, pending.join().bridge.availability)
        assertFalse(KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(pending) { shutdowns++ })
        assertEquals(1, shutdowns)
        val original = KagemushaOmapiDeviceLifecycleV1.classifyFailure("eSE1", SecurityException("denied"))
        val complete = CompletableFuture.completedFuture(KagemushaOmapiDeviceLifecycleV1.DiscoveryResult(
            KagemushaDeviceLifecycleBridgeV1.onlineOnly(), KagemushaOmapiDeviceLifecycleV1.DiscoveryStatus.UNAVAILABLE, listOf(original)))
        assertFalse(KagemushaOmapiDeviceLifecycleV1.completeDiagnosticTimeoutUnlessResolved(complete) { shutdowns++ })
        assertTrue(complete.join().failures.single() === original)
        assertEquals(1, shutdowns)
        assertFailsWith<IllegalArgumentException> { KagemushaOmapiDeviceLifecycleV1.DiscoveryResult(
            KagemushaDeviceLifecycleBridgeV1.onlineOnly(), KagemushaOmapiDeviceLifecycleV1.DiscoveryStatus.AVAILABLE, emptyList()) }
    }

    @Test
    fun `cancelled bridge projection disposes already completed undelivered available owner once`() {
        val discovery = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        val projected = KagemushaOmapiDeviceLifecycleV1.projectDiscovery(discovery)
        var closed = 0
        val available = KagemushaOmapiDeviceLifecycleV1.DiscoveryResult(
            availableBridge(), KagemushaOmapiDeviceLifecycleV1.DiscoveryStatus.AVAILABLE,
            emptyList(), { closed++ },
        )
        // Completion callbacks are stacked: cancel after discovery completes but before projection.
        discovery.whenComplete { _, _ -> projected.cancel(false) }
        assertTrue(discovery.complete(available))
        assertTrue(discovery.isDone)
        assertFalse(discovery.isCancelled)
        assertTrue(projected.isCancelled)
        assertEquals(1, closed)
        available.discardIfUndelivered()
        assertEquals(1, closed)
    }

    @Test
    fun `delivered discovery bridge retains its owner and cancellation cannot dispose it`() {
        val discovery = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        val projected = KagemushaOmapiDeviceLifecycleV1.projectDiscovery(discovery)
        var closed = 0
        val bridge = availableBridge()
        discovery.complete(KagemushaOmapiDeviceLifecycleV1.DiscoveryResult(
            bridge, KagemushaOmapiDeviceLifecycleV1.DiscoveryStatus.AVAILABLE, emptyList(), { closed++ },
        ))
        assertTrue(projected.join() === bridge)
        assertFalse(projected.cancel(false))
        assertEquals(0, closed)
    }

    @Test
    fun `cancelled projection cancels still pending discovery and preserves discovery failures`() {
        val pending = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        val projected = KagemushaOmapiDeviceLifecycleV1.projectDiscovery(pending)
        assertTrue(projected.cancel(false))
        assertTrue(pending.isCancelled)

        val failed = CompletableFuture<KagemushaOmapiDeviceLifecycleV1.DiscoveryResult>()
        val failedProjection = KagemushaOmapiDeviceLifecycleV1.projectDiscovery(failed)
        val original = IllegalStateException("original discovery failure")
        failed.completeExceptionally(original)
        val failure = assertFailsWith<java.util.concurrent.CompletionException> { failedProjection.join() }
        assertTrue(failure.cause === original)
    }

    private fun availableBridge(): KagemushaDeviceLifecycleBridgeV1 =
        KagemushaDeviceLifecycleBridgeV1.withEndpointForTests(object : KagemushaDeviceLifecycleBridgeV1.Endpoint {
            override fun capabilities() = KagemushaDeviceLifecycleBridgeV1.Codec.encodeCapabilitiesForTests(
                1, ByteArray(32) { 1 }, ByteArray(32) { 2 },
            )
            override fun execute(command: ByteArray): ByteArray = error("mapping-only discovery fixture cannot execute")
        })

    @Test
    fun `service cleanup runs after late completion without a caller executor`() {
        val pending = CompletableFuture<Int>()
        var closed: Int? = null
        val firstCleanup = CountDownLatch(1)
        KagemushaOmapiDeviceLifecycleV1.closeServiceWhenReady(pending) {
            closed = it
            firstCleanup.countDown()
        }
        assertEquals(null, closed)
        pending.complete(7)
        assertTrue(firstCleanup.await(5, TimeUnit.SECONDS))
        assertEquals(7, closed)

        val alreadyCompleted = CompletableFuture.completedFuture(9)
        val secondCleanup = CountDownLatch(1)
        KagemushaOmapiDeviceLifecycleV1.closeServiceWhenReady(alreadyCompleted) {
            closed = it
            secondCleanup.countDown()
        }
        assertTrue(secondCleanup.await(5, TimeUnit.SECONDS))
        assertEquals(9, closed)
    }
}
