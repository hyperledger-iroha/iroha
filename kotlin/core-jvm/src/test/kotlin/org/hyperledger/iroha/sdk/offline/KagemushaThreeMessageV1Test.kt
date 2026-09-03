package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals

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
        val command = KagemushaDeviceControlCommandV1.FoldReceiveCredit(operationId, creditId)
        assertEquals(17, command.operation)
        assertContentEquals(creditId, command.creditId())
        val encoded = KagemushaDeviceOperationCodecV1.encodeControlCommand(command)
        val decoded = KagemushaDeviceOperationCodecV1.decodeControlCommand(17, operationId, encoded)
            as KagemushaDeviceControlCommandV1.FoldReceiveCredit
        assertContentEquals(creditId, decoded.creditId())
    }
}
