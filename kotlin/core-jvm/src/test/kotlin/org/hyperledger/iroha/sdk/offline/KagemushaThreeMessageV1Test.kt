package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import org.hyperledger.iroha.sdk.norito.NoritoHeader

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
    fun `lifecycle operation inventory matches Rust`() {
        assertEquals(
            listOf(
                "BOOTSTRAP",
                "MINT_FOLD",
                "SEND_SPLIT",
                "RECEIVE_FOLD",
                "REDEEM_SPLIT",
                "ROTATE",
            ),
            KagemushaOperationKindV1.values().map { it.name },
        )
        assertEquals((0..5).toList(), KagemushaOperationKindV1.values().map { it.wireTag })
    }

    @Test
    fun `peer payload alignment matches the canonical Rust layout`() {
        val model = "iroha_data_model::kagemusha::kagemusha_v1::"
        val layouts = listOf(
            Triple("KagemushaPaymentRequestV1", 16, 8),
            Triple("KagemushaPaymentV1", 16, 8),
            Triple("KagemushaAcknowledgementV1", 2, 0),
        )

        layouts.forEach { (type, expectedAlignment, expectedPadding) ->
            val alignment = KagemushaNoritoV1.canonicalAlignment(model + type)
            val padding =
                (alignment - NoritoHeader.HEADER_LENGTH % alignment) % alignment
            assertEquals(expectedAlignment, alignment, type)
            assertEquals(expectedPadding, padding, type)
        }
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
        val command = KagemushaDeviceControlCommandV1.FoldReceiveCredit(operationId, creditId)
        assertEquals(17, command.operation)
        assertContentEquals(creditId, command.creditId())
        val encoded = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
        val decoded = KagemushaDeviceOperationCodecV1.decodeControlCommand(17, operationId, encoded)
            as KagemushaDeviceControlCommandV1.FoldReceiveCredit
        assertContentEquals(creditId, decoded.creditId())
    }
}
