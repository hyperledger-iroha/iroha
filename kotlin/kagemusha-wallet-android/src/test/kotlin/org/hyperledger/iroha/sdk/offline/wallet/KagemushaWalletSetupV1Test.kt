package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.junit.jupiter.api.Test

class KagemushaWalletSetupV1Test {
    @Test fun `activation transport has its own bound and no foreign inputs`() {
        KagemushaWalletSetupInputV1(15)
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(15, token = 1) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(15, first = byteArrayOf(1)) }
        KagemushaWalletCallV1(17, -1, 0, 0, 0, 0, ByteArray(16_384))
        for (count in listOf(0, 16_385)) assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(17, -1, 0, 0, 0, 0, ByteArray(count)) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(17, -1, 0, 1, 0, 0, byteArrayOf(1)) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(1, -1, 0, 0, 0, 0, ByteArray(10_001)) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(17, -1, 0, 0, 0, 0, byteArrayOf(1)).completion() }
    }
    @Test fun `setup fields are bounded copied and cannot inject time`() {
        val id = ByteArray(32) { 7 }
        val offer = byteArrayOf(1, 2, 3)
        val value = KagemushaWalletSetupInputV1(2, id, first = offer)
        id[0] = 4; offer[0] = 5
        assertEquals(7, value.identity()[0].toInt())
        assertContentEquals(byteArrayOf(1, 2, 3), value.first())
        KagemushaWalletSetupInputV1(1, value.identity(), KagemushaWalletUInt128V1(-1, -1))
        for (selector in 7..14) {
            KagemushaWalletSetupInputV1(selector, first = offer)
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(selector) }
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(selector, first = ByteArray(10_001)) }
        }
        KagemushaWalletSetupInputV1(0)
        KagemushaWalletSetupInputV1(4)
        KagemushaWalletSetupInputV1(6, token = 1)
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(6) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(6, token = 1, first = offer) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(4, id) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(1, id) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(2, id, first = offer, second = byteArrayOf(1)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(2, id, first = offer, second = byteArrayOf(1), third = ByteArray(513)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(5, token = 0, first = offer, second = offer) }
    }
    @Test fun `time challenge has nonce and native token distinct from completion`() {
        val nonce = ByteArray(32) { 1 }
        val value = KagemushaWalletCallV1(13, -1, 0, 7, 0, 0, nonce)
        val owner = Any()
        val token = value.exchange(owner)
        assertEquals(7L, token.tokenFor(owner))
        nonce[0] = 2
        assertEquals(1, token.nonce()[0].toInt())
        KagemushaWalletCallV1(12, -1, 0, 0, 0, 0, byteArrayOf(1))
        KagemushaWalletCallV1(14, -1, 0, 0, 0, 0, byteArrayOf())
        for (count in listOf(0, 31, 33)) assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(13, -1, 0, 7, 0, 0, ByteArray(count)) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(13, -1, 0, 0, 0, 0, ByteArray(32)) }
    }
    @Test fun `native time token can be consumed by only one racing caller`() {
        val owner = Any()
        val exchange = KagemushaWalletTimeExchangeV1(owner, 7, ByteArray(32) { 1 })
        val ready = java.util.concurrent.CountDownLatch(2)
        val start = java.util.concurrent.CountDownLatch(1)
        val accepted = java.util.concurrent.atomic.AtomicInteger()
        val refused = java.util.concurrent.atomic.AtomicInteger()
        val workers = List(2) {
            Thread {
                ready.countDown()
                start.await()
                try {
                    exchange.consume(owner)
                    accepted.incrementAndGet()
                } catch (_: IllegalArgumentException) {
                    refused.incrementAndGet()
                }
            }.apply { start() }
        }
        ready.await()
        start.countDown()
        workers.forEach { it.join() }
        assertEquals(1, accepted.get())
        assertEquals(1, refused.get())
        assertFailsWith<IllegalArgumentException> { exchange.tokenFor(owner) }
    }

}
