package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonParser
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Exact AccountId bytes generated and self-verified by the canonical Rust implementation. */
class CanonicalMultisigWireParityTest {
    @Test
    fun `all signing algorithms and complete weighted multisig controllers match Rust`() {
        val fixture = fixture()
        val positives = fixture["positive"] as List<*>
        assertEquals(16, positives.size)

        for (raw in positives) {
            val vector = raw as Map<*, *>
            val literal = vector["i105"] as String
            val payload = hex(vector["account_id_payload_hex"] as String)
            assertEquals(2, (vector["layout_flags"] as Number).toInt())
            assertContentEquals(payload, TransferWirePayloadEncoder.encodeAccountIdPayload(literal), vector["name"] as String)
            assertEquals(literal, TransferWirePayloadEncoder.decodeAccountIdPayload(payload, 753))
            assertContentEquals(hex(vector["canonical_address_hex"] as String), AccountAddress.fromI105(literal, 753).canonicalBytes)
            val join = JoinKaigiInstruction(KaigiInstructionUtils.CallId("wonderland.sora", "complete-policy"), literal)
            assertEquals(join, KaigiWirePayloadEncoderV1.decode(join.wireName, join.payloadBytes))
        }
        val large = positives.map { it as Map<*, *> }.single { it["name"] == "members-256" }
        assertEquals(256, AccountAddress.fromI105(large["i105"] as String, 753).multisigPolicyPayload()!!.members.size)
    }

    @Test
    fun `all malformed Rust policy fixtures reject as external account identities`() {
        val negatives = fixture()["negative"] as List<*>
        assertEquals(7, negatives.size)
        for (raw in negatives) {
            val vector = raw as Map<*, *>
            assertFailsWith<IllegalArgumentException>(vector["name"] as String) {
                TransferWirePayloadEncoder.decodeAccountIdPayload(hex(vector["account_id_payload_hex"] as String), 753)
            }
        }
    }

    private fun fixture(): Map<*, *> {
        val path = "fixtures/account/multisig_wire_v1.json"
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }.map { File(it, path) }.first(File::isFile)
        return (JsonParser.parse(file.readText(Charsets.UTF_8)) as Map<*, *>).also {
            assertEquals("iroha.account.multisig-wire.v1", it["schema"])
            assertEquals(753, (it["chain_discriminant"] as Number).toInt())
        }
    }
    private fun hex(value: String): ByteArray = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
