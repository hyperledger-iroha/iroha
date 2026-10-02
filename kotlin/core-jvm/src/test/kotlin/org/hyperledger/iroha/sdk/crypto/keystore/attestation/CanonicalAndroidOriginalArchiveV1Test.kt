package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.nio.file.Files
import java.nio.file.Paths
import org.hyperledger.iroha.sdk.offline.KagemushaPlatformAttestationOriginalV1
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Five retained archive invariants migrated to the sole canonical Norito original. DATA only. */
class CanonicalAndroidOriginalArchiveV1Test {
    private val leaf = byteArrayOf(0x30, 3, 2, 1, 1)
    private val root = byteArrayOf(0x30, 0)
    private fun archive() = KagemushaPlatformAttestationOriginalV1.android(listOf(leaf, root)).canonicalBytes()

    @Test fun originalArchivePreservesExactNativeGoldenAndLeafFirstBytes() {
        // Full wire equality uses the actual Rust golden, not an SDK-generated expectation.
        val base = Paths.get("../../fixtures/kagemusha/platform-original-container-v1")
        val chain = listOf(Files.readAllBytes(base.resolve("mock_osp-0.der")), Files.readAllBytes(base.resolve("mock_osp-1.der")))
        val golden = String(Files.readAllBytes(base.resolve("android-mock-osp-unit.hex")), Charsets.US_ASCII)
            .trim().chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        assertArrayEquals(golden, KagemushaPlatformAttestationOriginalV1.android(chain).canonicalBytes())
        val parsed = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(archive())
        assertArrayEquals(archive(), parsed.canonicalBytes())
        assertArrayEquals(leaf, parsed.androidCertificateChainDer()!![0]); assertArrayEquals(root, parsed.androidCertificateChainDer()!![1])
    }

    @Test fun truncationTrailingBytesForeignVersionCountAndUnsignedLengthsAreRejected() {
        val bytes = archive()
        val wrongMaterials = listOf(uint(1, 64), uint(9, 64),
            uint(2, 64) + field(uint(6, 64) + leaf) + field(vector(root)),
            uint(2, 64) + field(uint(-1, 64) + leaf) + field(vector(root)),
            uint(2, 64) + field(uint(0, 64)) + field(vector(root)))
        val bad = listOf(bytes.copyOfRange(0, bytes.lastIndex), bytes + 0,
            frame(uint(2, 16), material(listOf(leaf, root)))) + wrongMaterials.map { frame(uint(1, 16), it) }
        assertEquals(8, bad.size)
        for (value in bad) assertThrows(IllegalArgumentException::class.java) {
            KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(value)
        }
    }

    @Test fun componentsAndCertificateCountsRetainTheirIndependentBounds() {
        // DER validity is checked by the real verifier, with its eight original negatives retained.
        for (bad in listOf(byteArrayOf(), ByteArray(16 * 1024 + 1))) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaPlatformAttestationOriginalV1.android(listOf(bad, root)) }
        }
        for (count in listOf(0, 1, 9)) assertThrows(IllegalArgumentException::class.java) {
            KagemushaPlatformAttestationOriginalV1.android(List(count) { root })
        }
    }

    @Test fun aggregateArchiveBoundIsAppliedInAdditionToIndividualCertificateBounds() {
        val der = ByteArray(16 * 1024).also { it[0] = 0x30; it[1] = 0x82.toByte(); it[2] = 0x3f; it[3] = 0xfc.toByte() }
        val original = KagemushaPlatformAttestationOriginalV1.android(List(7) { der })
        assertEquals(7, KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(original.canonicalBytes()).androidCertificateChainDer()!!.size)
        assertThrows(IllegalArgumentException::class.java) { KagemushaPlatformAttestationOriginalV1.android(List(8) { der }) }
        assertThrows(IllegalArgumentException::class.java) { KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(ByteArray(128 * 1024 + 1)) }
    }

    @Test fun inputsAndEveryProjectionAreCopied() {
        val source = arrayListOf(leaf.copyOf(), root.copyOf())
        val original = KagemushaPlatformAttestationOriginalV1.android(source)
        source[0].fill(0); source.clear()
        original.androidCertificateChainDer()!![0].fill(0); original.canonicalBytes().fill(0)
        assertArrayEquals(leaf, original.androidCertificateChainDer()!![0]); assertArrayEquals(archive(), original.canonicalBytes())
        val bytes = archive(); val parsed = KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(bytes); bytes.fill(0)
        assertArrayEquals(archive(), parsed.canonicalBytes())
    }

    private fun uint(value: Long, bits: Int) = NoritoEncoder(NoritoHeader.COMPACT_LEN).also { it.writeUInt(value, bits) }.toByteArray()
    private fun field(bytes: ByteArray) = Varint.encode(bytes.size.toLong()) + bytes
    private fun vector(bytes: ByteArray) = uint(bytes.size.toLong(), 64) + bytes
    private fun material(chain: List<ByteArray>) = uint(chain.size.toLong(), 64) + chain.fold(byteArrayOf()) { out, cert -> out + field(vector(cert)) }
    private fun frame(version: ByteArray, material: ByteArray): ByteArray {
        val payload = field(version) + field(uint(0, 32) + field(material))
        return NoritoHeader(SchemaHash.hash16("iroha_data_model::kagemusha::KagemushaPlatformAttestationOriginalV1"), payload.size,
            CRC64.compute(payload), NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE).encode() + payload
    }
}
