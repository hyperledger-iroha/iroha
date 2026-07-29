package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class IrohaPeerNfcReaderExchangeV1Test {
    @Test
    fun `4096-byte peers burst value chunks with only phase-boundary status reads`() {
        val messages = messages(requestBytes = 4_500, paymentBytes = 9_000, acknowledgementBytes = 4_500)
        val limits = limits(read = 4_096, write = 4_096)
        val scenario = runFresh(messages, limits, limits)

        assertContentEquals(messages.acknowledgement.encode(), scenario.result.acknowledgement.encode())
        assertEquals(2, scenario.persisted.size)
        assertTrue(
            scenario.events.indexOf("loadOrCreate") < scenario.events.indexOf("BEGIN_PAYMENT"),
        )
        assertEquals(3, scenario.commands.count { it.type == IrohaPeerNfcCommandTypeV1.GET_STATUS })
        assertEquals(
            listOf(4_096, 4_096, messages.payment.encode().size - 8_192),
            scenario.commands.filter { it.type == IrohaPeerNfcCommandTypeV1.WRITE }
                .map { it.bytes.size },
        )
        assertEquals(
            listOf(4_096, messages.request.encode().size - 4_096),
            scenario.commands.filter { it.type == IrohaPeerNfcCommandTypeV1.READ_REQUEST }
                .map { it.length },
        )
        assertEquals(IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT, scenario.commands.last().type)
    }

    @Test
    fun `Android short-APDU limit emits 203-byte writes while preserving burst recovery`() {
        val messages = messages(requestBytes = 700, paymentBytes = 700, acknowledgementBytes = 500)
        val local = limits(read = 240, write = 203)
        val remote = limits(read = 4_096, write = 4_096)
        val scenario = runFresh(messages, local, remote)
        val writes = scenario.commands.filter { it.type == IrohaPeerNfcCommandTypeV1.WRITE }

        assertTrue(writes.size > 1)
        assertTrue(writes.all { it.bytes.size <= 203 })
        assertEquals(203, writes.maxOf { it.bytes.size })
        writes.forEach { command ->
            val apdu = IrohaPeerNfcAPDUCodecV1.encode(command)
            assertTrue((apdu[4].toInt() and 0xff) != 0, "WRITE must use a short Lc")
        }
        assertEquals(3, scenario.commands.count { it.type == IrohaPeerNfcCommandTypeV1.GET_STATUS })
        assertContentEquals(messages.acknowledgement.encode(), scenario.result.acknowledgement.encode())
    }

    @Test
    fun `lost value-boundary responses resume from the exact durable checkpoint`() {
        val messages = messages(requestBytes = 600, paymentBytes = 900, acknowledgementBytes = 620)
        val limits = limits(read = 240, write = 203)
        for (lostType in listOf(
            IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT,
            IrohaPeerNfcCommandTypeV1.WRITE,
            IrohaPeerNfcCommandTypeV1.COMMIT,
        )) {
            val receiver = receiver(messages, limits)
            val loopback = Loopback(receiver, messages.acknowledgement, lostType)
            val persisted = mutableListOf<ByteArray>()
            var factoryCalls = 0
            val store = IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
                factoryCalls += 1
                checkpoint(info, request, messages.payment, limits).also {
                    persisted += it.encode()
                }
            }
            val updater = IrohaPeerNfcSenderCheckpointUpdaterV1 {
                persisted += it.copyOf()
            }

            assertFailsWith<LostResponse> {
                IrohaPeerNfcReaderExchangeV1.run(
                    policy,
                    loopback,
                    store,
                    updater,
                    limits = limits,
                )
            }
            assertTrue(persisted.isNotEmpty(), "ISC1 must exist before losing $lostType")

            val result = IrohaPeerNfcReaderExchangeV1.run(
                policy,
                loopback,
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ ->
                    error("A resumed exchange must not create value")
                },
                updater,
                restoredCheckpoint = persisted.last(),
                limits = limits,
            )
            assertEquals(1, factoryCalls, lostType.toString())
            assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, receiver.phase, lostType.toString())
            assertContentEquals(
                messages.acknowledgement.encode(),
                result.acknowledgement.encode(),
                lostType.toString(),
            )
        }
    }

    @Test
    fun `lost final confirm response is complete once the exact acknowledgement is durable`() {
        val messages = messages(requestBytes = 600, paymentBytes = 900, acknowledgementBytes = 620)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val loopback = Loopback(
            receiver,
            messages.acknowledgement,
            IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT,
        )
        val persisted = mutableListOf<ByteArray>()
        var factoryCalls = 0

        val result = IrohaPeerNfcReaderExchangeV1.run(
            policy,
            loopback,
            IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
                factoryCalls += 1
                checkpoint(info, request, messages.payment, limits).also {
                    persisted += it.encode()
                }
            },
            IrohaPeerNfcSenderCheckpointUpdaterV1 { persisted += it.copyOf() },
            limits = limits,
        )

        assertEquals(1, factoryCalls)
        assertEquals(2, persisted.size)
        assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, receiver.phase)
        assertEquals(IrohaPeerNfcConfirmationStateV1.RESPONSE_UNKNOWN, result.confirmationState)
        assertContentEquals(
            messages.acknowledgement.encode(),
            result.acknowledgement.encode(),
        )
    }

    @Test
    fun `ordinary final confirm failure is not reported as ambiguous success`() {
        val messages = messages(requestBytes = 600, paymentBytes = 900, acknowledgementBytes = 620)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val delegate = Loopback(receiver, messages.acknowledgement)
        val definiteFailure = IrohaPeerNfcReaderTransceiverV1 { command ->
            if (command.type == IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT) {
                throw DefiniteFailure()
            }
            delegate.transceive(command)
        }

        assertFailsWith<DefiniteFailure> {
            IrohaPeerNfcReaderExchangeV1.run(
                policy,
                definiteFailure,
                IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
                    checkpoint(info, request, messages.payment, limits)
                },
                IrohaPeerNfcSenderCheckpointUpdaterV1 {},
                limits = limits,
            )
        }
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.phase)
    }

    @Test
    fun `explicit final confirm error status remains a failure`() {
        val messages = messages(requestBytes = 600, paymentBytes = 900, acknowledgementBytes = 620)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val delegate = Loopback(receiver, messages.acknowledgement)
        val rejectingConfirm = IrohaPeerNfcReaderTransceiverV1 { command ->
            if (command.type == IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT) {
                IrohaPeerNfcReaderResponseV1(
                    status = IrohaPeerNfcReaderStatusV1.CONDITIONS_NOT_SATISFIED,
                )
            } else {
                delegate.transceive(command)
            }
        }
        val persisted = mutableListOf<ByteArray>()

        val failure = assertFailsWith<IrohaPeerNfcReaderStatusExceptionV1> {
            IrohaPeerNfcReaderExchangeV1.run(
                policy,
                rejectingConfirm,
                IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
                    checkpoint(info, request, messages.payment, limits).also {
                        persisted += it.encode()
                    }
                },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { persisted += it.copyOf() },
                limits = limits,
            )
        }

        assertEquals(IrohaPeerNfcReaderStatusV1.CONDITIONS_NOT_SATISFIED, failure.status)
        assertEquals(2, persisted.size)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.phase)

        val resumed = IrohaPeerNfcReaderExchangeV1.run(
            policy,
            Loopback(
                receiver,
                messages.acknowledgement,
                IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT,
            ),
            IrohaPeerNfcSenderCheckpointStoreV1 { _, _ ->
                error("A resumed ACK-ready exchange must not create value")
            },
            IrohaPeerNfcSenderCheckpointUpdaterV1 { persisted += it.copyOf() },
            restoredCheckpoint = persisted.last(),
            limits = limits,
        )
        assertEquals(IrohaPeerNfcPhaseV1.COMPLETE, receiver.phase)
        assertContentEquals(
            messages.acknowledgement.encode(),
            resumed.acknowledgement.encode(),
        )
    }

    @Test
    fun `fresh exchange refuses a receiver already past request-ready`() {
        val messages = messages(requestBytes = 300, paymentBytes = 400, acknowledgementBytes = 200)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val begin = IrohaPeerNfcCommandV1.beginPayment(
            receiver.identity.sessionId,
            messages.request.canonicalHash,
            messages.payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        receiver.installPaymentAdmission(
            IrohaPeerNfcDurablePaymentAdmissionV1(
                (receiver.preparePaymentAdmission(begin) as
                    IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission).context,
                limits,
            ),
        )
        val loopback = Loopback(receiver, messages.acknowledgement)
        var factoryCalls = 0

        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReaderExchangeV1.run(
                policy,
                loopback,
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ ->
                    factoryCalls += 1
                    error("must not create value")
                },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { error("must not update") },
                limits = limits,
            )
        }
        assertEquals(0, factoryCalls)
        assertEquals(
            listOf(
                IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION,
                IrohaPeerNfcCommandTypeV1.GET_INFO,
            ),
            loopback.commands.map { it.type },
        )
    }

    @Test
    fun `tiny advertised request chunks cannot escape the whole-exchange budget`() {
        val messages = messages(requestBytes = 300, paymentBytes = 400, acknowledgementBytes = 200)
        val local = limits(read = 4_096, write = 4_096)
        val remote = limits(read = 1, write = 1)
        val receiver = receiver(messages, remote)
        val loopback = Loopback(receiver, messages.acknowledgement)
        var factoryCalls = 0
        var persistenceCalls = 0

        assertFailsWith<IllegalStateException> {
            IrohaPeerNfcReaderExchangeV1.run(
                policy,
                loopback,
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ ->
                    factoryCalls += 1
                    error("budget exhaustion must precede value creation")
                },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { persistenceCalls += 1 },
                limits = local,
                maximumActions = 6,
            )
        }
        assertEquals(6, loopback.commands.size)
        assertEquals(4, loopback.commands.count {
            it.type == IrohaPeerNfcCommandTypeV1.READ_REQUEST && it.length == 1
        })
        assertEquals(0, factoryCalls)
        assertEquals(0, persistenceCalls)
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, receiver.phase)
        assertTrue(
            IrohaPeerNfcReaderExchangeV1.DEFAULT_MAXIMUM_ACTIONS >=
                3 * IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES + 9,
        )
    }

    @Test
    fun `store failure and request-ready restart commit exactly one debit`() {
        val messages = messages(requestBytes = 300, paymentBytes = 400, acknowledgementBytes = 200)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val loopback = Loopback(receiver, messages.acknowledgement)
        var durableCheckpoint: ByteArray? = null
        var failNextStore = true
        var creationAttempts = 0
        var durableDebits = 0
        var storeCalls = 0
        val store = IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
            storeCalls += 1
            val existing = durableCheckpoint
            if (existing != null) {
                IrohaPeerNfcSenderCheckpointV1.decode(existing, policy, limits)
            } else {
                creationAttempts += 1
                val created = checkpoint(info, request, messages.payment, limits)
                if (failNextStore) {
                    failNextStore = false
                    throw StoreFailure()
                }
                // Models one database transaction: the monetary debit becomes
                // visible only with the exact durable ISC1.
                durableCheckpoint = created.encode()
                durableDebits += 1
                created
            }
        }
        val updater = IrohaPeerNfcSenderCheckpointUpdaterV1 {
            durableCheckpoint = it.copyOf()
        }

        assertFailsWith<StoreFailure> {
            IrohaPeerNfcReaderExchangeV1.run(policy, loopback, store, updater, limits = limits)
        }
        assertEquals(0, durableDebits)
        assertEquals(0, loopback.commands.count {
            it.type == IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT
        })

        val actionsThroughDurableStore = 3 +
            (messages.request.encode().size + limits.maximumReadChunkBytes - 1) /
            limits.maximumReadChunkBytes
        assertFailsWith<IllegalStateException> {
            IrohaPeerNfcReaderExchangeV1.run(
                policy,
                loopback,
                store,
                updater,
                limits = limits,
                maximumActions = actionsThroughDurableStore,
            )
        }
        assertEquals(1, durableDebits)
        assertEquals(0, loopback.commands.count {
            it.type == IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT
        })

        val result = IrohaPeerNfcReaderExchangeV1.run(
            policy,
            loopback,
            store,
            updater,
            limits = limits,
        )
        assertEquals(3, storeCalls)
        assertEquals(2, creationAttempts)
        assertEquals(1, durableDebits)
        assertContentEquals(messages.acknowledgement.encode(), result.acknowledgement.encode())
    }

    @Test
    fun `ack checkpoint store failure never emits confirm and resumes from payment checkpoint`() {
        val messages = messages(requestBytes = 300, paymentBytes = 400, acknowledgementBytes = 200)
        val limits = limits(read = 240, write = 203)
        val receiver = receiver(messages, limits)
        val loopback = Loopback(receiver, messages.acknowledgement)
        var durableCheckpoint: ByteArray? = null
        var failNextUpdate = true
        var updateCalls = 0
        val store = IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
            checkpoint(info, request, messages.payment, limits).also {
                durableCheckpoint = it.encode()
            }
        }
        val updater = IrohaPeerNfcSenderCheckpointUpdaterV1 {
            updateCalls += 1
            if (failNextUpdate) {
                failNextUpdate = false
                throw StoreFailure()
            }
            durableCheckpoint = it.copyOf()
        }

        assertFailsWith<StoreFailure> {
            IrohaPeerNfcReaderExchangeV1.run(policy, loopback, store, updater, limits = limits)
        }
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.phase)
        assertEquals(0, loopback.commands.count {
            it.type == IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT
        })
        val paymentOnlyCheckpoint = checkNotNull(durableCheckpoint).copyOf()
        assertEquals(
            null,
            IrohaPeerNfcSenderCheckpointV1.decode(
                paymentOnlyCheckpoint,
                policy,
                limits,
            ).durableAcknowledgement,
        )

        val result = IrohaPeerNfcReaderExchangeV1.run(
            policy,
            loopback,
            IrohaPeerNfcSenderCheckpointStoreV1 { _, _ -> error("must load restored ISC1") },
            updater,
            restoredCheckpoint = paymentOnlyCheckpoint,
            limits = limits,
        )
        assertEquals(2, updateCalls)
        assertEquals(1, loopback.commands.count {
            it.type == IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT
        })
        assertContentEquals(messages.acknowledgement.encode(), result.acknowledgement.encode())
    }

    private fun runFresh(
        messages: Messages,
        localLimits: IrohaPeerNfcLimitsV1,
        remoteLimits: IrohaPeerNfcLimitsV1,
    ): Scenario {
        val events = mutableListOf<String>()
        val loopback = Loopback(receiver(messages, remoteLimits), messages.acknowledgement, events = events)
        val persisted = mutableListOf<ByteArray>()
        var factoryCalls = 0
        val result = IrohaPeerNfcReaderExchangeV1.run(
            policy,
            loopback,
            IrohaPeerNfcSenderCheckpointStoreV1 { info, request ->
                factoryCalls += 1
                checkpoint(info, request, messages.payment, localLimits).also {
                    events += "loadOrCreate"
                    persisted += it.encode()
                }
            },
            IrohaPeerNfcSenderCheckpointUpdaterV1 {
                events += "update"
                persisted += it.copyOf()
            },
            limits = localLimits,
        )
        assertEquals(1, factoryCalls)
        return Scenario(result, loopback.commands, persisted, events)
    }

    private fun checkpoint(
        info: IrohaPeerNfcInfoV1,
        request: IrohaPeerWireMessageV1,
        payment: IrohaPeerWireMessageV1,
        limits: IrohaPeerNfcLimitsV1,
    ) = IrohaPeerNfcSenderCheckpointV1(
        info.identity.sessionId,
        request.encode(),
        payment.encode(),
        profilePolicy = policy,
        limits = limits,
    )

    private fun receiver(messages: Messages, limits: IrohaPeerNfcLimitsV1) =
        IrohaPeerNfcReceiverSessionV1(
            SESSION,
            messages.request.encode(),
            profilePolicy = policy,
            limits = limits,
        )

    private fun messages(
        requestBytes: Int,
        paymentBytes: Int,
        acknowledgementBytes: Int,
    ) = Messages(
        message(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x41, requestBytes),
        message(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, IrohaPeerPayloadKind.PAYMENT, 0x42, paymentBytes),
        message(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x43, acknowledgementBytes),
    )

    private fun message(
        profile: IrohaPeerPayloadProfile,
        kind: IrohaPeerPayloadKind,
        byte: Int,
        count: Int,
    ) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
        profile,
        kind,
        profile.requiredSchemaVersion,
        IrohaPeerKagemushaStructuralTestV1.archive(
            kind,
            ByteArray(count) { byte.toByte() },
        ),
    ))

    private fun limits(read: Int, write: Int) = IrohaPeerNfcLimitsV1(
        maximumReadChunkBytes = read,
        maximumWriteChunkBytes = write,
    )

    private class Loopback(
        private val receiver: IrohaPeerNfcReceiverSessionV1,
        private val acknowledgement: IrohaPeerWireMessageV1,
        private val loseAfter: IrohaPeerNfcCommandTypeV1? = null,
        private val events: MutableList<String> = mutableListOf(),
    ) : IrohaPeerNfcReaderTransceiverV1 {
        val commands = mutableListOf<IrohaPeerNfcCommandV1>()
        private var didLose = false

        override fun transceive(command: IrohaPeerNfcCommandV1): IrohaPeerNfcReaderResponseV1 {
            commands += command
            events += command.type.name
            val response = when (command.type) {
                IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT -> when (
                    val disposition = receiver.preparePaymentAdmission(command)
                ) {
                    IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted -> byteArrayOf()
                    is IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission -> {
                        receiver.installPaymentAdmission(
                            IrohaPeerNfcDurablePaymentAdmissionV1(
                                disposition.context,
                                receiver.limits,
                            ),
                        )
                        byteArrayOf()
                    }
                }
                IrohaPeerNfcCommandTypeV1.COMMIT -> when (val disposition = receiver.prepareCommit(command)) {
                    IrohaPeerNfcCommitDispositionV1.AlreadyCommitted -> byteArrayOf()
                    is IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit -> {
                        receiver.installDurableAcknowledgement(IrohaPeerNfcDurableAcknowledgementV1(
                            disposition.context,
                            acknowledgement.encode(),
                            receiver.limits,
                        ))
                        byteArrayOf()
                    }
                }
                else -> receiver.handle(command)
            }
            if (!didLose && command.type == loseAfter) {
                didLose = true
                throw LostResponse()
            }
            return IrohaPeerNfcReaderResponseV1.success(response)
        }
    }

    private class LostResponse : RuntimeException(), IrohaPeerNfcAmbiguousResponseErrorV1

    private class DefiniteFailure : RuntimeException()

    private class StoreFailure : RuntimeException()

    private class Messages(
        val request: IrohaPeerWireMessageV1,
        val payment: IrohaPeerWireMessageV1,
        val acknowledgement: IrohaPeerWireMessageV1,
    )

    private class Scenario(
        val result: IrohaPeerNfcReaderExchangeResultV1,
        val commands: List<IrohaPeerNfcCommandV1>,
        val persisted: List<ByteArray>,
        val events: List<String>,
    )

    companion object {
        private val SESSION = ByteArray(16) { (it + 1).toByte() }
        private val policy =
            IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND)
    }
}
