// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Unsigned transport specimens only; no Native holder, issuer or Google admission. */
class KagemushaPlayIntegrityRefreshPreparationV1Test {
    @Test fun `original signature selectors and transport are copied without granting custody`() {
        val original = fixture(); val parsed = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original)
        original.fill(0); parsed.transportBytes().fill(0); parsed.signatureBytes().fill(0); parsed.nonce().fill(0)
        assertContentEquals(fixture(), parsed.transportBytes())
        assertContentEquals(ByteArray(64) { 0x40 }, parsed.signatureBytes())
        assertContentEquals(ByteArray(32) { 12 }, parsed.nonce())
        assertContentEquals(ByteArray(32) { 13 }, parsed.originalEnrollmentChallengeDigest())
    }

    @Test fun `attempt hashes full Core signing transcript rather than signed transport or another hash`() {
        val original = fixture(); val parsed = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original)
        val full = domain("challenge") + le64(450) + original.copyOfRange(0, 450)
        assertContentEquals(full, parsed.canonicalSigningBytes())
        assertContentEquals(sha(full), parsed.operationId())
        assertFalse(parsed.operationId().contentEquals(sha(original)))
        assertFalse(parsed.operationId().contentEquals(sha(sha(full))))
        // A signature byte is outside the Core signing body; it cannot create a new attempt ID.
        original[513] = 0x41
        assertContentEquals(parsed.operationId(), KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original).operationId())
    }

    @Test fun `Google request and possession are distinct full transcript equations`() {
        val parsed = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(fixture())
        val full = parsed.canonicalSigningBytes()
        assertContentEquals(sha(domain("request") + full + ByteArray(32) { 2 }), parsed.playIntegrityRequestHash())
        assertContentEquals(domain("possession") + le64(full.size.toLong()) + full, parsed.possessionSigningBytes())
        assertFalse(parsed.operationId().contentEquals(parsed.playIntegrityRequestHash()))
        assertFalse(full.contentEquals(parsed.possessionSigningBytes()))
        val changed = fixture().also { it[2 + 32] = 17 }
        val other = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(changed)
        assertFalse(parsed.playIntegrityRequestHash().contentEquals(other.playIntegrityRequestHash()))
    }

    @Test fun `trailing truncated enrollment and changed version frames are rejected`() {
        for (bytes in listOf(fixture() + byteArrayOf(0), fixture().copyOf(513), ByteArray(515),
            fixture().also { it[0] = 2 }, fixture().also { it[1] = 1 })) {
            assertFailsWith<IllegalArgumentException> { KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(bytes) }
        }
    }

    @Test fun `missing original selectors and either epoch are rejected`() {
        repeat(13) { field ->
            val broken = fixture().also { it.fill(0, 2 + field * 32, 34 + field * 32) }
            assertFailsWith<IllegalArgumentException> { KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(broken) }
        }
        for (offset in listOf(418, 426)) {
            assertFailsWith<IllegalArgumentException> { KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(fixture().also { it.fill(0, offset, offset + 8) }) }
        }
    }

    @Test fun `original lifetime cannot be absent reversed or longer than native limit`() {
        for ((issue, expiry) in listOf(0L to 1L, 1000L to 1000L, 1000L to 999L, 1000L to 121001L)) {
            val original = fixture().also { ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(434, issue).putLong(442, expiry) }
            assertFailsWith<IllegalArgumentException> { KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original) }
        }
    }

    @Test fun `unsigned epochs and times retain values above signed Long`() {
        val original = fixture().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(418, -1).putLong(426, -1)
                .putLong(434, Long.MIN_VALUE).putLong(442, Long.MIN_VALUE + 1000)
        }
        val parsed = KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original)
        assertEquals(BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE), parsed.policyEpoch)
        assertEquals(BigInteger.ONE.shiftLeft(63), parsed.issuedAtMs)
        assertEquals(BigInteger.valueOf(1000), parsed.expiresAtMs.subtract(parsed.issuedAtMs))
    }

    private fun fixture(): ByteArray = ByteArray(514).also {
        it[0] = 1; repeat(13) { field -> it.fill((field + 1).toByte(), 2 + field * 32, 34 + field * 32) }
        ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(418, 1).putLong(426, 2).putLong(434, 1000).putLong(442, 121000)
        it.fill(0x40, 450, 514)
    }
    private fun domain(kind: String) = "iroha:kagemusha:v1:play-integrity-refresh-$kind\u0000".toByteArray(Charsets.US_ASCII)
    private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
}
