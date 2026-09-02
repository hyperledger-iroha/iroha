package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertFailsWith
import kotlin.test.assertNull

class KagemushaPeerTextBoundsTest {
    @Test
    fun `oversized text fails before Base64 allocation`() {
        val maximum = KagemushaPeerTransportContract.MAXIMUM_TEXT_ENVELOPE_BYTES
        val oversized = "A".repeat(maximum + 4)

        assertFailsWith<IllegalArgumentException> {
            KagemushaPeerTextCodec.decode(oversized)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaPeerTextCodec.decodeUserPresented(oversized)
        }
        assertNull(KagemushaPeerTextCodec.base64UrlDecode(oversized))
    }
}
