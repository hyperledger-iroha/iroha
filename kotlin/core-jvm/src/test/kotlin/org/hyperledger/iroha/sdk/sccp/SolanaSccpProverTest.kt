package org.hyperledger.iroha.sdk.sccp

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class SolanaSccpProverTest {
    @Test
    fun buildsSolanaSccpProofRequest() {
        val request = SccpSolana.buildProofRequest(sampleWitness())

        assertEquals(1, request.version)
        assertEquals(SccpSolana.RECURSIVE_PROOF_BACKEND_V1, request.backend)
        assertEquals(SccpSolana.DOMAIN_SOLANA, request.sourceDomain)
        assertEquals(SccpSolana.DOMAIN_SORA, request.targetDomain)
        assertEquals("0x" + "dd".repeat(32), request.publicInputs.messageId)
        assertTrue(request.witnessHash.matches(Regex("0x[0-9a-f]{64}")))
    }

    @Test
    fun requiresSourceEventDigest() {
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.normalizeWitness(sampleWitness(sourceEventDigest = ""))
        }
        assertTrue(error.message?.contains("sourceEventDigest") == true)
    }

    @Test
    fun buildsMessageProofHashFromInclusionWitness() {
        val branch = listOf(ByteArray(32) { 0x56.toByte() })
        val hash = SccpSolana.messageProofHash(
            sourceEventDigest = "34".repeat(32),
            transactionStatusRoot = "bb".repeat(32),
            inclusionBranch = branch,
        )

        assertTrue(hash.matches(Regex("0x[0-9a-f]{64}")))
        assertTrue(
            SccpSolana.canonicalMessageProofBytes(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = "bb".repeat(32),
                inclusionBranch = branch,
            ).isNotEmpty(),
        )
        val error = assertFailsWith<IllegalArgumentException> {
            SccpSolana.messageProofHash(
                sourceEventDigest = "34".repeat(32),
                transactionStatusRoot = "bb".repeat(32),
                inclusionBranch = listOf(ByteArray(31) { 0xab.toByte() }),
            )
        }
        assertTrue(error.message?.contains("inclusionBranch[0]") == true)
    }

    @Test
    fun proverRequiresLinkedProofEngine() {
        val error = assertFailsWith<IllegalStateException> {
            SolanaSccpProver().prove(sampleWitness())
        }
        assertTrue(error.message?.contains("not linked") == true)
    }

    @Test
    fun proverWrapsExternalProofBytes() {
        val prover = SolanaSccpProver(
            proofEngine = SolanaSccpProofEngine { request ->
                assertEquals(SccpSolana.RECURSIVE_PROOF_BACKEND_V1, request.backend)
                byteArrayOf(1, 2, 3, 4)
            },
        )

        val result = prover.prove(sampleWitness())

        assertEquals(listOf(1, 2, 3, 4), result.proofBytes.map { it.toInt() })
        assertEquals("AQIDBA==", result.proofBase64)
        assertTrue(result.envelopeHash.matches(Regex("0x[0-9a-f]{64}")))
    }

    private fun sampleWitness(sourceEventDigest: String = "34".repeat(32)): SolanaSccpWitnessInput =
        SolanaSccpWitnessInput(
            finalizedSlot = "321",
            blockhash = "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
            bankHash = "aa".repeat(32),
            transactionStatusRoot = "bb".repeat(32),
            messageProofHash = "cc".repeat(32),
            transactionSignature = "5eykt4Signature111111111111111111111111111111",
            emitterProgramId = "Bridge111111111111111111111111111111111111",
            messageId = "dd".repeat(32),
            payloadHash = "ee".repeat(32),
            commitmentRoot = "12".repeat(32),
            sourceEventDigest = sourceEventDigest,
        )
}
