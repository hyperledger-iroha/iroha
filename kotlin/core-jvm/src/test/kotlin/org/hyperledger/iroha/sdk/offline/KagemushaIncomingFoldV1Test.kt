// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

/** Shape-only inputs never supply a native proof, hardware class or monetary authority. */
class KagemushaIncomingFoldV1Test {
    @Test fun `installed policy projection preserves distinct digest and registry root defensively`() {
        val release = digest(41)
        val policyDigest = digest(42)
        val root = digest(43)
        val policy = KagemushaAuthenticatedHardwarePolicyV1(release, policyDigest, root)
        release.fill(0); policyDigest.fill(0); root.fill(0)
        assertContentEquals(digest(41), policy.releaseId())
        assertContentEquals(digest(42), policy.hardwarePolicyDigest())
        assertContentEquals(digest(43), policy.providerPolicyRoot())
        policy.releaseId().fill(0); policy.hardwarePolicyDigest().fill(0); policy.providerPolicyRoot().fill(0)
        assertContentEquals(digest(43), policy.providerPolicyRoot())
        for (invalid in listOf(ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> { KagemushaAuthenticatedHardwarePolicyV1(invalid, digest(42), digest(43)) }
            assertFailsWith<IllegalArgumentException> { KagemushaAuthenticatedHardwarePolicyV1(digest(41), invalid, digest(43)) }
            assertFailsWith<IllegalArgumentException> { KagemushaAuthenticatedHardwarePolicyV1(digest(41), digest(42), invalid) }
        }
    }

    @Test fun `method18 is a closed empty request and exact three nonzero digest response`() {
        val method = KagemushaCoreCoordinatorMethodV1.AUTHENTICATED_HARDWARE_POLICY
        assertEquals(18, method.code)
        assertEquals(21, KagemushaCoreCoordinatorMethodV1.entries.size)
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, emptyList())
        val response = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request,
            listOf(digest(41), digest(42), digest(43)))
        assertEquals(3, KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, response).size)
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(digest(1))) }
        for (fields in listOf(emptyList(), listOf(digest(41), digest(42)),
            listOf(digest(41), digest(42), digest(43), digest(44)), listOf(digest(41), digest(42), ByteArray(32)))) {
            assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, fields) }
        }
        assertFailsWith<IllegalArgumentException> { KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, response + byteArrayOf(0)) }
    }

    @Test fun `native work and physical originals defensively retain every mutable field`() {
        val fields = fields()
        val original = fields.map { it.copyOf() }
        val work = work(fields)
        val certificate = byteArrayOf(31, 32)
        val signature = signature()
        val evidence = KagemushaIncomingFoldEvidenceV1(fields[0], fields[9], certificate, signature)
        fields.forEach { it.fill(0) }; certificate.fill(0); signature.fill(0)
        assertContentEquals(original[0], work.historyOperationId())
        assertContentEquals(original[1], work.creditId())
        assertContentEquals(original[2], work.canonicalHardwareStatement())
        assertContentEquals(original[3], work.proofStatementDigest())
        assertContentEquals(original[4], work.normalizedGuardDigest())
        assertContentEquals(original[5], work.rootSelectionSigningBytes())
        assertContentEquals(original[6], work.deviceKeyReference())
        assertContentEquals(original[8], work.hardwareEpochId())
        assertContentEquals(original[9], work.canonicalPairedProof())
        work.requireEvidence(evidence)
        listOf(work.historyOperationId(), work.creditId(), work.canonicalHardwareStatement(),
            work.proofStatementDigest(), work.normalizedGuardDigest(), work.rootSelectionSigningBytes(),
            work.deviceKeyReference(), work.hardwareEpochId(), work.canonicalPairedProof(),
            evidence.historyOperationId(), evidence.canonicalPairedProof(),
            evidence.canonicalHardwareTransitionCertificate(), evidence.deviceRootSelectionSignature()).forEach { it.fill(0) }
        work.requireEvidence(evidence)
        assertContentEquals(byteArrayOf(31, 32), evidence.canonicalHardwareTransitionCertificate())
        assertContentEquals(signature(), evidence.deviceRootSelectionSignature())
        assertEquals(BigInteger.ONE, work.hardwareEpochGeneration)
    }

    @Test fun `physical evidence cannot substitute history or the original canonical paired proof`() {
        val work = work(fields())
        val evidence = evidence(work)
        work.requireEvidence(evidence)
        assertFailsWith<IllegalArgumentException> {
            work.requireEvidence(KagemushaIncomingFoldEvidenceV1(digest(99), evidence.canonicalPairedProof(),
                evidence.canonicalHardwareTransitionCertificate(), signature()))
        }
        assertFailsWith<IllegalArgumentException> {
            work.requireEvidence(KagemushaIncomingFoldEvidenceV1(work.historyOperationId(), pair(99),
                evidence.canonicalHardwareTransitionCertificate(), signature()))
        }
    }

    @Test fun `native incoming frame correlates credit history and the exact closed ten field work`() {
        val fields = fields()
        val prepare = KagemushaCoreCoordinatorMethodV1.PREPARE_INCOMING_FOLD
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(prepare, listOf(u32(0), fields[1]))
        val response = KagemushaCoreCoordinatorFrameV1.encodeResponse(prepare, request, fields)
        assertEquals(10, KagemushaCoreCoordinatorFrameV1.decodeResponse(prepare, request, response).size)
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(prepare, request, fields.toMutableList().also { it[1] = digest(99) })
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(prepare, request, fields.dropLast(1))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeRequest(prepare, listOf(u32(2), fields[1]))
        }
        val work = work(fields)
        val complete = KagemushaCoreCoordinatorMethodV1.COMPLETE_INCOMING_FOLD
        val completedRequest = KagemushaCoreCoordinatorFrameV1.encodeRequest(complete,
            listOf(work.historyOperationId(), work.canonicalPairedProof(), byteArrayOf(1), signature()))
        KagemushaCoreCoordinatorFrameV1.encodeResponse(complete, completedRequest, listOf(work.historyOperationId()))
        assertFailsWith<IllegalArgumentException> {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(complete, completedRequest, listOf(digest(99)))
        }
        for (kind in KagemushaIncomingStageKindV1.entries) {
            val stage = KagemushaCoreCoordinatorMethodV1.STAGE_INCOMING_ORIGINAL
            val staged = KagemushaCoreCoordinatorFrameV1.encodeRequest(stage, listOf(u32(kind.code), work.creditId()))
            KagemushaCoreCoordinatorFrameV1.encodeResponse(stage, staged, listOf(work.creditId()))
            assertFailsWith<IllegalArgumentException> {
                KagemushaCoreCoordinatorFrameV1.encodeResponse(stage, staged, listOf(digest(99)))
            }
        }
    }

    @Test fun `zero identities invalid epoch changed proof tails and invalid hardware originals fail before framing`() {
        for (index in listOf(0, 1, 3, 4, 6, 8)) {
            assertFailsWith<IllegalArgumentException> { work(fields().toMutableList().also { it[index] = ByteArray(32) }) }
        }
        for (epoch in listOf(BigInteger.ZERO, BigInteger.valueOf(-1), BigInteger.ONE.shiftLeft(128))) {
            assertFailsWith<IllegalArgumentException> { work(fields(), epoch) }
        }
        assertFailsWith<IllegalArgumentException> { work(fields().toMutableList().also { it[9] += byteArrayOf(0) }) }
        assertFailsWith<IllegalArgumentException> { work(fields().toMutableList().also { it[9] = ByteArray(6_529) }) }
        val work = work(fields())
        for (certificate in listOf(ByteArray(0), ByteArray(96 * 1024 + 1))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaIncomingFoldEvidenceV1(work.historyOperationId(), work.canonicalPairedProof(), certificate, signature())
            }
        }
        for (signature in listOf(ByteArray(0), ByteArray(63), ByteArray(64), ByteArray(65))) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaIncomingFoldEvidenceV1(work.historyOperationId(), work.canonicalPairedProof(), byteArrayOf(1), signature)
            }
        }
    }

    private fun evidence(work: KagemushaNativeIncomingFoldPreparationV1) = KagemushaIncomingFoldEvidenceV1(
        work.historyOperationId(), work.canonicalPairedProof(), byteArrayOf(31), signature())
    private fun work(fields: List<ByteArray>, epoch: BigInteger = BigInteger.ONE) = KagemushaNativeIncomingFoldPreparationV1(
        KagemushaPendingCreditKindV1.MINT, fields[0], fields[1], fields[2], fields[3], fields[4], fields[5], fields[6], epoch, fields[8], fields[9])
    private fun fields() = listOf(digest(1), digest(2), byteArrayOf(3), digest(4), digest(5), byteArrayOf(6),
        digest(7), ByteArray(16).also { it[0] = 1 }, digest(8), pair())
    private fun pair(semantic: Int = 13) = KagemushaNoritoV1.encodePairedProofShape(KagemushaPairedProofV1(1,
        digest(11), digest(12), digest(semantic), digest(14), digest(15), digest(16), digest(17),
        byteArrayOf(18), byteArrayOf(19), ByteArray(544) { 20 }, ByteArray(544) { 21 }))
    private fun digest(value: Int) = ByteArray(32) { value.toByte() }
    private fun signature() = ByteArray(64).also { it[31] = 1; it[63] = 1 }
    private fun u32(value: Int) = KagemushaCoreCoordinatorFrameV1.u32(value)
}
