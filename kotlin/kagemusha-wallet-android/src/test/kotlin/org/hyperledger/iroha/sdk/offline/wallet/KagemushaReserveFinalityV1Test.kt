package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.Assertions.*

/** Coordinate/hint boundary tests only; no positive native monetary verification is simulated. */
class KagemushaReserveFinalityV1Test {
    private val network = ByteArray(32) { 3 }
    private val context = ByteArray(32) { 5 }
    private fun hint(height: String = "7"): String = """{"version":1,"network_id":"${"03".repeat(32)}","block_height":"$height","height_context_id":"${"05".repeat(32)}"}"""

    @Test fun anchorDefensivelyCopiesInputAndOutput() {
        val anchor = KagemushaFinalityTrustAnchorV1(network, BigInteger.valueOf(7), context)
        network[0] = 9; context[0] = 9
        val outNetwork = anchor.networkId(); val outContext = anchor.heightContextId()
        outNetwork[0] = 8; outContext[0] = 8
        assertEquals(3, anchor.networkId()[0].toInt()); assertEquals(5, anchor.heightContextId()[0].toInt())
    }
    @Test fun anchorPreservesTheFullUnsigned64BitHeight() {
        val max = BigInteger("18446744073709551615")
        val anchor = KagemushaFinalityTrustAnchorV1(network, max, context)
        assertEquals(max, anchor.blockHeight)
        assertEquals(-1L, anchor.blockHeight.toLong()) // Exact unsigned JNI bit pattern.
    }
    @Test fun anchorRejectsReservedAndOverflowHeights() {
        for (height in listOf(BigInteger.ZERO, BigInteger.valueOf(-1), BigInteger.ONE.shiftLeft(64))) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(network, height, context) }
        }
    }
    @Test fun anchorRejectsZeroAndWrongWidthHashes() {
        for (hash in listOf(byteArrayOf(), ByteArray(31) { 1 }, ByteArray(32), ByteArray(33) { 1 })) {
            assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(hash, BigInteger.ONE, context) }
            assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(network, BigInteger.ONE, hash) }
        }
    }
    @Test fun pendingHintIsAbsent() { assertNull(parseReserveFinalityHint("null".toByteArray())) }
    @Test fun anchorRejectsUnmarkedCoordinatesWithoutNormalizingThem() {
        val unmarked = ByteArray(32) { 4 }
        assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(unmarked, BigInteger.ONE, context) }
        assertThrows(IllegalArgumentException::class.java) { KagemushaFinalityTrustAnchorV1(network, BigInteger.ONE, unmarked) }
    }
    @Test fun hintRejectsUnmarkedCoordinatesWithoutNormalizingThem() {
        for (original in listOf("03", "05")) {
            val json = hint().replace(original.repeat(32), "04".repeat(32))
            assertThrows(Exception::class.java) { parseReserveFinalityHint(json.toByteArray()) }
        }
    }
    @Test fun nativeHintRetainsExactCoordinatesWithoutGrantingTrust() {
        val value = assertNotNullHint(parseReserveFinalityHint(hint("18446744073709551615").toByteArray()))
        assertArrayEquals(network, value.networkId()); assertArrayEquals(context, value.heightContextId())
        assertEquals(BigInteger("18446744073709551615"), value.blockHeight)
        value.networkId()[0] = 0; value.heightContextId()[0] = 0
        assertEquals(3, value.networkId()[0].toInt()); assertEquals(5, value.heightContextId()[0].toInt())
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
