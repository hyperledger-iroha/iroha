// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.TimeUnit
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue

/** Ordering/ownership controls only; these do not qualify Android lifecycle or Native folding. */
class KagemushaProbeActivityControlTest {
    private class Queue : Executor {
        private val tasks = LinkedBlockingQueue<Runnable>()
        var reject = false
        override fun execute(command: Runnable) {
            if (reject) throw RejectedExecutionException("control-rejected")
            tasks.add(command)
        }
        fun next() { requireNotNull(tasks.poll(2, TimeUnit.SECONDS)) { "missing control task" }.run() }
        fun empty() = tasks.isEmpty()
    }
    private class Observation(
        val initialForeground: Boolean = false,
        val initialCharging: Boolean = false,
    ) : KagemushaProbeActivityObservation {
        lateinit var emit: (Boolean, Boolean) -> Unit
        lateinit var fail: (Throwable) -> Unit
        val subscribed = CountDownLatch(1)
        val detached = CountDownLatch(1)
        var startFailure: Throwable? = null
        var detachFailure: Throwable? = null
        var detaches = 0
        var sample = true
        override fun start(changed: (Boolean, Boolean) -> Unit, failed: (Throwable) -> Unit) {
            emit = changed; fail = failed
            if (sample) changed(initialForeground, initialCharging)
            subscribed.countDown()
            startFailure?.let { throw it }
        }
        override fun detach() {
            detaches++; detached.countDown(); detachFailure?.let { throw it }
        }
    }
    private class Fixture(val observation: Observation = Observation(), val queue: Queue = Queue(), generation: Long = 7) {
        val publications = mutableListOf<Pair<Boolean, Boolean>>()
        val closes = mutableListOf<Boolean>()
        var shutdowns = 0
        var publicationFailure: Throwable? = null
        var closeFailure: Throwable? = null
        val control = KagemushaProbeActivityControl(generation, queue, observation,
            { foreground, charging ->
                publications.add(foreground to charging)
                publicationFailure?.let { throw it }
            }, { retry -> closes.add(retry); closeFailure?.let { throw it } }, { shutdowns++ })
        fun start() {
            val result = async { control.start() }
            assertTrue(observation.subscribed.await(2, TimeUnit.SECONDS))
            queue.next(); await(result)
        }
        fun close(): CompletableFuture<Unit> = async { control.close() }.also {
            assertTrue(observation.detached.await(2, TimeUnit.SECONDS))
        }
    }
    companion object {
        private fun async(action: () -> Unit): CompletableFuture<Unit> {
            val result = CompletableFuture<Unit>()
            Thread {
                try { action(); result.complete(Unit) }
                catch (problem: Throwable) { result.completeExceptionally(problem) }
            }.apply { isDaemon = true }.start()
            return result
        }
        private fun await(value: CompletableFuture<Unit>) {
            try { value.get(2, TimeUnit.SECONDS) }
            catch (problem: ExecutionException) { throw problem.cause ?: problem }
        }
    }

    @Test fun initialPublicationIsQueuedAndAcknowledgedBeforeCommands() {
        val f = Fixture(Observation(true, false))
        val opened = async { f.control.start() }
        assertTrue(f.observation.subscribed.await(2, TimeUnit.SECONDS))
        assertFalse(opened.isDone)
        assertTrue(f.publications.isEmpty())
        assertFailsWith<IllegalStateException> { f.control.checkHealthy() }
        f.queue.next(); await(opened)
        f.control.checkHealthy()
        assertEquals(listOf(true to false), f.publications)
        val closed = f.close(); f.queue.next(); await(closed)
        assertEquals(listOf(false), f.closes)
        assertTrue(f.control.released)
    }

    @Test fun completePairsPreserveBothAxesInCallbackOrderIncludingNoVisibleActivity() {
        val f = Fixture(); f.start()
        f.observation.emit(true, false)
        f.observation.emit(true, true)
        f.observation.emit(false, true)
        f.observation.emit(false, false)
        assertEquals(listOf(false to false), f.publications)
        repeat(4) { f.queue.next() }
        assertEquals(listOf(false to false, true to false, true to true, false to true, false to false), f.publications)
        val closed = f.close(); f.queue.next(); await(closed)
    }

    @Test fun retirementDiscardsQueuedEnablingAndFinishesInitialWaiter() {
        val f = Fixture(Observation(true, true))
        val opened = async { f.control.start() }
        assertTrue(f.observation.subscribed.await(2, TimeUnit.SECONDS))
        f.control.retire()
        assertFailsWith<IllegalStateException> { await(opened) }
        val closed = f.close()
        f.queue.next() // Dropped initial enabling; its waiter has already completed.
        f.queue.next() // Final inactivity is the only published pair.
        assertFailsWith<IllegalStateException> { await(closed) }
        assertEquals(listOf(false to false), f.publications)
        assertTrue(f.control.released)
    }

    @Test fun staleGenerationAndCallbacksCannotAffectReopenedOwner() {
        val old = Fixture(); old.start()
        old.control.changed(8, true, true)
        assertTrue(old.queue.empty())
        val oldCallback = old.observation.emit
        val closed = old.close(); old.queue.next(); await(closed)
        val replacement = Fixture(Observation(false, true), generation = 8); replacement.start()
        oldCallback(true, true)
        old.control.changed(7, true, true)
        assertTrue(old.queue.empty())
        assertEquals(listOf(false to true), replacement.publications)
        assertEquals(listOf(false to false, false to false), old.publications)
        val replaced = replacement.close(); replacement.queue.next(); await(replaced)
    }

    @Test fun stickyPublicationFailureStillDrainsAndClosesExactOwner() {
        val f = Fixture(); val problem = IllegalArgumentException("publication")
        f.publicationFailure = problem
        val opened = async { f.control.start() }
        assertTrue(f.observation.subscribed.await(2, TimeUnit.SECONDS)); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalArgumentException> { await(opened) })
        val closed = f.close(); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalArgumentException> { await(closed) })
        assertEquals(listOf(false), f.closes)
        assertTrue(f.control.released)
        assertSame(problem, assertFailsWith<IllegalArgumentException> { f.control.checkHealthy() })
    }

    @Test fun failureOnLaterCallbackRemainsVisibleAfterSuccessfulCleanup() {
        val f = Fixture(); f.start()
        val problem = IllegalStateException("observation")
        f.observation.fail(problem)
        f.observation.emit(true, true)
        assertTrue(f.queue.empty())
        assertSame(problem, assertFailsWith<IllegalStateException> { f.control.checkHealthy() })
        val closed = f.close(); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalStateException> { await(closed) })
        assertTrue(f.control.released)
        assertEquals(1, f.shutdowns)
    }

    @Test fun partialRegistrationFailureRetainsCleanupAndDiscardsQueuedSample() {
        val observation = Observation(true, false)
        val problem = IllegalStateException("partial-registration")
        observation.startFailure = problem
        val f = Fixture(observation)
        assertSame(problem, assertFailsWith<IllegalStateException> { f.control.start() })
        val closed = f.close(); f.queue.next(); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalStateException> { await(closed) })
        assertEquals(listOf(false to false), f.publications)
        assertEquals(1, observation.detaches)
        assertTrue(f.control.released)
    }

    @Test fun missingInitialSampleFailsWithoutInventingForeground() {
        val observation = Observation(); observation.sample = false
        val f = Fixture(observation)
        assertFailsWith<IllegalStateException> { f.control.start() }
        assertTrue(f.publications.isEmpty())
        val closed = f.close(); f.queue.next()
        assertFailsWith<IllegalStateException> { await(closed) }
        assertEquals(listOf(false to false), f.publications)
        assertTrue(f.control.released)
    }

    @Test fun detachFailureCannotPreventNativeCloseAndRetainsObserverRetry() {
        val f = Fixture(); f.start()
        val problem = IllegalArgumentException("detach")
        f.observation.detachFailure = problem
        val closed = f.close(); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalArgumentException> { await(closed) })
        assertEquals(listOf(false), f.closes)
        assertFalse(f.control.released)
        assertEquals(0, f.shutdowns)
        f.observation.detachFailure = null
        assertSame(problem, assertFailsWith<IllegalArgumentException> { f.control.close() })
        assertEquals(listOf(false), f.closes)
        assertEquals(2, f.publications.size)
        assertTrue(f.control.released)
        assertEquals(1, f.shutdowns)
    }

    @Test fun nativeCloseFailureRetriesExactOwnerWithoutRepublishingActivity() {
        val f = Fixture(); f.start()
        val problem = IllegalStateException("native-close")
        f.closeFailure = problem
        val closed = f.close(); f.queue.next()
        assertSame(problem, assertFailsWith<IllegalStateException> { await(closed) })
        assertFalse(f.control.released)
        assertSame(problem, assertFailsWith<IllegalStateException> { f.control.checkHealthy() })
        f.closeFailure = null
        assertSame(problem, assertFailsWith<IllegalStateException> { f.control.close() })
        assertEquals(listOf(false, true), f.closes)
        assertEquals(2, f.publications.size)
        assertTrue(f.control.released)
    }

    @Test fun distinctCleanupFailuresAreRetainedAndUnfencedExecutorCannotCloseOwner() {
        val f = Fixture(); f.start()
        val observationFailure = IllegalStateException("observation")
        f.observation.fail(observationFailure)
        f.queue.reject = true
        assertSame(observationFailure, assertFailsWith<IllegalStateException> { f.control.close() })
        assertTrue(observationFailure.suppressed.any { it is RejectedExecutionException })
        assertTrue(f.closes.isEmpty())
        assertFalse(f.control.released)
        f.queue.reject = false
        val closed = f.close(); f.queue.next()
        assertSame(observationFailure, assertFailsWith<IllegalStateException> { await(closed) })
        assertTrue(f.control.released)
    }

    @Test fun inactivityReachesIndependentControlWhileMonetaryCommandIsBlocked() {
        val executor = Executors.newSingleThreadExecutor()
        val observation = Observation(true, false)
        val commandEntered = CountDownLatch(1)
        val commandRelease = CountDownLatch(1)
        val inactive = CountDownLatch(1)
        val callbackThread = Thread.currentThread()
        val control = KagemushaProbeActivityControl(1, executor, observation,
            { foreground, charging ->
                assertFalse(Thread.currentThread() === callbackThread)
                if (!foreground && !charging) inactive.countDown()
            }, {}, executor::shutdown)
        try {
            control.start()
            val command = async { commandEntered.countDown(); check(commandRelease.await(10, TimeUnit.SECONDS)) }
            assertTrue(commandEntered.await(2, TimeUnit.SECONDS))
            observation.emit(false, false)
            assertTrue(inactive.await(2, TimeUnit.SECONDS))
            assertFalse(command.isDone)
            commandRelease.countDown(); await(command)
            control.close()
        } finally { commandRelease.countDown(); executor.shutdownNow() }
    }

    @Test fun interruptedCleanupStillFencesBeforeNativeCloseAndRestoresInterrupt() {
        val f = Fixture(); f.start()
        val interrupted = CompletableFuture<Boolean>()
        val closed = async {
            Thread.currentThread().interrupt()
            f.control.close()
            interrupted.complete(Thread.currentThread().isInterrupted)
        }
        assertTrue(f.observation.detached.await(2, TimeUnit.SECONDS))
        assertTrue(f.closes.isEmpty())
        f.queue.next(); await(closed)
        assertTrue(interrupted.get(2, TimeUnit.SECONDS))
        assertEquals(listOf(false), f.closes)
        assertTrue(f.control.released)
    }

    @Test fun finalFenceWaitsForAdmittedPublicationBeforeNativeClose() {
        val executor = Executors.newSingleThreadExecutor()
        val observation = Observation(true, false)
        val publicationEntered = CountDownLatch(1)
        val publicationRelease = CountDownLatch(1)
        val nativeClosed = CountDownLatch(1)
        val control = KagemushaProbeActivityControl(1, executor, observation,
            { foreground, _ ->
                if (foreground) {
                    publicationEntered.countDown()
                    check(publicationRelease.await(10, TimeUnit.SECONDS))
                }
            }, { nativeClosed.countDown() }, executor::shutdown)
        try {
            val opened = async { control.start() }
            assertTrue(publicationEntered.await(2, TimeUnit.SECONDS))
            val closed = async { control.close() }
            assertTrue(observation.detached.await(2, TimeUnit.SECONDS))
            assertFailsWith<IllegalStateException> { await(opened) }
            assertEquals(1L, nativeClosed.count)
            assertFalse(closed.isDone)
            publicationRelease.countDown()
            assertFailsWith<IllegalStateException> { await(closed) }
            assertEquals(0L, nativeClosed.count)
            assertTrue(control.released)
        } finally { publicationRelease.countDown(); executor.shutdownNow() }
    }
}
