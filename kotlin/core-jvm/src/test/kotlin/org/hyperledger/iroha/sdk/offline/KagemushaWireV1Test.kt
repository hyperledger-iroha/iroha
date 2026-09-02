// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class KagemushaWireV1Test {
    @Test
    fun `authoritative three message exchange round trips canonically`() {
        val request = KagemushaV1TestSupport.request(BigInteger.TEN)
        val payment = KagemushaV1TestSupport.payment(request)
        val acknowledgement = KagemushaV1TestSupport.acknowledgement(request, payment)

        val requestRaw = KagemushaNoritoV1.encodePaymentRequestShape(request)
        val paymentRaw = KagemushaNoritoV1.encodePaymentShape(payment, request)
        val acknowledgementRaw =
            KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment)

        assertContentEquals(
            requestRaw,
            KagemushaNoritoV1.encodePaymentRequestShape(
                KagemushaNoritoV1.decodePaymentRequestShapeExact(requestRaw),
            ),
        )
        assertContentEquals(
            paymentRaw,
            KagemushaNoritoV1.encodePaymentShape(
                KagemushaNoritoV1.decodePaymentShapeExact(paymentRaw, request),
                request,
            ),
        )
        assertContentEquals(
            acknowledgementRaw,
            KagemushaNoritoV1.encodeAcknowledgementShape(
                KagemushaNoritoV1.decodeAcknowledgementShapeExact(
                    acknowledgementRaw,
                    request,
                    payment,
                ),
                request,
                payment,
            ),
        )
        assertEquals(
            requestRaw.size + paymentRaw.size + acknowledgementRaw.size,
            KagemushaNoritoV1.validateTerminalDeliveryShape(request, payment, acknowledgement),
        )
    }

    @Test
    fun `request and payment bind receiver lane key sender heads time and hardware transition`() {
        val request = KagemushaV1TestSupport.request()
        val payment = KagemushaV1TestSupport.payment(request)
        val statement = payment.statement
        val context = KagemushaNoritoV1.peerCreditContextShape(statement, request)

        assertContentEquals(request.recipientLaneId(), context.recipientLaneId())
        assertEquals(request.recipientEncryptionKey, context.recipientEncryptionKey)
        assertContentEquals(statement.senderBeforeCommitment().eq(), context.senderBeforeCommitment().eq())
        assertContentEquals(statement.senderBeforeCommitment().ep(), context.senderBeforeCommitment().ep())
        assertContentEquals(statement.senderAfterCommitment().eq(), context.senderAfterCommitment().eq())
        assertContentEquals(statement.senderAfterCommitment().ep(), context.senderAfterCommitment().ep())
        assertEquals(statement.committedAtMs, context.committedAtMs)
        assertContentEquals(
            statement.hardwareTransitionCommitment(),
            context.hardwareTransitionCommitment(),
        )
        assertContentEquals(
            statement.hardwareTransitionCommitment(),
            KagemushaNoritoV1.encryptedCreditAadForPeerShape(statement, request)
                .issuanceOrTransitionCommitment(),
        )
    }

    @Test
    fun `three messages use sole kgm1 text transport`() {
        val request = KagemushaV1TestSupport.request()
        val payment = KagemushaV1TestSupport.payment(request)
        val acknowledgement = KagemushaV1TestSupport.acknowledgement(request, payment)
        val messages = listOf(
            KagemushaWirePayloadKindV1.PAYMENT_REQUEST to
                KagemushaNoritoV1.encodePaymentRequestShape(request),
            KagemushaWirePayloadKindV1.PAYMENT to
                KagemushaNoritoV1.encodePaymentShape(payment, request),
            KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT to
                KagemushaNoritoV1.encodeAcknowledgementShape(acknowledgement, request, payment),
        )

        messages.forEach { (kind, raw) ->
            val text = KagemushaWireV1.encodeText(kind, raw)
            assertTrue(text.startsWith("kgm1:"))
            assertContentEquals(raw, KagemushaWireV1.decodeText(kind, text))
            assertFailsWith<IllegalArgumentException> {
                KagemushaWireV1.decodeText(kind, "oc" + "1:" + text.substringAfter(':'))
            }
        }
    }

    @Test
    fun `mint credit and redemption retain standalone recursive shapes`() {
        val (authorization, credit) = KagemushaV1TestSupport.mintAuthorizationAndCredit()
        val authorizationRaw = KagemushaNoritoV1.encodeMintAuthorizationShape(authorization)
        val creditRaw = KagemushaNoritoV1.encodeMintCreditShape(credit, authorization)
        val redemption = KagemushaV1TestSupport.redemption()
        val redemptionRaw = KagemushaNoritoV1.encodeRedemptionVoucherShape(redemption)

        assertContentEquals(
            authorizationRaw,
            KagemushaNoritoV1.encodeMintAuthorizationShape(
                KagemushaNoritoV1.decodeMintAuthorizationShapeExact(authorizationRaw),
            ),
        )
        assertContentEquals(
            creditRaw,
            KagemushaNoritoV1.encodeMintCreditShape(
                KagemushaNoritoV1.decodeMintCreditShapeExact(creditRaw),
            ),
        )
        assertContentEquals(
            redemptionRaw,
            KagemushaNoritoV1.encodeRedemptionVoucherShape(
                KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(redemptionRaw),
            ),
        )
    }

    @Test
    fun `public peer records contain no retired negotiation or certificate fields`() {
        val forbidden = listOf(
            "acceptanceintent",
            "acceptanceticket",
            "commitcertificate",
            "commitwrapper",
            "commitevidence",
            "outboxreservation",
            "artifactmanifest",
        )
        listOf(
            KagemushaPaymentRequestV1::class.java,
            KagemushaTransferStatementV1::class.java,
            KagemushaPaymentV1::class.java,
            KagemushaAcknowledgementV1::class.java,
            KagemushaRedemptionStatementV1::class.java,
            KagemushaRedemptionVoucherV1::class.java,
        ).forEach { type ->
            val names = type.declaredFields.map { it.name } + type.declaredMethods.map { it.name }
            forbidden.forEach { fragment ->
                assertFalse(
                    names.any { it.lowercase().contains(fragment) },
                    "${type.simpleName} leaked $fragment",
                )
            }
        }
    }

    @Test
    fun `wire constants preserve compact history independent envelope`() {
        assertEquals(6_528, KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES)
        assertEquals(9_211, KagemushaWireV1.MAXIMUM_SESSION_RAW_BYTES)
        assertEquals(12_288, KagemushaWireV1.MAXIMUM_SESSION_TEXT_BYTES)
        assertEquals(
            listOf(
                KagemushaWirePayloadKindV1.PAYMENT_REQUEST,
                KagemushaWirePayloadKindV1.PAYMENT,
                KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT,
                KagemushaWirePayloadKindV1.MINT_AUTHORIZATION,
                KagemushaWirePayloadKindV1.MINT_CREDIT,
                KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER,
            ),
            KagemushaWirePayloadKindV1.values().toList(),
        )
    }
}
