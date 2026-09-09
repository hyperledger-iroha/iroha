package org.hyperledger.iroha.sdk.tx.norito

import org.hyperledger.iroha.sdk.core.model.InstructionBox
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Custom JSON field lengths and multisig hash preimages agree with the Rust codec. */
class CustomInstructionCanonicalParityTest {
    @Test
    fun `custom null frame and complete batch bytes match Rust`() {
        val customFrame = TransactionPayloadAdapter.encodeCanonicalCustomInstructionJson("null")
        assertContentEquals(InstructionBatchHashFixture.bytes("custom_instruction_frame_hex"), customFrame)
        val instruction = InstructionBox.fromWirePayload("iroha.custom", customFrame)
        assertEquals("null", TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(instruction))
        val encoded = NoritoJavaCodecAdapter.encodeInstructionBox(instruction)
        assertContentEquals(
            InstructionBatchHashFixture.bytes("instruction_batch_hex"),
            TransactionPayloadAdapter.encodeCanonicalInstructionBoxes(listOf(encoded)),
        )
        assertContentEquals(
            InstructionBatchHashFixture.bytes("instruction_batch_hash_hex"),
            NoritoJavaCodecAdapter.hashCanonicalInstructionBoxes(listOf(encoded)),
        )
    }

    @Test
    fun `extra JSON field length and truncated custom frames are rejected`() {
        val redundant = InstructionBox.fromWirePayload(
            "iroha.custom",
            InstructionBatchHashFixture.bytes("rejected_extra_length_custom_frame_hex"),
        )
        assertFailsWith<IllegalArgumentException> {
            TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(redundant)
        }
        val canonical = InstructionBatchHashFixture.bytes("custom_instruction_frame_hex")
        for (length in 1 until canonical.size) {
            val truncated = InstructionBox.fromWirePayload("iroha.custom", canonical.copyOf(length))
            assertFailsWith<IllegalArgumentException>("truncated custom frame at $length") {
                TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(truncated)
            }
        }
    }

    @Test
    fun `JSON fields roundtrip across compact length boundaries`() {
        for (length in listOf(0, 1, 63, 64, 126, 127, 128, 255, 256)) {
            val json = "\"${"x".repeat(length)}\""
            val encoded = TransactionPayloadAdapter.encodeCanonicalCustomInstructionJson(json)
            val instruction = InstructionBox.fromWirePayload("iroha.custom", encoded)
            assertEquals(json, TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(instruction))
            assertContentEquals(
                encoded,
                TransactionPayloadAdapter.encodeCanonicalCustomInstructionJson(
                    TransactionPayloadAdapter.decodeCanonicalCustomInstructionJson(instruction),
                ),
            )
        }
    }
}
