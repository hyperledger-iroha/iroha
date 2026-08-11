package org.hyperledger.iroha.sdk.core.model

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class JsonValueTest {

    @Test
    fun textConstructionUsesOneNoritoLexicalForm() {
        val cases = mapOf(
            "1 " to "1",
            "{\"z\":0,\"a\":1}" to "{\"a\":1,\"z\":0}",
            "\"\\u0061\"" to "\"a\"",
            "\"\\u0008\\u000c\"" to "\"\\b\\f\"",
            "1e0" to "1.0",
            "-0" to "-0.0",
            "1e20" to "1e+20",
            "5e-324" to "5e-324",
        )

        for ((input, expected) in cases) {
            assertEquals(expected, JsonValue.parse(input).canonicalJson, input)
        }
    }

    @Test
    fun objectKeysUseUnicodeScalarOrder() {
        assertEquals(
            "{\"\uE000\":2,\"\uD800\uDC00\":1}",
            JsonValue.parse("{\"\uD800\uDC00\":1,\"\uE000\":2}").canonicalJson,
        )
    }

    @Test
    fun signedWireRequiresTheAlreadyCanonicalSpelling() {
        assertEquals("{\"a\":1}", JsonValue.fromCanonicalWire("{\"a\":1}").canonicalJson)
        for (alternate in listOf("1 ", "{\"z\":0,\"a\":1}", "1e0", "-0")) {
            assertFailsWith<IllegalArgumentException>(alternate) {
                JsonValue.fromCanonicalWire(alternate)
            }
        }
    }

    @Test
    fun invalidAndOutOfRangeJsonNeverEntersTheWrapper() {
        for (invalid in listOf("", "plain", "{\"a\":1,\"a\":2}", "1e400")) {
            assertFailsWith<IllegalArgumentException>(invalid) { JsonValue.parse(invalid) }
        }
    }
}
