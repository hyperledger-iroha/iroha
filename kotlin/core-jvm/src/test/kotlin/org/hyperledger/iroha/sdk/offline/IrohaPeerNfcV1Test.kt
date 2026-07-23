package org.hyperledger.iroha.sdk.offline

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.junit.jupiter.api.Test
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
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
        val expected = fixture().hex("aid_hex")
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
    fun `matches shared INF1 NST1 and APDU vectors`() {
        val fixture = fixture()
        val session = fixture.hex("session_hex")
        assertEquals(fixture.text("aid_hex"), IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX)
        assertContentEquals(fixture.hex("aid_hex"), IrohaPeerNfcV1.applicationIdentifier())

        val syntheticIdentity = IrohaPeerNfcRequestIdentityV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            session,
            ByteArray(32) { 0x11 },
            ByteArray(32) { 0x22 },
        )
        val info = IrohaPeerNfcInfoV1(
            IrohaPeerNfcPhaseV1.REQUEST_READY,
            IrohaPeerNfcFlagsV1.REQUEST,
            syntheticIdentity,
            300,
            240,
            240,
        )
        assertContentEquals(fixture.hex("info_hex"), info.encode())
        assertEquals(info, IrohaPeerNfcInfoV1.decode(info.encode()))

        val status = IrohaPeerNfcStatusV1(
            IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY,
            IrohaPeerNfcFlagsV1.DURABLE,
            syntheticIdentity,
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            700,
            700,
            ByteArray(32) { 0x33 },
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            270,
            ByteArray(32) { 0x44 },
            240,
            240,
        )
        assertContentEquals(fixture.hex("ack_ready_status_hex"), status.encode())
        assertEquals(status, IrohaPeerNfcStatusV1.decode(status.encode()))

        val messages = messages(fixture)
        val request = messages.request
        val payment = messages.payment
        val acknowledgement = messages.acknowledgement
        val apdu = fixture.getValue("apdu_hex").jsonObject
        val commands = linkedMapOf(
            "select" to IrohaPeerNfcCommandV1.SELECT_APPLICATION,
            "get_info" to IrohaPeerNfcCommandV1.GET_INFO,
            "read_request" to IrohaPeerNfcCommandV1.readRequest(
                session,
                request.canonicalHash,
                0x0102_0304,
                240,
            ),
            "begin_payment" to IrohaPeerNfcCommandV1.beginPayment(
                session,
                request.canonicalHash,
                payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
            ),
            "write_300" to IrohaPeerNfcCommandV1.write(
                session,
                payment.wireHash,
                0x0102_0304,
                ByteArray(300) { 0x55 },
            ),
            "commit" to IrohaPeerNfcCommandV1.commit(
                session,
                request.canonicalHash,
                payment.wireHash,
            ),
            "read_ack_1024" to IrohaPeerNfcCommandV1.readAcknowledgement(
                session,
                payment.wireHash,
                0x0102_0304,
                1_024,
            ),
            "confirm_ack" to IrohaPeerNfcCommandV1.confirmAcknowledgement(
                session,
                payment.wireHash,
                acknowledgement.wireHash,
            ),
            "get_status" to IrohaPeerNfcCommandV1.getStatus(
                session,
                request.canonicalHash,
            ),
        )
        commands.forEach { (name, command) ->
            val encoded = IrohaPeerNfcAPDUCodecV1.encode(command)
            assertContentEquals(apdu.hex(name), encoded, name)
            assertEquals(command, IrohaPeerNfcAPDUCodecV1.decode(encoded), name)
        }
    }

    @Test
    fun `matches all-retail durable and checkpoint vectors`() {
        val fixture = fixture()
        val session = fixture.hex("session_hex")
        val messages = messages(fixture)
        val policy = retailPolicy()
        val checkpoint = IrohaPeerNfcSenderCheckpointV1(
            session,
            messages.request.encode(),
            messages.payment.encode(),
            profilePolicy = policy,
        )
        val checkpointFixture = fixture.getValue("checkpoint").jsonObject
        assertEquals(checkpointFixture.int("without_ack_length"), checkpoint.encode().size)
        assertEquals(
            checkpointFixture.text("without_ack_blake2b_256_hex"),
            Blake2b.digest256(checkpoint.encode()).hex(),
        )
        assertEquals(checkpoint, IrohaPeerNfcSenderCheckpointV1.decode(
            checkpoint.encode(),
            policy,
        ))
        assertEquals(checkpoint, IrohaPeerNfcSenderCheckpointV1.decode(checkpoint.encode()))

        val identity = IrohaPeerNfcRequestIdentityV1(
            messages.request.canonicalPayload.profile,
            session,
            messages.request.canonicalHash,
            messages.request.wireHash,
        )
        val admissionContext = IrohaPeerNfcPaymentAdmissionContextV1(
            identity,
            policy,
            messages.payment.encode().copyOfRange(0, IrohaPeerWireMessageV1.HEADER_LENGTH),
        )
        val admission = IrohaPeerNfcDurablePaymentAdmissionV1(admissionContext)
        val admissionFixture = fixture.getValue("payment_admission").jsonObject
        assertEquals(admissionFixture.int("length"), admission.encode().size)
        assertContentEquals(
            admissionFixture.hex("encoded_hex"),
            admission.encode(),
        )
        assertEquals(
            admissionFixture.text("blake2b_256_hex"),
            Blake2b.digest256(admission.encode()).hex(),
        )
        assertEquals(admission, IrohaPeerNfcDurablePaymentAdmissionV1.decode(
            admission.encode(),
            policy,
        ))
        assertEquals(admission, IrohaPeerNfcDurablePaymentAdmissionV1.decode(admission.encode()))

        val context = IrohaPeerNfcCommitContextV1(identity, policy, messages.payment)
        val durable = IrohaPeerNfcDurableAcknowledgementV1(
            context,
            messages.acknowledgement.encode(),
        )
        val durableFixture = fixture.getValue("durable_ack").jsonObject
        assertEquals(durableFixture.int("length"), durable.encode().size)
        assertEquals(
            durableFixture.text("blake2b_256_hex"),
            Blake2b.digest256(durable.encode()).hex(),
        )
        assertEquals(durable, IrohaPeerNfcDurableAcknowledgementV1.decode(
            durable.encode(),
            policy,
        ))
        assertEquals(durable, IrohaPeerNfcDurableAcknowledgementV1.decode(durable.encode()))

        val checkpointWithAck = IrohaPeerNfcSenderCheckpointV1(
            session,
            messages.request.encode(),
            messages.payment.encode(),
            messages.acknowledgement.encode(),
            policy,
        )
        assertEquals(checkpointFixture.int("with_ack_length"), checkpointWithAck.encode().size)
        assertEquals(
            checkpointFixture.text("with_ack_blake2b_256_hex"),
            Blake2b.digest256(checkpointWithAck.encode()).hex(),
        )
        assertEquals(checkpointWithAck, IrohaPeerNfcSenderCheckpointV1.decode(
            checkpointWithAck.encode(),
            policy,
        ))
    }

    @Test
    fun `reader intersects local and remote limits in both directions`() {
        listOf(240 to 4_096, 4_096 to 240).forEach { (localChunk, remoteChunk) ->
            val policy = retailPolicy()
            val session = ByteArray(16) { (it + 1).toByte() }
            val request = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.RECEIVE_REQUEST,
                1,
                ByteArray(900) { 0x61 },
            ))
            val payment = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.PAYMENT,
                1,
                ByteArray(1_100) { 0x62 },
            ))
            val acknowledgement = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.ACKNOWLEDGEMENT,
                1,
                ByteArray(700) { 0x63 },
            ))
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
        val fixture = fixture()
        val session = fixture.hex("session_hex")
        val messages = messages(fixture)
        val policy = retailPolicy()
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
        assertEquals(IrohaPeerPayloadProfile.OFFLINE_NOTE, status.paymentProfile)
        assertEquals(IrohaPeerPayloadProfile.OFFLINE_NOTE, status.acknowledgementProfile)
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
        val differentPayment = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            ByteArray(200) { 0x7f },
        ))
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

    private fun messages(fixture: JsonObject): Messages {
        val vectors = fixture.getValue("messages").jsonObject
        val request = message(vectors.getValue("request").jsonObject)
        val payment = message(vectors.getValue("payment").jsonObject)
        val acknowledgement = message(vectors.getValue("acknowledgement").jsonObject)
        return Messages(request, payment, acknowledgement)
    }

    private fun message(vector: JsonObject): IrohaPeerWireMessageV1 {
        val profile = requireNotNull(IrohaPeerPayloadProfile.fromCode(vector.int("profile")))
        val kind = requireNotNull(IrohaPeerPayloadKind.fromCode(vector.int("kind")))
        val message = IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            profile,
            kind,
            vector.int("schema_version"),
            ByteArray(vector.int("count")) { vector.int("repeat_byte").toByte() },
        ))
        assertEquals(vector.text("wire_hash_hex"), message.wireHash.hex())
        return message
    }

    private fun retailPolicy() =
        IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.OFFLINE_NOTE)

    private class Messages(
        val request: IrohaPeerWireMessageV1,
        val payment: IrohaPeerWireMessageV1,
        val acknowledgement: IrohaPeerWireMessageV1,
    )

    private fun fixture(): JsonObject = Json.parseToJsonElement(
        String(Files.readAllBytes(sharedFixture()), Charsets.UTF_8),
    ).jsonObject

    private fun JsonObject.text(key: String): String = getValue(key).jsonPrimitive.content

    private fun JsonObject.int(key: String): Int = text(key).toInt()

    private fun JsonObject.hex(key: String): ByteArray {
        val value = getValue(key)
        val encoded = if (value is JsonArray) {
            value.joinToString("") { it.jsonPrimitive.content }
        } else {
            value.jsonPrimitive.content
        }
        return encoded.hexBytes()
    }

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private fun String.hexBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private fun sharedFixture(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = current.resolve("fixtures/offline/peer_nfc_v1.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: error("peer_nfc_v1.json was not found")
        }
    }
}
