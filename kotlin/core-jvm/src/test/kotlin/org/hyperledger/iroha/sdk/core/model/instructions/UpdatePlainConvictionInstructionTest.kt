package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.File
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class UpdatePlainConvictionInstructionTest {
    private val owner =
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"

    @Test
    fun `four field direct instruction round trips through Norito and instruction box`() {
        val instruction = UpdatePlainConvictionInstruction(
            "referendum_1",
            owner,
            "18446744073709551616.25",
            "18446744073709551615",
        )
        assertEquals(
            setOf("referendum_id", "owner", "amount", "duration_blocks"),
            instruction.arguments.keys,
        )
        assertEquals("18446744073709551615", instruction.durationBlocks)
        val wire = assertIs<WirePayload>(instruction.toInstructionBox().payload)
        assertEquals(UpdatePlainConvictionInstruction.WIRE_NAME, wire.wireName)
        assertEquals(
            instruction,
            UpdatePlainConvictionInstruction.fromWirePayload(
                wire.payloadBytes,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            ),
        )
        val boxBytes = NoritoJavaCodecAdapter.encodeInstructionBox(instruction.toInstructionBox())
        assertContentEquals(
            boxBytes,
            NoritoJavaCodecAdapter.encodeInstructionBox(
                NoritoJavaCodecAdapter.decodeInstructionBox(boxBytes),
            ),
        )
    }

    @Test
    fun `direct conviction frame matches the Rust owned golden`() {
        val fixture = rustGolden()
        val inputs = fixture["inputs"] as Map<*, *>
        assertEquals(
            setOf("referendum_id", "owner", "amount", "duration_blocks"),
            inputs.keys,
        )
        assertEquals(1L, (fixture["version"] as Number).toLong())
        val fields = mapOf(
            "referendum_id" to (inputs["referendum_id"] as String),
            "owner" to (inputs["owner"] as String),
            "amount" to (inputs["amount"] as String),
            "duration_blocks" to (inputs["duration_blocks"] as Number).toLong().toString(),
        )
        val update = UpdatePlainConvictionInstruction.fromCanonicalFields(fields)
        assertEquals(fields, update.arguments)
        assertEquals(UpdatePlainConvictionInstruction.WIRE_NAME, fixture["wire_id"])
        assertEquals(UpdatePlainConvictionInstruction.SCHEMA_NAME, fixture["concrete_schema_name"])
        val schemaName = fixture["concrete_schema_name"] as String
        assertEquals(fixture["concrete_schema_hash"], SchemaHash.hash16(schemaName).toLowerHex())

        val frame = UpdatePlainConvictionWirePayloadEncoder.encodePayload(update)
        assertContentEquals(hex(fixture["concrete_frame_hex"] as String), frame)
        assertContentEquals(
            Base64.getDecoder().decode(fixture["framed_instruction_base64"] as String),
            frame,
        )
        assertEquals((fixture["framed_instruction_len"] as Number).toInt(), frame.size)
        val concrete = NoritoHeader.decode(frame, SchemaHash.hash16(schemaName))
        concrete.header.validateChecksum(concrete.payload)
        assertEquals((fixture["header_flags"] as Number).toInt(), concrete.header.flags)
        assertContentEquals(hex(fixture["bare_payload_hex"] as String), concrete.payload)
        val decoded = UpdatePlainConvictionInstruction.fromWirePayload(
            frame,
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
        )
        assertEquals(update, decoded)
        assertContentEquals(frame, UpdatePlainConvictionWirePayloadEncoder.encodePayload(decoded))

        val box = update.toInstructionBox()
        val wire = assertIs<WirePayload>(box.payload)
        assertEquals(fixture["wire_id"], wire.wireName)
        assertContentEquals(frame, wire.payloadBytes)
        val standalone = NoritoJavaCodecAdapter.encodeInstructionBox(box)
        assertContentEquals(hex(fixture["standalone_instruction_box_frame_hex"] as String), standalone)
        val boxed = NoritoHeader.decode(
            standalone,
            SchemaHash.hash16("(alloc::string::String, alloc::vec::Vec<u8>)"),
        )
        boxed.header.validateChecksum(boxed.payload)
        assertEquals((fixture["header_flags"] as Number).toInt(), boxed.header.flags)
        assertContentEquals(hex(fixture["instruction_box_pair_hex"] as String), boxed.payload)
        val decodedBox = NoritoJavaCodecAdapter.decodeInstructionBox(standalone)
        val decodedWire = assertIs<WirePayload>(decodedBox.payload)
        assertEquals(fixture["wire_id"], decodedWire.wireName)
        assertContentEquals(frame, decodedWire.payloadBytes)
        assertContentEquals(standalone, NoritoJavaCodecAdapter.encodeInstructionBox(decodedBox))
    }

    @Test
    fun `canonical fields reject choice aliases and malformed values`() {
        val fields = mapOf(
            "referendum_id" to "referendum_1",
            "owner" to owner,
            "amount" to "20",
            "duration_blocks" to "10",
        )
        assertEquals(
            UpdatePlainConvictionInstruction("referendum_1", owner, "20", 10L),
            UpdatePlainConvictionInstruction.fromCanonicalFields(fields),
        )
        listOf(
            fields + ("direction" to "1"),
            fields + ("action" to "UpdatePlainConviction"),
            fields - "duration_blocks",
            fields - "owner",
            fields + ("durationBlocks" to "10"),
        ).forEach { rejected ->
            assertFailsWith<IllegalArgumentException> {
                UpdatePlainConvictionInstruction.fromCanonicalFields(rejected)
            }
        }
        for (selector in listOf("", ".hidden", "with space", "a/b", "a".repeat(129))) {
            assertFailsWith<IllegalArgumentException> {
                UpdatePlainConvictionInstruction(selector, owner, "20", "10")
            }
        }
        for (amount in listOf("-1", "01", "1.0", "1e1", " 1")) {
            assertFailsWith<IllegalArgumentException> {
                UpdatePlainConvictionInstruction("referendum_1", owner, amount, "10")
            }
        }
        for (duration in listOf("-1", "01", "+1", "1.0", "18446744073709551616")) {
            assertFailsWith<IllegalArgumentException> {
                UpdatePlainConvictionInstruction("referendum_1", owner, "20", duration)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            UpdatePlainConvictionInstruction("referendum_1", "$owner@domain", "20", "10")
        }
    }

    @Test
    fun `wire and transaction boundary reject an appended choice field`() {
        val valid = UpdatePlainConvictionInstruction("referendum_1", owner, "20", "10")
        val frame = UpdatePlainConvictionWirePayloadEncoder.encodePayload(valid)
        val bare = NoritoCodec.decode(frame, RawAdapter, UpdatePlainConvictionInstruction.SCHEMA_NAME)
        val forged = NoritoCodec.encode(
            bare + byteArrayOf(1, 1),
            UpdatePlainConvictionInstruction.SCHEMA_NAME,
            RawAdapter,
        )
        assertFailsWith<IllegalArgumentException> {
            UpdatePlainConvictionInstruction.fromWirePayload(
                forged,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }
        assertFailsWith<NoritoException> {
            NoritoJavaCodecAdapter.encodeInstructionBox(
                InstructionBox.fromWirePayload(UpdatePlainConvictionInstruction.WIRE_NAME, forged),
            )
        }
        val changed = frame.copyOf()
        changed[changed.lastIndex] = (changed.last().toInt() xor 1).toByte()
        assertFailsWith<IllegalArgumentException> {
            UpdatePlainConvictionInstruction.fromWirePayload(
                changed,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }
    }

    private fun rustGolden(): Map<*, *> {
        val relative = "fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json"
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, relative) }
            .first(File::isFile)
        return JsonParser.parse(file.readText(Charsets.UTF_8)) as Map<*, *>
    }

    private fun hex(value: String): ByteArray {
        require(value.length % 2 == 0 && value.all { it in "0123456789abcdef" })
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun ByteArray.toLowerHex(): String =
        joinToString("") { "%02x".format(it.toInt() and 0xff) }

    private object RawAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray =
            decoder.readBytes(decoder.remaining())
    }
}
