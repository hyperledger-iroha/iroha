package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Assertions.*

/** Coordinate/hint boundary tests only; no positive native monetary verification is simulated. */
class KagemushaReserveFinalityV1Test {
    private val network = ByteArray(32) { 3 }
    private val context = ByteArray(32) { 5 }
    private fun checkpoint(): ByteArray = generateSequence(java.io.File(System.getProperty("user.dir"))) { it.parentFile }
        .map { java.io.File(it, "fixtures/sumeragi/native-finality/height-2-checkpoint.nrt") }
        .first { it.isFile }.readBytes()
    private fun hint(height: String = "7"): String = """{"version":1,"network_id":"${"03".repeat(32)}","block_height":"$height","block_hash":"${"05".repeat(32)}"}"""

    @Test fun anchorDefensivelyCopiesInputAndOutput() {
        val bytes = checkpoint(); val original = bytes.copyOf()
        val anchor = KagemushaFinalityTrustAnchorV1(network, bytes)
        network[0] = 9; bytes[0] = 9
        val outNetwork = anchor.networkId(); val outCheckpoint = anchor.checkpoint()
        outNetwork[0] = 8; outCheckpoint[0] = 8
        assertEquals(3, anchor.networkId()[0].toInt()); assertArrayEquals(original, anchor.checkpoint())
    }
    @Test fun anchorPreservesTheCompleteCanonicalCheckpoint() {
        val bytes = checkpoint()
        val anchor = KagemushaFinalityTrustAnchorV1(network, bytes)
        assertArrayEquals(bytes, anchor.checkpoint())
        assertTrue(bytes.size > 32)
    }
    @Test fun anchorRejectsEmptyAndOversizedCheckpoints() {
        for (bytes in listOf(byteArrayOf(), ByteArray(KagemushaFinalityTrustAnchorV1.MAXIMUM_CHECKPOINT_BYTES + 1))) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(network, bytes) }
        }
    }
    @Test fun anchorRejectsZeroAndWrongWidthNetworkHashes() {
        for (hash in listOf(byteArrayOf(), ByteArray(31) { 1 }, ByteArray(32), ByteArray(33) { 1 })) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(hash, checkpoint()) }
        }
    }
    @Test fun pendingHintIsAbsent() { assertNull(parseReserveFinalityHint("null".toByteArray())) }
    @Test fun anchorRejectsUnmarkedNetworksWithoutNormalizingThem() {
        assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(ByteArray(32) { 4 }, checkpoint()) }
    }
    @Test fun hintRejectsUnmarkedCoordinatesWithoutNormalizingThem() {
        for (original in listOf("03", "05")) {
            val json = hint().replace(original.repeat(32), "04".repeat(32))
            assertThrows(Exception::class.java) { parseReserveFinalityHint(json.toByteArray()) }
        }
    }
    @Test fun nativeHintRetainsExactCoordinatesWithoutGrantingTrust() {
        val value = assertNotNullHint(parseReserveFinalityHint(hint("18446744073709551615").toByteArray()))
        assertArrayEquals(network, value.networkId()); assertArrayEquals(context, value.blockHash())
        assertEquals(BigInteger("18446744073709551615"), value.blockHeight)
        value.networkId()[0] = 0; value.blockHash()[0] = 0
        assertEquals(3, value.networkId()[0].toInt()); assertEquals(5, value.blockHash()[0].toInt())
    }
    @Test fun hintRejectsNoncanonicalAndOverflowHeights() {
        for (height in listOf("0", "-1", "+7", "07", "7.0", "18446744073709551616", "٧")) {
            assertThrows(Exception::class.java) { parseReserveFinalityHint(hint(height).toByteArray()) }
        }
    }
    @Test fun hintRejectsUnknownAndDuplicateFieldsAndInvalidVersion() {
        val original = hint()
        val variants = listOf(original.replace("\"version\":1", "\"version\":true"),
            original.replace("\"version\":1", "\"version\":1.0"), original.replace("\"version\":1", "\"version\":2"),
            original.replace("\"version\":1", "\"version\":1,\"version\":1"), original.dropLast(1) + ",\"extra\":1}",
            original + "null")
        for (value in variants) assertThrows(Exception::class.java) { parseReserveFinalityHint(value.toByteArray()) }
    }
    @Test fun hintRejectsZeroUppercaseAndWrongWidthHashes() {
        for (hash in listOf("00".repeat(32), "AB".repeat(32), "03".repeat(31), "03".repeat(33))) {
            val value = hint().replace("03".repeat(32), hash)
            assertThrows(Exception::class.java) { parseReserveFinalityHint(value.toByteArray()) }
        }
    }
    @Test fun hintRejectsInvalidUtf8AndOversizedProjections() {
        for (bytes in listOf(byteArrayOf(), byteArrayOf(0xc3.toByte(), 0x28), ByteArray(513) { 32 })) {
            assertThrows(Exception::class.java) { parseReserveFinalityHint(bytes) }
        }
    }
    private fun assertNotNullHint(value: KagemushaUntrustedFinalityHintV1?): KagemushaUntrustedFinalityHintV1 {
        assertNotNull(value); return checkNotNull(value)
    }
}
