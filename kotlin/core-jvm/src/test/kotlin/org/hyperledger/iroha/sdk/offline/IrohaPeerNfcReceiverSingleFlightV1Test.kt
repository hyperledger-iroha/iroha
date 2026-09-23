package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class IrohaPeerNfcReceiverSingleFlightV1Test {
    @Test
    fun `two pending commits cannot stage twice and a late failure cannot erase the ack`() {
        val fixture = fixture()
        val request = message(fixture, "payment_request", IrohaPeerPayloadKind.REQUEST)
        val payment = message(fixture, "payment", IrohaPeerPayloadKind.PAYMENT)
        val acknowledgement = message(fixture, "acknowledgement", IrohaPeerPayloadKind.ACKNOWLEDGEMENT)
        val receiver = receiver(request)
        writePayment(receiver, payment)

        val first = receiver.handle(IrohaPeerNfcCommandV1.COMMIT_PAYMENT)
            as IrohaPeerNfcPaymentAdmissionDispositionV1.Persist
        assertFailsWith<IllegalArgumentException> { receiver.handle(IrohaPeerNfcCommandV1.COMMIT_PAYMENT) }
        assertFailsWith<IllegalArgumentException> { receiver.handle(IrohaPeerNfcCommandV1.RESET_SESSION) }
        receiver.completePayment(first.context, IrohaPeerNfcDurablePaymentAdmissionV1(first.context, acknowledgement))
        receiver.rejectPayment(first.context)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.status().phase)
        assertContentEquals(
            acknowledgement,
            receiver.handle(IrohaPeerNfcCommandV1.readAcknowledgement(0, acknowledgement.size)) as ByteArray,
        )
    }

    @Test
    fun `deactivation permits exact retry while stale context cannot reject it`() {
        val fixture = fixture()
        val request = message(fixture, "payment_request", IrohaPeerPayloadKind.REQUEST)
        val payment = message(fixture, "payment", IrohaPeerPayloadKind.PAYMENT)
        val acknowledgement = message(fixture, "acknowledgement", IrohaPeerPayloadKind.ACKNOWLEDGEMENT)
        val receiver = receiver(request)
        writePayment(receiver, payment)
        val old = (receiver.handle(IrohaPeerNfcCommandV1.COMMIT_PAYMENT)
            as IrohaPeerNfcPaymentAdmissionDispositionV1.Persist).context
        receiver.abandonPendingPayment()
        assertEquals(IrohaPeerNfcPhaseV1.REQUEST_READY, receiver.status().phase)
        assertFailsWith<IllegalArgumentException> { receiver.handle(IrohaPeerNfcCommandV1.RESET_SESSION) }

        val exactDescriptor = IrohaPeerNfcPaymentDescriptorV1(IrohaPeerWireMessageV1.decode(payment))
        val changedDescriptor = IrohaPeerNfcPaymentDescriptorV1.decode(
            exactDescriptor.encode().copyOf().apply { this[7] = (this[7].toInt() xor 1).toByte() },
        )
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.beginPayment(changedDescriptor))
        }
        receiver.handle(IrohaPeerNfcCommandV1.beginPayment(exactDescriptor))
        assertFailsWith<IllegalArgumentException> {
            receiver.handle(IrohaPeerNfcCommandV1.writePayment(0, payment.copyOf().apply {
                this[lastIndex] = (this[lastIndex].toInt() xor 1).toByte()
            }))
        }
        assertEquals(0, receiver.status().receivedPaymentBytes)

        receiver.handle(IrohaPeerNfcCommandV1.writePayment(0, payment))
        val retry = (receiver.handle(IrohaPeerNfcCommandV1.COMMIT_PAYMENT)
            as IrohaPeerNfcPaymentAdmissionDispositionV1.Persist).context
        receiver.rejectPayment(old)
        assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, receiver.status().phase)
        receiver.completePayment(retry, IrohaPeerNfcDurablePaymentAdmissionV1(retry, acknowledgement))
        receiver.rejectPayment(old)
        receiver.rejectPayment(retry)
        assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.status().phase)
    }

    private fun receiver(request: ByteArray) = IrohaPeerNfcReceiverSessionV1(
        request,
        ByteArray(IrohaPeerNfcV1.SESSION_ID_BYTES) { 1 },
        IrohaPeerNfcProfilePolicyV1(IrohaPeerPayloadProfile.KAGEMUSHA_V1),
    )

    private fun writePayment(receiver: IrohaPeerNfcReceiverSessionV1, payment: ByteArray) {
        receiver.handle(IrohaPeerNfcCommandV1.beginPayment(IrohaPeerNfcPaymentDescriptorV1(
            IrohaPeerWireMessageV1.decode(payment),
        )))
        receiver.handle(IrohaPeerNfcCommandV1.writePayment(0, payment))
    }

    private fun message(fixture: String, section: String, kind: IrohaPeerPayloadKind): ByteArray {
        val match = Regex(
            "\\\"${Regex.escape(section)}\\\"\\s*:\\s*\\{.*?\\\"norito_hex\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"",
            RegexOption.DOT_MATCHES_ALL,
        ).find(fixture) ?: error("fixture section $section was not found")
        val bytes = match.groupValues[1].chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        return IrohaPeerWireMessageV1(IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.KAGEMUSHA_V1,
            kind,
            IrohaPeerPayloadProfile.KAGEMUSHA_V1.requiredSchemaVersion,
            bytes,
        )).encode()
    }

    private fun fixture(): String {
        var current = Paths.get("").toAbsolutePath().normalize()
        while (current != null) {
            val candidate = current.resolve("fixtures/offline/kagemusha_v1.json")
            if (Files.isRegularFile(candidate)) {
                return String(Files.readAllBytes(candidate), StandardCharsets.UTF_8)
            }
            current = current.parent
        }
        error("fixtures/offline/kagemusha_v1.json was not found")
    }
}
