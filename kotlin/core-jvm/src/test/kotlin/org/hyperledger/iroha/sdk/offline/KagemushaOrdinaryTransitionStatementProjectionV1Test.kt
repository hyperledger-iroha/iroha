// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class KagemushaOrdinaryTransitionStatementProjectionV1Test {
    @Test
    fun `full model preimage correlates both cash purposes without a second prefix or codec`() {
        val original = modelSpecimen()
        for (preparation in listOf(false, true)) {
            val cash = approval(original, preparation)
            val projection = KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(original, cash)
            assertEquals(2, projection.operationTag())
            assertContentEquals(original, projection.canonicalPreimage())
            assertContentEquals(sha(original), projection.digest())
            assertContentEquals(cash.transitionStatementDigest(), projection.digest())
            // S hashes its complete 460-byte original independently of this 1145-byte transcript.
            assertContentEquals(sha(cash.selectionBytes()), cash.subjectSigningDigest())
        }
    }

    @Test
    fun `each of all1089 body bytes and every canonical framing byte is bound`() {
        val original = modelSpecimen()
        val cash = approval(original, true)
        for (offset in original.indices) {
            val changed = original.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }
            assertFailsWith<IllegalArgumentException>("original byte $offset") {
                KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(changed, cash)
            }
        }
        for (size in original.indices) assertFailsWith<IllegalArgumentException>("truncated $size") {
            KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(original.copyOf(size), cash)
        }
        assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(original + byteArrayOf(0), cash) }
        assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(original.copyOfRange(56, 1145), cash) }
        assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(ByteArray(93 * 32), cash) }
    }

    @Test
    fun `coherent hash substitutions cannot relabel operation scope version or endianness`() {
        val original = modelSpecimen()
        for (offset in listOf(0, 8, 47, 48, 55, 56, 57, 58, 59, 188, 56 + 373, 56 + 405,
            56 + 501, 56 + 533, 56 + 541, 56 + 573)) {
            val changed = original.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }
            assertFailsWith<IllegalArgumentException>("coherent changed byte $offset") {
                KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(changed, approval(changed, true))
            }
        }
        val littleHeader = original.copyOf().also {
            ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(0, 40).putLong(48, 1089)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(littleHeader, approval(littleHeader, true))
        }
        val bootstrap = original.copyOf().also { it[188] = 0 }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(bootstrap, approval(bootstrap, true))
        }
    }

    @Test
    fun `complete State original and returned digest remain immutable`() {
        val original = modelSpecimen()
        val expected = original.copyOf()
        val projection = KagemushaOrdinaryTransitionStatementProjectionV1.requireOriginal(original, approval(original, true))
        original.fill(0)
        projection.canonicalPreimage().fill(0); projection.digest().fill(0)
        assertContentEquals(expected, projection.canonicalPreimage())
        assertContentEquals(sha(expected), projection.digest())
    }

    // Synthetic data-only model layout specimen. Field order and widths follow the actual
    // commitments.rs transcript and one_use_key_ratchet.rs field-by-field test. This creates
    // no Native State, verified asset/credential, Guard, signature or financial authority.
    private fun modelSpecimen(): ByteArray {
        val selection = vectors().getValue("s_send_split_9")
        val body = ByteArrayOutputStream()
        fun uint(value: Long, width: Int) = body.write(ByteBuffer.allocate(width).order(ByteOrder.LITTLE_ENDIAN).let {
            when (width) { 2 -> it.putShort(value.toShort()); 4 -> it.putInt(value.toInt()); else -> it.putLong(value) }
            it.array()
        })
        fun u128(value: Long) { uint(value, 8); uint(0, 8) }
        fun digest(marker: Int) = body.write(ByteArray(32) { marker.toByte() })
        fun sDigest(offset: Int) = body.write(selection.copyOfRange(offset, offset + 32))
        uint(1, 2); uint(1, 2)
        digest(0x22); digest(0x23); digest(0x22); digest(0x23)
        body.write(2); u128(7)
        digest(0); digest(0); digest(0x40); digest(0x41); digest(0); digest(0x42); digest(0)
        sDigest(59); sDigest(59); digest(0x43); digest(0x44); sDigest(251)
        body.write(selection.copyOfRange(283, 291)); sDigest(187); sDigest(219); digest(0x45); uint(2, 4)
        digest(0x30); digest(0x31); u128(9); u128(10)
        u128(1); digest(0x46); u128(1); digest(0x46)
        digest(0x20); digest(0x21); digest(0x20); digest(0x21)
        digest(0x32); digest(0x33); u128(19); u128(20); digest(0x34)
        assertEquals(1089, body.size())
        val domain = "iroha:kagemusha:v1:transition-statement\u0000".toByteArray(Charsets.US_ASCII)
        return ByteBuffer.allocate(8).order(ByteOrder.BIG_ENDIAN).putLong(domain.size.toLong()).array() + domain +
            ByteBuffer.allocate(8).order(ByteOrder.BIG_ENDIAN).putLong(body.size().toLong()).array() + body.toByteArray()
    }

    private fun approval(preimage: ByteArray, preparation: Boolean): KagemushaOrdinaryCashApprovalProjectionV1 {
        val vectors = vectors()
        val s = vectors.getValue("s_send_split_9").copyOf()
        val w = vectors.getValue("w_send_split_9").copyOf()
        w.copyInto(s, 155, 213, 245)
        sha(preimage).copyInto(s, 332)
        if (preparation) { w[52] = 2; s.fill(0, 364, 428) }
        sha(s).copyInto(w, 245)
        val binding = KagemushaOrdinaryCashApprovalOriginalBindingV1(w.copyOfRange(53, 85), w.copyOfRange(117, 149),
            w.copyOfRange(149, 181), w.copyOfRange(181, 213), w.copyOfRange(213, 245), w.copyOfRange(277, 309), s)
        return if (preparation) KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(w, s, binding)
        else KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(w, s, binding)
    }

    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun vectors(): Map<String, ByteArray> {
        var directory = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val path = directory.resolve("fixtures/offline/kagemusha_app_platform_messages_v1.tsv")
            if (Files.isRegularFile(path)) return Files.readAllLines(path, Charsets.UTF_8)
                .filter { !it.startsWith("#") && it.isNotEmpty() }.associate { line ->
                    val columns = line.split('\t')
                    require(columns.size == 2)
                    columns[0] to columns[1].chunked(2).map { it.toInt(16).toByte() }.toByteArray()
                }
            directory = directory.parent
        }
        error("missing Rust app platform message fixture")
    }
}
