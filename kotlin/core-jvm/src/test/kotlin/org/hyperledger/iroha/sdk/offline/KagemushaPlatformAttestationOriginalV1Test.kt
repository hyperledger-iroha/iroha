// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint
import org.junit.jupiter.api.Test

/** Shared Rust data fixtures exercise the archive contract, never verifier or device authority. */
class KagemushaPlatformAttestationOriginalV1Test {
    @Test
    fun `both roles encode decode and digest the exact Rust archives`() {
        val chain = fixtureChain()
        val apple = fixtureBytes("apple-guide-original.cbor")
        val cases = listOf(
            "android-mock-osp-unit.hex" to KagemushaPlatformAttestationOriginalV1.android(chain),
            "apple-guide-unit.hex" to KagemushaPlatformAttestationOriginalV1.apple(apple),
        )
        for ((name, value) in cases) {
            val golden = fixtureHex(name)
            assertContentEquals(golden, value.canonicalBytes(), name)
            val decoded = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(golden)
            assertContentEquals(golden, decoded.canonicalBytes(), name)
            assertContentEquals(MessageDigest.getInstance("SHA-256").digest(golden), value.canonicalDigest(), name)
            assertContentEquals(value.canonicalDigest(), decoded.canonicalDigest(), name)
        }
        val android = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(cases[0].second.canonicalBytes())
        assertChainEquals(chain, assertNotNull(android.androidCertificateChainDer()))
        assertNull(android.appleAttestationObjectCbor())
        val enrolledApple = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(cases[1].second.canonicalBytes())
        assertContentEquals(apple, enrolledApple.appleAttestationObjectCbor())
        assertNull(enrolledApple.androidCertificateChainDer())
    }

    @Test
    fun `original order byte substitution and role are bound by the whole archive digest`() {
        val chain = fixtureChain()
        val android = KagemushaPlatformAttestationOriginalV1.android(chain)
        assertDifferent(android, KagemushaPlatformAttestationOriginalV1.android(chain.reversed()))
        val substituted = chain.map { it.copyOf() }
        substituted[0][0] = (substituted[0][0].toInt() xor 1).toByte()
        assertDifferent(android, KagemushaPlatformAttestationOriginalV1.android(substituted))
        val apple = fixtureBytes("apple-guide-original.cbor")
        val alteredApple = apple.copyOf().also { it[0] = (it[0].toInt() xor 1).toByte() }
        assertDifferent(KagemushaPlatformAttestationOriginalV1.apple(apple),
            KagemushaPlatformAttestationOriginalV1.apple(alteredApple))
        // Opaque bounded bytes are data. A successful round trip does not attest them.
        val opaque = byteArrayOf(0xfe.toByte(), 0, 0x80.toByte())
        val androidOpaque = KagemushaPlatformAttestationOriginalV1.android(listOf(opaque, opaque))
        val appleOpaque = KagemushaPlatformAttestationOriginalV1.apple(opaque)
        assertDifferent(androidOpaque, appleOpaque)
        assertContentEquals(opaque, appleOpaque.appleAttestationObjectCbor())
        assertChainEquals(listOf(opaque, opaque), assertNotNull(androidOpaque.androidCertificateChainDer()))
    }

    @Test
    fun `cross role material replay and wrong integer byte order are rejected`() {
        val chain = fixtureChain()
        val apple = fixtureBytes("apple-guide-original.cbor")
        rejectPayload("Android material relabelled Apple", payload(1, androidMaterial(chain)))
        rejectPayload("Apple material relabelled Android", payload(0, vector(apple)))
        rejectPayload("unknown role", payload(2, vector(apple)))
        rejectPayload("big endian version", payload(1, vector(apple), byteArrayOf(0, 1)))
        rejectPayload("big endian Apple role", field(uint(1, 16)) + field(byteArrayOf(0, 0, 0, 1) + field(vector(apple))))
        rejectPayload("big endian vector count", payload(1, uint(apple.size.toLong(), 64).reversedArray() + apple))
        rejectPayload("big endian chain count", payload(0, uint(2, 64).reversedArray() + androidMaterial(chain).copyOfRange(8, androidMaterial(chain).size)))
    }

    @Test
    fun `header archive tails truncation checksum and excessive lengths are rejected`() {
        for (name in listOf("android-mock-osp-unit.hex", "apple-guide-unit.hex")) {
            val golden = fixtureHex(name)
            for (invalid in listOf(ByteArray(0), golden.copyOf(39), golden.copyOf(golden.size - 1),
                golden + byteArrayOf(0), ByteArray(128 * 1024 + 1))) reject(name, invalid)
            for ((label, offset, replacement) in listOf(
                Triple("magic", 0, 0), Triple("major", 4, 1), Triple("minor", 5, 1),
                Triple("schema", 6, golden[6].toInt() xor 1), Triple("compression", 22, 1),
                Triple("fixed lengths", 39, 0), Triple("unsupported flags", 39, 3),
            )) reject(label, golden.copyOf().also { it[offset] = replacement.toByte() })
            reject("checksum", golden.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() })
            reject("huge header length", golden.copyOf().also { uint(-1, 64).copyInto(it, 23) })
            reject("wrong header length", golden.copyOf().also { uint(0, 64).copyInto(it, 23) })
            reject("extra zero alignment padding", golden.copyOfRange(0, 40) + ByteArray(8) + golden.copyOfRange(40, golden.size))
        }
    }

    @Test
    fun `every nested boundary rejects an extra byte with a valid frame checksum`() {
        val chain = fixtureChain()
        val material = androidMaterial(chain)
        rejectPayload("version tail", payload(0, material, uint(1, 16) + byteArrayOf(0)))
        rejectPayload("payload tail", payload(0, material) + byteArrayOf(0))
        rejectPayload("evidence tail", field(uint(1, 16)) + field(uint(0, 32) + field(material) + byteArrayOf(0)))
        rejectPayload("material tail", payload(0, material + byteArrayOf(0)))
        rejectPayload("certificate vector tail", payload(0,
            uint(2, 64) + field(vector(chain[0]) + byteArrayOf(0)) + field(vector(chain[1]))))
        rejectPayload("Apple vector tail", payload(1, vector(fixtureBytes("apple-guide-original.cbor")) + byteArrayOf(0)))
    }

    @Test
    fun `huge counts noncanonical and overflowing varints reject bounded input`() {
        val material = androidMaterial(fixtureChain())
        val evidence = uint(0, 32) + field(material)
        rejectPayload("huge chain count", payload(0, uint(-1, 64)))
        rejectPayload("huge vector count", payload(1, uint(-1, 64)))
        rejectPayload("overlong version field", byteArrayOf(0x82.toByte(), 0, 1, 0) + field(evidence))
        rejectPayload("overlong evidence field", field(uint(1, 16)) + overlongField(evidence))
        rejectPayload("overlong material field", field(uint(1, 16)) + field(uint(0, 32) + overlongField(material)))
        rejectPayload("overlong certificate field", payload(0,
            uint(2, 64) + overlongField(vector(byteArrayOf(1))) + field(vector(byteArrayOf(2)))))
        rejectPayload("overflowing field varint", ByteArray(10) { 0xff.toByte() } + byteArrayOf(1))
        rejectPayload("unterminated field varint", byteArrayOf(0x80.toByte()))
    }

    @Test
    fun `input getter archive and digest buffers cannot change retained originals`() {
        val expected = fixtureChain()
        val input = arrayListOf(expected[0].copyOf(), expected[1].copyOf())
        val android = KagemushaPlatformAttestationOriginalV1.android(input)
        val canonical = android.canonicalBytes()
        val digest = android.canonicalDigest()
        input[0][0] = 0
        input.clear()
        val output = assertNotNull(android.androidCertificateChainDer())
        output[0][0] = 0
        android.canonicalBytes()[0] = 0
        android.canonicalDigest()[0] = 0
        assertChainEquals(expected, assertNotNull(android.androidCertificateChainDer()))
        assertContentEquals(canonical, android.canonicalBytes())
        assertContentEquals(digest, android.canonicalDigest())
        val decodeInput = canonical.copyOf()
        val decoded = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(decodeInput)
        decodeInput.fill(0)
        assertContentEquals(canonical, decoded.canonicalBytes())
        val appleExpected = fixtureBytes("apple-guide-original.cbor")
        val appleInput = appleExpected.copyOf()
        val apple = KagemushaPlatformAttestationOriginalV1.apple(appleInput)
        val appleCanonical = apple.canonicalBytes()
        appleInput.fill(0)
        assertNotNull(apple.appleAttestationObjectCbor()).fill(0)
        apple.canonicalBytes().fill(0)
        assertContentEquals(appleExpected, apple.appleAttestationObjectCbor())
        assertContentEquals(appleCanonical, apple.canonicalBytes())
    }

    @Test
    fun `Android certificate counts two through eight accept small originals`() {
        for (count in 2..8) {
            val chain = List(count) { byteArrayOf((it + 1).toByte()) }
            val value = KagemushaPlatformAttestationOriginalV1.android(chain)
            val decoded = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(value.canonicalBytes())
            assertChainEquals(chain, assertNotNull(decoded.androidCertificateChainDer()))
        }
        for (count in listOf(0, 1, 9)) {
            val chain = List(count) { byteArrayOf(1) }
            assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.android(chain) }
            rejectPayload("chain count $count", payload(0, androidMaterial(chain)))
        }
    }

    @Test
    fun `original length includes sixteen KiB and rejects empty or larger originals`() {
        val maximum = ByteArray(16 * 1024) { (it and 255).toByte() }
        val apple = KagemushaPlatformAttestationOriginalV1.apple(maximum)
        assertContentEquals(maximum, KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(apple.canonicalBytes()).appleAttestationObjectCbor())
        val chain = listOf(maximum, byteArrayOf(1))
        assertChainEquals(chain, assertNotNull(KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(
            KagemushaPlatformAttestationOriginalV1.android(chain).canonicalBytes()).androidCertificateChainDer()))
        for (invalid in listOf(ByteArray(0), ByteArray(16 * 1024 + 1))) {
            assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.apple(invalid) }
            assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.android(listOf(invalid, byteArrayOf(1))) }
            rejectPayload("Apple original size ${invalid.size}", payload(1, vector(invalid)))
            rejectPayload("Android original size ${invalid.size}", payload(0, androidMaterial(listOf(invalid, byteArrayOf(1)))))
        }
    }

    @Test
    fun `complete archive ceiling counts framing for seven and eight maximum originals`() {
        val seven = List(7) { ByteArray(16 * 1024) { 1 } }
        val accepted = KagemushaPlatformAttestationOriginalV1.android(seven).canonicalBytes()
        assertTrue(accepted.size < 128 * 1024)
        assertChainEquals(seven, assertNotNull(KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(accepted).androidCertificateChainDer()))
        val eight = seven + ByteArray(16 * 1024) { 1 }
        assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.android(eight) }
        val excessive = frame(payload(0, androidMaterial(eight)))
        assertTrue(excessive.size > 128 * 1024)
        reject("eight maximum originals plus framing", excessive)
    }

    @Test
    fun `caller collection count drift is refused after bounded reference capture`() {
        val changing = object : AbstractList<ByteArray>() {
            var reads = 0
            override val size: Int get() = if (reads < 2) 2 else 9
            override fun get(index: Int): ByteArray { reads++; return byteArrayOf(1) }
        }
        assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.android(changing) }
        assertEquals(2, changing.reads)
    }

    @Test
    fun `caller collection oversized reference substitution is refused during capture`() {
        val changing = object : AbstractList<ByteArray>() {
            var reads = 0
            override val size: Int get() = 2
            override fun get(index: Int): ByteArray {
                reads++
                return if (reads == 2) ByteArray(16 * 1024 + 1) else byteArrayOf(1)
            }
        }
        assertFailsWith<IllegalArgumentException> { KagemushaPlatformAttestationOriginalV1.android(changing) }
        assertEquals(2, changing.reads)
    }

    private fun reject(label: String, bytes: ByteArray) {
        assertFailsWith<IllegalArgumentException>(label) { KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(bytes) }
    }

    // Shared Norito primitives regenerate checksums; semantic negatives are not checksum failures.
    private fun rejectPayload(label: String, bytes: ByteArray) = reject(label, frame(bytes))
    private fun frame(payload: ByteArray): ByteArray = NoritoHeader(
        SchemaHash.hash16(SCHEMA), payload.size, CRC64.compute(payload),
        NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE,
    ).encode() + payload
    private fun uint(value: Long, bits: Int): ByteArray = NoritoEncoder(NoritoHeader.COMPACT_LEN).also { it.writeUInt(value, bits) }.toByteArray()
    private fun field(bytes: ByteArray): ByteArray = Varint.encode(bytes.size.toLong()) + bytes
    private fun overlongField(bytes: ByteArray): ByteArray {
        val prefix = Varint.encode(bytes.size.toLong())
        return prefix.copyOfRange(0, prefix.lastIndex) + byteArrayOf((prefix.last().toInt() or 0x80).toByte(), 0) + bytes
    }
    private fun vector(bytes: ByteArray): ByteArray = uint(bytes.size.toLong(), 64) + bytes
    private fun androidMaterial(chain: List<ByteArray>): ByteArray =
        chain.fold(uint(chain.size.toLong(), 64)) { bytes, original -> bytes + field(vector(original)) }
    private fun payload(role: Long, material: ByteArray, version: ByteArray = uint(1, 16)): ByteArray =
        field(version) + field(uint(role, 32) + field(material))
    private fun assertChainEquals(expected: List<ByteArray>, actual: List<ByteArray>) {
        assertEquals(expected.size, actual.size)
        expected.indices.forEach { assertContentEquals(expected[it], actual[it], "original $it") }
    }
    private fun assertDifferent(first: KagemushaPlatformAttestationOriginalV1, second: KagemushaPlatformAttestationOriginalV1) {
        assertFalse(first.canonicalBytes().contentEquals(second.canonicalBytes()))
        assertFalse(first.canonicalDigest().contentEquals(second.canonicalDigest()))
    }
    private fun fixtureChain(): List<ByteArray> = listOf(fixtureBytes("mock_osp-0.der"), fixtureBytes("mock_osp-1.der"))
    private fun fixtureHex(name: String): ByteArray = String(fixtureBytes(name), Charsets.UTF_8).trim()
        .also { require(it.length % 2 == 0) }.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun fixtureBytes(name: String): ByteArray {
        var directory = Paths.get("").toAbsolutePath().normalize()
        while (directory != null) {
            val path = directory.resolve("fixtures/kagemusha/platform-original-container-v1/$name")
            if (Files.isRegularFile(path)) return Files.readAllBytes(path)
            directory = directory.parent
        }
        error("missing shared Rust platform-original fixture $name")
    }

    private companion object {
        const val SCHEMA = "iroha_data_model::kagemusha::KagemushaPlatformAttestationOriginalV1"
    }
}
