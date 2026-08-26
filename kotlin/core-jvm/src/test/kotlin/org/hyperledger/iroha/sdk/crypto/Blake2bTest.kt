package org.hyperledger.iroha.sdk.crypto

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertFailsWith

class Blake2bTest {
    @Test
    fun `fixed and variable digests match reference vectors`() {
        val message = "abc".toByteArray()
        val digest256 = hex("bddd813c634239723171ef3fee98579b94964e3bb1cb3e427262c8c068d52319")
        val digest512 = hex(
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d" +
                "17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923",
        )

        assertContentEquals(digest256, Blake2b.digest256(message))
        assertContentEquals(digest256, Blake2b.digest(message))
        assertContentEquals(digest512, Blake2b.digest512(message))
        assertContentEquals(hex("d8bb14d833d59559"), Blake2b.digest(message, 8))
    }

    @Test
    fun `variable digest rejects invalid output lengths`() {
        assertFailsWith<IllegalArgumentException> { Blake2b.digest(byteArrayOf(), 0) }
        assertFailsWith<IllegalArgumentException> { Blake2b.digest(byteArrayOf(), 65) }
    }
}

private fun hex(value: String): ByteArray =
    ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
