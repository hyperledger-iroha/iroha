package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class TonSccpProverTest {
    @Test
    fun buildsTonMessageBodyBoc() {
        val body = SccpTon.buildMessageBodyBoc(sampleMessageBodyInput())

        assertEquals(listOf(0xb5, 0xee, 0x9c, 0x72), body.take(4).map { it.toInt() and 0xff })
        assertTrue(body.size > SccpTon.canonicalPublicInputsBytes(samplePublicInputs()).size)

        val submission = SccpTon.buildSubmission(sampleMessageBodyInput())
        assertEquals(SccpTon.MESSAGE_BODY_BOC_V1, submission.envelopeEncoding)
        assertEquals(body.toList(), submission.messageBodyBoc.toList())
        assertTrue(submission.messageBodyBocHex.startsWith("0xb5ee9c72"))
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            TonSccpProver().prove(sampleProofRequestInput())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val prover = TonSccpProver(
            proofEngine = TonSccpProofEngine { request ->
                assertEquals(SccpTon.CONTRACT_PROOF_BACKEND_V1, request.backend)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(sampleProofRequestInput())

        assertEquals(listOf(1, 2, 3, 4), result.proofBytes.map { it.toInt() })
        assertEquals("AQIDBA==", result.proofBase64)
        assertTrue(result.requestHash.matches(Regex("0x[0-9a-f]{64}")))
    }

    private fun sampleMessageBodyInput(): TonSccpMessageBodyInput =
        TonSccpMessageBodyInput(
            publicInputs = samplePublicInputs(),
            proofBytes = byteArrayOf(1, 2, 3, 4),
            bundleBytes = byteArrayOf(5, 6, 7),
            statementHash = "bb".repeat(32),
            destinationBindingHash = "56".repeat(32),
            metadataBytes = byteArrayOf(8, 9),
        )

    private fun sampleProofRequestInput(): TonSccpProofRequestInput =
        TonSccpProofRequestInput(
            publicInputs = samplePublicInputs(),
            bundleBytes = byteArrayOf(5, 6, 7),
        )

    private fun samplePublicInputs(): TonSccpPublicInputsInput =
        TonSccpPublicInputsInput(
            messageId = "dd".repeat(32),
            payloadHash = "ee".repeat(32),
            commitmentRoot = "12".repeat(32),
            finalityHeight = "19",
            finalityBlockHash = "aa".repeat(32),
        )
}
