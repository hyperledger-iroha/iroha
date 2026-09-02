// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

class IrohaPeerKagemushaV1Test {
    @Test
    fun `current three message exchange crosses IPM QR and NFC byte identically`() {
        val request = KagemushaV1TestSupport.request()
        val payment = KagemushaV1TestSupport.payment(request)
        val acknowledgement = KagemushaV1TestSupport.acknowledgement(request, payment)
        val values = listOf(
            IrohaPeerPayloadKind.RECEIVE_REQUEST to KagemushaNoritoV1.encodePaymentRequestShape(request),
            IrohaPeerPayloadKind.PAYMENT to KagemushaNoritoV1.encodePaymentShape(payment, request),
            IrohaPeerPayloadKind.ACKNOWLEDGEMENT to
                KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment),
        )

        values.forEach { (kind, raw) ->
            val payload = IrohaPeerCanonicalPayload(IrohaPeerPayloadProfile.KAGEMUSHA_V1, kind, 1, raw)
            val message = IrohaPeerWireMessageV1(payload)
            assertEquals(message, IrohaPeerWireMessageV1.decode(message.encode()))

            val scanner = IrohaPeerQRScanSessionV1(expectedProfile = IrohaPeerPayloadProfile.KAGEMUSHA_V1)
            var scanned: IrohaPeerWireMessageV1? = null
            IrohaPeerQRCodecV1.encode(message).forEach { text ->
                scanner.ingest(text).message?.let { scanned = it }
            }
            assertEquals(message, assertNotNull(scanned))

            if (kind == IrohaPeerPayloadKind.RECEIVE_REQUEST) {
                val session = ByteArray(IrohaPeerNfcV1.SESSION_ID_BYTES) { 0x11 }
                val receiver = IrohaPeerNfcReceiverSessionV1(session, message.encode())
                val read = IrohaPeerNfcCommandV1.readRequest(session, message.canonicalHash, 0, message.encode().size)
                assertContentEquals(message.encode(), receiver.handle(read))
            }
        }
    }
}
