package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse

class KagemushaThreeMessageV1Test {
    @Test
    fun `peer message inventory is exactly request payment acknowledgement`() {
        assertEquals(listOf(1, 2, 3), IrohaPeerPayloadKind.values().map { it.code })
        assertEquals(
            listOf("REQUEST", "PAYMENT", "ACKNOWLEDGEMENT"),
            IrohaPeerPayloadKind.values().map { it.name },
        )
    }

    @Test
    fun `direct NFC control commands round trip`() {
        val commands = listOf(
            IrohaPeerNfcCommandV1.GET_INFO,
            IrohaPeerNfcCommandV1.readRequest(0, 64),
            IrohaPeerNfcCommandV1.COMMIT_PAYMENT,
            IrohaPeerNfcCommandV1.readAcknowledgement(0, 32),
            IrohaPeerNfcCommandV1.CONFIRM_ACKNOWLEDGEMENT,
            IrohaPeerNfcCommandV1.GET_STATUS,
        )
        commands.forEach { command ->
            val encoded = IrohaPeerNfcAPDUCodecV1.encode(command)
            assertContentEquals(encoded, IrohaPeerNfcAPDUCodecV1.encode(IrohaPeerNfcAPDUCodecV1.decode(encoded)))
        }
    }

    @Test
    fun `fold command names one exact credit`() {
        val operationId = ByteArray(32) { 1 }
        val creditId = ByteArray(32) { 2 }
        val command = KagemushaDeviceControlCommandV1.FoldReceiveCredit(
            operationId,
            KagemushaPendingCreditSelectorV1(KagemushaPendingCreditKindV1.RECEIVE, creditId),
        )
        assertEquals(17, command.operation)
        assertEquals(KagemushaPendingCreditKindV1.RECEIVE, command.selector.kind)
        assertContentEquals(creditId, command.selector.creditId())
        val encoded = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
        val decoded = KagemushaDeviceOperationCodecV1.decodeControlCommand(17, operationId, encoded)
            as KagemushaDeviceControlCommandV1.FoldReceiveCredit
        assertEquals(KagemushaPendingCreditKindV1.RECEIVE, decoded.selector.kind)
        assertContentEquals(creditId, decoded.selector.creditId())
    }

    @Test
    fun `shared fixture round trips only request payment acknowledgement`() {
        val fixture = loadFixture()
        assertEquals(1, fixtureInt(fixture, "fixture_version"))
        assertFalse(fixture.contains("\"acceptance_intent\""))
        assertFalse(fixture.contains("\"acceptance_ticket\""))
        assertFalse(fixture.contains("\"complete_five_message\""))

        val requestBytes = fixtureBytes(fixture, "payment_request")
        val paymentBytes = fixtureBytes(fixture, "payment")
        val acknowledgementBytes = fixtureBytes(fixture, "acknowledgement")
        val request = KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
        val payment = KagemushaNoritoV1.decodePaymentShapeExact(paymentBytes, request)
        val acknowledgement = KagemushaNoritoV1.decodeAcknowledgementShapeExact(
            acknowledgementBytes,
            request,
            payment,
        )

        assertContentEquals(requestBytes, KagemushaNoritoV1.encodePaymentRequestShape(request))
        assertContentEquals(paymentBytes, KagemushaNoritoV1.encodePaymentShape(payment, request))
        assertContentEquals(
            acknowledgementBytes,
            KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment),
        )
    }

    private fun loadFixture(): String {
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

    private fun fixtureBytes(fixture: String, section: String): ByteArray {
        val match = Regex(
            "\\\"${Regex.escape(section)}\\\"\\s*:\\s*\\{.*?\\\"norito_hex\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"",
            RegexOption.DOT_MATCHES_ALL,
        ).find(fixture) ?: error("fixture section $section was not found")
        val hex = match.groupValues[1]
        require(hex.length % 2 == 0) { "fixture hex length is odd" }
        return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }

    private fun fixtureInt(fixture: String, field: String): Int =
        Regex("\\\"${Regex.escape(field)}\\\"\\s*:\\s*(\\d+)")
            .find(fixture)
            ?.groupValues
            ?.get(1)
            ?.toInt()
            ?: error("fixture field $field was not found")
}
