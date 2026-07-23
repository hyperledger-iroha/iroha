package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class IrohaPeerNfcAdversarialV1Test {
    private val session = ByteArray(16) { (it + 1).toByte() }

    @Test
    fun `non-canonical extended APDU aliases are rejected`() {
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcAPDUCodecV1.decode(byteArrayOf(
                0x80.toByte(), 0x10, 0, 0, 0, 0, 0x62,
            ))
        }

        val hash = ByteArray(32) { 0x31 }
        val write = IrohaPeerNfcAPDUCodecV1.encode(IrohaPeerNfcCommandV1.write(
            session,
            hash,
            0,
            byteArrayOf(0x55),
        ))
        val writeBody = write.copyOfRange(5, write.size)
        val aliasedWrite = byteArrayOf(
            0x80.toByte(), 0x21, 0, 0, 0, 0, writeBody.size.toByte(),
        ) + writeBody
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcAPDUCodecV1.decode(aliasedWrite)
        }

        val status = IrohaPeerNfcAPDUCodecV1.encode(
            IrohaPeerNfcCommandV1.getStatus(session, hash),
        )
        val statusBody = status.copyOfRange(5, status.size - 1)
        val aliasedStatus = byteArrayOf(
            0x80.toByte(), 0x25, 0, 0, 0, 0, statusBody.size.toByte(),
        ) + statusBody + byteArrayOf(0, IrohaPeerNfcV1.STATUS_BYTES.toByte())
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcAPDUCodecV1.decode(aliasedStatus)
        }
    }

    @Test
    fun `hash-correct wrong-phase header and extreme offsets do not mutate receiver`() {
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x41, 180)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x42, 300)
        val acknowledgement = message(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x43, 120)
        val receiver = IrohaPeerNfcReceiverSessionV1(
            session,
            request.encode(),
            limits = limits(maximumMessage = IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES),
        )

        assertFailsWith<IllegalArgumentException> {
            receiver.preparePaymentAdmission(IrohaPeerNfcCommandV1.beginPayment(
                session,
                request.canonicalHash,
                acknowledgement.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
            ))
        }
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, receiver.phase)
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.readRequest(
                session,
                request.canonicalHash,
                0xffff_ffffL,
                1,
            ))
        }

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
            payment.encode().copyOfRange(0, 100),
        ))
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.write(
                session,
                payment.wireHash,
                0xffff_ffffL,
                byteArrayOf(1),
            ))
        }
        val conflicting = payment.encode().copyOfRange(50, 150)
        conflicting[0] = (conflicting[0].toInt() xor 0x80).toByte()
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.write(
                session,
                payment.wireHash,
                50,
                conflicting,
            ))
        }
        assertEquals(100, receiver.status().receivedPaymentBytes)
    }

    @Test
    fun `peer advertisements and persisted u32 lengths respect local bounds`() {
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x51, 300)
        val payment = message(IrohaPeerPayloadKind.PAYMENT, 0x52, 300)
        val identity = IrohaPeerNfcRequestIdentityV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            session,
            request.canonicalHash,
            request.wireHash,
        )
        val info = IrohaPeerNfcInfoV1(
            IrohaPeerNfcPhaseV1.REQUEST_READY,
            IrohaPeerNfcFlagsV1.REQUEST,
            identity,
            request.encode().size,
            240,
            240,
        )
        val local = limits(maximumMessage = 128)
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReaderPlanningV1.readRequestCommand(info, 0, local)
        }

        val hostileInfo = info.encode()
        hostileInfo[94] = 0xff.toByte()
        hostileInfo[95] = 0xff.toByte()
        assertFailsWith<IllegalArgumentException> { IrohaPeerNfcInfoV1.decode(hostileInfo) }

        val checkpoint = IrohaPeerNfcSenderCheckpointV1(
            session,
            request.encode(),
            payment.encode(),
        ).encode()
        for (index in 24..27) checkpoint[index] = 0xff.toByte()
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcSenderCheckpointV1.decode(checkpoint)
        }
    }

    @Test
    fun `reader rejects oversize request before read or value creation`() {
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x61, 300)
        val identity = IrohaPeerNfcRequestIdentityV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            session,
            request.canonicalHash,
            request.wireHash,
        )
        val info = IrohaPeerNfcInfoV1(
            IrohaPeerNfcPhaseV1.REQUEST_READY,
            IrohaPeerNfcFlagsV1.REQUEST,
            identity,
            request.encode().size,
            240,
            240,
        )
        val commands = mutableListOf<IrohaPeerNfcCommandTypeV1>()
        var valueCreationCalls = 0
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReaderExchangeV1.run(
                IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.OFFLINE_NOTE),
                IrohaPeerNfcReaderTransceiverV1 { command ->
                    commands += command.type
                    when (command.type) {
                        IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION ->
                            IrohaPeerNfcReaderResponseV1.success()
                        IrohaPeerNfcCommandTypeV1.GET_INFO ->
                            IrohaPeerNfcReaderResponseV1.success(info.encode())
                        else -> error("Oversize INF1 reached a request read")
                    }
                },
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ ->
                    valueCreationCalls += 1
                    error("Oversize INF1 reached value creation")
                },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { error("Unexpected update") },
                limits = limits(maximumMessage = 128),
            )
        }
        assertEquals(0, valueCreationCalls)
        assertEquals(
            listOf(
                IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION,
                IrohaPeerNfcCommandTypeV1.GET_INFO,
            ),
            commands,
        )
    }

    @Test
    fun `reader rejects unexpected control response data`() {
        val commands = mutableListOf<IrohaPeerNfcCommandTypeV1>()
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReaderExchangeV1.run(
                IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.OFFLINE_NOTE),
                IrohaPeerNfcReaderTransceiverV1 { command ->
                    commands += command.type
                    IrohaPeerNfcReaderResponseV1.success(byteArrayOf(0))
                },
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ -> error("Unexpected creation") },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { error("Unexpected update") },
            )
        }
        assertContentEquals(
            arrayOf(IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION),
            commands.toTypedArray(),
        )
    }

    @Test
    fun `reader rejects response longer than requested read`() {
        val request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x71, 120)
        val identity = IrohaPeerNfcRequestIdentityV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            session,
            request.canonicalHash,
            request.wireHash,
        )
        val info = IrohaPeerNfcInfoV1(
            IrohaPeerNfcPhaseV1.REQUEST_READY,
            IrohaPeerNfcFlagsV1.REQUEST,
            identity,
            request.encode().size,
            1,
            1,
        )
        val commands = mutableListOf<IrohaPeerNfcCommandTypeV1>()
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerNfcReaderExchangeV1.run(
                IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.OFFLINE_NOTE),
                IrohaPeerNfcReaderTransceiverV1 { command ->
                    commands += command.type
                    when (command.type) {
                        IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION ->
                            IrohaPeerNfcReaderResponseV1.success()
                        IrohaPeerNfcCommandTypeV1.GET_INFO ->
                            IrohaPeerNfcReaderResponseV1.success(info.encode())
                        IrohaPeerNfcCommandTypeV1.READ_REQUEST ->
                            IrohaPeerNfcReaderResponseV1.success(byteArrayOf(0x71, 0x71))
                        else -> error("Unexpected command")
                    }
                },
                IrohaPeerNfcSenderCheckpointStoreV1 { _, _ -> error("Unexpected creation") },
                IrohaPeerNfcSenderCheckpointUpdaterV1 { error("Unexpected update") },
            )
        }
        assertEquals(
            listOf(
                IrohaPeerNfcCommandTypeV1.SELECT_APPLICATION,
                IrohaPeerNfcCommandTypeV1.GET_INFO,
                IrohaPeerNfcCommandTypeV1.READ_REQUEST,
            ),
            commands,
        )
    }

    private fun limits(maximumMessage: Int) = IrohaPeerNfcLimitsV1(
        maximumMessageBytes = maximumMessage,
        maximumReadChunkBytes = 128,
        maximumWriteChunkBytes = 128,
    )

    private fun message(
        kind: IrohaPeerPayloadKind,
        repeated: Int,
        count: Int,
    ) = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
        IrohaPeerPayloadProfile.OFFLINE_NOTE,
        kind,
        1,
        ByteArray(count) { repeated.toByte() },
    ))
}
