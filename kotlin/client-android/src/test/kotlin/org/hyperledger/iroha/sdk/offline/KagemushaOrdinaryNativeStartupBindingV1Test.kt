package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Exact production metadata/monitor path with an inert src/test owner, never actual JNI. */
class KagemushaOrdinaryNativeStartupBindingV1Test {
    @Test fun callerSelectedStartupIsRejectedBeforeDescriptorReadOrRevocation() {
        val core = CoreEndpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
        val binding = KagemushaOrdinaryCurrentControlTransportBindingV1(bridge)
        var calls = 0
        val offered = KagemushaOrdinaryNativeStartupEndpointV1 { _, _, _ -> calls++; first() }
        for (phase in listOf(1, 6)) {
            val id = if (phase == 1) 0L else 17L
            assertFailsWith<IllegalArgumentException> { binding.invokeStartup(offered, phase, id) }
            assertFailsWith<IllegalArgumentException> { bridge.invokeOrdinaryStartup(offered, phase, id) }
        }
        assertEquals(0, calls); assertEquals(0, core.closes.get()); binding.requireOpen()
        bridge.close()
        // A closed descriptor still refuses foreign metadata first, without exposing its state.
        assertFailsWith<IllegalArgumentException> { binding.invokeStartup(offered, 1) }
        assertEquals(0, calls); assertEquals(1, core.closes.get())
    }

    @Test fun sameNamedOwnerInAnotherLoaderFailsTheActualDispatchMetadataPredicate() {
        val name = KagemushaOrdinaryRuntimeJniV1.javaClass.name
        requireOrdinaryRuntimeJniOwnerClassV1(KagemushaOrdinaryRuntimeJniV1.javaClass)
        val loader = object : ClassLoader(javaClass.classLoader) {
            override fun loadClass(className: String, resolve: Boolean): Class<*> {
                if (className != name) return super.loadClass(className, resolve)
                val type = findLoadedClass(className) ?: run {
                    val bytes = checkNotNull(parent.getResourceAsStream(className.replace('.', '/') + ".class")).use { it.readBytes() }
                    defineClass(className, bytes, 0, bytes.size)
                }
                if (resolve) resolveClass(type)
                return type
            }
        }
        // Class loading alone does not initialize or construct an endpoint. Real dispatch
        // uses this exact pure predicate before reading any coordinator descriptor.
        val offeredClass = loader.loadClass(name)
        assertEquals(name, offeredClass.name)
        assertTrue((offeredClass.modifiers and 0x0010) != 0)
        assertNotSame(KagemushaCoreCoordinatorBridgeV1::class.java.classLoader, offeredClass.classLoader)
        assertFailsWith<IllegalArgumentException> { requireOrdinaryRuntimeJniOwnerClassV1(offeredClass) }
    }

    @Test fun startupGrammarAndBoundedRepliesAreDetachedAndRejectSubstitution() {
        val core = CoreEndpoint()
        val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
        val binding = KagemushaOrdinaryCurrentControlTransportBindingV1(bridge)
        var calls = 0
        val raw = first()
        KagemushaOrdinaryRuntimeJniV1.startupScript = { phase, id, original ->
            calls++; assertTrue(Thread.holdsLock(bridge)); assertTrue(original.isEmpty())
            if (phase == 1) { assertEquals(0L, id); raw } else { assertEquals(17L, id); finished() }
        }
        val retained = binding.invokeStartup(KagemushaOrdinaryRuntimeJniV1, 1)
        raw[3].fill(0); raw[4].fill(0)
        assertContentEquals(ByteArray(32) { 2 }, retained[3]); assertContentEquals(byteArrayOf(3), retained[4])
        assertEquals(3, binding.invokeStartup(KagemushaOrdinaryRuntimeJniV1, 6, 17L).size)
        for ((phase, id) in listOf(0 to 0L, 1 to 1L, 5 to 0L, 6 to 0L, 7 to 17L)) {
            assertFailsWith<IllegalArgumentException> { binding.invokeStartup(KagemushaOrdinaryRuntimeJniV1, phase, id) }
            assertFailsWith<IllegalArgumentException> { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, phase, id) }
        }
        assertEquals(2, calls); assertEquals(0, core.closes.get()); bridge.close()
        val malformed = listOf(
            first().take(5).toTypedArray(), first() + byteArrayOf(9),
            first().also { it[0] = byteArrayOf(2, 0) }, first().also { it[1] = byteArrayOf(6) },
            first().also { it[2] = ByteArray(8) }, first().also { it[2] = byteArrayOf(17) },
            first().also { it[3] = ByteArray(32) }, first().also { it[3] = ByteArray(31) },
            first().also { it[4] = ByteArray(0) }, first().also { it[5] = ByteArray(4097) })
        for (response in malformed) assertFails { ordinaryNativeStartupResponseFieldsV1(1, response) }
        for (response in listOf(finished() + byteArrayOf(9), finished().also { it[1] = byteArrayOf(1) },
            finished().also { it[2] = ByteArray(8) })) assertFails { ordinaryNativeStartupResponseFieldsV1(6, response) }
    }

    @Test fun uncertainStartupReplyRevokesCoreAndNeverRetriesNative() {
        val responses = listOf<Array<ByteArray>?>(null, first().also { it[2] = ByteArray(8) })
        for (reply in responses) {
            val core = CoreEndpoint(); val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
            var calls = 0
            KagemushaOrdinaryRuntimeJniV1.startupScript = { _, _, _ -> calls++; reply }
            assertFails { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, 1, 0L) }
            assertFailsWith<IllegalStateException> { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, 1, 0L) }
            assertEquals(1, calls); assertEquals(1, core.closes.get())
        }
        val core = CoreEndpoint(); val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
        KagemushaOrdinaryRuntimeJniV1.startupScript = { _, _, _ -> throw UnsatisfiedLinkError("inert missing Native") }
        assertFailsWith<IllegalStateException> { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, 1, 0L) }
        assertEquals(1, core.closes.get()); assertFails { bridge.requireOrdinaryDescriptorOpen() }
    }

    @Test fun closeCannotPassAnAlreadyDispatchedStartupOnTheSameMonitor() {
        val core = CoreEndpoint(); val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
        val entered = CountDownLatch(1); val release = CountDownLatch(1); val closing = CountDownLatch(1)
        val startupError = AtomicReference<Throwable?>(); val closeError = AtomicReference<Throwable?>()
        KagemushaOrdinaryRuntimeJniV1.startupScript = { _, _, _ ->
            assertTrue(Thread.holdsLock(bridge)); entered.countDown(); check(release.await(5, TimeUnit.SECONDS)); first()
        }
        val startup = Thread { try { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, 1, 0L) } catch (error: Throwable) { startupError.set(error) } }
        val close = Thread { closing.countDown(); try { bridge.close() } catch (error: Throwable) { closeError.set(error) } }
        startup.start()
        try {
            assertTrue(entered.await(5, TimeUnit.SECONDS)); close.start(); assertTrue(closing.await(5, TimeUnit.SECONDS))
            awaitBlocked(close); assertEquals(0, core.closes.get())
        } finally { release.countDown(); startup.join(5000); if (close.state != Thread.State.NEW) close.join(5000) }
        assertFalse(startup.isAlive); assertFalse(close.isAlive); assertNull(startupError.get()); assertNull(closeError.get())
        assertEquals(1, core.closes.get()); assertFails { bridge.requireOrdinaryDescriptorOpen() }
    }

    @Test fun queuedStartupAfterCloseNeverDispatchesEvenWithCanonicalOwnerMetadata() {
        val core = CoreEndpoint(); val bridge = KagemushaCoreCoordinatorBridgeV1.openEndpoint("/fixture/startup", core)
        val started = CountDownLatch(1); val calls = AtomicInteger(); val failure = AtomicReference<Throwable?>()
        KagemushaOrdinaryRuntimeJniV1.startupScript = { _, _, _ -> calls.incrementAndGet(); first() }
        val queued = Thread { started.countDown(); try { bridge.invokeOrdinaryStartup(KagemushaOrdinaryRuntimeJniV1, 1, 0L) } catch (error: Throwable) { failure.set(error) } }
        synchronized(bridge) {
            queued.start(); assertTrue(started.await(5, TimeUnit.SECONDS)); awaitBlocked(queued)
            bridge.close() // Reentrant monitor: Native/Core close commits before queued startup.
        }
        queued.join(5000); assertFalse(queued.isAlive); assertIs<IllegalStateException>(failure.get())
        assertEquals(0, calls.get()); assertEquals(1, core.closes.get())
    }

    private fun awaitBlocked(thread: Thread) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (thread.state != Thread.State.BLOCKED && thread.isAlive && System.nanoTime() < deadline) Thread.yield()
        assertEquals(Thread.State.BLOCKED, thread.state)
    }
    private class CoreEndpoint : KagemushaCoreCoordinatorEndpointV1 {
        val closes = AtomicInteger()
        override fun contract() = intArrayOf(2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21)
        override fun install(storagePath: String) = 0
        override fun open(storagePath: String) = 23L
        override fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? = error("No generic Native dispatch")
        override fun close(handle: Long): Int { assertEquals(23L, handle); closes.incrementAndGet(); return 0 }
    }
    companion object {
        private fun le(id: Long) = ByteArray(8) { (id ushr (it * 8)).toByte() }
        private fun first() = arrayOf(byteArrayOf(1, 0), byteArrayOf(1), le(17), ByteArray(32) { 2 }, byteArrayOf(3), byteArrayOf(4))
        private fun finished() = arrayOf(byteArrayOf(1, 0), byteArrayOf(6), le(19))
    }
}
