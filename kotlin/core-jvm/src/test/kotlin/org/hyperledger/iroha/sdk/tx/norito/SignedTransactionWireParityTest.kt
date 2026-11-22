package org.hyperledger.iroha.sdk.tx.norito

import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.norito.WireLayout
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Canonical transaction framing against signed Rust and independent wallet originals. */
class SignedTransactionWireParityTest {
    @Test
    fun `canonical versioned transactions retain their exact signed bytes`() {
        val signerCounts = mapOf(
            "direct" to 0,
            "one_member" to 1,
            "weighted" to 2,
            "below_threshold" to 1,
        )
        for ((name, count) in signerCounts) {
            val wire = vector(name)
            val transaction = SignedTransactionEncoder.decodeVersioned(wire)
            assertEquals(count, transaction.multisigSignatures().orElse(null)?.signatures?.size ?: 0, name)
            assertContentEquals(wire, SignedTransactionEncoder.encodeVersioned(transaction), name)
        }
    }

    @Test
    fun `weighted authority and signatures round-trip in both supported explicit layouts`() {
        val versioned = vector("weighted")
        val compact = versioned.copyOfRange(1, versioned.size)
        for ((layout, original) in listOf(WireLayout.COMPACT to compact, WireLayout.DENSE to vector("weighted_dense"))) {
            val transaction = SignedTransactionEncoder.decode(original, layout)
            assertEquals(2, transaction.multisigSignatures().get().signatures.size)
            assertContentEquals(original, SignedTransactionEncoder.encode(transaction, layout), layout.name)
        }
    }

    @Test
    fun `unframed fields in either multisignature structure are refused`() {
        for (name in listOf("unframed_authority_and_bundle", "unframed_authority", "unframed_bundle")) {
            assertFailsWith<NoritoException>(name) { SignedTransactionEncoder.decodeVersioned(vector(name)) }
        }
    }

    @Test
    fun `payload reader refuses a weighted authority whose members are out of order`() {
        val codec = NoritoJavaCodecAdapter(753)
        assertFailsWith<NoritoException> { codec.decodeTransaction(vector("reordered_authority_payload")) }
    }

    private val vectors: Map<*, *> by lazy {
        val input = checkNotNull(javaClass.getResourceAsStream("signed_transaction_wire_v1.json"))
        input.bufferedReader(Charsets.UTF_8).use { JsonParser.parse(it.readText()) as Map<*, *> }
    }

    private fun vector(name: String): ByteArray = Base64.getDecoder().decode(vectors[name] as String)
}
