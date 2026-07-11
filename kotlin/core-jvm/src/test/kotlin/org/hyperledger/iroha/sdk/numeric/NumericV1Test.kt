package org.hyperledger.iroha.sdk.numeric

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

class NumericV1Test {
    @Test
    fun exactValuesCanonicalizeWithoutLossyHostNumbers() {
        assertEquals(NumericV1Codec.intMin.toString(), KotodamaInt.of(NumericV1Codec.intMin).toString())
        assertEquals(NumericV1Codec.intMax.toString(), KotodamaInt.of(NumericV1Codec.intMax).toString())
        assertEquals("1.23", KotodamaDecimal.parse("1.2300").toString())
        assertEquals("0", KotodamaDecimal.parse("0.000").toString())
        assertEquals("12.5", KotodamaQuantity.parse("12.50").toString())
        assertCode(NumericV1ErrorCode.NEGATIVE_QUANTITY) { KotodamaQuantity.parse("-0.1") }
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) {
            KotodamaQuantity.parse("-" + "9".repeat(154))
        }
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) {
            KotodamaInt.of(NumericV1Codec.intMax.add(BigInteger.ONE))
        }
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) {
            KotodamaInt.of(NumericV1Codec.intMin.subtract(BigInteger.ONE))
        }
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) { KotodamaInt.parse("1".repeat(10_000)) }
        assertCode(NumericV1ErrorCode.INVALID_TEXT) { KotodamaInt.parse("x".repeat(10_000)) }
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) { KotodamaDecimal.parse("1".repeat(10_000)) }
        assertEquals("1", KotodamaDecimal.parse("1.00000000000000000000000000000").toString())
        assertEquals("1", KotodamaDecimal.parse("1." + "0".repeat(10_000)).toString())
        assertEquals(NumericV1Codec.intMax.toString(), KotodamaDecimal.parse("${NumericV1Codec.intMax}.0").toString())
        assertEquals(
            NumericV1Codec.intMax.toString(),
            KotodamaDecimal.of(NumericV1Codec.intMax.multiply(BigInteger.TEN), 1).toString(),
        )
        assertCode(NumericV1ErrorCode.MANTISSA_OVERFLOW) {
            KotodamaDecimal.parse("${NumericV1Codec.intMax}.1")
        }
        assertCode(NumericV1ErrorCode.INVALID_SCALE) { KotodamaDecimal.parse("0.00000000000000000000000000001") }
        assertCode(NumericV1ErrorCode.INVALID_TEXT) { KotodamaInt.parse("01") }
        assertEquals("1.23", NumericV1Codec.decodeDecimalJson("1.23").toString())
        assertEquals("0", NumericV1Codec.decodeQuantityJson("0").toString())
        listOf("+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0").forEach { alternate ->
            assertCode(NumericV1ErrorCode.INVALID_TEXT) { NumericV1Codec.decodeDecimalJson(alternate) }
        }
        listOf("+1", "01", "-0", "1.0", "1e0").forEach { alternate ->
            assertCode(NumericV1ErrorCode.INVALID_TEXT) { NumericV1Codec.decodeIntJson(alternate) }
        }
        assertCode(NumericV1ErrorCode.INVALID_TEXT) { NumericV1Codec.decodeQuantityJson("1.0") }
        assertCode(NumericV1ErrorCode.NEGATIVE_QUANTITY) { NumericV1Codec.decodeQuantityJson("-1") }
    }

    @Test
    fun canonicalFramesAndEnvelopesRoundtrip() {
        val integer = KotodamaInt.parse("-129")
        val integerEnvelope = NumericV1Codec.encodeIntEnvelope(integer)
        assertEquals(listOf(0x00, 0x11), integerEnvelope.take(2).map { it.toInt() and 0xFF })
        assertEquals(integer, NumericV1Codec.decodeIntFrame(NumericV1Codec.encodeIntFrame(integer)))
        assertEquals(integer, NumericV1Codec.decodeIntEnvelope(integerEnvelope))

        val decimal = KotodamaDecimal.parse("-1.25")
        val decimalEnvelope = NumericV1Codec.encodeDecimalEnvelope(decimal)
        assertEquals(listOf(0x00, 0x12), decimalEnvelope.take(2).map { it.toInt() and 0xFF })
        assertEquals(decimal, NumericV1Codec.decodeDecimalFrame(NumericV1Codec.encodeDecimalFrame(decimal)))
        assertEquals(decimal, NumericV1Codec.decodeDecimalEnvelope(decimalEnvelope))

        val quantity = KotodamaQuantity.parse("1.25")
        val quantityEnvelope = NumericV1Codec.encodeQuantityEnvelope(quantity)
        assertEquals(listOf(0x00, 0x13), quantityEnvelope.take(2).map { it.toInt() and 0xFF })
        assertEquals(quantity, NumericV1Codec.decodeQuantityFrame(NumericV1Codec.encodeQuantityFrame(quantity)))
        assertEquals(quantity, NumericV1Codec.decodeQuantityEnvelope(quantityEnvelope))

        assertCode(NumericV1ErrorCode.WRONG_TYPE) {
            NumericV1Codec.decodeDecimalEnvelope(NumericV1Codec.encodeIntEnvelope(KotodamaInt.parse("1")))
        }
    }

    @Test
    fun malformedAuthenticatedInputsAreRejected() {
        val frame = NumericV1Codec.encodeIntFrame(KotodamaInt.parse("128"))
        for (length in 0 until frame.size) {
            assertFailsWith<NumericV1Exception> { NumericV1Codec.decodeIntFrame(frame.copyOf(length)) }
        }
        val badChecksum = frame.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertCode(NumericV1ErrorCode.CHECKSUM_MISMATCH) { NumericV1Codec.decodeIntFrame(badChecksum) }

        val badHash = NumericV1Codec.encodeIntEnvelope(KotodamaInt.parse("1"))
            .also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertCode(NumericV1ErrorCode.PAYLOAD_HASH_MISMATCH) { NumericV1Codec.decodeIntEnvelope(badHash) }

        val retired = NumericV1Codec.encodeIntEnvelope(KotodamaInt.parse("1"))
            .also { it[0] = 0; it[1] = 0x10.toByte(); it[2] = 2 }
        assertCode(NumericV1ErrorCode.TYPE_NOT_ALLOWED) { NumericV1Codec.decodeIntEnvelope(retired) }

        val knownWrong = NumericV1Codec.encodeIntEnvelope(KotodamaInt.parse("1"))
            .also { it[0] = 0; it[1] = 0x01; it[2] = 2 }
        assertCode(NumericV1ErrorCode.WRONG_TYPE) { NumericV1Codec.decodeIntEnvelope(knownWrong) }

        val unknown = NumericV1Codec.encodeIntEnvelope(KotodamaInt.parse("1"))
            .also { it[0] = 0; it[1] = 0x14.toByte(); it[2] = 2 }
        assertCode(NumericV1ErrorCode.UNKNOWN_TYPE) { NumericV1Codec.decodeIntEnvelope(unknown) }
    }

    @Test
    fun consumesRustAuthoredSharedGoldenFixture() {
        val fixture = Json.parseToJsonElement(
            String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8),
        ).jsonObject
        assertEquals("iroha.numeric.v1", fixture.getValue("format").jsonPrimitive.content)
        assertEquals(512, fixture.getValue("signed_bits").jsonPrimitive.int)
        assertEquals(28, fixture.getValue("maximum_scale").jsonPrimitive.int)

        fixture.getValue("text").jsonArray.forEach { element ->
            val vector = element.jsonObject
            val canonical = when (vector.getValue("kind").jsonPrimitive.content) {
                "decimal" -> KotodamaDecimal.parse(vector.getValue("input").jsonPrimitive.content).toString()
                "quantity" -> KotodamaQuantity.parse(vector.getValue("input").jsonPrimitive.content).toString()
                else -> error("unknown text fixture kind")
            }
            assertEquals(vector.getValue("canonical").jsonPrimitive.content, canonical, vector.getValue("id").jsonPrimitive.content)
        }

        fixture.getValue("valid").jsonArray.forEach { element ->
            val vector = element.jsonObject
            val id = vector.getValue("id").jsonPrimitive.content
            val kind = vector.getValue("kind").jsonPrimitive.content
            val canonical = vector.getValue("canonical").jsonPrimitive.content
            val frame: ByteArray
            val envelope: ByteArray
            when (kind) {
                "int" -> {
                    val value = NumericV1Codec.decodeIntJson(canonical)
                    frame = NumericV1Codec.encodeIntFrame(value)
                    envelope = NumericV1Codec.encodeIntEnvelope(value)
                }
                "decimal" -> {
                    val value = NumericV1Codec.decodeDecimalJson(canonical)
                    frame = NumericV1Codec.encodeDecimalFrame(value)
                    envelope = NumericV1Codec.encodeDecimalEnvelope(value)
                }
                "quantity" -> {
                    val value = NumericV1Codec.decodeQuantityJson(canonical)
                    frame = NumericV1Codec.encodeQuantityFrame(value)
                    envelope = NumericV1Codec.encodeQuantityEnvelope(value)
                }
                else -> error("unknown fixture kind $kind")
            }
            assertEquals(vector.getValue("body_hex").jsonPrimitive.content, frame.copyOfRange(40, frame.size).hex(), "$id body")
            assertEquals(vector.getValue("frame_hex").jsonPrimitive.content, frame.hex(), "$id frame")
            assertEquals(vector.getValue("envelope_hex").jsonPrimitive.content, envelope.hex(), "$id envelope")
        }

        fixture.getValue("invalid").jsonArray.forEach { element ->
            val vector = element.jsonObject
            val input = vector.getValue("input").jsonPrimitive.content
            val decodeAs = vector.getValue("decode_as").jsonPrimitive.content
            val expected = NumericV1ErrorCode.valueOf(vector.getValue("expected").jsonPrimitive.content.uppercase())
            val bytes = vector.getValue("hex").jsonPrimitive.content.hexBytes()
            assertCode(expected) {
                when (input to decodeAs) {
                    "frame" to "int" -> NumericV1Codec.decodeIntFrame(bytes)
                    "frame" to "decimal" -> NumericV1Codec.decodeDecimalFrame(bytes)
                    "frame" to "quantity" -> NumericV1Codec.decodeQuantityFrame(bytes)
                    "envelope" to "int" -> NumericV1Codec.decodeIntEnvelope(bytes)
                    "envelope" to "decimal" -> NumericV1Codec.decodeDecimalEnvelope(bytes)
                    "envelope" to "quantity" -> NumericV1Codec.decodeQuantityEnvelope(bytes)
                    else -> error("unknown fixture decoder $input/$decodeAs")
                }
            }
        }
    }

    private fun assertCode(expected: NumericV1ErrorCode, block: () -> Unit) {
        assertEquals(expected, assertFailsWith<NumericV1Exception>(block = block).code)
    }

    private fun sharedFixture(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = current.resolve("fixtures/numeric_v1_golden.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: error("fixtures/numeric_v1_golden.json was not found")
        }
    }

    private fun ByteArray.hex(): String = joinToString("") { "%02x".format(it.toInt() and 0xFF) }

    private fun String.hexBytes(): ByteArray = chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
