package org.hyperledger.iroha.sdk.crypto

import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.bouncycastle.math.ec.rfc8032.Ed25519

class Ed25519PublicKeyAdmissionCacheTest {
    private fun publicKey(seed: Int): ByteArray = ByteArray(32).also { key ->
        Ed25519.generatePublicKey(ByteArray(32) { seed.toByte() }, 0, key, 0)
    }

    @Test
    fun `equal public byte contents reuse only a fully validated positive`() {
        val validations = AtomicInteger()
        val cache = PositivePublicKeyAdmissionCache(2) { key ->
            validations.incrementAndGet()
            Ed25519PublicKeyAdmission.isValidUncached(key)
        }
        val original = publicKey(17)
        assertTrue(cache.isValid(original))
        repeat(20) { assertTrue(cache.isValid(original.copyOf())) }
        assertEquals(1, validations.get())
    }

    @Test
    fun `caller mutation cannot replace admitted contents`() {
        val validations = AtomicInteger()
        val cache = PositivePublicKeyAdmissionCache(2) { key ->
            validations.incrementAndGet()
            Ed25519PublicKeyAdmission.isValidUncached(key)
        }
        val key = publicKey(18)
        val original = key.copyOf()
        assertTrue(cache.isValid(key))
        key.fill(0)
        repeat(2) { assertFalse(cache.isValid(key)) }
        assertTrue(cache.isValid(original))
        assertEquals(3, validations.get())
        assertTrue(Ed25519PublicKeyAdmission.isValid(original))
        assertFalse(Ed25519PublicKeyAdmission.isValid(key))
    }

    @Test
    fun `hash collision never substitutes different public bytes`() {
        val valid = byteArrayOf(0x3b, 0x6a) + hex("27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29")
        val collision = valid.copyOf().also { key ->
            key[0] = (key[0] + 1).toByte()
            key[1] = (key[1] - 31).toByte()
        }
        assertEquals(valid.contentHashCode(), collision.contentHashCode())
        assertTrue(Ed25519PublicKeyAdmission.isValidUncached(valid))
        assertFalse(Ed25519PublicKeyAdmission.isValidUncached(collision))
        val cache = PositivePublicKeyAdmissionCache(2, Ed25519PublicKeyAdmission::isValidUncached)
        assertTrue(cache.isValid(valid))
        assertFalse(cache.isValid(collision))
        assertTrue(cache.isValid(valid.copyOf()))
    }

    @Test
    fun `negative curve torsion and noncanonical results are never retained`() {
        val validations = AtomicInteger()
        val cache = PositivePublicKeyAdmissionCache(2) { key ->
            validations.incrementAndGet()
            Ed25519PublicKeyAdmission.isValidUncached(key)
        }
        val invalid = listOf(
            ByteArray(32),
            byteArrayOf(1) + ByteArray(31),
            hex("eeffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f"),
            hex("0100000000000000000000000000000000000000000000000000000000000080"),
            ByteArray(32) { 2 },
            ByteArray(32) { 0x11 },
            hex("6aebc0b955ce4a2f1344029986b775e6ea5c40f93f1112b86ec51678eb9dc0fb"),
        )
        for (key in invalid) repeat(3) { assertFalse(cache.isValid(key.copyOf())) }
        assertEquals(invalid.size * 3, validations.get())
        assertTrue(cache.isValid(publicKey(19)))
        for (key in invalid) assertFalse(cache.isValid(key))
        assertEquals(invalid.size * 4 + 1, validations.get())
    }

    @Test
    fun `least recently used positive is evicted at the exact bound`() {
        val validations = AtomicInteger()
        val cache = PositivePublicKeyAdmissionCache(2) { key ->
            validations.incrementAndGet()
            Ed25519PublicKeyAdmission.isValidUncached(key)
        }
        val a = publicKey(20); val b = publicKey(21); val c = publicKey(22)
        assertTrue(cache.isValid(a)); assertTrue(cache.isValid(b))
        assertTrue(cache.isValid(a.copyOf()))
        assertTrue(cache.isValid(c))
        assertEquals(3, validations.get())
        assertTrue(cache.isValid(a.copyOf()))
        assertEquals(3, validations.get())
        assertTrue(cache.isValid(b.copyOf()))
        assertEquals(4, validations.get())
        // Re-admission does not preserve a point evicted to stay within two entries.
        assertTrue(cache.isValid(c.copyOf()))
        assertEquals(5, validations.get())
    }

    @Test
    fun `concurrent cold and warm admissions preserve strict immutable contents`() {
        val cache = PositivePublicKeyAdmissionCache(2, Ed25519PublicKeyAdmission::isValidUncached)
        val valid = publicKey(23)
        val pool = Executors.newFixedThreadPool(8)
        try {
            val work = (0 until 16).map {
                Callable {
                    repeat(200) { assertTrue(cache.isValid(valid.copyOf())) }
                    assertFalse(cache.isValid(byteArrayOf(1) + ByteArray(31)))
                }
            }
            pool.invokeAll(work).forEach { it.get(30, TimeUnit.SECONDS) }
            assertTrue(cache.isValid(valid.copyOf()))
        } finally {
            pool.shutdownNow()
            assertTrue(pool.awaitTermination(30, TimeUnit.SECONDS))
        }
    }

    @Test
    fun `cached and uncached samples exercise the same full validator`() {
        val key = publicKey(24)
        repeat(5) { assertTrue(Ed25519PublicKeyAdmission.isValidUncached(key)) }
        assertTrue(Ed25519PublicKeyAdmission.isValid(key))
        val uncached = LongArray(20) {
            val before = System.nanoTime()
            assertTrue(Ed25519PublicKeyAdmission.isValidUncached(key))
            System.nanoTime() - before
        }.sorted()
        val before = System.nanoTime()
        repeat(10_000) { assertTrue(Ed25519PublicKeyAdmission.isValid(key.copyOf())) }
        val cachedAverage = (System.nanoTime() - before) / 10_000
        println("Ed25519 admission nanoseconds: uncached median=${uncached[10]} min=${uncached.first()} max=${uncached.last()} cached average=$cachedAverage (same validated public point)")
    }

    private fun hex(text: String): ByteArray = ByteArray(text.length / 2) { i ->
        text.substring(i * 2, i * 2 + 2).toInt(16).toByte()
    }
}
