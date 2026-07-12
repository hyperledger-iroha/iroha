package org.hyperledger.iroha.sdk.client

import java.math.BigDecimal
import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class JsonParserTest {

    @Test
    fun checkedLongCoercionAcceptsLongBounds() {
        assertEquals(
            Long.MAX_VALUE,
            JsonNumbers.asLong(JsonParser.parse(Long.MAX_VALUE.toString()), "max"),
        )
        assertEquals(
            Long.MIN_VALUE,
            JsonNumbers.asLong(JsonParser.parse(Long.MIN_VALUE.toString()), "min"),
        )
    }

    @Test
    fun checkedLongCoercionRejectsOutOfRangeIntegers() {
        assertFailsWith<IllegalStateException> {
            JsonNumbers.asLong(JsonParser.parse("9223372036854775808"), "height")
        }
        assertFailsWith<IllegalStateException> {
            JsonNumbers.asLong(JsonParser.parse("-9223372036854775809"), "height")
        }
    }

    @Test
    fun oversizedIntegerTokensRemainAvailableForBigIntegerConsumers() {
        val raw = "184467440737095516160000000000000000000"
        assertEquals(BigInteger(raw), JsonParser.parse(raw))
    }

    @Test
    fun decimalAndExponentTokensRemainExact() {
        assertEquals(BigDecimal("-0.5"), JsonParser.parse("-0.5"))
        assertEquals(BigDecimal("1e400"), JsonParser.parse("1e400"))
    }

    @Test
    fun duplicateObjectKeysAreRejectedBeforeLastKeyWinsParsing() {
        assertFailsWith<IllegalStateException> {
            JsonParser.parse("""{"bundle_id":"forged","bundle_id":"trusted"}""")
        }
        assertFailsWith<IllegalStateException> {
            JsonParser.parse("""{"outer":{"key":1,"key":2}}""")
        }
        assertFailsWith<IllegalStateException> {
            JsonParser.parse("""{"bundle\u005fid":"forged","bundle_id":"trusted"}""")
        }
    }

    @Test
    fun stringsRequireValidJsonControlsAndUnicodeScalars() {
        assertEquals("emoji: 😀", JsonParser.parse("\"emoji: \\uD83D\\uDE00\""))
        assertEquals("emoji: 😀", JsonParser.parse("\"emoji: 😀\""))
        assertFailsWith<IllegalStateException> { JsonParser.parse("\"raw\u0001control\"") }
        assertFailsWith<IllegalStateException> { JsonParser.parse("\"\\uD800\"") }
        assertFailsWith<IllegalStateException> { JsonParser.parse("\"${'\uD800'}\"") }
        assertFailsWith<IllegalStateException> { JsonParser.parse("\"\\uDC00\"") }
    }

    @Test
    fun nestingIsBoundedBeforeTheRuntimeStack() {
        val accepted = "[".repeat(128) + "0" + "]".repeat(128)
        JsonParser.parse(accepted)
        val rejected = "[".repeat(129) + "0" + "]".repeat(129)
        assertFailsWith<IllegalStateException> { JsonParser.parse(rejected) }
    }
}
