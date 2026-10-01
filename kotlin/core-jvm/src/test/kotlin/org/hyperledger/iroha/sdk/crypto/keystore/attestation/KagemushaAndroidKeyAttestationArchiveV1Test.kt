package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Public inert DER shape fixtures, never attestation, trust-root or app-key qualification. */
class KagemushaAndroidKeyAttestationArchiveV1Test {
    private val leaf = byteArrayOf(0x30, 3, 2, 1, 1)
    private val root = byteArrayOf(0x30, 0)
    private fun archive() = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(listOf(leaf, root)).transportBytes()

    @Test fun originalArchivePreservesExactVersionCountLengthsAndLeafFirstBytes() {
        val bytes = archive()
        assertArrayEquals(byteArrayOf(0x4b, 0x4d, 0x43, 0x41, 1, 2,
            0, 0, 0, 5, 0x30, 3, 2, 1, 1, 0, 0, 0, 2, 0x30, 0), bytes)
        val parsed = KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(bytes)
        assertArrayEquals(bytes, parsed.transportBytes())
        assertArrayEquals(leaf, parsed.certificateChainDer()[0]); assertArrayEquals(root, parsed.certificateChainDer()[1])
    }

    @Test fun truncationTrailingBytesForeignVersionCountAndUnsignedLengthsAreRejected() {
        val bytes = archive()
        for (bad in listOf(bytes.copyOfRange(0, bytes.lastIndex), bytes + 0,
            bytes.clone().also { it[5] = 1 }, bytes.clone().also { it[5] = 9 },
            bytes.clone().also { it[9] = 6 }, bytes.clone().also { it[4] = 2 },
            bytes.clone().also { it[6] = 0xff.toByte() }, bytes.clone().also { it[9] = 0 })) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(bad) }
        }
    }

    @Test fun certificatesRequireSingleMinimalBoundedDerSequence() {
        val malformed = listOf(leaf + 0, leaf.copyOfRange(0, 4), byteArrayOf(0x31, 0),
            byteArrayOf(0x30, 0x80.toByte(), 0, 0), byteArrayOf(0x30, 0x81.toByte(), 0),
            byteArrayOf(0x30, 0x82.toByte(), 0, 1, 0), byteArrayOf(0x30, 0x81.toByte(), 1, 0),
            ByteArray(16 * 1024 + 1))
        for (bad in malformed) assertThrows(IllegalArgumentException::class.java) {
            KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(listOf(bad, root))
        }
        for (count in listOf(0, 1, 9)) assertThrows(IllegalArgumentException::class.java) {
            KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(List(count) { root })
        }
    }

    @Test fun aggregateArchiveBoundIsAppliedInAdditionToIndividualCertificateBounds() {
        val der = ByteArray(16 * 1024).also { it[0] = 0x30; it[1] = 0x82.toByte(); it[2] = 0x3f; it[3] = 0xfc.toByte() }
        val original = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(List(7) { der })
        assertEquals(7, KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(original.transportBytes()).certificateChainDer().size)
        assertThrows(IllegalArgumentException::class.java) { KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(List(8) { der }) }
        assertThrows(IllegalArgumentException::class.java) { KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(ByteArray(128 * 1024 + 1)) }
    }

    @Test fun inputsAndEveryProjectionAreCopied() {
        val source = arrayListOf(leaf.copyOf(), root.copyOf())
        val original = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(source)
        source[0].fill(0); source.clear()
        original.certificateChainDer()[0].fill(0); original.transportBytes().fill(0)
        assertArrayEquals(leaf, original.certificateChainDer()[0]); assertArrayEquals(archive(), original.transportBytes())
        val bytes = archive(); val parsed = KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(bytes); bytes.fill(0)
        assertArrayEquals(archive(), parsed.transportBytes())
    }
}
