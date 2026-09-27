package org.hyperledger.iroha.sdk.proof

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.core.util.HashLiteral

class ContractStateValueProofV1Test {
    private fun hex(value: String): ByteArray =
        value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    private val path = "sc/alpha/Balance"
    private val root = hex("194a8961806570284bf970836427142baa1ddcab853f1ee2c3ae672e1da8acb3")
    private val sibling = hex("482931df820458f6bf299fa0e88e37f2378b3e8558c2c254ad03755ed8790947")

    @Test
    fun `independent two-leaf vector proves exact accumulated value`() {
        // Python hashlib.blake2b(digest_size=32) reference, with Iroha's marker.
        val proof = ContractStateValueInclusionProofV1(
            1, path, "one".toByteArray(), 2,
            listOf(ContractStateProofStepV1(0, ByteArray(32), sibling)),
        )
        assertTrue(ContractStateValueProofVerifierV1.verify(proof, path, root))
        assertFalse(ContractStateValueProofVerifierV1.verify(proof, "sc/beta/Balance", root))
        assertFalse(ContractStateValueProofVerifierV1.verify(proof, path, sibling))
        assertFalse(ContractStateValueProofVerifierV1.verify(
            ContractStateValueInclusionProofV1(1, "sc/alpha/\uD800", proof.value, 2, proof.steps),
            "sc/alpha/\uD800", root,
        ))
        assertFalse(ContractStateValueProofVerifierV1.verify(
            ContractStateValueInclusionProofV1(1, path, "two".toByteArray(), 2, proof.steps),
            path, root,
        ))
        assertFalse(ContractStateValueProofVerifierV1.verify(
            ContractStateValueInclusionProofV1(1, path, proof.value, 1, proof.steps),
            path, root,
        ))
    }

    @Test
    fun `canonical JSON decoder rejects duplicate and unknown fields`() {
        val siblingLiteral = HashLiteral.canonicalize(sibling)
        val fields = """"version":1,"path":"$path","value":[111,110,101],"leaf_count":2,"steps":[{"bit":0,"prefix":[${List(32) { "0" }.joinToString(",")}],"sibling":"$siblingLiteral"}]"""
        val parsed = ContractStateValueInclusionProofV1.parseJson("{$fields}".toByteArray())
        assertTrue(ContractStateValueProofVerifierV1.verify(parsed, path, root))
        assertFailsWith<IllegalArgumentException> {
            ContractStateValueInclusionProofV1.parseJson("{$fields,\"unknown\":0}".toByteArray())
        }
        assertFailsWith<IllegalStateException> {
            ContractStateValueInclusionProofV1.parseJson("{$fields,\"version\":1}".toByteArray())
        }
    }
}
