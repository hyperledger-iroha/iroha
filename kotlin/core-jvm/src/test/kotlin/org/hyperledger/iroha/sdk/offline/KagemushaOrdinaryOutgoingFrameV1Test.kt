// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Shape-only TESTDATA. No specimen installs Native, hardware, a signed FI or a cash grant. */
class KagemushaOrdinaryOutgoingFrameV1Test {
    @Test fun closedPhasesAndRoleBoundsRefusePrepareAndCallerSelections() {
        for (phase in listOf(1,2,3,4,19,255)) assertFails { KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, emptyList()) }
        for (phase in listOf(5,6,9,11,12,13,16,18)) {
            KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, emptyList())
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, listOf(ByteArray(32) { 1 })) }
        }
        for (phase in listOf(8,14,15,17)) {
            KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, listOf(ByteArray(32) { 1 }))
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireRequest(phase, listOf(ByteArray(32))) }
        }
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireRequest(10, listOf(ByteArray(4097))) }
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireRequest(7, listOf(byteArrayOf(1), byteArrayOf(2))) }
    }
    @Test fun purposeOneTerminalOriginalHasExactCKeyAndWSubjectCorrelation() {
        val fields = specimen()
        KagemushaOrdinaryOutgoingFrameV1.requireTerminal(fields)
        KagemushaOrdinaryOutgoingFrameV1.requireResponse(8, fields)
        KagemushaOrdinaryOutgoingFrameV1.requireResponse(18, fields)
        for (index in listOf(0,1,3,4,5,6,7,8,9,11)) {
            val changed = fields.map(ByteArray::copyOf).toMutableList()
            changed[index][0] = (changed[index][0].toInt() xor 1).toByte()
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireTerminal(changed) }
        }
        val preparation = fields.map(ByteArray::copyOf)
        preparation[1][52] = 2
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireTerminal(preparation) }
    }
    @Test fun hardwareMaskAndTerminalCommitmentsCannotBeErasedByCoherentRehash() {
        val fields = specimen().map(ByteArray::copyOf)
        fields[11][0] = 3 // Caller-choice TEE/StrongBox is not the actual C-selected security level.
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireTerminal(fields) }
        for (offset in listOf(364,396)) {
            val changed = specimen().map(ByteArray::copyOf)
            changed[9].fill(0,offset,offset+32)
            sha(changed[9]).copyInto(changed[1],245)
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireTerminal(changed) }
        }
    }
    @Test fun completedEvidenceAndDistinctAckCannotUsePartialReadyShapes() {
        for (phase in listOf(9,11)) {
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireResponse(phase, listOf(byteArrayOf(1), ByteArray(0), ByteArray(0))) }
            assertFails { KagemushaOrdinaryOutgoingFrameV1.requireResponse(phase, listOf(byteArrayOf(0), byteArrayOf(1), byteArrayOf(2))) }
        }
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireResponse(15,listOf(byteArrayOf(1))) }
        assertFails { KagemushaOrdinaryOutgoingFrameV1.requireResponse(17,listOf(ByteArray(0))) }
        for (phase in listOf(6,13)) assertFails { KagemushaOrdinaryOutgoingFrameV1.requireResponse(phase,emptyList()) }
    }
    private fun specimen(): List<ByteArray> {
        val path = sequenceOf("fixtures","../fixtures","../../fixtures").map {
            Paths.get(it,"offline/kagemusha_app_platform_messages_v1.tsv") }.first { Files.isRegularFile(it) }
        val lines = Files.readAllLines(path)
        fun vector(name: String) = lines.first { it.startsWith("$name\t") }.substringAfter('\t')
            .chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        val w=vector("w_send_split_9"); val s=vector("s_send_split_9")
        val point=byteArrayOf(4)+ByteArray(64) { 7 }; val key=sha(point)
        key.copyInto(w,181); w.copyOfRange(213,245).copyInto(s,155); sha(s).copyInto(w,245)
        val body=ByteArray(451);body[0]=1;body[2]=1
        repeat(13) { i -> ByteArray(32) { (i+40).toByte() }.copyInto(body,3+i*32) }
        fun field(i: Int, bytes: ByteArray) = bytes.copyInto(body,3+i*32)
        field(3,w.copyOfRange(117,149));field(10,w.copyOfRange(149,181))
        field(4,s.copyOfRange(187,219));field(5,s.copyOfRange(219,251))
        field(6,s.copyOfRange(59,91));field(7,s.copyOfRange(251,283))
        s.copyOfRange(283,291).copyInto(body,419);s.copyOfRange(323,331).copyInto(body,427)
        ByteBuffer.wrap(body).order(ByteOrder.LITTLE_ENDIAN).putLong(435,1000).putLong(443,2000)
        val domain="iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
        val c=domain+ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(451).array()+body
        return listOf(w.copyOfRange(53,85),w,byteArrayOf(5),KagemushaOrdinaryAppKeyAliasV1.originalAlias(c).toByteArray(Charsets.UTF_8),
            sha(c),point,key,c,w.copyOfRange(213,245),s,ByteArray(0),byteArrayOf(1),ByteArray(32) { 1 },byteArrayOf(1))
    }
    private fun sha(raw: ByteArray)=MessageDigest.getInstance("SHA-256").digest(raw)
}
