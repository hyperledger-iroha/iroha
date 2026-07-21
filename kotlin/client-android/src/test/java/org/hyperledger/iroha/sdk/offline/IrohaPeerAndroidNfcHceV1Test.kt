package org.hyperledger.iroha.sdk.offline

import java.util.concurrent.CountDownLatch
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.SynchronousQueue
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class IrohaPeerNfcHceResponseGateV1Test {
    @Test
    fun `response completed during invocation is returned directly`() {
        val epoch = IrohaPeerNfcActivationEpochV1()
        val posts = mutableListOf<() -> Unit>()
        val sends = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcHceResponseGateV1(
            epoch.capture(),
            epoch,
            post = { posts += it; true },
            send = sends::add,
        )
        val response = IrohaPeerNfcApduResponseV1(
            byteArrayOf(1, 2),
            IrohaPeerNfcStatusWordV1.SUCCESS,
        )

        gate.respond(response)
        assertContentEquals(response.encoded, gate.finishInvocation())
        assertTrue(posts.isEmpty())
        assertTrue(sends.isEmpty())

        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
        ))
        assertTrue(posts.isEmpty())
    }

    @Test
    fun `genuinely asynchronous response is posted once after null handoff`() {
        val epoch = IrohaPeerNfcActivationEpochV1()
        val posts = mutableListOf<() -> Unit>()
        val sends = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcHceResponseGateV1(
            epoch.capture(),
            epoch,
            post = { posts += it; true },
            send = sends::add,
        )
        val response = IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
        )

        assertNull(gate.finishInvocation())
        gate.respond(response)
        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
        ))
        assertEquals(1, posts.size)
        assertTrue(sends.isEmpty())

        posts.single().invoke()
        assertEquals(1, sends.size)
        assertContentEquals(response.encoded, sends.single())
    }

    @Test
    fun `posted response rechecks activation epoch`() {
        val epoch = IrohaPeerNfcActivationEpochV1()
        val posts = mutableListOf<() -> Unit>()
        val sends = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcHceResponseGateV1(
            epoch.capture(),
            epoch,
            post = { posts += it; true },
            send = sends::add,
        )

        assertNull(gate.finishInvocation())
        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
        ))
        epoch.invalidate()
        posts.single().invoke()

        assertTrue(sends.isEmpty())
    }

    @Test
    fun `rejected post consumes async completion without inline send`() {
        val epoch = IrohaPeerNfcActivationEpochV1()
        var postAttempts = 0
        val sends = mutableListOf<ByteArray>()
        val gate = IrohaPeerNfcHceResponseGateV1(
            epoch.capture(),
            epoch,
            post = { postAttempts += 1; false },
            send = sends::add,
        )

        assertNull(gate.finishInvocation())
        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
        ))
        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
        ))

        assertEquals(1, postAttempts)
        assertTrue(sends.isEmpty())
    }

    @Test
    fun `exception fallback wins exactly once when handler has not responded`() {
        val epoch = IrohaPeerNfcActivationEpochV1()
        val gate = IrohaPeerNfcHceResponseGateV1(
            epoch.capture(),
            epoch,
            post = { true },
            send = { error("fallback must be synchronous") },
        )
        val failure = IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
        )

        assertContentEquals(failure.encoded, gate.finishInvocation(failure))
        gate.respond(IrohaPeerNfcApduResponseV1(
            statusWord = IrohaPeerNfcStatusWordV1.SUCCESS,
        ))
        assertNull(gate.finishInvocation())
    }
}

class IrohaPeerNfcDurabilityBoundaryV1Test {
    @Test
    fun `durability lease spans bridge instances until original callback settles`() {
        val firstFixture = admissionFixture()
        val secondFixture = admissionFixture()
        val firstScheduler = ManualDurabilityScheduler()
        val secondScheduler = ManualDurabilityScheduler()
        val sharedLease = IrohaPeerNfcDurabilityLeaseV1()
        lateinit var firstContext: IrohaPeerNfcPaymentAdmissionContextV1
        lateinit var firstCompletion: IrohaPeerNfcDurableAdmissionCompletionV1
        var secondInvocations = 0
        val firstBridge = IrohaPeerNfcReceiverApduBridgeV1(
            firstFixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                firstContext = context
                firstCompletion = completion
            },
            unusedCommit(),
            firstScheduler,
            IrohaPeerNfcDurabilityExecutorV1 { action -> action(); true },
            sharedLease,
        )
        val secondBridge = IrohaPeerNfcReceiverApduBridgeV1(
            secondFixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                secondInvocations += 1
                completion.complete(
                    IrohaPeerNfcDurablePaymentAdmissionV1(
                        context,
                        secondFixture.receiver.limits,
                    ),
                    null,
                )
            },
            unusedCommit(),
            secondScheduler,
            IrohaPeerNfcDurabilityExecutorV1 { action -> action(); true },
            sharedLease,
        )
        val firstResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        firstBridge.handle(
            firstFixture.begin,
            IrohaPeerNfcApduResponseHandlerV1(firstResponses::add),
        )
        firstScheduler.fire(0)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            firstResponses.single().statusWord)

        val saturated = mutableListOf<IrohaPeerNfcApduResponseV1>()
        secondBridge.handle(
            secondFixture.begin,
            IrohaPeerNfcApduResponseHandlerV1(saturated::add),
        )
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            saturated.single().statusWord)
        assertEquals(0, secondInvocations)
        assertEquals(0, secondScheduler.taskCount)

        firstCompletion.complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(
                firstContext,
                firstFixture.receiver.limits,
            ),
            null,
        )
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, firstFixture.receiver.phase)

        val recovered = mutableListOf<IrohaPeerNfcApduResponseV1>()
        secondBridge.handle(
            secondFixture.begin,
            IrohaPeerNfcApduResponseHandlerV1(recovered::add),
        )
        assertEquals(1, secondInvocations)
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, recovered.single().statusWord)
    }

    @Test
    fun `retained callback holds process lease past timeout until exact late completion`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        val durabilityLease = IrohaPeerNfcDurabilityLeaseV1()
        val contexts = mutableListOf<IrohaPeerNfcPaymentAdmissionContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableAdmissionCompletionV1>()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                contexts += context
                completions += completion
            },
            unusedCommit(),
            scheduler,
            IrohaPeerNfcDurabilityExecutorV1 { action -> action(); true },
            durabilityLease,
        )
        val abandoned = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(abandoned::add))
        assertEquals(1, completions.size)

        repeat(32) {
            bridge.onDeactivated(0)
            val denied = mutableListOf<IrohaPeerNfcApduResponseV1>()
            bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(denied::add))
            assertEquals(
                IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED,
                denied.single().statusWord,
            )
        }
        assertEquals(1, completions.size)
        assertEquals(1, scheduler.taskCount)

        scheduler.fire(0)
        assertTrue(abandoned.isEmpty())
        repeat(32) {
            val saturated = mutableListOf<IrohaPeerNfcApduResponseV1>()
            bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(saturated::add))
            assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
                saturated.single().statusWord)
        }
        assertEquals(1, completions.size)
        assertEquals(1, scheduler.taskCount)
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, fixture.receiver.phase)

        completions[0].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[0], fixture.receiver.limits),
            null,
        )
        completions[0].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[0], fixture.receiver.limits),
            null,
        )
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, fixture.receiver.phase)

        val recovered = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(recovered::add))
        assertEquals(2, completions.size)
        assertTrue(recovered.isEmpty())

        // A duplicate completion from the timed-out operation must not release
        // the exact lease now owned by its retry.
        completions[0].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[0], fixture.receiver.limits),
            null,
        )
        val probeToken = Any()
        assertFalse(durabilityLease.acquire(probeToken))

        completions[1].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[1], fixture.receiver.limits),
            null,
        )
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, recovered.single().statusWord)
        assertTrue(durabilityLease.acquire(probeToken))
        assertTrue(durabilityLease.release(probeToken))
    }

    @Test
    fun `deactivation before worker start suppresses storage and late task`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        val executor = HoldingDurabilityExecutor()
        var admissions = 0
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                admissions += 1
                completion.complete(
                    IrohaPeerNfcDurablePaymentAdmissionV1(context, fixture.receiver.limits),
                    null,
                )
            },
            unusedCommit(),
            scheduler,
            executor,
        )
        val abandoned = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(abandoned::add))

        bridge.onDeactivated(0)
        executor.runHeld()
        assertEquals(0, admissions)
        scheduler.fire(0)
        assertTrue(abandoned.isEmpty())

        val recovered = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(recovered::add))
        executor.runHeld()
        assertEquals(1, admissions)
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, recovered.single().statusWord)
    }

    @Test
    fun `timeout suppresses late admission result and exact retry succeeds`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        val contexts = mutableListOf<IrohaPeerNfcPaymentAdmissionContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableAdmissionCompletionV1>()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                contexts += context
                completions += completion
            },
            unusedCommit(),
            scheduler,
        )
        val timedOut = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(timedOut::add))
        scheduler.fire(0)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE, timedOut.single().statusWord)

        completions[0].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[0], fixture.receiver.limits),
            null,
        )
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, fixture.receiver.phase)
        assertEquals(1, timedOut.size)

        val retried = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(retried::add))
        completions[1].complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(contexts[1], fixture.receiver.limits),
            null,
        )
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, retried.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, fixture.receiver.phase)
    }

    @Test
    fun `worker rejection fails admission once without invoking app storage`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        var admissions = 0
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { _, _ -> admissions += 1 },
            unusedCommit(),
            scheduler,
            IrohaPeerNfcDurabilityExecutorV1 { false },
        )
        val responses = mutableListOf<IrohaPeerNfcApduResponseV1>()

        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(responses::add))

        assertEquals(0, admissions)
        assertEquals(1, responses.size)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE, responses.single().statusWord)
        assertTrue(scheduler.isCancelled(0))
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, fixture.receiver.phase)
    }

    @Test
    fun `synchronous app throw fails exactly once`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { _, _ ->
                throw IllegalStateException("storage failed")
            },
            unusedCommit(),
            scheduler,
        )
        val responses = mutableListOf<IrohaPeerNfcApduResponseV1>()

        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(responses::add))

        assertEquals(1, responses.size)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE, responses.single().statusWord)
        scheduler.forceFire(0)
        assertEquals(1, responses.size)
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, fixture.receiver.phase)
    }

    @Test
    fun `worker rejection fails commit once without installing acknowledgement`() {
        val fixture = commitFixture()
        val scheduler = ManualDurabilityScheduler()
        var commits = 0
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { _, _ -> error("unused") },
            IrohaPeerNfcDurableCommitHandlerV1 { _, _ -> commits += 1 },
            scheduler,
            IrohaPeerNfcDurabilityExecutorV1 { false },
        )
        val responses = mutableListOf<IrohaPeerNfcApduResponseV1>()

        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(responses::add))

        assertEquals(0, commits)
        assertEquals(1, responses.size)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE, responses.single().statusWord)
        assertTrue(scheduler.isCancelled(0))
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, fixture.receiver.phase)
    }

    @Test
    fun `blocking app handler never blocks HCE caller and saturation is bounded`() {
        val fixture = admissionFixture()
        val scheduler = ManualDurabilityScheduler()
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val invocations = AtomicInteger()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            fixture.receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { _, _ ->
                invocations.incrementAndGet()
                entered.countDown()
                release.await()
            },
            unusedCommit(),
            scheduler,
            SingleWorkerDurabilityExecutor(),
        )
        val firstResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()

        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(firstResponses::add))
        assertTrue(entered.await(2, TimeUnit.SECONDS))
        assertTrue(firstResponses.isEmpty())
        scheduler.fire(0)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            firstResponses.single().statusWord)

        val saturated = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.begin, IrohaPeerNfcApduResponseHandlerV1(saturated::add))
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            saturated.single().statusWord)
        assertEquals(1, invocations.get())
        release.countDown()
    }

    private data class AdmissionFixture(
        val receiver: IrohaPeerNfcReceiverSessionV1,
        val begin: IrohaPeerNfcCommandV1,
    )

    private data class CommitFixture(
        val receiver: IrohaPeerNfcReceiverSessionV1,
        val commit: IrohaPeerNfcCommandV1,
    )

    private fun admissionFixture(): AdmissionFixture {
        val session = ByteArray(16) { (it + 1).toByte() }
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x51, 120)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x52, 180)
        val receiver = IrohaPeerNfcReceiverSessionV1(session, request.encode())
        return AdmissionFixture(
            receiver,
            IrohaPeerNfcCommandV1.beginPayment(
                session,
                request.canonicalHash,
                payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
            ),
        )
    }

    private fun commitFixture(): CommitFixture {
        val session = ByteArray(16) { (it + 1).toByte() }
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x61, 120)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x62, 180)
        val receiver = IrohaPeerNfcReceiverSessionV1(session, request.encode())
        val begin = IrohaPeerNfcCommandV1.beginPayment(
            session,
            request.canonicalHash,
            payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        val admission = receiver.preparePaymentAdmission(begin) as
            IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission
        receiver.installPaymentAdmission(
            IrohaPeerNfcDurablePaymentAdmissionV1(admission.context, receiver.limits),
        )
        receiver.handle(IrohaPeerNfcCommandV1.write(
            session,
            payment.wireHash,
            0,
            payment.encode(),
        ))
        return CommitFixture(
            receiver,
            IrohaPeerNfcCommandV1.commit(
                session,
                request.canonicalHash,
                payment.wireHash,
            ),
        )
    }

    private fun message(
        kind: IrohaPeerPayloadKind,
        byte: Int,
        count: Int,
    ) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
        IrohaPeerPayloadProfile.OFFLINE_NOTE,
        kind,
        1,
        ByteArray(count) { byte.toByte() },
    ))

    private fun unusedCommit() = IrohaPeerNfcDurableCommitHandlerV1 { _, _ ->
        error("COMMIT is not expected")
    }

    private class HoldingDurabilityExecutor : IrohaPeerNfcDurabilityExecutorV1 {
        private var held: (() -> Unit)? = null

        override fun execute(action: () -> Unit): Boolean {
            if (held != null) return false
            held = action
            return true
        }

        fun runHeld() {
            val action = requireNotNull(held)
            held = null
            action()
        }
    }

    private class SingleWorkerDurabilityExecutor : IrohaPeerNfcDurabilityExecutorV1 {
        private val executor = ThreadPoolExecutor(
            1,
            1,
            0,
            TimeUnit.MILLISECONDS,
            SynchronousQueue(),
            { runnable -> Thread(runnable).apply { isDaemon = true } },
            ThreadPoolExecutor.AbortPolicy(),
        ).apply { prestartAllCoreThreads() }

        override fun execute(action: () -> Unit): Boolean = try {
            executor.execute(Runnable(action))
            true
        } catch (_: RejectedExecutionException) {
            false
        }
    }

    private class ManualDurabilityScheduler : IrohaPeerNfcDurabilityTimeoutSchedulerV1 {
        private class Task(
            val action: () -> Unit,
        ) : IrohaPeerNfcDurabilityTimeoutV1 {
            var cancelled = false
            var fired = false

            override fun cancel() {
                cancelled = true
            }

            fun fire() {
                if (!cancelled && !fired) {
                    fired = true
                    action()
                }
            }
        }

        private val tasks = mutableListOf<Task>()
        val taskCount: Int get() = tasks.size

        override fun schedule(
            delayMillis: Long,
            action: () -> Unit,
        ): IrohaPeerNfcDurabilityTimeoutV1 {
            assertEquals(IrohaPeerNfcReceiverApduBridgeV1.DEFAULT_DURABILITY_TIMEOUT_MILLIS,
                delayMillis)
            return Task(action).also(tasks::add)
        }

        fun fire(index: Int) = tasks[index].fire()
        fun forceFire(index: Int) = tasks[index].run {
            if (!fired) {
                fired = true
                action()
            }
        }
        fun isCancelled(index: Int): Boolean = tasks[index].cancelled
    }
}
