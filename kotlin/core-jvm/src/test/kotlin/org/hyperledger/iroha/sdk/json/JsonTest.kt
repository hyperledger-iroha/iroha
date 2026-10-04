package org.hyperledger.iroha.sdk.json

import java.math.BigDecimal
import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class JsonTest {
    @Test
    fun numbersKeepTheirExactSpelling() {
        val parsed = Json.parse("""{"u128":340282366920938463463374607431768211455,"amount":10.50,"small":-0.000000001}""")
            as JsonObject
        val u128 = parsed["u128"] as JsonNumber
        assertEquals("340282366920938463463374607431768211455", u128.text)
        assertEquals(BigInteger("340282366920938463463374607431768211455"), u128.toBigInteger())
        assertEquals(BigDecimal("10.50"), (parsed["amount"] as JsonNumber).toBigDecimal())
        assertEquals("-0.000000001", (parsed["small"] as JsonNumber).text)
        assertEquals(
            """{"u128":340282366920938463463374607431768211455,"amount":10.50,"small":-0.000000001}""",
            parsed.toJsonString(),
        )
        assertFailsWith<IllegalStateException> { (parsed["amount"] as JsonNumber).toBigInteger() }
        assertFailsWith<IllegalStateException> { u128.toLongExact() }
        assertEquals("1000", JsonNumber.of(BigDecimal("1E+3")).text)
    }

    @Test
    fun parsingIsStrict() {
        val rejected = listOf(
            """{"a":1,"a":2}""",
            """{"a":1}x""",
            "[1,]",
            "01",
            "1.",
            ".5",
            "+1",
            "\"\\x\"",
            "\"tab\there\"",
            "\"\\ud800\"",
            "nul",
            "",
            "[" .repeat(Json.MAX_DEPTH + 2),
        )
        for (text in rejected) {
            assertFailsWith<JsonSyntaxException>("`$text` must be rejected") { Json.parse(text) }
        }
        assertFailsWith<JsonSyntaxException> { Json.parse(byteArrayOf(0x22, 0xC3.toByte(), 0x28, 0x22)) }
    }

    @Test
    fun stringsUseNoritoEscapes() {
        val value = JsonString("q\" b\\ n\n r\r t\t b\b f\u000C c\u0001 é 😀 /")
        assertEquals("\"q\\\" b\\\\ n\\n r\\r t\\t b\\b f\\f c\\u0001 é 😀 /\"", value.toJsonString())
        assertEquals(value, Json.parse(value.toJsonString()))
        assertEquals(JsonString("😀"), Json.parse("\"\\ud83d\\ude00\""))
    }

    @Test
    fun objectsCompareLikeJsonAndStayImmutable() {
        val left = Json.parse("""{"a":1,"b":[true,null]}""")
        val right = Json.parse("""{"b":[true,null],"a":1}""")
        assertEquals(left, right)
        assertEquals(left.hashCode(), right.hashCode())
        assertNotEquals(Json.parse("[1,2]"), Json.parse("[2,1]"))
        assertNotEquals<Json>(JsonNumber.of(1L), JsonNumber.parse("1.0"))
        val obj = left as JsonObject
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (obj.members as MutableMap<String, Json>)["c"] = Json.NULL
        }
        assertTrue(obj.containsKey("a"))
        assertEquals(null, obj.stringOrNull("missing"))
        assertFailsWith<IllegalStateException> { obj.stringOrNull("a") }
    }

    @Test
    fun builderRejectsDuplicateMembers() {
        val built = JsonObject.builder().put("id", "x").put("n", 1L).put("ok", true).build()
        assertEquals("""{"id":"x","n":1,"ok":true}""", built.toJsonString())
        assertFailsWith<IllegalArgumentException> { JsonObject.builder().put("id", "x").put("id", "y") }
    }
}
