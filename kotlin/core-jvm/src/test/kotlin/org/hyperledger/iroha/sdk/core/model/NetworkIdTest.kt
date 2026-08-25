package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals

class NetworkIdTest {
    @Test
    fun `round trips exact lowercase text and raw bytes`() {
        val parsed = NetworkId.parse(CANONICAL)
        val fromBytes = NetworkId.fromBytes(parsed.bytes())

        assertEquals(CANONICAL, parsed.literal)
        assertEquals(CANONICAL, parsed.toString())
        assertEquals(parsed, fromBytes)
        assertContentEquals(HashLiteral.decode(GENERIC_HASH_LITERAL), parsed.bytes())
    }

    @Test
    fun `rejects noncanonical public text`() {
        for (value in listOf(
            CANONICAL.uppercase(),
            CANONICAL.dropLast(1) + "8",
            CANONICAL.dropLast(1),
            "g" + CANONICAL.drop(1),
            GENERIC_HASH_LITERAL,
            " $CANONICAL",
            "$CANONICAL ",
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                NetworkId.parse(value)
            }
            assertEquals(true, error.message?.contains("64 lowercase hexadecimal characters"))
        }
    }

    @Test
    fun `fromBytes enforces marker and defensive copies`() {
        val source = HashLiteral.decode(GENERIC_HASH_LITERAL)
        val networkId = NetworkId.fromBytes(source)
        source[0] = (source[0].toInt() xor 0x7f).toByte()
        val exposed = networkId.bytes()
        exposed[1] = (exposed[1].toInt() xor 0x7f).toByte()

        assertEquals(CANONICAL, networkId.literal)
        assertNotEquals(source[0], networkId.bytes()[0])
        assertNotEquals(exposed[1], networkId.bytes()[1])

        val missingMarker = networkId.bytes()
        missingMarker[missingMarker.lastIndex] =
            (missingMarker.last().toInt() and 0xfe).toByte()
        assertFailsWith<IllegalArgumentException> {
            NetworkId.fromBytes(missingMarker)
        }
    }

    @Test
    fun `generic hash literal remains checksummed and separate`() {
        assertEquals(
            GENERIC_HASH_LITERAL,
            HashLiteral.canonicalize(HashLiteral.decode(GENERIC_HASH_LITERAL)),
        )
        val fromJson = NetworkId.parseNoritoJsonLiteral(GENERIC_HASH_LITERAL)
        assertEquals(CANONICAL, fromJson.literal)
        assertEquals(GENERIC_HASH_LITERAL, fromJson.noritoJsonLiteral)
        assertFailsWith<IllegalArgumentException> {
            NetworkId.parse(GENERIC_HASH_LITERAL)
        }
        for (invalid in listOf(
            GENERIC_HASH_LITERAL.lowercase(),
            GENERIC_HASH_LITERAL.replace("#A2F0", "#A2F1"),
            CANONICAL,
            " $GENERIC_HASH_LITERAL",
        )) {
            assertFailsWith<IllegalArgumentException> {
                NetworkId.parseNoritoJsonLiteral(invalid)
            }
        }
    }

    private companion object {
        const val CANONICAL =
            "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"
        const val GENERIC_HASH_LITERAL =
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    }
}
