package org.hyperledger.iroha.sdk.offline

import com.google.android.gms.nearby.connection.PayloadTransferUpdate
import org.junit.jupiter.api.Test
import java.util.concurrent.Executor
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledThreadPoolExecutor
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import kotlin.test.assertFailsWith

class IrohaPeerNearbyConnectionsReducerV1Test {
    @Test
    fun `default adapter bounds state callbacks records and transcript budget`() {
        val configuration = IrohaPeerNearbyConnectionsConfigurationV1()
        assertEquals(64, configuration.maximumPendingWorkerActions)
        assertEquals(16, configuration.maximumPendingCallbacks)
        assertEquals(4, configuration.maximumPendingReceiveCallbacks)
        assertEquals(4, configuration.maximumReceiveRecordsPerConnection)
        assertEquals(8, configuration.maximumPendingSends)
        assertEquals(
            300_000,
            IrohaPeerNearbyConnectionsConfigurationV1(operationTimeoutMillis = 300_000)
                .operationTimeoutMillis,
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNearbyConnectionsConfigurationV1(operationTimeoutMillis = 300_001)
        }
    }

    @Test
    fun `callback epoch gate suppresses stop restart stale delivery`() {
        val gate = IrohaPeerNearbyCallbackEpochGateV1(initialEpoch = 7)
        val delivered = mutableListOf<String>()
        val delayedOldOperation = {
            gate.performIfCurrent(7) {
                delivered.add("old")
            }
        }

        gate.update(8)
        assertFalse(delayedOldOperation())
        assertTrue(delivered.isEmpty())
        assertTrue(gate.performIfCurrent(8) { delivered.add("new") })
        assertEquals(listOf("new"), delivered)
    }

    @Test
    fun `callback epoch gate invalidates without waiting for application code`() {
        val gate = IrohaPeerNearbyCallbackEpochGateV1(initialEpoch = 11)
        val callbackEntered = CountDownLatch(1)
        val releaseCallback = CountDownLatch(1)
        val callbackFinished = CountDownLatch(1)
        val updateStarted = CountDownLatch(1)
        val updateFinished = CountDownLatch(1)

        val callbackThread = thread(start = true) {
            gate.performIfCurrent(11) {
                callbackEntered.countDown()
                releaseCallback.await()
            }
            callbackFinished.countDown()
        }
        assertTrue(callbackEntered.await(1, TimeUnit.SECONDS))

        val updateThread = thread(start = true) {
            updateStarted.countDown()
            gate.update(12)
            updateFinished.countDown()
        }
        assertTrue(updateStarted.await(1, TimeUnit.SECONDS))
        assertTrue(updateFinished.await(1, TimeUnit.SECONDS))
        assertFalse(gate.performIfCurrent(11) { })
        assertTrue(gate.performIfCurrent(12) { })

        releaseCallback.countDown()
        assertTrue(callbackFinished.await(1, TimeUnit.SECONDS))
        callbackThread.join(1_000)
        updateThread.join(1_000)
        assertFalse(callbackThread.isAlive)
        assertFalse(updateThread.isAlive)
    }

    @Test
    fun `repeated and conflicting starts preserve the live operation`() {
        val receiver = discoveryContext(IrohaPeerNearbyRoleV1.RECEIVER, 1)
        val otherReceiver = discoveryContext(IrohaPeerNearbyRoleV1.RECEIVER, 2)

        assertEquals(
            IrohaPeerNearbyStartDecisionV1.START,
            IrohaPeerNearbyConnectionsReducerV1.decideStart(
                null,
                null,
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                receiver,
            ),
        )
        assertEquals(
            IrohaPeerNearbyStartDecisionV1.KEEP_ACTIVE_REPLAY,
            IrohaPeerNearbyConnectionsReducerV1.decideStart(
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                receiver,
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                receiver,
            ),
        )
        assertEquals(
            IrohaPeerNearbyStartDecisionV1.KEEP_ACTIVE_CONFLICT,
            IrohaPeerNearbyConnectionsReducerV1.decideStart(
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                receiver,
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                otherReceiver,
            ),
        )
        assertEquals(
            IrohaPeerNearbyStartDecisionV1.KEEP_ACTIVE_CONFLICT,
            IrohaPeerNearbyConnectionsReducerV1.decideStart(
                IrohaPeerNearbyConnectionsModeV1.ADVERTISING,
                receiver,
                IrohaPeerNearbyConnectionsModeV1.DISCOVERING,
                IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
                    IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                ),
            ),
        )
    }

    @Test
    fun `only terminal framework success crosses the delivery barrier`() {
        assertEquals(
            IrohaPeerNearbyTransferDecisionV1.IGNORE,
            IrohaPeerNearbyConnectionsReducerV1.decideTransfer(
                PayloadTransferUpdate.Status.IN_PROGRESS,
            ),
        )
        assertEquals(
            IrohaPeerNearbyTransferDecisionV1.IGNORE,
            IrohaPeerNearbyConnectionsReducerV1.decideTransfer(Int.MAX_VALUE),
        )
        assertEquals(
            IrohaPeerNearbyTransferDecisionV1.FAILURE,
            IrohaPeerNearbyConnectionsReducerV1.decideTransfer(
                PayloadTransferUpdate.Status.FAILURE,
            ),
        )
        assertEquals(
            IrohaPeerNearbyTransferDecisionV1.FAILURE,
            IrohaPeerNearbyConnectionsReducerV1.decideTransfer(
                PayloadTransferUpdate.Status.CANCELED,
            ),
        )
        assertEquals(
            IrohaPeerNearbyTransferDecisionV1.SUCCESS,
            IrohaPeerNearbyConnectionsReducerV1.decideTransfer(
                PayloadTransferUpdate.Status.SUCCESS,
            ),
        )
    }

    @Test
    fun `duplicate payload id cannot replace a live completion`() {
        val deliveries = IrohaPeerNearbyPendingDeliveryRegistryV1<String>()
        assertTrue(deliveries.add(7, "first"))
        assertFalse(deliveries.add(7, "replacement"))
        assertEquals(1, deliveries.size)
        assertSame("first", deliveries.removeIf(7) { true })
        assertEquals(0, deliveries.size)
    }

    @Test
    fun `action pump owns one runner across clear and replacement work`() {
        val scheduled = mutableListOf<() -> Unit>()
        val delivered = mutableListOf<String>()
        val pump = IrohaPeerNearbySerialActionPumpV1(4) { action ->
            scheduled += action
            true
        }
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            delivered += "old"
        })
        pump.clear()
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            delivered += "new"
        })
        assertEquals(1, scheduled.size)
        scheduled.single().invoke()
        assertEquals(listOf("new"), delivered)
        assertEquals(0, pump.pendingCount)
    }

    @Test
    fun `dequeued action invalidated before lifecycle lock drops exactly once`() {
        val generation = IrohaPeerNearbyWorkGenerationV1(3)
        val scheduled = mutableListOf<() -> Unit>()
        val pump = IrohaPeerNearbySerialActionPumpV1(2) {
            scheduled += it
            true
        }
        val wrapperEntered = CountDownLatch(1)
        val resumeWrapper = CountDownLatch(1)
        var drops = 0
        val dropOnce = IrohaPeerNearbyDropOnceV1 { drops += 1 }
        val capturedGeneration = generation.current()
        assertEquals(
            IrohaPeerNearbyActionAdmissionV1.ACCEPTED,
            pump.enqueue(onDropped = dropOnce::perform) {
                wrapperEntered.countDown()
                resumeWrapper.await()
                if (!generation.isCurrent(capturedGeneration)) dropOnce.perform()
            },
        )
        val runner = thread { scheduled.single().invoke() }
        assertTrue(wrapperEntered.await(1, TimeUnit.SECONDS))
        generation.invalidate()
        pump.clear().forEach { it() }
        resumeWrapper.countDown()
        runner.join(1_000)
        assertFalse(runner.isAlive)
        assertEquals(1, drops)
    }

    @Test
    fun `two producers both observe a rejecting worker`() {
        val schedulerEntered = CountDownLatch(1)
        val releaseScheduler = CountDownLatch(1)
        val results = mutableListOf<IrohaPeerNearbyActionAdmissionV1>()
        val resultsLock = Any()
        val pump = IrohaPeerNearbySerialActionPumpV1(4) {
            schedulerEntered.countDown()
            releaseScheduler.await()
            false
        }
        val first = thread {
            val result = pump.enqueue { }
            synchronized(resultsLock) { results += result }
        }
        assertTrue(schedulerEntered.await(1, TimeUnit.SECONDS))
        val second = thread {
            val result = pump.enqueue { }
            synchronized(resultsLock) { results += result }
        }
        releaseScheduler.countDown()
        first.join(1_000)
        second.join(1_000)
        assertFalse(first.isAlive)
        assertFalse(second.isAlive)
        assertEquals(
            listOf(
                IrohaPeerNearbyActionAdmissionV1.SCHEDULER_REJECTED,
                IrohaPeerNearbyActionAdmissionV1.SCHEDULER_REJECTED,
            ),
            synchronized(resultsLock) { results.toList() },
        )
    }

    @Test
    fun `queued event saturation stop and late success preserve control order`() {
        val generation = IrohaPeerNearbyWorkGenerationV1(7)
        val scheduled = mutableListOf<() -> Unit>()
        val delivered = mutableListOf<String>()
        val pump = IrohaPeerNearbySerialActionPumpV1(1) {
            scheduled += it
            true
        }
        val eventGeneration = generation.current()
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            if (generation.isCurrent(eventGeneration)) delivered += "A"
        })
        assertEquals(IrohaPeerNearbyActionAdmissionV1.FULL, pump.enqueue {
            delivered += "overflow"
        })

        generation.invalidate()
        pump.clear()
        delivered += "stop-B"
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            if (generation.isCurrent(eventGeneration)) delivered += "late-success-C"
        })
        assertEquals(1, scheduled.size)
        scheduled.single().invoke()
        assertEquals(listOf("stop-B"), delivered)
    }

    @Test
    fun `receive flood retains two records and one callback runner`() {
        val scheduled = mutableListOf<() -> Unit>()
        val delivered = mutableListOf<Int>()
        val pump = IrohaPeerNearbyReceiveCallbackPumpV1(2, 4) {
            scheduled += it
            true
        }
        pump.activate(7, "peer")
        val admissions = (0 until 100).map { value ->
            pump.enqueue(7, "peer") { delivered += value }
        }
        assertEquals(2, admissions.count { it == IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED })
        assertEquals(98, admissions.count { it == IrohaPeerNearbyReceiveAdmissionV1.FULL })
        assertEquals(2, pump.pendingCount)
        assertEquals(1, scheduled.size)

        pump.deactivate()
        pump.activate(8, "new")
        assertEquals(
            IrohaPeerNearbyReceiveAdmissionV1.INACTIVE,
            pump.enqueue(7, "peer") { delivered += -1 },
        )
        assertEquals(
            IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED,
            pump.enqueue(8, "new") { delivered += 8 },
        )
        scheduled.single().invoke()
        assertEquals(listOf(8), delivered)
    }

    @Test
    fun `receive transcript admits four sequential records and rejects fifth`() {
        val scheduled = mutableListOf<() -> Unit>()
        val delivered = mutableListOf<Int>()
        val pump = IrohaPeerNearbyReceiveCallbackPumpV1(2, 4) {
            scheduled += it
            true
        }
        pump.activate(4, "peer")
        assertEquals(IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED, pump.enqueue(4, "peer") {
            delivered += 1
        })
        assertEquals(IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED, pump.enqueue(4, "peer") {
            delivered += 2
        })
        scheduled.removeFirst().invoke()
        assertEquals(IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED, pump.enqueue(4, "peer") {
            delivered += 3
        })
        assertEquals(IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED, pump.enqueue(4, "peer") {
            delivered += 4
        })
        scheduled.removeFirst().invoke()
        assertEquals(
            IrohaPeerNearbyReceiveAdmissionV1.BUDGET_EXCEEDED,
            pump.enqueue(4, "peer") { delivered += 5 },
        )
        assertEquals(listOf(1, 2, 3, 4), delivered)
    }

    @Test
    fun `suspended default receive executor admits four records only`() {
        val configuration = IrohaPeerNearbyConnectionsConfigurationV1()
        val scheduled = mutableListOf<() -> Unit>()
        val pump = IrohaPeerNearbyReceiveCallbackPumpV1(
            configuration.maximumPendingReceiveCallbacks,
            configuration.maximumReceiveRecordsPerConnection,
        ) {
            scheduled += it
            true
        }
        pump.activate(5, "peer")

        repeat(4) {
            assertEquals(
                IrohaPeerNearbyReceiveAdmissionV1.ACCEPTED,
                pump.enqueue(5, "peer") { },
            )
        }
        assertEquals(
            IrohaPeerNearbyReceiveAdmissionV1.BUDGET_EXCEEDED,
            pump.enqueue(5, "peer") { },
        )
        assertEquals(4, pump.pendingCount)
        assertEquals(1, scheduled.size)
        while (scheduled.isNotEmpty()) scheduled.removeFirst().invoke()
        assertEquals(0, pump.pendingCount)
    }

    @Test
    fun `rejecting callback executor suppresses listener but completes exactly once`() {
        val rejecting = Executor { throw RejectedExecutionException("test") }
        var fallbackCount = 0
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            rejecting,
            Executor { it.run() },
            1,
            1,
        )
        assertFalse(dispatcher.execute { throw AssertionError("listener ran") })
        dispatcher.executeCritical { fallbackCount += 1 }
        assertEquals(1, fallbackCount)
        assertEquals(0, dispatcher.pendingCount)
    }

    @Test
    fun `throwing listener cannot strand the callback drain or next completion`() {
        val delivered = mutableListOf<String>()
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            Executor { it.run() },
            Executor { it.run() },
            2,
            2,
        )

        assertTrue(dispatcher.execute {
            delivered += "throwing-listener"
            throw AssertionError("application listener failure")
        })
        dispatcher.executeCritical { delivered += "completion" }

        assertEquals(listOf("throwing-listener", "completion"), delivered)
        assertEquals(0, dispatcher.pendingCount)
    }

    @Test
    fun `callback rejection is linearized for concurrent essential admission`() {
        val executorEntered = CountDownLatch(1)
        val releaseExecutor = CountDownLatch(1)
        val secondReturned = CountDownLatch(1)
        val results = mutableListOf<Boolean>()
        val resultLock = Any()
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            Executor {
                executorEntered.countDown()
                releaseExecutor.await()
                throw RejectedExecutionException("test")
            },
            Executor { it.run() },
            2,
            2,
        )
        val first = thread {
            val result = dispatcher.execute { }
            synchronized(resultLock) { results += result }
        }
        assertTrue(executorEntered.await(1, TimeUnit.SECONDS))
        val second = thread {
            val result = dispatcher.execute { }
            synchronized(resultLock) { results += result }
            secondReturned.countDown()
        }
        assertFalse(secondReturned.await(50, TimeUnit.MILLISECONDS))
        releaseExecutor.countDown()
        first.join(1_000)
        second.join(1_000)
        assertFalse(first.isAlive)
        assertFalse(second.isAlive)
        assertEquals(2, synchronized(resultLock) { results.size })
        assertTrue(synchronized(resultLock) { results.all { !it } })
        assertEquals(0, dispatcher.pendingCount)
    }

    @Test
    fun `completion fallback cannot self deadlock when its bounded queue is full`() {
        val fallback = IrohaPeerNearbyCompletionFallbackV1(capacity = 1)
        val callbackEntered = CountDownLatch(1)
        val permitNested = CountDownLatch(1)
        val nestedFinished = CountDownLatch(1)
        val queuedFinished = CountDownLatch(1)
        fallback.execute {
            callbackEntered.countDown()
            permitNested.await()
            fallback.execute { nestedFinished.countDown() }
        }
        assertTrue(callbackEntered.await(1, TimeUnit.SECONDS))
        fallback.execute { queuedFinished.countDown() }
        permitNested.countDown()
        assertTrue(nestedFinished.await(1, TimeUnit.SECONDS))
        assertTrue(queuedFinished.await(1, TimeUnit.SECONDS))
    }

    @Test
    fun `throwing completion cannot terminate the shared fallback consumer`() {
        val fallback = IrohaPeerNearbyCompletionFallbackV1(capacity = 1)
        val throwingStarted = CountDownLatch(1)
        val nextFinished = CountDownLatch(1)

        fallback.execute {
            throwingStarted.countDown()
            throw AssertionError("application callback failure")
        }
        assertTrue(throwingStarted.await(1, TimeUnit.SECONDS))
        fallback.execute { nextFinished.countDown() }

        assertTrue(nextFinished.await(1, TimeUnit.SECONDS))
    }

    @Test
    fun `stalled configured and saturated fallback deliver exactly once with lane order`() {
        val callbackRunnables = mutableListOf<Runnable>()
        val fallback = IrohaPeerNearbyCompletionFallbackV1(capacity = 1)
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            Executor { callbackRunnables += it },
            fallback,
            1,
            1,
        )
        val fallbackEntered = CountDownLatch(1)
        val releaseFallback = CountDownLatch(1)
        val fallbackFinished = CountDownLatch(1)
        val delivered = mutableListOf<Int>()
        val deliveredLock = Any()
        fun record(value: Int) = synchronized(deliveredLock) { delivered += value }

        dispatcher.executeCritical { record(0) }
        dispatcher.executeCritical {
            fallbackEntered.countDown()
            releaseFallback.await()
            record(1)
            fallbackFinished.countDown()
        }
        assertTrue(fallbackEntered.await(1, TimeUnit.SECONDS))
        assertEquals(1, dispatcher.pendingCount)
        assertEquals(1, fallback.pendingCount)
        (2 until 10).forEach { value -> dispatcher.executeCritical { record(value) } }
        assertEquals((2 until 10).toList(), synchronized(deliveredLock) { delivered.toList() })
        assertEquals(1, dispatcher.pendingCount)
        assertEquals(1, fallback.pendingCount)

        releaseFallback.countDown()
        assertTrue(fallbackFinished.await(1, TimeUnit.SECONDS))
        while (callbackRunnables.isNotEmpty()) callbackRunnables.removeFirst().run()
        val fallbackDeadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(1)
        while (fallback.pendingCount != 0 && System.nanoTime() < fallbackDeadline) {
            Thread.yield()
        }
        assertEquals(0, fallback.pendingCount)
        assertEquals(0, dispatcher.pendingCount)
        val final = synchronized(deliveredLock) { delivered.toList() }
        assertEquals((2 until 10).toList() + listOf(1, 0), final)
        assertEquals((0 until 10).toSet(), final.toSet())
        assertTrue(final.groupingBy { it }.eachCount().values.all { it == 1 })
    }

    @Test
    fun `throwing worker action runs its drop once and cannot kill the sole drain`() {
        val scheduled = mutableListOf<() -> Unit>()
        val delivered = mutableListOf<String>()
        var drops = 0
        val pump = IrohaPeerNearbySerialActionPumpV1(4) {
            scheduled += it
            true
        }
        assertEquals(
            IrohaPeerNearbyActionAdmissionV1.ACCEPTED,
            pump.enqueue(onDropped = {
                drops += 1
                throw AssertionError("drop callback failure")
            }) { throw AssertionError("worker action failure") },
        )
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            delivered += "next"
        })

        while (scheduled.isNotEmpty()) scheduled.removeFirst().invoke()

        assertEquals(1, drops)
        assertEquals(listOf("next"), delivered)
        assertEquals(0, pump.pendingCount)
    }

    @Test
    fun `internal failure drops queued restart and send exactly once`() {
        val scheduled = mutableListOf<() -> Unit>()
        val generation = IrohaPeerNearbyWorkGenerationV1(1)
        val pump = IrohaPeerNearbySerialActionPumpV1(4) {
            scheduled += it
            true
        }
        var restarted = 0
        var sendDrops = 0

        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue {
            generation.invalidate()
            pump.clear().forEach { it() }
        })
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue(
            onDropped = { },
            action = { restarted += 1 },
        ))
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, pump.enqueue(
            onDropped = { sendDrops += 1 },
            action = { throw AssertionError("failed-generation send ran") },
        ))

        while (scheduled.isNotEmpty()) scheduled.removeFirst().invoke()
        assertEquals(0, restarted)
        assertEquals(1, sendDrops)
        assertEquals(0, pump.pendingCount)
    }

    @Test
    fun `callback stop and worker failure cannot invert lifecycle and gate locks`() {
        val gate = IrohaPeerNearbyCallbackEpochGateV1(initialEpoch = 1)
        val lifecycle = Any()
        val callbackEntered = CountDownLatch(1)
        val workerEntered = CountDownLatch(1)
        val callbackFinished = CountDownLatch(1)
        val workerFinished = CountDownLatch(1)

        val callbackThread = thread {
            gate.performIfCurrent(1) {
                callbackEntered.countDown()
                workerEntered.await()
                gate.update(Long.MIN_VALUE)
                synchronized(lifecycle) { }
            }
            callbackFinished.countDown()
        }
        assertTrue(callbackEntered.await(1, TimeUnit.SECONDS))
        val workerThread = thread {
            synchronized(lifecycle) {
                workerEntered.countDown()
                gate.update(2)
            }
            workerFinished.countDown()
        }

        assertTrue(callbackFinished.await(1, TimeUnit.SECONDS))
        assertTrue(workerFinished.await(1, TimeUnit.SECONDS))
        callbackThread.join(1_000)
        workerThread.join(1_000)
        assertFalse(callbackThread.isAlive)
        assertFalse(workerThread.isAlive)
    }

    @Test
    fun `direct callback submission runs only after lifecycle monitor release`() {
        val deferrer = IrohaPeerNearbyLifecycleCallbackDeferrerV1()
        val sequence = mutableListOf<String>()
        var otherThreadAcquired = false

        deferrer.withLock {
            sequence += "state"
            assertTrue(deferrer.defer {
                val acquired = CountDownLatch(1)
                val other = thread {
                    deferrer.withLock { acquired.countDown() }
                }
                otherThreadAcquired = acquired.await(1, TimeUnit.SECONDS)
                other.join(1_000)
                sequence += "callback"
            })
            sequence += "unlock"
        }

        assertTrue(otherThreadAcquired)
        assertEquals(listOf("state", "unlock", "callback"), sequence)
    }

    @Test
    fun `action and callback pumps yield after one item`() {
        val workerRunnables = mutableListOf<() -> Unit>()
        val workerEvents = mutableListOf<String>()
        val actionPump = IrohaPeerNearbySerialActionPumpV1(4) {
            workerRunnables += it
            true
        }
        actionPump.enqueue { workerEvents += "first" }
        actionPump.enqueue { workerEvents += "second" }
        workerRunnables += { workerEvents += "timer" }
        while (workerRunnables.isNotEmpty()) workerRunnables.removeFirst().invoke()
        assertEquals(listOf("first", "timer", "second"), workerEvents)

        val callbackRunnables = mutableListOf<Runnable>()
        val callbackEvents = mutableListOf<String>()
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            Executor { callbackRunnables += it },
            Executor { it.run() },
            4,
            4,
        )
        assertTrue(dispatcher.execute { callbackEvents += "first" })
        assertTrue(dispatcher.execute { callbackEvents += "second" })
        callbackRunnables += Runnable { callbackEvents += "ui" }
        while (callbackRunnables.isNotEmpty()) callbackRunnables.removeFirst().run()
        assertEquals(listOf("first", "ui", "second"), callbackEvents)
    }

    @Test
    fun `direct worker executor never loses an action and executing counts toward cap`() {
        var directCount = 0
        val direct = IrohaPeerNearbySerialActionPumpV1(1) { action ->
            action()
            true
        }
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, direct.enqueue {
            directCount += 1
        })
        assertEquals(1, directCount)
        assertEquals(0, direct.pendingCount)

        val runnable = arrayOfNulls<(() -> Unit)>(1)
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val bounded = IrohaPeerNearbySerialActionPumpV1(1) {
            runnable[0] = it
            true
        }
        assertEquals(IrohaPeerNearbyActionAdmissionV1.ACCEPTED, bounded.enqueue {
            entered.countDown()
            release.await()
        })
        val runner = thread { runnable[0]!!.invoke() }
        assertTrue(entered.await(1, TimeUnit.SECONDS))
        assertEquals(IrohaPeerNearbyActionAdmissionV1.FULL, bounded.enqueue { })
        release.countDown()
        runner.join(1_000)
        assertFalse(runner.isAlive)
    }

    @Test
    fun `eight send completions retain order through executor rejection fallback`() {
        val delivered = mutableListOf<Int>()
        val dispatcher = IrohaPeerNearbyCallbackDispatcherV1(
            Executor { throw RejectedExecutionException("test") },
            Executor { it.run() },
            1,
            8,
        )
        (0 until 8).forEach { value -> dispatcher.executeCritical { delivered += value } }
        assertEquals((0 until 8).toList(), delivered)
        assertEquals(0, dispatcher.pendingCount)
    }

    @Test
    fun `false handler post is an executor rejection`() {
        val executor = irohaPeerPostingExecutorV1 { false }
        assertFailsWith<RejectedExecutionException> { executor.execute { } }
    }

    @Test
    fun `essential verification context and record throws fail closed`() {
        val failed = mutableListOf<String>()
        listOf("verification", "context", "record").forEach { kind ->
            runIrohaPeerEssentialCallbackV1(onFailure = { failed += kind }) {
                throw AssertionError(kind)
            }
        }
        assertEquals(listOf("verification", "context", "record"), failed)
        assertEquals(
            IrohaPeerNearbyConnectionsErrorV1.VERIFICATION_REJECTED,
            irohaPeerVerificationAdmissionErrorV1("123", verificationOpen = false),
        )
        assertNull(irohaPeerVerificationAdmissionErrorV1("1234", verificationOpen = false))
    }

    @Test
    fun `concurrent receive scheduler rejection never accepts then drops a record`() {
        val schedulerEntered = CountDownLatch(1)
        val releaseScheduler = CountDownLatch(1)
        var scheduleCount = 0
        val scheduleLock = Any()
        val results = mutableListOf<IrohaPeerNearbyReceiveAdmissionV1>()
        val resultLock = Any()
        val pump = IrohaPeerNearbyReceiveCallbackPumpV1(2, 4) {
            val count = synchronized(scheduleLock) {
                scheduleCount += 1
                scheduleCount
            }
            if (count == 1) {
                schedulerEntered.countDown()
                releaseScheduler.await()
            }
            false
        }
        pump.activate(9, "peer")
        val first = thread {
            val result = pump.enqueue(9, "peer") { }
            synchronized(resultLock) { results += result }
        }
        assertTrue(schedulerEntered.await(1, TimeUnit.SECONDS))
        val second = thread {
            val result = pump.enqueue(9, "peer") { }
            synchronized(resultLock) { results += result }
        }
        releaseScheduler.countDown()
        first.join(1_000)
        second.join(1_000)
        assertFalse(first.isAlive)
        assertFalse(second.isAlive)
        assertEquals(
            listOf(
                IrohaPeerNearbyReceiveAdmissionV1.FULL,
                IrohaPeerNearbyReceiveAdmissionV1.FULL,
            ),
            synchronized(resultLock) { results.toList() },
        )
        assertEquals(2, synchronized(scheduleLock) { scheduleCount })
        assertEquals(0, pump.pendingCount)
    }

    @Test
    fun `send completion once suppresses worker failure after terminal delivery`() {
        val errors = mutableListOf<IrohaPeerNearbyConnectionsErrorV1?>()
        val completion = IrohaPeerNearbySendCompletionOnceV1 { errors += it }

        completion.complete(null)
        completion.complete(IrohaPeerNearbyConnectionsErrorV1.CANCELLED)

        assertEquals(listOf<IrohaPeerNearbyConnectionsErrorV1?>(null), errors)
    }

    @Test
    fun `scheduler cancellation policies remove long uptime deadlines`() {
        val scheduler = configureIrohaPeerNearbySchedulerV1(ScheduledThreadPoolExecutor(1))
        val futures = (0 until 100).map {
            scheduler.schedule({ }, 90, TimeUnit.SECONDS)
        }
        futures.forEach { it.cancel(false) }
        assertTrue(scheduler.removeOnCancelPolicy)
        assertFalse(scheduler.executeExistingDelayedTasksAfterShutdownPolicy)
        assertFalse(scheduler.continueExistingPeriodicTasksAfterShutdownPolicy)
        assertEquals(0, scheduler.queue.size)
        scheduler.shutdownNow()
    }

    @Test
    fun `stop restart ignores stale callback even when payload and peer ids collide`() {
        val deliveries = IrohaPeerNearbyPendingDeliveryRegistryV1<DeliveryAttempt>()
        val restarted = DeliveryAttempt(epoch = 2, peerId = "peer", marker = "new")
        assertTrue(deliveries.add(19, restarted))

        val stale = deliveries.removeIf(19) {
            IrohaPeerNearbyConnectionsReducerV1.matchesAttempt(
                it.epoch,
                1,
                it.peerId,
                "peer",
            )
        }
        assertNull(stale)
        assertEquals(1, deliveries.size)

        val current = deliveries.removeIf(19) {
            IrohaPeerNearbyConnectionsReducerV1.matchesAttempt(
                it.epoch,
                2,
                it.peerId,
                "peer",
            )
        }
        assertSame(restarted, current)
        assertEquals(0, deliveries.size)
    }

    private class DeliveryAttempt(
        val epoch: Long,
        val peerId: String,
        @Suppress("unused") val marker: String,
    )

    private fun discoveryContext(
        role: IrohaPeerNearbyRoleV1,
        seed: Int,
    ): IrohaPeerNearbyDiscoveryContextV1 = IrohaPeerNearbyDiscoveryContextV1(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        role,
        ByteArray(16) { (seed + it).toByte() },
        ByteArray(32) { (seed + it + 16).toByte() },
    )
}
