// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import kotlin.test.*

/** Actual public framing checks for the separate C20 FI ceremony; fixtures confer no Native authority. */
class KagemushaOrdinaryRetailFrameV1Test {
    private val method = KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION
    private val ticket = byteArrayOf(7, 0, 0, 0, 0, 0, 0, 0)
    private val original = byteArrayOf(1, 2, 3)
    private val message = ByteArray(32) { 8 }
    private val digest = ByteArray(32) { 9 }
    private val signature = ByteArray(64) { 10 }

    @Test fun allSixPhasesHaveClosedRequestsAndOriginalCorrelations() {
        val cases = listOf(
            listOf(phase(9), ticket, original, message) to listOf(ticket, original, message, digest, digest),
            listOf(phase(10), ticket) to listOf(byteArrayOf(1), byteArrayOf()),
            listOf(phase(11), ticket, signature) to listOf(MessageDigest.getInstance("SHA-256").digest(signature)),
            listOf(phase(12), ticket, original) to listOf(digest, digest),
            listOf(phase(13), ticket) to listOf(byteArrayOf(3), signature, original),
            listOf(phase(14), ticket) to emptyList(),
        )
        for ((request, response) in cases) {
            val wire = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request)
            val result = KagemushaCoreCoordinatorFrameV1.encodeResponse(method, wire, response)
            assertEquals(response.size, KagemushaCoreCoordinatorFrameV1.decodeResponse(method, wire, result).size)
            assertFails { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, request + byteArrayOf(1)) }
            assertFails { KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL, request) }
        }
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, cases[0].first)
        for (index in 1..2) {
            val substituted = cases[0].second.map(ByteArray::copyOf)
            substituted[index][0] = (substituted[index][0].toInt() xor 1).toByte()
            assertFails { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, substituted) }
        }
    }

    @Test fun nativeRecoveryStatesRequireFullSignatureAndCertificateByState() {
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(phase(13), ticket))
        for (bad in listOf(listOf(byteArrayOf(0), signature, byteArrayOf()),
            listOf(byteArrayOf(1), byteArrayOf(), original), listOf(byteArrayOf(2), signature, original),
            listOf(byteArrayOf(3), signature, byteArrayOf()), listOf(byteArrayOf(4), byteArrayOf(), byteArrayOf()))) {
            assertFails { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, bad) }
        }
        for (state in 0..3) {
            KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, listOf(byteArrayOf(state.toByte()),
                if (state >= 2) signature else byteArrayOf(), if (state == 3) original else byteArrayOf()))
        }
    }

    @Test fun signingFenceAndFullOriginalBoundsRejectPartialOrRetiredData() {
        val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(phase(10), ticket))
        for (bad in listOf(listOf(byteArrayOf(0), byteArrayOf()), listOf(byteArrayOf(1), signature),
            listOf(byteArrayOf(2), signature.copyOf(63)))) {
            assertFails { KagemushaCoreCoordinatorFrameV1.encodeResponse(method, request, bad) }
        }
        assertFails { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(phase(9), ticket, ByteArray(32769), message)) }
        assertFails { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(phase(11), ticket, signature.copyOf(63))) }
        assertFails { KagemushaCoreCoordinatorFrameV1.encodeRequest(method, listOf(phase(12), ticket, ByteArray(16385))) }
    }
    private fun phase(value: Int) = KagemushaCoreCoordinatorFrameV1.u32(value)
}
