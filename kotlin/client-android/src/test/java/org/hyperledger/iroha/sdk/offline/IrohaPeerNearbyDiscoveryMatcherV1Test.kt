package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class IrohaPeerNearbyDiscoveryMatcherV1Test {
    @Test
    fun `bootstrap sender adopts exact nonzero receiver context`() {
        val sender = IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
        )
        val receiver = IrohaPeerNearbyDiscoveryContextV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            ByteArray(16) { (it + 1).toByte() },
            ByteArray(32) { (it + 2).toByte() },
        )
        val selected = IrohaPeerNearbyDiscoveryMatcherV1.selectLocalContext(
            sender,
            receiver,
            IrohaPeerNearbyRoleV1.RECEIVER,
        )!!
        assertEquals(IrohaPeerNearbyRoleV1.SENDER, selected.role)
        assertContentEquals(receiver.sessionId, selected.sessionId)
        assertContentEquals(receiver.requestCanonicalHash, selected.requestCanonicalHash)
    }

    @Test
    fun `bootstrap never selects another bootstrap or wrong role`() {
        val sender = IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
        )
        assertNull(IrohaPeerNearbyDiscoveryMatcherV1.selectLocalContext(
            sender,
            sender,
            IrohaPeerNearbyRoleV1.RECEIVER,
        ))
        val nonzeroSender = IrohaPeerNearbyDiscoveryContextV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            ByteArray(16) { 1 },
            ByteArray(32) { 2 },
        )
        assertNull(IrohaPeerNearbyDiscoveryMatcherV1.selectLocalContext(
            sender,
            nonzeroSender,
            IrohaPeerNearbyRoleV1.RECEIVER,
        ))
    }
}

class IrohaPeerIsoDepLimitsV1Test {
    @Test
    fun `short APDU subtracts write metadata and never advertises 240`() {
        val limits = IrohaPeerIsoDepLimitsV1.derive(261, false)
        assertEquals(203, limits.maximumWriteChunkBytes)
        assertEquals(256, limits.maximumReadChunkBytes)
    }

    @Test
    fun `extended same-platform path reaches protocol maximum when hardware allows`() {
        val limits = IrohaPeerIsoDepLimitsV1.derive(5_000, true)
        assertEquals(4_096, limits.maximumWriteChunkBytes)
        assertEquals(4_096, limits.maximumReadChunkBytes)
    }
}

class IrohaPeerNfcActivationEpochV1Test {
    @Test
    fun `stale callback is suppressed after RF deactivation`() {
        val gate = IrohaPeerNfcActivationEpochV1()
        val firstEpoch = gate.capture()
        var callbacks = 0
        assertTrue(gate.performIfCurrent(firstEpoch) { callbacks += 1 })

        gate.invalidate()
        assertFalse(gate.performIfCurrent(firstEpoch) { callbacks += 1 })
        assertEquals(1, callbacks)

        val secondEpoch = gate.capture()
        assertTrue(gate.performIfCurrent(secondEpoch) { callbacks += 1 })
        assertEquals(2, callbacks)
    }
}

class IrohaPeerNfcApduFailureClassifierV1Test {
    @Test
    fun `non-canonical and out-of-range APDU lengths stay length failures`() {
        val aliasedGetInfo = byteArrayOf(
            0x80.toByte(), 0x10, 0, 0, 0, 0, 0x62,
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcAPDUCodecV1.decode(aliasedGetInfo)
        }
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_LENGTH,
            IrohaPeerNfcApduFailureClassifierV1.classify(aliasedGetInfo),
        )

        val sessionAndHashAndOffset = ByteArray(52).also {
            for (index in 0 until 16) it[index] = 1
            for (index in 16 until 48) it[index] = 2
        }
        val oversizedRead = byteArrayOf(
            0x80.toByte(), 0x11, 0, 0, 0, 0, 52,
        ) + sessionAndHashAndOffset + byteArrayOf(0, 0)
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_LENGTH,
            IrohaPeerNfcApduFailureClassifierV1.classify(oversizedRead),
        )
    }

    @Test
    fun `raw APDU response is bounded before status parsing`() {
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcApduResponseV1.decode(
                ByteArray(IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES + 3),
            )
        }
    }

    @Test
    fun `decode failures preserve ISO 7816 failure category`() {
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_LENGTH,
            IrohaPeerNfcApduFailureClassifierV1.classify(byteArrayOf(0, 0, 0)),
        )
        assertEquals(
            IrohaPeerNfcStatusWordV1.CLASS_NOT_SUPPORTED,
            IrohaPeerNfcApduFailureClassifierV1.classify(
                byteArrayOf(0x7f, 0x10, 0, 0),
            ),
        )
        assertEquals(
            IrohaPeerNfcStatusWordV1.INSTRUCTION_NOT_SUPPORTED,
            IrohaPeerNfcApduFailureClassifierV1.classify(
                byteArrayOf(0x80.toByte(), 0x7f, 0, 0),
            ),
        )

        val missingAid = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.SELECT_APPLICATION,
        ).also { it[5] = 0 }
        assertEquals(
            IrohaPeerNfcStatusWordV1.NOT_FOUND,
            IrohaPeerNfcApduFailureClassifierV1.classify(missingAid),
        )

        val wrongGetInfoLength = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.GET_INFO,
        ).copyOf(4)
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_LENGTH,
            IrohaPeerNfcApduFailureClassifierV1.classify(wrongGetInfoLength),
        )

        val wrongParameters = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.GET_INFO,
        ).also { it[2] = 1 }
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_DATA,
            IrohaPeerNfcApduFailureClassifierV1.classify(wrongParameters),
        )

        val invalidSession = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.getStatus(
                ByteArray(16) { 1 },
                ByteArray(32) { 2 },
            ),
        ).also { apdu ->
            // Short APDU: four-byte header, one-byte Lc, then session ID.
            for (index in 5 until 21) apdu[index] = 0
        }
        assertEquals(
            IrohaPeerNfcStatusWordV1.WRONG_DATA,
            IrohaPeerNfcApduFailureClassifierV1.classify(invalidSession),
        )
    }
}

class IrohaPeerNfcReceiverApduBridgeV1Test {
    @Test
    fun `begin response waits for the exact durable admission`() {
        val session = ByteArray(16) { (it + 1).toByte() }
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x31, 120)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x32, 180)
        val otherPayment = message(IrohaPeerPayloadKind.PAYMENT, 0x33, 180)
        val receiver = IrohaPeerNfcReceiverSessionV1(session, request.encode())
        val begin = IrohaPeerNfcCommandV1.beginPayment(
            session,
            request.canonicalHash,
            payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        val contexts = mutableListOf<IrohaPeerNfcPaymentAdmissionContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableAdmissionCompletionV1>()
        val scheduler = ManualDurabilityTimeoutScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(
            receiver,
            IrohaPeerNfcDurableAdmissionHandlerV1 { context, completion ->
                contexts += context
                completions += completion
            },
            IrohaPeerNfcDurableCommitHandlerV1 { _, _ -> error("No COMMIT expected") },
            scheduler,
        )

        val rejected = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(begin, IrohaPeerNfcApduResponseHandlerV1(rejected::add))
        assertTrue(rejected.isEmpty())
        assertContentEquals(
            payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
            contexts.single().paymentHeader,
        )
        completions.single().complete(
            IrohaPeerNfcDurablePaymentAdmissionV1(
                IrohaPeerNfcPaymentAdmissionContextV1(
                    receiver.identity,
                    receiver.profilePolicy,
                    otherPayment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
                ),
            ),
            null,
        )
        assertEquals(
            IrohaPeerNfcStatusWordV1.SECURITY_STATUS_NOT_SATISFIED,
            rejected.single().statusWord,
        )
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, receiver.phase)

        val accepted = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(begin, IrohaPeerNfcApduResponseHandlerV1(accepted::add))
        completions[1].complete(IrohaPeerNfcDurablePaymentAdmissionV1(contexts[1]), null)
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, accepted.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, receiver.phase)
        assertEquals(0, receiver.status().receivedPaymentBytes)
        assertTrue(scheduler.isCancelled(1))
    }

    @Test
    fun `commit response waits for exact durable acknowledgement`() {
        val fixture = receiverFixture()
        var durableContext: IrohaPeerNfcCommitContextV1? = null
        var durableCompletion: IrohaPeerNfcDurableCommitCompletionV1? = null
        val scheduler = ManualDurabilityTimeoutScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(fixture.receiver,
            unusedAdmission(),
            IrohaPeerNfcDurableCommitHandlerV1 { context, completion ->
                durableContext = context
                durableCompletion = completion
            }, scheduler)
        val responses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(responses::add))
        assertTrue(responses.isEmpty())
        val record = IrohaPeerNfcDurableAcknowledgementV1(
            requireNotNull(durableContext),
            fixture.acknowledgement.encode(),
        )
        requireNotNull(durableCompletion).complete(record, null)
        assertEquals(1, responses.size)
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, responses.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, fixture.receiver.phase)
        assertTrue(scheduler.isCancelled(0))
        scheduler.forceFire(0)
        assertEquals(1, responses.size)
    }

    @Test
    fun `lost durable callback times out and retry succeeds`() {
        val fixture = receiverFixture()
        val contexts = mutableListOf<IrohaPeerNfcCommitContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableCommitCompletionV1>()
        val scheduler = ManualDurabilityTimeoutScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(fixture.receiver,
            unusedAdmission(),
            IrohaPeerNfcDurableCommitHandlerV1 { context, completion ->
                contexts.add(context)
                completions.add(completion)
            }, scheduler)
        val firstResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()

        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(firstResponses::add))
        assertEquals(IrohaPeerNfcReceiverApduBridgeV1.DEFAULT_DURABILITY_TIMEOUT_MILLIS,
            scheduler.delayMillis(0))
        scheduler.fire(0)
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            firstResponses.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, fixture.receiver.phase)

        // A callback lost past its deadline is stale even if storage later reports success.
        completions[0].complete(
            IrohaPeerNfcDurableAcknowledgementV1(contexts[0], fixture.acknowledgement.encode()),
            null,
        )
        assertEquals(1, firstResponses.size)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, fixture.receiver.phase)

        val retryResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(retryResponses::add))
        assertEquals(2, completions.size)
        completions[1].complete(
            IrohaPeerNfcDurableAcknowledgementV1(contexts[1], fixture.acknowledgement.encode()),
            null,
        )
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, retryResponses.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, fixture.receiver.phase)
    }

    @Test
    fun `deactivation detaches response and retains one durability lease`() {
        val fixture = receiverFixture()
        val contexts = mutableListOf<IrohaPeerNfcCommitContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableCommitCompletionV1>()
        val scheduler = ManualDurabilityTimeoutScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(fixture.receiver,
            unusedAdmission(),
            IrohaPeerNfcDurableCommitHandlerV1 { context, completion ->
                contexts.add(context)
                completions.add(completion)
            }, scheduler)
        val firstResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(firstResponses::add))

        bridge.onDeactivated(0)
        assertFalse(scheduler.isCancelled(0))
        scheduler.fire(0)
        assertTrue(firstResponses.isEmpty())

        val secondResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(secondResponses::add))
        assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE,
            secondResponses.single().statusWord)
        assertEquals(1, completions.size)

        // The late durable callback from the expired activation must not answer
        // or mutate the second activation.
        scheduler.forceFire(0)
        completions[0].complete(
            IrohaPeerNfcDurableAcknowledgementV1(contexts[0], fixture.acknowledgement.encode()),
            null,
        )
        assertTrue(firstResponses.isEmpty())
        assertEquals(1, secondResponses.size)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, fixture.receiver.phase)

        val recoveredResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(recoveredResponses::add))
        assertEquals(2, completions.size)
        completions[1].complete(
            IrohaPeerNfcDurableAcknowledgementV1(contexts[1], fixture.acknowledgement.encode()),
            null,
        )
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS,
            recoveredResponses.single().statusWord)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, fixture.receiver.phase)
    }

    @Test
    fun `explicit reset retains the bounded lease until its deadline`() {
        val fixture = receiverFixture()
        val contexts = mutableListOf<IrohaPeerNfcCommitContextV1>()
        val completions = mutableListOf<IrohaPeerNfcDurableCommitCompletionV1>()
        val scheduler = ManualDurabilityTimeoutScheduler()
        val bridge = IrohaPeerNfcReceiverApduBridgeV1(fixture.receiver,
            unusedAdmission(),
            IrohaPeerNfcDurableCommitHandlerV1 { context, completion ->
                contexts.add(context)
                completions.add(completion)
            }, scheduler)
        val abandonedResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(abandonedResponses::add))

        bridge.reset()
        assertFalse(scheduler.isCancelled(0))
        val blockedResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(blockedResponses::add))
        assertEquals(IrohaPeerNfcStatusWordV1.CONDITIONS_NOT_SATISFIED,
            blockedResponses.single().statusWord)
        assertEquals(1, completions.size)
        completions[0].complete(
            IrohaPeerNfcDurableAcknowledgementV1(contexts[0], fixture.acknowledgement.encode()),
            null,
        )
        assertTrue(abandonedResponses.isEmpty())

        val retryResponses = mutableListOf<IrohaPeerNfcApduResponseV1>()
        bridge.handle(fixture.commit, IrohaPeerNfcApduResponseHandlerV1(retryResponses::add))
        assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, retryResponses.single().statusWord)
        assertEquals(1, completions.size)
    }

    private class ReceiverFixture(
        val receiver: IrohaPeerNfcReceiverSessionV1,
        val commit: IrohaPeerNfcCommandV1,
        val acknowledgement: IrohaPeerWireMessageV1,
    )

    private class ManualDurabilityTimeoutScheduler : IrohaPeerNfcDurabilityTimeoutSchedulerV1 {
        private class Task(
            val delayMillis: Long,
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

            fun forceFire() {
                if (!fired) {
                    fired = true
                    action()
                }
            }
        }

        private val tasks = mutableListOf<Task>()

        override fun schedule(
            delayMillis: Long,
            action: () -> Unit,
        ): IrohaPeerNfcDurabilityTimeoutV1 = Task(delayMillis, action).also(tasks::add)

        fun fire(index: Int) = tasks[index].fire()

        fun forceFire(index: Int) = tasks[index].forceFire()

        fun isCancelled(index: Int): Boolean = tasks[index].cancelled

        fun delayMillis(index: Int): Long = tasks[index].delayMillis
    }

    private fun receiverFixture(): ReceiverFixture {
        val session = ByteArray(16) { (it + 1).toByte() }
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x41, 120)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x42, 180)
        val acknowledgement = message(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x43, 100)
        val receiver = IrohaPeerNfcReceiverSessionV1(session, request.encode())
        val begin = IrohaPeerNfcCommandV1.beginPayment(
            session,
            request.canonicalHash,
            payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        receiver.installPaymentAdmission(
            IrohaPeerNfcDurablePaymentAdmissionV1(
                (receiver.preparePaymentAdmission(begin) as
                    IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission).context,
                receiver.limits,
            ),
        )
        receiver.handle(IrohaPeerNfcCommandV1.write(
            session,
            payment.wireHash,
            0,
            payment.encode(),
        ))
        return ReceiverFixture(
            receiver,
            IrohaPeerNfcCommandV1.commit(
                session,
                request.canonicalHash,
                payment.wireHash,
            ),
            acknowledgement,
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

    private fun unusedAdmission() = IrohaPeerNfcDurableAdmissionHandlerV1 { _, _ ->
        error("No BEGIN_PAYMENT is expected in this commit-only test")
    }
}
