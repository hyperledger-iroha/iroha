// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.MethodSource

class KagemushaOrdinaryCashApprovalProjectionV1Test {
    @ParameterizedTest
    @MethodSource("cashCases")
    fun `Rust terminal model originals preserve full signing digests`(operation: String, before: String) {
        val vectors = fixtures()
        val s = vectors.getValue("s_${operation}_$before")
        val w = vectors.getValue("w_${operation}_$before")
        val projection = KagemushaOrdinaryCashApprovalProjectionV1.requireModelMessageShape(
            KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, w, s)
        assertContentEquals(vectors.getValue("s_${operation}_${before}_sha256"), projection.subjectSigningDigest())
        assertContentEquals(vectors.getValue("w_${operation}_${before}_sha256"), projection.approvalSigningDigest())
        assertEquals(BigInteger(before), projection.logicalIndexBefore())
        assertEquals(BigInteger(before).add(BigInteger.ONE), projection.logicalIndexAfter())
        // Generic Rust message fixtures are not issued ordinary credentials. Their independent
        // enrollment and S credential markers deliberately differ; ordinary correlation rejects.
        assertFailsWith<IllegalArgumentException> {
            KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(w, s, binding(w, s))
        }
    }

    @ParameterizedTest
    @MethodSource("cashCases")
    fun `preparation and terminal remain distinct for every cash operation`(operation: String, before: String) {
        val terminal = specimen(operation, before, false)
        val preparation = specimen(operation, before, true)
        val terminalProjection = project(terminal, false)
        val preparationProjection = project(preparation, true)
        assertEquals(KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, terminalProjection.purpose)
        assertEquals(KagemushaOrdinaryCashApprovalPurposeV1.PREPARE_TRANSITION, preparationProjection.purpose)
        assertEquals(terminalProjection.operationTag(), preparationProjection.operationTag())
        assertContentEquals(terminalProjection.operationId(), preparationProjection.operationId())
        assertContentEquals(terminal.s.copyOfRange(332, 364), preparationProjection.transitionStatementDigest())
        assertFailsWith<IllegalArgumentException> { project(terminal, true) }
        assertFailsWith<IllegalArgumentException> { project(preparation, false) }
        assertNotEquals(terminalProjection.approvalSigningDigest().toList(), preparationProjection.approvalSigningDigest().toList())
        for (offset in listOf(364, 396)) {
            val changed = preparation.s.copyOf().also { it[offset] = 1 }
            rejectCoherent(preparation, changed, true)
            val changedTerminal = terminal.s.copyOf().also {
                if (operation != "rotate") it.fill(0, offset, offset + 32)
                else it[offset] = 1
            }
            rejectCoherent(terminal, changedTerminal, false)
        }
    }

    @Test
    fun `incoming model terminal fixtures retain separate generic grammar`() {
        for (operation in listOf("mint_fold", "receive_fold")) {
            for (before in listOf("9", BigInteger.ONE.shiftLeft(128).subtract(BigInteger.valueOf(2)).toString())) {
                val original = specimen(operation, before, false)
                assertEquals(BigInteger(before), project(original, false).logicalIndexBefore())
                assertFailsWith<IllegalArgumentException> {
                    KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(original.w, original.s, binding(original.w, original.s))
                }
                for (offset in listOf(364, 396)) {
                    val changed = original.s.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }
                    assertFailsWith<IllegalArgumentException> {
                        KagemushaOrdinaryCashApprovalProjectionV1.requireModelMessageShape(
                            KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, original.w, changed)
                    }
                    rejectCoherent(original, original.s.copyOf().also { it.fill(0, offset, offset + 32) }, false)
                }
            }
        }
    }

    @Test
    fun `canonical domains widths versions identity epochs and exact-next indices reject coherent mutations`() {
        for (preparation in listOf(false, true)) {
            val original = specimen("send_split", "9", preparation)
            for (offset in listOf(0, 42, 50, 51, 52)) {
                assertFailsWith<IllegalArgumentException> {
                    project(Specimen(original.w.copyOf().also { it[offset] = (it[offset].toInt() xor 0x40).toByte() }, original.s), preparation)
                }
            }
            repeat(8) { field ->
                assertFailsWith<IllegalArgumentException> {
                    project(Specimen(original.w.copyOf().also { it.fill(0, 53 + field * 32, 85 + field * 32) }, original.s), preparation)
                }
            }
            for (offset in listOf(0, 49, 57, 58, 331)) {
                rejectCoherent(original, original.s.copyOf().also { it[offset] = (it[offset].toInt() xor 0x40).toByte() }, preparation)
            }
            for (offset in listOf(59, 91, 123, 155, 187, 219, 251, 291, 332, 283, 323)) {
                rejectCoherent(original, original.s.copyOf().also {
                    it.fill(0, offset, offset + if (offset in listOf(283, 323)) 8 else 32)
                }, preparation)
            }
            for (offset in listOf(428, 444)) {
                rejectCoherent(original, original.s.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }, preparation)
            }
            rejectCoherent(original, original.s.copyOf().also { it.fill(0xff.toByte(), 428, 444); it.fill(0, 444, 460) }, preparation)
            rejectCoherent(original, original.s.copyOf().also { it[331] = 0; it.fill(0, 364, 460) }, preparation)
            for (w in listOf(original.w + byteArrayOf(0), original.w.copyOf(324))) {
                assertFailsWith<IllegalArgumentException> { project(Specimen(w, original.s), preparation) }
            }
            for (s in listOf(original.s + byteArrayOf(0), original.s.copyOf(459))) {
                assertFailsWith<IllegalArgumentException> {
                    KagemushaOrdinaryCashApprovalProjectionV1.requireModelMessageShape(
                        if (preparation) KagemushaOrdinaryCashApprovalPurposeV1.PREPARE_TRANSITION
                        else KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, original.w, s)
                }
            }
        }
    }

    @Test
    fun `original interval uses unsigned u64 and does not consult the platform clock`() {
        val original = specimen("rotate", "9", true)
        fun withTime(issued: BigInteger, expires: BigInteger): Specimen = Specimen(original.w.copyOf().also {
            writeUnsigned(it, 309, 8, issued); writeUnsigned(it, 317, 8, expires)
        }, original.s)
        val maximum = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        project(withTime(maximum.subtract(BigInteger.valueOf(120_000)), maximum), true)
        project(withTime(BigInteger.ONE, BigInteger.valueOf(120_001)), true)
        for ((issued, expires) in listOf(0L to 1L, 1L to 1L, 2L to 1L, 1L to 120_002L)) {
            assertFailsWith<IllegalArgumentException> { project(withTime(BigInteger.valueOf(issued), BigInteger.valueOf(expires)), true) }
        }
    }

    @Test
    fun `ordinary original fields full selection and credential binding cannot be substituted`() {
        val original = specimen("mint_fold", "9", true)
        val retained = binding(original.w, original.s)
        for (offset in listOf(53, 117, 149, 181, 213, 277)) {
            val changed = original.w.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }
            assertFailsWith<IllegalArgumentException> {
                KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(changed, original.s, retained)
            }
        }
        val otherSelection = original.s.copyOf().also { it[59] = (it[59].toInt() xor 1).toByte() }
        val coherentW = original.w.copyOf().also { sha(otherSelection).copyInto(it, 245) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(coherentW, otherSelection, retained)
        }
        val wrongCredential = original.s.copyOf().also { it[155] = (it[155].toInt() xor 1).toByte() }
        val credentialW = original.w.copyOf().also { sha(wrongCredential).copyInto(it, 245) }
        assertFailsWith<IllegalArgumentException> { project(Specimen(credentialW, wrongCredential), true) }
        for (digest in listOf(ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaOrdinaryCashApprovalOriginalBindingV1(digest, ByteArray(32) { 1 }, ByteArray(32) { 2 },
                    ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 }, original.s)
            }
        }
        assertFailsWith<IllegalArgumentException> { binding(original.w, original.s.copyOf(459)) }
    }

    @Test
    fun `projection and retained public originals defensively copy every array`() {
        val original = specimen("redeem_split", "9", false)
        val fields = listOf(53, 117, 149, 181, 213, 277).map { original.w.copyOfRange(it, it + 32) }
        val retainedS = original.s.copyOf()
        val retained = KagemushaOrdinaryCashApprovalOriginalBindingV1(fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], retainedS)
        fields.forEach { it.fill(0) }; retainedS.fill(0)
        val expectedW = original.w.copyOf(); val expectedS = original.s.copyOf()
        val projection = KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(original.w, original.s, retained)
        original.w.fill(0); original.s.fill(0)
        for (bytes in listOf(projection.signingBytes(), projection.selectionBytes(), projection.operationId(),
            projection.subjectSigningDigest(), projection.approvalSigningDigest(), projection.transitionStatementDigest())) bytes.fill(0)
        assertContentEquals(expectedW, projection.signingBytes())
        assertContentEquals(expectedS, projection.selectionBytes())
        assertContentEquals(expectedW.copyOfRange(53, 85), projection.operationId())
        assertContentEquals(sha(expectedS), projection.subjectSigningDigest())
        assertContentEquals(sha(expectedW), projection.approvalSigningDigest())
        assertContentEquals(expectedS.copyOfRange(332, 364), projection.transitionStatementDigest())
    }

    private class Specimen(val w: ByteArray, val s: ByteArray)

    private fun specimen(operation: String, before: String, preparation: Boolean): Specimen {
        val vectors = fixtures()
        val s = vectors.getValue("s_${operation}_$before").copyOf()
        val w = vectors.getValue("w_${operation}_$before").copyOf()
        // Inert shape specimen: align independent fixture credential markers. No issuer,
        // Native owner, proof, platform signature or admitted credential is created here.
        w.copyInto(s, 155, 213, 245)
        if (preparation) { s.fill(0, 364, 428); w[52] = 2 }
        sha(s).copyInto(w, 245)
        return Specimen(w, s)
    }

    private fun binding(w: ByteArray, s: ByteArray) = KagemushaOrdinaryCashApprovalOriginalBindingV1(
        w.copyOfRange(53, 85), w.copyOfRange(117, 149), w.copyOfRange(149, 181), w.copyOfRange(181, 213),
        w.copyOfRange(213, 245), w.copyOfRange(277, 309), s)

    private fun project(original: Specimen, preparation: Boolean): KagemushaOrdinaryCashApprovalProjectionV1 {
        val binding = binding(original.w, original.s)
        return if (preparation) KagemushaOrdinaryCashApprovalProjectionV1.requirePreparation(original.w, original.s, binding)
        else if (original.s[331].toInt() == 1 || original.s[331].toInt() == 3)
            KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingTerminal(original.w, original.s, binding)
        else KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(original.w, original.s, binding)
    }

    private fun rejectCoherent(original: Specimen, s: ByteArray, preparation: Boolean) {
        val w = original.w.copyOf().also { sha(s).copyInto(it, 245) }
        assertFailsWith<IllegalArgumentException> { project(Specimen(w, s), preparation) }
    }

    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun writeUnsigned(bytes: ByteArray, offset: Int, width: Int, value: BigInteger) {
        require(value.signum() >= 0 && value.bitLength() <= width * 8)
        val little = value.toByteArray().reversedArray()
        bytes.fill(0, offset, offset + width)
        little.copyInto(bytes, offset, 0, minOf(width, little.size))
    }

    private fun fixtures(): Map<String, ByteArray> {
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

    companion object {
        @JvmStatic
        fun cashCases(): List<Array<String>> = listOf("mint_fold", "send_split", "receive_fold", "redeem_split", "rotate")
            .flatMap { operation -> listOf("9", BigInteger.ONE.shiftLeft(128).subtract(BigInteger.valueOf(2)).toString())
                .map { before -> arrayOf(operation, before) } }
    }
}
