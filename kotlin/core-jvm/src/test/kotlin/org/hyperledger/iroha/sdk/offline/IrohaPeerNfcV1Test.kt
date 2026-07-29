package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue

class IrohaPeerNfcV1Test {
    @Test
    fun `message limit cannot exceed portable V1 maximum`() {
        IrohaPeerNfcLimitsV1(
            IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES,
            IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
            IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcLimitsV1(
                IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES + 1,
                IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
                IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
            )
        }
    }

    @Test
    fun `application identifier access cannot mutate process-wide NFC state`() {
        val expected = IrohaPeerNfcV1.applicationIdentifier()
        val callerCopy = IrohaPeerNfcV1.applicationIdentifier()
        callerCopy.fill(0)

        assertContentEquals(expected, IrohaPeerNfcV1.applicationIdentifier())
        val select = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.SELECT_APPLICATION,
        )
        assertEquals(
            IrohaPeerNfcCommandV1.SELECT_APPLICATION,
            IrohaPeerNfcAPDUCodecV1.decode(select),
        )
        assertContentEquals(expected, select.copyOfRange(5, 5 + expected.size))
    }

    @Test
    fun `reader intersects local and remote limits in both directions`() {
        listOf(240 to 4_096, 4_096 to 240).forEach { (localChunk, remoteChunk) ->
            val policy = kagemushaPolicy()
            val session = ByteArray(16) { (it + 1).toByte() }
            val request = IrohaPeerKagemushaStructuralTestV1.message(
                IrohaPeerPayloadKind.RECEIVE_REQUEST,
                ByteArray(900) { 0x61 },
            )
            val payment = IrohaPeerKagemushaStructuralTestV1.message(
                IrohaPeerPayloadKind.PAYMENT,
                ByteArray(1_100) { 0x62 },
            )
            val acknowledgement = IrohaPeerKagemushaStructuralTestV1.message(
                IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
                ByteArray(700) { 0x63 },
            )
            val local = IrohaPeerNfcLimitsV1(
                maximumReadChunkBytes = localChunk,
                maximumWriteChunkBytes = localChunk,
            )
            val remote = IrohaPeerNfcLimitsV1(
                maximumReadChunkBytes = remoteChunk,
                maximumWriteChunkBytes = remoteChunk,
            )
            val expectedChunk = minOf(localChunk, remoteChunk)
            val receiver = IrohaPeerNfcReceiverSessionV1(
                session,
                request.encode(),
                profilePolicy = policy,
                limits = remote,
            )
            val readRequest = IrohaPeerNfcReaderPlanningV1.readRequestCommand(
                receiver.info(),
                0,
                local,
            )
            assertEquals(expectedChunk, readRequest.length)

            val reducer = IrohaPeerNfcTwoTapReducerV1(IrohaPeerNfcSenderCheckpointV1(
                session,
                request.encode(),
                payment.encode(),
                profilePolicy = policy,
                limits = remote,
            ), local)
            val begin = assertIs<IrohaPeerNfcSenderActionV1.Send>(
                reducer.nextAction(receiver.status()),
            )
            receiver.installPaymentAdmission(
                IrohaPeerNfcDurablePaymentAdmissionV1(
                    assertIs<IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission>(
                        receiver.preparePaymentAdmission(begin.command),
                    ).context,
                    remote,
                ),
            )
            val firstWrite = assertIs<IrohaPeerNfcSenderActionV1.Send>(
                reducer.nextAction(receiver.status()),
            )
            assertEquals(IrohaPeerNfcCommandTypeV1.WRITE, firstWrite.command.type)
            assertEquals(expectedChunk, firstWrite.command.bytes.size)
            receiver.handle(firstWrite.command)

            while (receiver.status().receivedPaymentBytes < payment.encode().size) {
                val send = assertIs<IrohaPeerNfcSenderActionV1.Send>(
                    reducer.nextAction(receiver.status()),
                )
                receiver.handle(send.command)
            }
            val commit = assertIs<IrohaPeerNfcSenderActionV1.Send>(
                reducer.nextAction(receiver.status()),
            )
            val required = assertIs<IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit>(
                receiver.prepareCommit(commit.command),
            )
            receiver.installDurableAcknowledgement(IrohaPeerNfcDurableAcknowledgementV1(
                required.context,
                acknowledgement.encode(),
                remote,
            ))
            val firstReadAck = assertIs<IrohaPeerNfcSenderActionV1.Send>(
                reducer.nextAction(receiver.status()),
            )
            assertEquals(IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT, firstReadAck.command.type)
            assertEquals(expectedChunk, firstReadAck.command.length)
        }
    }

    @Test
    fun `receiver retries writes and commits only after durable acknowledgement`() {
        val session = ByteArray(16) { (it + 1).toByte() }
        val messages = currentMessages()
        val policy = kagemushaPolicy()
        val limits = IrohaPeerNfcLimitsV1(
            maximumReadChunkBytes = 97,
            maximumWriteChunkBytes = 113,
        )
        val receiver = IrohaPeerNfcReceiverSessionV1(
            session,
            messages.request.encode(),
            profilePolicy = policy,
            limits = limits,
        )
        val begin = IrohaPeerNfcCommandV1.beginPayment(
            session,
            messages.request.canonicalHash,
            messages.payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        val admission = assertIs<IrohaPeerNfcPaymentAdmissionDispositionV1.RequiresDurableAdmission>(
            receiver.preparePaymentAdmission(begin),
        ).context
        assertFailsWith<IllegalStateException> { receiver.handle(begin) }
        assertContentEquals(
            messages.payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
            admission.paymentHeader,
        )
        val admissionRecord = IrohaPeerNfcDurablePaymentAdmissionV1(admission, limits)
        receiver.installPaymentAdmission(admissionRecord)
        receiver.installPaymentAdmission(admissionRecord)
        assertIs<IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted>(
            receiver.preparePaymentAdmission(begin),
        )

        val paymentBytes = messages.payment.encode()
        receiver.handle(IrohaPeerNfcCommandV1.write(
            session,
            messages.payment.wireHash,
            0,
            paymentBytes.copyOfRange(0, 100),
        ))
        receiver.handle(IrohaPeerNfcCommandV1.write(
            session,
            messages.payment.wireHash,
            50,
            paymentBytes.copyOfRange(50, 150),
        ))
        assertEquals(150, receiver.status().receivedPaymentBytes)
        val conflicting = paymentBytes.copyOfRange(50, 100)
        conflicting[0] = (conflicting[0].toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.write(
                session,
                messages.payment.wireHash,
                50,
                conflicting,
            ))
        }
        var offset = 150
        while (offset < paymentBytes.size) {
            val end = minOf(offset + limits.maximumWriteChunkBytes, paymentBytes.size)
            receiver.handle(IrohaPeerNfcCommandV1.write(
                session,
                messages.payment.wireHash,
                offset.toLong(),
                paymentBytes.copyOfRange(offset, end),
            ))
            offset = end
        }

        val commit = IrohaPeerNfcCommandV1.commit(
            session,
            messages.request.canonicalHash,
            messages.payment.wireHash,
        )
        val required = assertIs<IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit>(
            receiver.prepareCommit(commit),
        )
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, receiver.phase)
        assertFailsWith<IllegalStateException> {
            receiver.handle(IrohaPeerNfcCommandV1.readAcknowledgement(
                session,
                messages.payment.wireHash,
                0,
                32,
            ))
        }
        val durable = IrohaPeerNfcDurableAcknowledgementV1(
            required.context,
            messages.acknowledgement.encode(),
            limits,
        )
        receiver.installDurableAcknowledgement(durable)
        receiver.installDurableAcknowledgement(durable)
        assertIs<IrohaPeerNfcCommitDispositionV1.AlreadyCommitted>(receiver.prepareCommit(commit))
        assertFailsWith<IllegalStateException> { receiver.preparePaymentAdmission(begin) }
        val status = receiver.status()
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, status.phase)
        assertEquals(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, status.paymentProfile)
        assertEquals(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND, status.acknowledgementProfile)
        assertTrue(status.flags.contains(IrohaPeerNfcFlagsV1.DURABLE_ACKNOWLEDGEMENT))

        val restoredPartial = IrohaPeerNfcReceiverSessionV1(
            session,
            messages.request.encode(),
            profilePolicy = policy,
            limits = limits,
            restoredPaymentAdmission = admissionRecord,
        )
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, restoredPartial.phase)
        assertEquals(0, restoredPartial.status().receivedPaymentBytes)
        assertIs<IrohaPeerNfcPaymentAdmissionDispositionV1.AlreadyAdmitted>(
            restoredPartial.preparePaymentAdmission(begin),
        )
        restoredPartial.handle(IrohaPeerNfcCommandV1.write(
            session,
            messages.payment.wireHash,
            0,
            paymentBytes.copyOfRange(0, 100),
        ))
        assertEquals(100, restoredPartial.status().receivedPaymentBytes)

        val restoredCommitted = IrohaPeerNfcReceiverSessionV1(
            session,
            messages.request.encode(),
            durable,
            policy,
            limits,
            admissionRecord,
        )
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, restoredCommitted.phase)
        assertFailsWith<IllegalStateException> {
            restoredCommitted.preparePaymentAdmission(begin)
        }
        val differentPayment = IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(200) { 0x7f },
        )
        val conflictingAdmission = IrohaPeerNfcDurablePaymentAdmissionV1(
            IrohaPeerNfcPaymentAdmissionContextV1(
                receiver.identity,
                policy,
                differentPayment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
                limits,
            ),
            limits,
        )
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReceiverSessionV1(
                session,
                messages.request.encode(),
                durable,
                policy,
                limits,
                conflictingAdmission,
            )
        }

        val reducer = IrohaPeerNfcTwoTapReducerV1(IrohaPeerNfcSenderCheckpointV1(
            session,
            messages.request.encode(),
            messages.payment.encode(),
            profilePolicy = policy,
            limits = limits,
        ), limits)
        while (reducer.checkpoint.durableAcknowledgement == null) {
            when (val action = reducer.nextAction(receiver.status())) {
                is IrohaPeerNfcSenderActionV1.Send -> {
                    assertEquals(IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT, action.command.type)
                    assertTrue(reducer.consumeAcknowledgementChunk(receiver.handle(action.command)) ||
                        reducer.checkpoint.durableAcknowledgement == null)
                }
                is IrohaPeerNfcSenderActionV1.PersistAcknowledgement -> {
                    assertContentEquals(messages.acknowledgement.encode(), action.bytes)
                    reducer.persistAcknowledgement { persisted ->
                        assertEquals(reducer.checkpoint.encode().size + messages.acknowledgement.encode().size,
                            persisted.size)
                    }
                }
                is IrohaPeerNfcSenderActionV1.Complete -> error("Cannot complete before CONFIRM_ACK")
            }
        }
        val confirm = assertIs<IrohaPeerNfcSenderActionV1.Send>(reducer.nextAction(receiver.status()))
        assertEquals(IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT, confirm.command.type)
        receiver.handle(confirm.command)
        val complete = assertIs<IrohaPeerNfcSenderActionV1.Complete>(reducer.nextAction(receiver.status()))
        assertContentEquals(messages.acknowledgement.encode(), complete.bytes)
    }

    private fun currentMessages() = Messages(
        IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.RECEIVE_REQUEST,
            ByteArray(900) { 0x31 },
        ),
        IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.PAYMENT,
            ByteArray(1_100) { 0x32 },
        ),
        IrohaPeerKagemushaStructuralTestV1.message(
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
            ByteArray(700) { 0x33 },
        ),
    )

    private fun kagemushaPolicy() =
        IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND)

    private class Messages(
        val request: IrohaPeerWireMessageV1,
        val payment: IrohaPeerWireMessageV1,
        val acknowledgement: IrohaPeerWireMessageV1,
    )

}
