package org.hyperledger.iroha.sdk.offline.wallet

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Strict local ABI33 envelope checks; no fixture grants Native maintenance authority. */
class KagemushaWalletDeferredArchiveV1Test {
    private fun reply(kind: Int, low: Long = 0, high: Long = 0, detail: Int = 0,
        bytes: ByteArray = byteArrayOf(), reason: Int = -1, platform: Int = 0) =
        KagemushaWalletCallV1(kind, reason, platform, low, high, detail, bytes)
    private fun refuses(action: () -> Unit) {
        assertThrows(KagemushaWalletExceptionV1::class.java) { action() }
    }
    private fun nested(kind: Int, payload: ByteArray = byteArrayOf(), detail: Int = 0,
        low: Long = 0, high: Long = 0): ByteArray = ByteArray(28 + payload.size).also { bytes ->
        fun word(offset: Int, value: Long, width: Int) {
            repeat(width) { bytes[offset + it] = (value ushr (8 * it)).toByte() }
        }
        word(0, kind.toLong(), 4); word(4, detail.toLong(), 4)
        word(8, low, 8); word(16, high, 8); word(24, payload.size.toLong(), 4)
        payload.copyInto(bytes, 28)
    }

    @Test fun `exchange controls require closed shapes and never become completions`() {
        assertEquals(9L, reply(60, low = 9).exchangeToken())
        for (detail in 0..2) assertEquals(KagemushaWalletExchangeEndV1.values()[detail], reply(61, detail = detail).exchangeEnd())
        for (kind in listOf(60, 61, 62, 63, 64, 65)) {
            refuses { reply(kind, low = 9, detail = 6, bytes = byteArrayOf(1)) }
        }
        refuses { reply(60) }; refuses { reply(60, low = -1) }; refuses { reply(60, low = 1, high = 1) }
        refuses { reply(61, low = 1) }; refuses { reply(61, detail = 3) }
        refuses { reply(60, low = 1, reason = 0) }; refuses { reply(61, platform = 1) }
        refuses { reply(60, low = 1).completion() }; refuses { reply(61).completion() }
    }

    @Test fun `no work and all six waits remain typed nonmonetary outcomes`() {
        assertSame(KagemushaWalletDeferredArchiveDrainV1.NoWork, KagemushaWalletDeferredArchiveDrainV1.from(reply(62)))
        for (tag in 1..6) {
            val result = KagemushaWalletDeferredArchiveDrainV1.from(reply(63, detail = tag)) as KagemushaWalletDeferredArchiveDrainV1.Blocked
            assertEquals(KagemushaWalletMaintenanceWaitV1.values()[tag - 1], result.wait)
        }
        refuses { reply(62, detail = 1) }; refuses { reply(62, low = 1) }
        for (tag in listOf(-1, 0, 7)) refuses { reply(63, detail = tag) }
        refuses { reply(63, high = 1, detail = 1) }
        refuses { KagemushaWalletDeferredArchiveDrainV1.from(reply(6)) }
    }

    @Test fun `nested completion preserves exact original and all sequence bits`() {
        for (kind in listOf(1, 2, 3, 4, 5, 10)) {
            val payload = if (kind == 1 || kind == 10) byteArrayOf(9, 8, 7) else byteArrayOf()
            val bytes = nested(kind, payload, low = -1, high = Long.MIN_VALUE)
            val outer = reply(64, detail = 1, bytes = bytes)
            bytes.fill(0)
            val result = KagemushaWalletDeferredArchiveDrainV1.from(outer) as KagemushaWalletDeferredArchiveDrainV1.Reconciled
            assertEquals(kind, result.completion.status)
            assertEquals(-1L, result.completion.sequenceLow); assertEquals(Long.MIN_VALUE, result.completion.sequenceHigh)
            assertArrayEquals(payload, result.completion.bytes())
            val executed = KagemushaWalletDeferredArchiveDrainV1.from(reply(64, detail = 2,
                bytes = nested(kind, payload))) as KagemushaWalletDeferredArchiveDrainV1.Executed
            assertEquals(kind, executed.completion.status)
        }
    }

    @Test fun `nested completion rejects unknown kind truncation expansion and nonexact length`() {
        for (kind in listOf(0, 6, 11, 47, 64, 65, -1)) {
            refuses { KagemushaWalletDeferredArchiveDrainV1.from(reply(64, detail = 1, bytes = nested(kind))) }
        }
        refuses { reply(64, detail = 1, bytes = ByteArray(27)) }
        refuses { reply(64, detail = 1, bytes = ByteArray(10_063)) }
        for (bytes in listOf(nested(1, byteArrayOf(7)).copyOf(28), nested(2) + byteArrayOf(0),
            nested(2).also { it[24] = 0xff.toByte(); it[27] = 0xff.toByte() })) {
            refuses { KagemushaWalletDeferredArchiveDrainV1.from(reply(64, detail = 1, bytes = bytes)) }
        }
        val maximum = KagemushaWalletDeferredArchiveDrainV1.from(reply(64, detail = 2,
            bytes = nested(1, ByteArray(10_034) { 3 }))) as KagemushaWalletDeferredArchiveDrainV1.Executed
        assertEquals(10_034, maximum.completion.bytes().size)
        refuses { reply(64, detail = 0, bytes = nested(2)) }
        refuses { reply(64, detail = 3, bytes = nested(2)) }
        refuses { reply(64, low = 1, detail = 1, bytes = nested(2)) }
    }

    @Test fun `retained review permits only exact maintenance tags and positive token`() {
        for (tag in listOf(1, 2, 6)) assertEquals(tag, reply(65, low = 7, detail = tag).detail)
        for (tag in listOf(-1, 0, 3, 4, 5, 7)) refuses { reply(65, low = 7, detail = tag) }
        refuses { reply(65, detail = 1) }; refuses { reply(65, low = -1, detail = 1) }
        refuses { reply(65, low = 7, high = 1, detail = 1) }
        refuses { reply(65, low = 7, detail = 1, bytes = byteArrayOf(1)) }
        refuses { reply(65, low = 7, detail = 1).completion() }
    }

    @Test fun `retired Archive producers are refused and exact intake remains bounded`() {
        for (selector in listOf(3, 41, 54)) assertThrows(IllegalArgumentException::class.java) {
            KagemushaWalletSetupInputV1(selector, identity = ByteArray(32) { 1 }, first = byteArrayOf(2))
        }
        val identity = ByteArray(32) { 1 }; val original = ByteArray(10_034) { 2 }
        val input = KagemushaWalletSetupInputV1(56, identity = identity, first = original)
        identity.fill(0); original.fill(0)
        assertEquals(1.toByte(), input.identity()[0]); assertEquals(2.toByte(), input.first()[0])
        assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(56, first = byteArrayOf(2)) }
        assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(56, identity = ByteArray(32) { 1 }) }
        assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(56, identity = ByteArray(32) { 1 }, first = ByteArray(10_035)) }
        for (selector in listOf(57, 58)) {
            KagemushaWalletSetupInputV1(selector)
            assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(selector, first = byteArrayOf(1)) }
            assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(selector, identity = ByteArray(32) { 1 }) }
        }
        assertThrows(IllegalArgumentException::class.java) { KagemushaWalletSetupInputV1(44,
            identity = ByteArray(32) { 1 }, first = byteArrayOf(2), second = byteArrayOf(3)) }
    }
}
