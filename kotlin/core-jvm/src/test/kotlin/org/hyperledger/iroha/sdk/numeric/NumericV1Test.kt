package org.hyperledger.iroha.sdk.numeric

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

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
            KotodamaInt.of(NumericV1Codec.intMax.add(BigInteger.ONE))
        }
        assertCode(NumericV1ErrorCode.INVALID_SCALE) {
            KotodamaDecimal.parse("1.00000000000000000000000000000")
        }
        assertCode(NumericV1ErrorCode.INVALID_TEXT) { KotodamaInt.parse("01") }
    }

    @Test
    fun canonicalFramesAndEnvelopesRoundtrip() {
        val integer = KotodamaInt.parse("-129")
        assertEquals(integer, NumericV1Codec.decodeIntFrame(NumericV1Codec.encodeIntFrame(integer)))
        assertEquals(integer, NumericV1Codec.decodeIntEnvelope(NumericV1Codec.encodeIntEnvelope(integer)))

        val decimal = KotodamaDecimal.parse("-1.25")
        assertEquals(decimal, NumericV1Codec.decodeDecimalFrame(NumericV1Codec.encodeDecimalFrame(decimal)))
        assertEquals(decimal, NumericV1Codec.decodeDecimalEnvelope(NumericV1Codec.encodeDecimalEnvelope(decimal)))

        val quantity = KotodamaQuantity.parse("1.25")
        assertEquals(quantity, NumericV1Codec.decodeQuantityFrame(NumericV1Codec.encodeQuantityFrame(quantity)))
        assertEquals(quantity, NumericV1Codec.decodeQuantityEnvelope(NumericV1Codec.encodeQuantityEnvelope(quantity)))

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
            .also { it[0] = 0; it[1] = 0x10 }
        assertCode(NumericV1ErrorCode.TYPE_NOT_ALLOWED) { NumericV1Codec.decodeIntEnvelope(retired) }
    }

    private fun assertCode(expected: NumericV1ErrorCode, block: () -> Unit) {
        assertEquals(expected, assertFailsWith<NumericV1Exception>(block = block).code)
    }
}
