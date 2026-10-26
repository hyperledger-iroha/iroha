package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Local lifetime orderings only; these fixtures do not qualify Native admission or phones. */
class KagemushaWalletExchangeLifetimeV1Test {
    private fun CountDownLatch.ready() = assertTrue(await(10, TimeUnit.SECONDS))
    private fun waiting(thread: Thread) {
        val until = System.nanoTime() + TimeUnit.SECONDS.toNanos(10)
        while (thread.state != Thread.State.WAITING && System.nanoTime() < until) Thread.yield()
        assertEquals(Thread.State.WAITING, thread.state)
    }

    @Test fun `end waits for tasks accepted before enqueue and closes later admission`() {
        val ends = AtomicInteger()
        val lifetime = KagemushaWalletExchangeLifetimeV1 { ends.incrementAndGet(); KagemushaWalletExchangeEndV1.ENDED }
        val task = lifetime.admit()
        val result = AtomicReference<KagemushaWalletExchangeEndV1>()
        val caller = Thread { result.set(lifetime.end()) }
        try {
            lifetime.requestEnd()
            assertThrows(IllegalStateException::class.java) { lifetime.admit() }
            caller.start(); waiting(caller)
            assertEquals(0, ends.get())
            assertEquals(7, task.useCall { 7 })
            task.close(); task.close()
            caller.join(10_000); assertFalse(caller.isAlive)
            assertEquals(KagemushaWalletExchangeEndV1.ENDED, result.get())
            assertTrue(lifetime.released())
            assertEquals(KagemushaWalletExchangeEndV1.ALREADY_ENDED, lifetime.end())
            assertEquals(1, ends.get())
        } finally { task.close(); caller.join(10_000) }
    }

    @Test fun `finishing an active call cannot release it early`() {
        val releases = AtomicInteger()
        val task = KagemushaWalletExchangeTaskV1 { releases.incrementAndGet() }
        val entered = CountDownLatch(1); val release = CountDownLatch(1)
        val pool = Executors.newSingleThreadExecutor()
        try {
            val work = pool.submit<Int> { task.useCall { entered.countDown(); release.ready(); 9 } }
            entered.ready(); task.close()
            assertEquals(0, releases.get())
            assertThrows(IllegalStateException::class.java) { task.useCall { fail("late call") } }
            release.countDown(); assertEquals(9, work.get(10, TimeUnit.SECONDS))
            assertEquals(1, releases.get()); task.close(); assertEquals(1, releases.get())
        } finally { release.countDown(); pool.shutdownNow() }
    }

    @Test fun `concurrent end callers share one Native attempt and preserve interruption`() {
        val entered = CountDownLatch(1); val release = CountDownLatch(1)
        val ends = AtomicInteger(); val failures = AtomicReference<Throwable>()
        val lifetime = KagemushaWalletExchangeLifetimeV1 {
            ends.incrementAndGet(); entered.countDown(); release.ready(); KagemushaWalletExchangeEndV1.ENDED
        }
        val first = Thread { try { lifetime.end() } catch (error: Throwable) { failures.set(error) } }
        val second = Thread {
            try {
                assertEquals(KagemushaWalletExchangeEndV1.ENDED, lifetime.end())
                assertTrue(Thread.currentThread().isInterrupted)
            } catch (error: Throwable) { failures.set(error) }
        }
        try {
            first.start(); entered.ready(); second.start(); waiting(second); second.interrupt()
            release.countDown(); first.join(10_000); second.join(10_000)
            assertFalse(first.isAlive); assertFalse(second.isAlive)
            failures.get()?.let { throw it }; assertEquals(1, ends.get())
        } finally { release.countDown(); first.join(10_000); second.join(10_000) }
    }

    @Test fun `failed end keeps the same closure fenced for explicit retry`() {
        val attempts = AtomicInteger(); val expected = IllegalStateException("end failed")
        val lifetime = KagemushaWalletExchangeLifetimeV1 {
            if (attempts.incrementAndGet() == 1) throw expected
            KagemushaWalletExchangeEndV1.ENDED
        }
        assertSame(expected, assertThrows(IllegalStateException::class.java) { lifetime.end() })
        assertFalse(lifetime.released())
        assertThrows(IllegalStateException::class.java) { lifetime.admit() }
        assertEquals(KagemushaWalletExchangeEndV1.ENDED, lifetime.end())
        assertEquals(2, attempts.get())
    }

    @Test fun `Native Ending is nonterminal and can be queried again`() {
        val attempts = AtomicInteger()
        val lifetime = KagemushaWalletExchangeLifetimeV1 {
            if (attempts.incrementAndGet() == 1) KagemushaWalletExchangeEndV1.ENDING
            else KagemushaWalletExchangeEndV1.ALREADY_ENDED
        }
        assertEquals(KagemushaWalletExchangeEndV1.ENDING, lifetime.end()); assertFalse(lifetime.released())
        assertThrows(IllegalStateException::class.java) { lifetime.admit() }
        assertEquals(KagemushaWalletExchangeEndV1.ALREADY_ENDED, lifetime.end()); assertTrue(lifetime.released())
    }

    @Test fun `future cancellation fences callbacks but retains one already entered`() {
        val releases = AtomicInteger(); val task = KagemushaWalletExchangeTaskV1 { releases.incrementAndGet() }
        val pending = CompletableFuture<Int>(); val entered = CountDownLatch(1); val release = CountDownLatch(1)
        lateinit var continuation: KagemushaWalletExchangeTaskV1
        assertSame(pending, task.useFuture { continuation = it; pending })
        val pool = Executors.newSingleThreadExecutor()
        try {
            val callback = pool.submit<Int> { continuation.useCall { entered.countDown(); release.ready(); 4 } }
            entered.ready(); task.close(); assertTrue(pending.cancel(false)); assertEquals(0, releases.get())
            assertThrows(IllegalStateException::class.java) { continuation.useCall { fail("cancelled callback") } }
            release.countDown(); assertEquals(4, callback.get(10, TimeUnit.SECONDS)); assertEquals(1, releases.get())
        } finally { release.countDown(); pool.shutdownNow() }
    }

    @Test fun `future construction failure releases its retained use without ending caller task`() {
        val releases = AtomicInteger(); val task = KagemushaWalletExchangeTaskV1 { releases.incrementAndGet() }
        val expected = IllegalArgumentException("before future")
        assertSame(expected, assertThrows(IllegalArgumentException::class.java) {
            task.useFuture<Int> { throw expected }
        })
        assertEquals(0, releases.get()); assertEquals(3, task.useCall { 3 })
        task.close(); assertEquals(1, releases.get())
    }

    @Test fun `already completed future releases exactly once after parent finish`() {
        val releases = AtomicInteger(); val task = KagemushaWalletExchangeTaskV1 { releases.incrementAndGet() }
        assertEquals(8, task.useFuture { CompletableFuture.completedFuture(8) }.join())
        assertEquals(0, releases.get()); task.close(); task.close(); assertEquals(1, releases.get())
    }

    @Test fun `closed accepted call refuses before touching Native`() {
        val wallet = KagemushaWalletV1(1)
        val exchange = KagemushaWalletExchangeV1(wallet, 7) { KagemushaWalletExchangeEndV1.ENDED }
        val call = exchange.admit(); call.close()
        assertThrows(IllegalStateException::class.java) { call.bootstrap() }
        assertEquals(KagemushaWalletExchangeEndV1.ENDED, exchange.end())
        assertThrows(IllegalStateException::class.java) { exchange.admit() }
    }

    @Test fun `ambiguous consumed cleanup retires exact owner and preserves failure`() {
        val primary = IllegalStateException("discard outcome unknown")
        val cleanup = IllegalStateException("owner close failed")
        var closes = 0
        val actual = assertThrows(IllegalStateException::class.java) {
            KagemushaWalletExchangeCleanupV1.run({ closes++; throw cleanup }) { throw primary }
        }
        assertSame(primary, actual); assertEquals(1, closes)
        assertArrayEquals(arrayOf(cleanup), primary.suppressed)
    }

    @Test fun `acknowledged cleanup leaves owner open and preserves result`() {
        var closes = 0
        assertEquals(5, KagemushaWalletExchangeCleanupV1.run({ closes++ }) { 5 })
        assertEquals(0, closes)
    }
}
